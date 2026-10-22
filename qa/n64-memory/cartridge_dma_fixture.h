#ifndef OMNIPLAY_CARTRIDGE_DMA_FIXTURE_H
#define OMNIPLAY_CARTRIDGE_DMA_FIXTURE_H
#include <stddef.h>
#include <stdint.h>
struct r4300_core;
struct rdram;
struct om_n64_dma_observed {
    unsigned int invalidations,random_calls,scheduled,validated,bad;
    uint32_t first_address,last_address;
    size_t first_length,last_length;
};
struct r4300_core* om_n64_case_dma_cpu(struct rdram*);
void om_n64_case_dma_clear(void);
struct om_n64_dma_observed om_n64_case_dma_observed(void);
#endif
