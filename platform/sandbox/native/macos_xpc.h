#pragma once
#include <stddef.h>
#ifdef __cplusplus
extern "C" {
#endif
typedef struct opmac_client opmac_client;
typedef struct { unsigned char* json; size_t bytes; int input; int output; } opmac_reply;
opmac_client* opmac_open(const char* service, const char* requirement, const char* executable);
int opmac_call(opmac_client*, const unsigned char* json, size_t bytes,
    const int* descriptors, size_t count, unsigned int timeout_ms, opmac_reply* reply);
void opmac_reply_free(opmac_reply*);
void opmac_close(opmac_client*);
#ifdef __cplusplus
}
#endif
