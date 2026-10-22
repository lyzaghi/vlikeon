/* Included after the actual pinned queue insertion. Owner serialization means
 * the pool index cannot change between admission, insertion and acknowledgement. */
#include "n64-core-owner.h"
int om_n64_add_interrupt_checked(struct cp0* cp0,int type,unsigned int delay)
{
    unsigned before;const uint32_t* regs;
    if(!om_n64_owner_access() || !cp0)return 0;
    before=cp0->q.pool.index;
    if(before>=INTERRUPT_NODES_POOL_CAPACITY){om_n64_owner_admission_failed();return 0;}
    regs=r4300_cp0_regs(cp0);
    add_interrupt_event_count(cp0,type,regs[CP0_COUNT_REG]+delay);
    if(cp0->q.pool.index!=before+1){om_n64_owner_admission_failed();return 0;}
    return 1;
}
