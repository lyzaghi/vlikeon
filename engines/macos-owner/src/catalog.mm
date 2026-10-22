#import <Foundation/Foundation.h>
#import <Security/Security.h>
#include "catalog.hpp"
#include <cctype>
#include <cstring>
#include <fcntl.h>
#include <mach-o/loader.h>
#include <set>
#include <string>
#include <sys/codesign.h>
#include <sys/stat.h>
#include <unistd.h>

namespace omniplay::macos {
namespace {
bool identifier(NSString* value) {
  if (![value isKindOfClass:NSString.class] || value.length == 0 || value.length > 160 ||
      [value hasPrefix:@"."] || [value hasSuffix:@"."]) return false;
  NSArray<NSString*>* parts = [value componentsSeparatedByString:@"."];
  if (parts.count < 3) return false;
  NSCharacterSet* allowed = [NSCharacterSet characterSetWithCharactersInString:
      @"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-"];
  for (NSString* part in parts) {
    if (part.length == 0 || part.length > 63 ||
        [part rangeOfCharacterFromSet:allowed.invertedSet].location != NSNotFound) return false;
  }
  return true;
}

bool team_identifier(NSString* value) {
  if (![value isKindOfClass:NSString.class] || value.length != 10) return false;
  NSCharacterSet* allowed = [NSCharacterSet characterSetWithCharactersInString:
      @"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"];
  return [value rangeOfCharacterFromSet:allowed.invertedSet].location == NSNotFound;
}

bool runtime_id(const char* value) {
  if (!value) return false;
  size_t size = strnlen(value, 65);
  if (size == 0 || size > 64) return false;
  for (size_t index = 0; index < size; ++index) {
    const auto byte = static_cast<unsigned char>(value[index]);
    if (!(std::isalnum(byte) || byte == '_' || byte == '-')) return false;
  }
  return true;
}

bool basename(NSString* value) {
  if (![value isKindOfClass:NSString.class] || value.length == 0 || value.length > 128 ||
      [value isEqualToString:@"."] || [value isEqualToString:@".."] ||
      [value hasPrefix:@"."] || [value hasSuffix:@"."]) return false;
  NSCharacterSet* allowed = [NSCharacterSet characterSetWithCharactersInString:
      @"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_+.-"];
  return [value rangeOfCharacterFromSet:allowed.invertedSet].location == NSNotFound;
}

bool exact(NSDictionary* value, NSSet<NSString*>* names) {
  return [value isKindOfClass:NSDictionary.class] && value.count == names.count &&
      [names isEqualToSet:[NSSet setWithArray:value.allKeys]];
}

bool thin_arm64(const char* path, uint32_t kind) {
  int descriptor = open(path, O_RDONLY | O_CLOEXEC | O_NOFOLLOW);
  if (descriptor < 0) return false;
  struct stat shape{};
  mach_header_64 header{};
  bool ok = fstat(descriptor, &shape) == 0 && S_ISREG(shape.st_mode) && shape.st_size > 0 &&
      pread(descriptor, &header, sizeof(header), 0) == sizeof(header) &&
      header.magic == MH_MAGIC_64 && header.cputype == CPU_TYPE_ARM64 &&
      header.filetype == kind && header.sizeofcmds <= 64 * 1024 * 1024 &&
      static_cast<uint64_t>(shape.st_size) >= sizeof(header) + header.sizeofcmds &&
      (kind != MH_EXECUTE || (header.flags & MH_PIE) != 0);
  close(descriptor);
  return ok;
}

bool safe_directory(NSURL* path) {
  struct stat shape{};
  return lstat(path.fileSystemRepresentation, &shape) == 0 && S_ISDIR(shape.st_mode);
}

bool safe_file(NSURL* root, NSString* name, uint32_t kind, NSURL** result) {
  if (!basename(name)) return false;
  NSURL* path = [root URLByAppendingPathComponent:name isDirectory:NO].URLByStandardizingPath;
  if (![path.URLByDeletingLastPathComponent isEqual:root.URLByStandardizingPath]) return false;
  struct stat shape{};
  if (lstat(path.fileSystemRepresentation, &shape) != 0 || !S_ISREG(shape.st_mode) ||
      shape.st_nlink != 1 || (shape.st_mode & (S_ISUID | S_ISGID)) != 0 ||
      !thin_arm64(path.fileSystemRepresentation, kind)) return false;
  *result = path;
  return true;
}

bool child_entitlements(NSDictionary* entitlements, NSString* signing_id, NSString* team) {
  if (![entitlements isKindOfClass:NSDictionary.class]) return false;
  NSSet* allowed = [NSSet setWithArray:@[
      @"com.apple.security.app-sandbox", @"com.apple.security.inherit",
      @"com.apple.application-identifier", @"com.apple.developer.team-identifier",
      @"keychain-access-groups"]];
  for (id key in entitlements) {
    if (![key isKindOfClass:NSString.class] || ![allowed containsObject:key]) return false;
  }
  if (![entitlements[@"com.apple.security.app-sandbox"] isEqual:@YES] ||
      ![entitlements[@"com.apple.security.inherit"] isEqual:@YES]) return false;
  NSString* application = [NSString stringWithFormat:@"%@.%@", team, signing_id];
  id declared_application = entitlements[@"com.apple.application-identifier"];
  if (declared_application && ![declared_application isEqual:application]) return false;
  id declared_team = entitlements[@"com.apple.developer.team-identifier"];
  if (declared_team && ![declared_team isEqual:team]) return false;
  id groups = entitlements[@"keychain-access-groups"];
  if (groups && (![groups isKindOfClass:NSArray.class] || [groups count] != 1 ||
      ![groups[0] isEqual:application])) return false;
  return true;
}

bool signed_code(NSURL* url, NSString* signing_id, NSString* team, bool child) {
  if (!identifier(signing_id) || !team_identifier(team)) return false;
  SecStaticCodeRef code = nullptr;
  if (SecStaticCodeCreateWithPath((__bridge CFURLRef)url, kSecCSDefaultFlags, &code) !=
      errSecSuccess || !code) return false;
  NSString* text = [NSString stringWithFormat:
      @"anchor apple generic and identifier \"%@\" and certificate leaf[subject.OU] = \"%@\"",
      signing_id, team];
  SecRequirementRef requirement = nullptr;
  OSStatus status = SecRequirementCreateWithString((__bridge CFStringRef)text,
      kSecCSDefaultFlags, &requirement);
  if (status == errSecSuccess && requirement) {
    status = SecStaticCodeCheckValidity(code,
        kSecCSStrictValidate | kSecCSCheckAllArchitectures, requirement);
  }
  if (requirement) CFRelease(requirement);
  if (status != errSecSuccess) { CFRelease(code); return false; }
  CFDictionaryRef raw = nullptr;
  status = SecCodeCopySigningInformation(code, kSecCSSigningInformation, &raw);
  CFRelease(code);
  if (status != errSecSuccess || !raw) return false;
  NSDictionary* info = CFBridgingRelease(raw);
  if (![info[(__bridge id)kSecCodeInfoIdentifier] isEqual:signing_id] ||
      ![info[(__bridge id)kSecCodeInfoTeamIdentifier] isEqual:team]) return false;
  NSNumber* flags = info[(__bridge id)kSecCodeInfoFlags];
  if (![flags isKindOfClass:NSNumber.class] || (flags.unsignedIntValue & CS_RUNTIME) == 0)
    return false;
  NSDictionary* entitlements = info[(__bridge id)kSecCodeInfoEntitlementsDict];
  if (child) return child_entitlements(entitlements, signing_id, team);
  return !entitlements || ([entitlements isKindOfClass:NSDictionary.class] && entitlements.count == 0);
}

std::string text(NSString* value) { return std::string(value.UTF8String ?: ""); }
}

std::optional<RuntimeSpec> admit_runtime(const char* requested) {
  if (!runtime_id(requested)) return std::nullopt;
  NSURL* catalog = [NSBundle.mainBundle.resourceURL
      URLByAppendingPathComponent:@"runtime-catalog.json" isDirectory:NO];
  NSData* raw = [NSData dataWithContentsOfURL:catalog options:NSDataReadingMappedIfSafe error:nil];
  if (!raw || raw.length == 0 || raw.length > 1024 * 1024) return std::nullopt;
  id decoded = [NSJSONSerialization JSONObjectWithData:raw options:0 error:nil];
  if (![decoded isKindOfClass:NSDictionary.class]) return std::nullopt;
  NSDictionary* root = decoded;
  NSSet* root_keys = [NSSet setWithArray:@[@"schema_version", @"runtimes"]];
  NSArray* runtimes = root[@"runtimes"];
  if (!exact(root, root_keys) || ![root[@"schema_version"] isEqual:@1] ||
      ![runtimes isKindOfClass:NSArray.class] || runtimes.count == 0 || runtimes.count > 32)
    return std::nullopt;
  NSDictionary* selected = nil;
  std::set<std::string> ids, signing_ids;
  NSSet* keys = [NSSet setWithArray:@[
      @"id", @"team_identifier", @"actor", @"actor_identifier",
      @"presentation", @"presentation_identifier", @"core", @"core_identifier"]];
  for (id item in runtimes) {
    if (!exact(item, keys)) return std::nullopt;
    NSString* identity = item[@"id"];
    if (![identity isKindOfClass:NSString.class] || !runtime_id(identity.UTF8String) ||
        !ids.insert(text(identity)).second || !team_identifier(item[@"team_identifier"]))
      return std::nullopt;
    NSSet* files = [NSSet setWithArray:@[item[@"actor"], item[@"presentation"], item[@"core"]]];
    if (files.count != 3 || !basename(item[@"actor"]) || !basename(item[@"presentation"]) ||
        !basename(item[@"core"])) return std::nullopt;
    for (NSString* code_id in @[
        item[@"actor_identifier"], item[@"presentation_identifier"], item[@"core_identifier"]]) {
      if (!identifier(code_id) || !signing_ids.insert(text(code_id)).second) return std::nullopt;
    }
    if (strcmp(identity.UTF8String, requested) == 0) selected = item;
  }
  if (!selected) return std::nullopt;
  NSString* identity = selected[@"id"];
  NSURL* runtimes_root = [NSBundle.mainBundle.resourceURL
      URLByAppendingPathComponent:@"Runtimes" isDirectory:YES].URLByStandardizingPath;
  NSURL* runtime_root = [runtimes_root
      URLByAppendingPathComponent:identity isDirectory:YES].URLByStandardizingPath;
  if (!safe_directory(runtimes_root) || !safe_directory(runtime_root) ||
      ![runtime_root.URLByDeletingLastPathComponent isEqual:runtimes_root]) return std::nullopt;
  NSURL* actor = nil;
  NSURL* presentation = nil;
  NSURL* core = nil;
  if (!safe_file(runtime_root, selected[@"actor"], MH_EXECUTE, &actor) ||
      !safe_file(runtime_root, selected[@"presentation"], MH_EXECUTE, &presentation) ||
      !safe_file(runtime_root, selected[@"core"], MH_DYLIB, &core)) return std::nullopt;
  NSString* team = selected[@"team_identifier"];
  if (!signed_code(actor, selected[@"actor_identifier"], team, true) ||
      !signed_code(presentation, selected[@"presentation_identifier"], team, true) ||
      !signed_code(core, selected[@"core_identifier"], team, false)) return std::nullopt;
  return RuntimeSpec{
      text(identity), text(team), actor.fileSystemRepresentation,
      text(selected[@"actor_identifier"]), presentation.fileSystemRepresentation,
      text(selected[@"presentation_identifier"]), core.fileSystemRepresentation,
      text(selected[@"core_identifier"])};
}
}
