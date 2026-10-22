#ifndef OMNIPLAY_N64_RTC_STORAGE_H
#define OMNIPLAY_N64_RTC_STORAGE_H
#define OM_N64_GB_RTC_DEVICE 0x04u
struct mbc3_rtc;
#ifdef __cplusplus
extern "C" {
#endif
/* Four serialized, stable RTC owners. Canonical content/port .rtc names only.
 * Bind stages an exact battery record; only literal absence permits cold start.
 * No clock/register output changes until the corresponding power-on. */
int om_n64_rtc_storage_bind(struct mbc3_rtc*,const char*);
/* Called before ordinary power-on initialization: 1 retains/restores battery
 * fields; 0 uses normal cold initialization. Both leave volatile latches reset. */
int om_n64_rtc_storage_poweron(struct mbc3_rtc*);
void om_n64_rtc_storage_changed(struct mbc3_rtc*);
/* Flush live state, or acknowledge an unstarted staged owner without writing.
 * A mutation invalidates this receipt. Detach consumes it after RAM retirement. */
int om_n64_rtc_storage_prepare_retire(struct mbc3_rtc*);
int om_n64_rtc_storage_detach(struct mbc3_rtc*);
int om_n64_rtc_storage_flush_all(void);
int om_n64_rtc_storage_quiescent(void);
/* Actual RTC owner/clock-checked synchronization, implemented by the RTC. */
void om_n64_mbc3_sync(struct mbc3_rtc*);
#ifdef __cplusplus
}
#endif
#endif
