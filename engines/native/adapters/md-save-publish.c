/* Only the two private MD save names may be replaced on Windows.
 * C stdio rename does not overwrite an existing destination on this platform.
 */
#ifndef _WIN32
#error This native publisher requires Windows
#endif
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <string.h>
int om_md_native_publish(const char* temporary, const char* destination) {
  const wchar_t* from;
  const wchar_t* to;
  if (!temporary || !destination) return 0;
  if (!strcmp(temporary, "save/MD/save.ram.omtmp") && !strcmp(destination, "save/MD/save.ram")) {
    from = L"save/MD/save.ram.omtmp"; to = L"save/MD/save.ram";
  } else if (!strcmp(temporary, "save/MD/save.eeprom.omtmp") && !strcmp(destination, "save/MD/save.eeprom")) {
    from = L"save/MD/save.eeprom.omtmp"; to = L"save/MD/save.eeprom";
  } else return 0;
  return MoveFileExW(from, to, MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH) ? 1 : 0;
}
