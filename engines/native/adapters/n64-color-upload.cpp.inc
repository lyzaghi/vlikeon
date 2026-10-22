    if(!om_n64_owner_access())return;
    Cleaner cleaner(this);
    if(!RDRAM || !m_pCurBuffer)return;
    const u32 address=m_pCurBuffer->m_startAddress,width=m_pCurBuffer->m_width,height=_height;
    const u32 x0=0,y0=0,y1=height;
    const u32 pixel_bytes=m_pCurBuffer->m_size==G_IM_SIZ_16b?2:4;
    if(m_pCurBuffer->m_size!=G_IM_SIZ_16b && m_pCurBuffer->m_size!=G_IM_SIZ_32b)return;
    std::size_t pixels=0,source_bytes=0;
    const std::uint64_t capacity=static_cast<std::uint64_t>(RDRAMSize)+1;
    if(capacity>0x800000 || !om_n64_color_extent(static_cast<std::size_t>(capacity),address,width,height,pixel_bytes,pixels,source_bytes))return;
    const bool partial=m_dirty.any(address,source_bytes);
    if(FBInfo::fbInfo.isSupported() && !_fullAlpha && !partial)return;
    if(!prepareImage(width,height))return;
    const auto& fbTexFormats=gfxContext.getFramebufferTextureFormats();
    std::unique_ptr<u32[]> temporary;
    u32* pDst=reinterpret_cast<u32*>(m_pbuf);
    if(fbTexFormats.colorType==datatype::FLOAT){
        temporary.reset(new(std::nothrow) u32[pixels]);if(!temporary)return;pDst=temporary.get();
    }
    bool nonzero=false,valid;
    const OmniN64DirtyBytes* selected=partial?&m_dirty:nullptr;
    if(pixel_bytes==2)valid=om_n64_color_copy<u16>(RDRAM,static_cast<std::size_t>(capacity),address,width,height,selected,pDst,pixels,RGBA16ToABGR32,1,_fullAlpha,nonzero);
    else valid=om_n64_color_copy<u32>(RDRAM,static_cast<std::size_t>(capacity),address,width,height,selected,pDst,pixels,RGBA32ToABGR32,0,_fullAlpha,nonzero);
    if(!valid)return; // private output was never submitted; pending coverage retained
    if(fbTexFormats.colorType==datatype::FLOAT){
        f32* output=reinterpret_cast<f32*>(m_pbuf);const u8* input=reinterpret_cast<const u8*>(pDst);
        for(std::size_t i=0;i<pixels*4;++i)output[i]=input[i]/255.0f;
    }
    const bool bUseAlpha=!_fullAlpha && m_pCurBuffer->m_changed;
    if(!FBInfo::fbInfo.isSupported() && bUseAlpha && config.frameBufferEmulation.copyToRDRAM==Config::ctDisable)
        std::memset(RDRAM+address,0,source_bytes);
    if(!nonzero && !_fullAlpha){m_dirty.clear(address,source_bytes);return;}
