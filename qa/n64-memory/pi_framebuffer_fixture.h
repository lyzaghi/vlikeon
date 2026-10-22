#ifndef OMNIPLAY_PI_FRAMEBUFFER_FIXTURE_H
#define OMNIPLAY_PI_FRAMEBUFFER_FIXTURE_H
#include <stdint.h>
#include "device/rcp/pi/pi_controller.h"
#include "device/rcp/rdp/rdp_core.h"
#include "device/rcp/ri/ri_controller.h"
#include "device/rdram/rdram.h"
struct om_n64_pi_fb_observed {
    unsigned int getter,transfers,scheduled,cycles,reads,writes,mappings,invalidations,raw_reads,raw_writes,bad;
    unsigned int reads_at_transfer;
    uint32_t transfer_address,transfer_length,first_read,last_read,first_write,last_write,last_size;
    uint32_t map_begin,map_end;
};
void om_n64_case_pi_fb_reset(struct pi_controller*,struct ri_controller*,struct rdp_core*,struct rdram*);
void om_n64_case_pi_fb_stage(const FrameBufferInfo*);
void om_n64_case_pi_fb_handler(unsigned int);
void om_n64_case_pi_start(struct pi_controller*,int);
struct om_n64_pi_fb_observed om_n64_case_pi_fb_observed(void);
#endif
