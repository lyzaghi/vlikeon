/* Preserve pinned pending-slot replacement behavior. Starting a descriptor
 * stages the slot until actual address/format admission succeeded. */
static void fifo_push(struct ai_controller* ai)
{
    uint32_t duration;struct ai_dma staged;
    if(!om_n64_ai_duration_for_request(ai,&duration))return;
    staged.address=ai->regs[AI_DRAM_ADDR_REG];
    staged.length=ai->regs[AI_LEN_REG]&~UINT32_C(7);staged.duration=duration;
    if(ai->regs[AI_STATUS_REG]&AI_STATUS_BUSY){
        ai->fifo[1]=staged;ai->regs[AI_STATUS_REG]|=AI_STATUS_FULL;
    }else{
        if(!do_dma(ai,&staged))return;
        ai->fifo[0]=staged;ai->regs[AI_STATUS_REG]|=AI_STATUS_BUSY;
    }
}
static void fifo_pop(struct ai_controller* ai)
{
    if(!om_n64_owner_access() || !ai)return;
    if(ai->regs[AI_STATUS_REG]&AI_STATUS_FULL){
        struct ai_dma staged=ai->fifo[1];
        if(!do_dma(ai,&staged))return;
        ai->fifo[0]=staged;ai->regs[AI_STATUS_REG]&=~AI_STATUS_FULL;
    }else{
        ai->regs[AI_STATUS_REG]&=~AI_STATUS_BUSY;ai->delayed_carry=0;
    }
}
