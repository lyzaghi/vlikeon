/* OmniPlay adaptation: the Web core has no native thread/atomic writer. */
#include "mc_async.h"
#if (defined(HAVE_THREADS) && HAVE_THREADS) || defined(__EMSCRIPTEN_PTHREADS__)
#error The OmniPlay Web card writer requires an unthreaded build
#endif
static mc_async_write_t om_card_writer;
void mc_async_init(mc_async_write_t writer) { om_card_writer = writer; }
void mc_async_enqueue(const char *path, const uint8_t *data, uint32_t size)
{
   if (om_card_writer && path && data && size == (1u << 17))
      om_card_writer(path, data, size);
}
void mc_async_flush_and_stop(void) { om_card_writer = 0; }
