/* Actual prepared disk.c, with only calloc supplied by the fault harness. */
#include <stdlib.h>
static int reject_allocation;
void om_n64_dd_case_reject_allocation(int value){reject_allocation=value;}
static void* om_n64_dd_case_calloc(size_t count,size_t bytes)
{return reject_allocation?NULL:calloc(count,bytes);}
#define calloc om_n64_dd_case_calloc
#include "mupen64plus-core/src/device/dd/disk.c"
#undef calloc
