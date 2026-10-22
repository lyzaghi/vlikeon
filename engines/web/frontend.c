/* Constants come from the same C library/SDK as the core, not JS errno guesses. */
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <sys/stat.h>
#include <stdbool.h>
#include <stdint.h>
#include <string.h>
int om_web_fs_constant(unsigned selector) {
    switch (selector) {
        case 1: return EIO; case 2: return EINVAL; case 3: return ENOENT;
        case 4: return EROFS; case 5: return EISDIR; case 6: return ENOTDIR;
        case 7: return EBADF; case 8: return ENOSYS;
        case 20: return S_IFDIR; case 21: return S_IFREG;
        case 22: return O_ACCMODE; case 23: return O_RDONLY;
        case 24: return SEEK_SET; case 25: return SEEK_CUR; case 26: return SEEK_END;
        case 27: return EPERM; case 28: return O_APPEND;
        case 29: return O_CREAT; case 30: return O_TRUNC; case 31: return O_EXCL;
        case 32: return ENOSPC; case 33: return ENOTEMPTY; case 34: return EEXIST;
        case 35: return S_IFMT; case 36: return ENOMEM; case 37: return EFBIG;
        default: return EINVAL;
    }
}

/* The first five callbacks have the same order/types in libretro's base and
 * extended disk interfaces. Copy only this used prefix; never retain its table.
 * Callback execution stays in the owning single-threaded wasm32 core instance. */
struct om_disc_prefix {
    bool (*set_eject)(bool);
    bool (*get_eject)(void);
    unsigned (*get_index)(void);
    bool (*set_index)(unsigned);
    unsigned (*get_count)(void);
};
_Static_assert(sizeof(void*) == 4, "OmniPlay web frontend requires wasm32");
_Static_assert(sizeof(struct om_disc_prefix) == 20, "Disk callback prefix ABI differs");
static struct om_disc_prefix om_disc;
void om_web_disc_clear(void) { memset(&om_disc, 0, sizeof(om_disc)); }
int om_web_disc_register(const void* callbacks) {
    struct om_disc_prefix next;
    if (!callbacks) { om_web_disc_clear(); return 1; }
    memcpy(&next, callbacks, sizeof(next));
    if (!next.set_eject || !next.get_eject || !next.get_index || !next.set_index || !next.get_count) return 0;
    om_disc = next; return 1;
}
unsigned om_web_disc_count(void) { return om_disc.get_count ? om_disc.get_count() : UINT32_MAX; }
unsigned om_web_disc_index(void) { return om_disc.get_index ? om_disc.get_index() : UINT32_MAX; }
int om_web_disc_ejected(void) { return om_disc.get_eject ? (om_disc.get_eject() ? 1 : 0) : -1; }
int om_web_disc_set_ejected(int value) { return om_disc.set_eject && (value == 0 || value == 1) ? om_disc.set_eject(value != 0) : 0; }
int om_web_disc_set_index(unsigned value) { return om_disc.set_index ? om_disc.set_index(value) : 0; }
/* This drains C stdio only. Core-private/threaded producers need a separate,
 * explicitly qualified core barrier; the frontend never claims to flush those. */
int om_web_flush_files(void) { return fflush(NULL); }

/* A C variadic callback cannot be implemented by an ordinary JS addFunction
 * trampoline. Keep optional upstream diagnostics off the UI/ROM data path. */
struct om_web_log_callback { void (*log)(int, const char*, ...); };
static void om_web_log_discard(int level, const char* format, ...) { (void)level; (void)format; }
int om_web_log_interface(void* output) {
    if (!output) return 0;
    ((struct om_web_log_callback*)output)->log = om_web_log_discard;
    return 1;
}
