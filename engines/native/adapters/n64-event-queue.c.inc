/* Included in the actual pinned interrupt.c, retaining its scheduler/pool. */
#include "n64-snapshot-bounds.h"
int save_eventqueue_infos(const struct cp0* cp0,char* buf,size_t bytes)
{
    unsigned char staged[INTERRUPT_NODES_POOL_CAPACITY*8+4];
    const struct node* e;
    unsigned int seen=0;
    size_t count=0,i;
    uint32_t sentinel=UINT32_MAX;
    if(!cp0 || !buf)return 0;
    for(e=cp0->q.first;e!=NULL;e=e->next){
        /* Check identity before dereferencing a potentially stale/cyclic link. */
        for(i=0;i<INTERRUPT_NODES_POOL_CAPACITY && e!=&cp0->q.pool.nodes[i];++i){}
        if(i==INTERRUPT_NODES_POOL_CAPACITY || (seen&(1u<<i)) || count==INTERRUPT_NODES_POOL_CAPACITY)return 0;
        seen|=1u<<i;
        memcpy(staged+count*8,&e->data.type,4);
        memcpy(staged+count*8+4,&e->data.count,4);
        ++count;
    }
    memcpy(staged+count*8,&sentinel,4);
    if(bytes<count*8+4 || !om_n64_snapshot_queue_valid(staged,count*8+4,0))return 0;
    /* Padding is serialized too; never publish uninitialized stack bytes. */
    memset(buf,0,bytes);memcpy(buf,staged,count*8+4);
    return (int)(count*8+4);
}
int load_eventqueue_infos(struct cp0* cp0,const char* buf,size_t bytes)
{
    size_t at=0;
    uint32_t type,count;
    uint32_t* cp0_regs;
    if(!cp0 || !om_n64_snapshot_queue_valid(buf,bytes,0))return 0;
    cp0_regs=r4300_cp0_regs(cp0);
    clear_queue(&cp0->q);
    for(;;at+=8){
        memcpy(&type,buf+at,4);
        if(type==UINT32_MAX)break;
        memcpy(&count,buf+at+4,4);
        /* Canonicalize SPECIAL exactly once, leaving room in the real pool. */
        if(type!=SPECIAL_INT)add_interrupt_event_count(cp0,(int)type,count);
    }
    add_interrupt_event_count(cp0,SPECIAL_INT,((cp0_regs[CP0_COUNT_REG]&UINT32_C(0x80000000))^UINT32_C(0x80000000)));
    return 1;
}
