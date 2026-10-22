/* Authored only. Exact prepared scheduler/pool/accessors and queue adapter;
 * CPU, plugin callbacks and guest execution are outside this focused fixture. */
#include <stddef.h>
#include <stdint.h>
#include <string.h>
#include "device/r4300/cp0.h"
#include "device/r4300/interrupt.h"
#include "api/callbacks.h"
void DebugMessage(int level,const char* format,...){(void)level;(void)format;}
#include "n64-event-queue-scheduler.c.inc"
void om_n64_case_clear_queue(struct cp0* cp0){clear_queue(&cp0->q);}
