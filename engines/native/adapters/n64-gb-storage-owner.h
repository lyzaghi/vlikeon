#ifndef OMNIPLAY_N64_GB_STORAGE_OWNER_H
#define OMNIPLAY_N64_GB_STORAGE_OWNER_H
#include "backends/file_storage.h"
#include "n64-rtc-storage.h"
struct video_capture_backend_interface;
/* Shared real layout for main.c and actual-helper qualification fixtures. */
struct gb_cart_data {
    int control_id;
    struct file_storage rom_fstorage;
    struct file_storage ram_fstorage;
    struct mbc3_rtc* rtc;
    void* gbcam_backend;
    const struct video_capture_backend_interface* igbcam_backend;
};
#endif
