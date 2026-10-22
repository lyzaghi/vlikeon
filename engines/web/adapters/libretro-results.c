/* Void outer export for the explicitly declared cooperative-result protocol.
 * The shared worker observes completion only after the fiber trampoline ends.
 */
#include <stdint.h>
#include "libretro.h"
#ifdef OMNIPLAY_WEB_FILES
extern int om_web_flush_save_io(void);
#endif
void om_web_libretro_call(unsigned operation, uintptr_t first,
                           uintptr_t second, uint32_t* result) {
  uint32_t value = 0;
  if (!result) return;
  switch (operation) {
    case 1: value = retro_load_game((const struct retro_game_info*)first) ? 1 : 0; break;
    case 2: value = (uint32_t)retro_serialize_size(); break;
    case 3: value = retro_serialize((void*)first, (size_t)second) ? 1 : 0; break;
    case 4: value = retro_unserialize((const void*)first, (size_t)second) ? 1 : 0; break;
    /* Protocol 2 additionally proves completion of the void lifecycle exports.
     * Protocol 1 callers continue to use only operations 1 through 5. */
    case 6: retro_run(); value = 1; break;
    case 7: retro_unload_game(); value = 1; break;
    case 8: retro_deinit(); value = 1; break;
    case 9: retro_init(); value = 1; break;
    case 10: retro_reset(); value = 1; break;
#ifdef OMNIPLAY_WEB_FILES
    case 5: value = om_web_flush_save_io() == 1 ? 1 : 0; break;
#endif
    default: return;
  }
  result[0] = value;
  result[1] = 1;
}
