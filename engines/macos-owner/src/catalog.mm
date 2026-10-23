#import <Foundation/Foundation.h>
#import <Security/Security.h>
#include "catalog.hpp"
#include <cctype>
#include <cerrno>
#include <CommonCrypto/CommonDigest.h>
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

namespace {
bool integer(id value, uint64_t minimum, uint64_t maximum) {
  return [value isKindOfClass:NSNumber.class] &&
      CFGetTypeID((__bridge CFTypeRef)value) != CFBooleanGetTypeID() &&
      !CFNumberIsFloatType((__bridge CFNumberRef)value) &&
      [value longLongValue] >= 0 && [value unsignedLongLongValue] >= minimum &&
      [value unsignedLongLongValue] <= maximum;
}
bool hash_text(NSString* value) {
  if (![value isKindOfClass:NSString.class] || value.length != 64) return false;
  return [value rangeOfCharacterFromSet:[NSCharacterSet characterSetWithCharactersInString:
      @"0123456789abcdef"].invertedSet].location == NSNotFound;
}
bool guest_file(NSURL* root, NSString* name, NSString* hash, uint64_t bytes, bool kernel, NSURL** result) {
  if (!basename(name) || !hash_text(hash)) return false;
  NSURL* path = [root URLByAppendingPathComponent:name];
  int fd = open(path.fileSystemRepresentation, O_RDONLY | O_CLOEXEC | O_NOFOLLOW);
  if (fd < 0) return false;
  struct stat shape{};
  bool valid = fstat(fd, &shape) == 0 && S_ISREG(shape.st_mode) && shape.st_nlink == 1 &&
      (shape.st_mode & (S_ISUID | S_ISGID)) == 0 && shape.st_size >= 64 &&
      static_cast<uint64_t>(shape.st_size) == bytes;
  unsigned char header[64]{};
  if (valid) valid = pread(fd, header, sizeof(header), 0) == sizeof(header) &&
      (kernel ? (header[56] == 'A' && header[57] == 'R' && header[58] == 'M' && header[59] == 0x64)
              : memcmp(header, "070701", 6) == 0);
  CC_SHA256_CTX context; CC_SHA256_Init(&context);
  unsigned char block[65536], digest[CC_SHA256_DIGEST_LENGTH];
  uint64_t length = 0;
  while (valid) {
    ssize_t count = read(fd, block, sizeof(block));
    if (count < 0 && errno == EINTR) continue;
    if (count < 0) { valid = false; break; }
    if (!count) break;
    length += count;
    if (length > bytes) { valid = false; break; }
    CC_SHA256_Update(&context, block, static_cast<CC_LONG>(count));
  }
  CC_SHA256_Final(digest, &context); close(fd);
  char hexadecimal[65]; constexpr char digits[] = "0123456789abcdef";
  for (size_t i = 0; i < sizeof(digest); ++i) {
    hexadecimal[i * 2] = digits[digest[i] >> 4]; hexadecimal[i * 2 + 1] = digits[digest[i] & 15];
  }
  hexadecimal[64] = 0;
  if (!valid || length != bytes || strcmp(hexadecimal, hash.UTF8String)) return false;
  *result = path; return true;
}
}
std::optional<RuntimeSpec> admit_runtime(const char* requested) {
  if (!runtime_id(requested)) return std::nullopt;
  NSURL* catalog = [NSBundle.mainBundle.resourceURL URLByAppendingPathComponent:@"runtime-catalog.json"];
  struct stat catalog_shape{};
  if (lstat(catalog.fileSystemRepresentation, &catalog_shape) ||
      !S_ISREG(catalog_shape.st_mode) || catalog_shape.st_nlink != 1 ||
      catalog_shape.st_size <= 0 || catalog_shape.st_size > 1024 * 1024) return std::nullopt;
  NSData* raw = [NSData dataWithContentsOfURL:catalog options:0 error:nil];
  id decoded = raw ? [NSJSONSerialization JSONObjectWithData:raw options:0 error:nil] : nil;
  NSSet* root_keys = [NSSet setWithArray:@[@"schema_version", @"runtimes"]];
  if (!exact(decoded, root_keys) || !integer(decoded[@"schema_version"], 2, 2)) return std::nullopt;
  NSArray* entries = decoded[@"runtimes"];
  if (![entries isKindOfClass:NSArray.class] || !entries.count || entries.count > 32) return std::nullopt;
  NSSet* keys = [NSSet setWithArray:@[@"id", @"team_identifier", @"presentation",
      @"presentation_identifier", @"presentation_memory_mb", @"kernel", @"kernel_sha256",
      @"kernel_bytes", @"initrd", @"initrd_sha256", @"initrd_bytes", @"guest_memory_mb",
      @"guest_cpu_count", @"scratch_mb"]];
  std::set<std::string> ids, identities; NSDictionary* selected = nil;
  for (id entry in entries) {
    if (!exact(entry, keys)) return std::nullopt;
    NSString* identity = entry[@"id"];
    if (![identity isKindOfClass:NSString.class] || !runtime_id(identity.UTF8String) ||
        !ids.insert(text(identity)).second || !team_identifier(entry[@"team_identifier"]) ||
        !identifier(entry[@"presentation_identifier"]) ||
        !identities.insert(text(entry[@"presentation_identifier"])).second) return std::nullopt;
    for (NSString* name in @[@"presentation", @"kernel", @"initrd"])
      if (!basename(entry[name])) return std::nullopt;
    if ([NSSet setWithArray:@[entry[@"presentation"], entry[@"kernel"], entry[@"initrd"]]].count != 3 ||
        !hash_text(entry[@"kernel_sha256"]) || !hash_text(entry[@"initrd_sha256"]) ||
        !integer(entry[@"kernel_bytes"], 64, 128 * 1024 * 1024) ||
        !integer(entry[@"initrd_bytes"], 64, 256 * 1024 * 1024) ||
        !integer(entry[@"guest_memory_mb"], 256, 8192) ||
        !integer(entry[@"presentation_memory_mb"], 256, 8192) ||
        !integer(entry[@"guest_cpu_count"], 1, 4) ||
        !integer(entry[@"scratch_mb"], 16, 512) ||
        [entry[@"scratch_mb"] unsignedIntValue] > [entry[@"guest_memory_mb"] unsignedIntValue] / 2 ||
        [entry[@"initrd_bytes"] unsignedLongLongValue] > [entry[@"guest_memory_mb"] unsignedLongLongValue] * 1024 * 1024 / 2)
      return std::nullopt;
    if (!strcmp(identity.UTF8String, requested)) selected = entry;
  }
  if (!selected) return std::nullopt;
  NSURL* runtimes = [NSBundle.mainBundle.resourceURL URLByAppendingPathComponent:@"Runtimes" isDirectory:YES];
  NSURL* directory = [runtimes URLByAppendingPathComponent:selected[@"id"] isDirectory:YES];
  if (!safe_directory(runtimes) || !safe_directory(directory)) return std::nullopt;
  NSURL *helper = nil, *kernel = nil, *initrd = nil;
  if (!safe_file(directory, selected[@"presentation"], MH_EXECUTE, &helper) ||
      !signed_code(helper, selected[@"presentation_identifier"], selected[@"team_identifier"], true) ||
      !guest_file(directory, selected[@"kernel"], selected[@"kernel_sha256"],
          [selected[@"kernel_bytes"] unsignedLongLongValue], true, &kernel) ||
      !guest_file(directory, selected[@"initrd"], selected[@"initrd_sha256"],
          [selected[@"initrd_bytes"] unsignedLongLongValue], false, &initrd)) return std::nullopt;
  return RuntimeSpec{text(selected[@"id"]), text(selected[@"team_identifier"]),
      helper.fileSystemRepresentation, text(selected[@"presentation_identifier"]),
      GuestImage{kernel.fileSystemRepresentation, initrd.fileSystemRepresentation,
          [selected[@"guest_memory_mb"] unsignedIntValue], [selected[@"guest_cpu_count"] unsignedIntValue],
          [selected[@"scratch_mb"] unsignedIntValue]},
      text(selected[@"kernel_sha256"]), text(selected[@"initrd_sha256"]),
      [selected[@"presentation_memory_mb"] unsignedIntValue]};
}
bool admit_owned_child(const RuntimeSpec& runtime, pid_t pid) {
  if (pid <= 1) return false;
  NSString* signing = [NSString stringWithUTF8String:
      runtime.presentation_identifier.c_str()];
  NSString* team = [NSString stringWithUTF8String:runtime.team_identifier.c_str()];
  if (!identifier(signing) || !team_identifier(team)) return false;
  NSDictionary* attributes = @{(__bridge id)kSecGuestAttributePid: @(pid)};
  SecCodeRef guest = nullptr;
  // This new child lookup is additional to the existing static bundle admission
  // and unchanged XPC peer authentication. Strict validity is required below.
  if (SecCodeCopyGuestWithAttributes(nullptr, (__bridge CFDictionaryRef)attributes,
      kSecCSDefaultFlags, &guest) != errSecSuccess || !guest) return false;
  NSString* text = [NSString stringWithFormat:
      @"anchor apple generic and identifier \"%@\" and certificate leaf[subject.OU] = \"%@\"",
      signing, team];
  SecRequirementRef requirement = nullptr;
  OSStatus status = SecRequirementCreateWithString((__bridge CFStringRef)text,
      kSecCSDefaultFlags, &requirement);
  if (status == errSecSuccess && requirement)
    status = SecCodeCheckValidity(guest,
        kSecCSStrictValidate | kSecCSCheckAllArchitectures, requirement);
  if (requirement) CFRelease(requirement);
  CFDictionaryRef raw = nullptr;
  if (status == errSecSuccess)
    status = SecCodeCopySigningInformation(guest, kSecCSSigningInformation, &raw);
  CFRelease(guest);
  if (status != errSecSuccess || !raw) { if (raw) CFRelease(raw); return false; }
  NSDictionary* info = CFBridgingRelease(raw);
  NSNumber* flags = info[(__bridge id)kSecCodeInfoFlags];
  return [info[(__bridge id)kSecCodeInfoIdentifier] isEqual:signing] &&
      [info[(__bridge id)kSecCodeInfoTeamIdentifier] isEqual:team] &&
      [flags isKindOfClass:NSNumber.class] && (flags.unsignedIntValue & CS_RUNTIME) != 0 &&
      child_entitlements(info[(__bridge id)kSecCodeInfoEntitlementsDict], signing, team);
}
}
