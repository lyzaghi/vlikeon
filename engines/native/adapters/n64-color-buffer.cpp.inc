/* Actual renderer methods. Pending writes retain byte addresses only, never
 * a FrameBuffer borrow which could outlive a list removal/replacement. */
void RDRAMtoColorBuffer::addAddress(u32 address,u32 bytes)
{
    if(!om_n64_owner_access() || !RDRAM || !bytes)return;
    const std::uint64_t capacity=static_cast<std::uint64_t>(RDRAMSize)+1;
    if(capacity>OmniN64DirtyBytes::capacity || address>capacity || bytes>capacity-address)return;
    if(m_dirty.mark(address,bytes))gDP.colorImage.changed=TRUE;
}
bool RDRAMtoColorBuffer::prepareImage(u32 width,u32 height)
{
    if(!m_pTexture || !width || !height)return false;
    const auto& formats=gfxContext.getFramebufferTextureFormats();
    const u32 stride=formats.colorType==datatype::FLOAT?16:4;
    const auto maximum=gfxContext.getMaxTextureSize();
    if(formats.colorFormatBytes!=stride || maximum<=0 || width>static_cast<u32>(maximum)
        || height>static_cast<u32>(maximum) || width>0x4000000u/stride
        || height>0x4000000u/(static_cast<std::size_t>(width)*stride))return false;
    const std::size_t bytes=static_cast<std::size_t>(width)*height*stride;
    if(bytes>m_capacity){
        void* next=std::realloc(m_pbuf,bytes);
        if(!next)return false; // old storage and all pending writes retained
        m_pbuf=static_cast<u8*>(next);m_capacity=bytes;
    }
    if(m_pTexture->width!=width || m_pTexture->height!=height){
        Context::InitTextureParams params;params.handle=m_pTexture->name;
        params.width=width;params.height=height;params.internalFormat=formats.colorInternalFormat;
        params.format=formats.colorFormat;params.dataType=formats.colorType;
        gfxContext.init2DTexture(params);
        m_pTexture->width=width;m_pTexture->height=height;
    }
    m_pTexture->textureBytes=static_cast<u32>(bytes);return true;
}
void RDRAMtoColorBuffer::copyFromRDRAM(u32 address,bool cfb)
{
    if(!om_n64_owner_access())return;
    if(!cfb && config.frameBufferEmulation.copyFromRDRAM==0 && !FBInfo::fbInfo.isSupported())return;
    FrameBuffer* buffer=frameBufferList().findBuffer(address);
    if(!buffer)return;
    m_pCurBuffer=buffer;
    if(buffer->m_size!=G_IM_SIZ_16b && buffer->m_size!=G_IM_SIZ_32b){reset();return;}
    const std::uint64_t capacity=static_cast<std::uint64_t>(RDRAMSize)+1;
    const u32 pixel_bytes=buffer->m_size==G_IM_SIZ_16b?2:4;
    if(capacity>0x800000 || buffer->m_startAddress>=capacity || !buffer->m_width
        || buffer->m_width>(capacity-buffer->m_startAddress)/pixel_bytes){reset();return;}
    const u32 requested=buffer->m_startAddress==address?VI.real_height:VI_GetMaxBufferHeight(buffer->m_width);
    const u32 available=static_cast<u32>((capacity-buffer->m_startAddress)/(static_cast<std::size_t>(buffer->m_width)*pixel_bytes));
    const u32 height=requested<available?requested:available;
    _copyFromRDRAM(height,cfb);
}
void RDRAMtoColorBuffer::copyFromRDRAM(FrameBuffer* buffer)
{
    if(!om_n64_owner_access() || !buffer)return;
    m_pCurBuffer=buffer;
    if(buffer->m_size!=G_IM_SIZ_16b && buffer->m_size!=G_IM_SIZ_32b){reset();return;}
    const std::uint64_t capacity=static_cast<std::uint64_t>(RDRAMSize)+1;
    const u32 pixel_bytes=buffer->m_size==G_IM_SIZ_16b?2:4;
    if(capacity>0x800000 || buffer->m_startAddress>=capacity || !buffer->m_width
        || buffer->m_width>(capacity-buffer->m_startAddress)/pixel_bytes){reset();return;}
    const u32 requested=VI_GetMaxBufferHeight(buffer->m_width);
    const u32 available=static_cast<u32>((capacity-buffer->m_startAddress)/(static_cast<std::size_t>(buffer->m_width)*pixel_bytes));
    _copyFromRDRAM(requested<available?requested:available,true);
}
void RDRAMtoColorBuffer::reset(){m_pCurBuffer=nullptr;}
