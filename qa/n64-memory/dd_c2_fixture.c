/* Same C2 implementation included by actual prepared DD controller source. */
#include "device/dd/dd_controller.h"
#include "device/dd/disk.h"
#include "n64-dd-c2.c.inc"
void om_n64_case_dd_c2(struct dd_controller* dd){read_C2(dd);}
