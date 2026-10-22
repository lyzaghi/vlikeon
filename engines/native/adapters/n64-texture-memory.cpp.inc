// OmniPlay replacement methods for the pinned GLideNHQ TxMemoryCache.
static_assert(sizeof(uint32)==4&&sizeof(uint64)==8&&sizeof(uint16)==2&&sizeof(int)==4,
  "Pinned texture cache scalar layout changed");
static om_n64_texture::Shape om_n64_texture_shape(const GHQTexInfo& info){
  om_n64_texture::Shape shape;shape.width=info.width;shape.height=info.height;shape.format=info.format;
  shape.texture_format=info.texture_format;shape.pixel_type=info.pixel_type;
  shape.hires=info.is_hires_tex;shape.n64_format_size=info.n64_format_size.formatsize();return shape;
}
static std::uint64_t om_n64_texture_bytes(const om_n64_texture::Shape& shape){
  if(!shape.width||!shape.height||shape.width>om_n64_texture::maximum_dimension||shape.height>om_n64_texture::maximum_dimension)return 0;
  const auto pool=om_n64_texture::pool().stats();
  if(!pool.active||shape.width>pool.width||shape.height>pool.height)return 0;
  const auto format=shape.format&~GL_TEXFMT_GZ;std::uint64_t stride=0;
  if(format==static_cast<u32>(graphics::internalcolorFormat::COLOR_INDEX8))stride=1;
  else if(format==static_cast<u32>(graphics::internalcolorFormat::RGBA4)||format==static_cast<u32>(graphics::internalcolorFormat::RGB5_A1)||format==static_cast<u32>(graphics::internalcolorFormat::RGB8))stride=2;
  else if(format==static_cast<u32>(graphics::internalcolorFormat::RGBA8))stride=4;
  std::uint16_t texture=static_cast<std::uint16_t>(static_cast<u32>(graphics::colorFormat::RGBA));
  std::uint16_t pixel=static_cast<std::uint16_t>(static_cast<u32>(graphics::datatype::UNSIGNED_BYTE));
  if(format==static_cast<u32>(graphics::internalcolorFormat::RGB8)){texture=static_cast<std::uint16_t>(static_cast<u32>(graphics::colorFormat::RED_GREEN_BLUE));pixel=static_cast<std::uint16_t>(static_cast<u32>(graphics::datatype::UNSIGNED_SHORT_5_6_5));}
  else if(format==static_cast<u32>(graphics::internalcolorFormat::RGBA4))pixel=static_cast<std::uint16_t>(static_cast<u32>(graphics::datatype::UNSIGNED_SHORT_4_4_4_4));
  else if(format==static_cast<u32>(graphics::internalcolorFormat::RGB5_A1))pixel=static_cast<std::uint16_t>(static_cast<u32>(graphics::datatype::UNSIGNED_SHORT_5_5_5_1));
  if(shape.texture_format!=texture||shape.pixel_type!=pixel)return 0;
  return std::uint64_t(shape.width)*shape.height*stride;
}
static GHQTexInfo om_n64_texture_info(const om_n64_texture::Record& record){
  GHQTexInfo info;info.width=record.shape.width;info.height=record.shape.height;info.format=record.shape.format;
  info.texture_format=record.shape.texture_format;info.pixel_type=record.shape.pixel_type;info.is_hires_tex=record.shape.hires;
  info.n64_format_size._formatsize=record.shape.n64_format_size;return info;
}

bool TxMemoryCache::add(Checksum checksum,GHQTexInfo* info,int dataSize){
  try{
    if(!checksum||!info||!info->data||dataSize<0||find(checksum,info->n64_format_size)!=_cache.end()||_cache.size()>=om_n64_texture::maximum_records)return false;
    auto shape=om_n64_texture_shape(*info);std::uint64_t decoded=0;
    if(!om_n64_texture::valid_shape(shape,om_n64_texture_bytes,decoded))return false;
    const bool packed=(shape.format&GL_TEXFMT_GZ)!=0;
    if(packed&&(!dataSize||!_gzdest0||!_gzdest1))return false;
    std::uint64_t length=dataSize?static_cast<unsigned>(dataSize):decoded;
    if(!om_n64_texture::valid_payload(shape,info->data,static_cast<std::size_t>(length),om_n64_texture_bytes))return false;
    uint8* source=info->data;
    if(!packed&&(_options&(GZ_TEXCACHE|GZ_HIRESTEXCACHE))&&_gzdest0&&_gzdest1){
      auto* compressed=source==_gzdest0?_gzdest1:_gzdest0;uLongf count=_gzdestLen;
      if(compress2(compressed,&count,source,static_cast<uLong>(length),1)==Z_OK){source=compressed;length=count;shape.format|=GL_TEXFMT_GZ;}
    }
    // Stage payload, metadata, LRU node and map insertion before evicting any
    // previously useful entry. Every failed allocation preserves the old cache.
    if(length>_cacheLimit||(!_evictionEnabled&&length>_cacheLimit-_totalSize))return false;
    std::unique_ptr<uint8,decltype(&free)> payload(static_cast<uint8*>(malloc(static_cast<std::size_t>(length))),&free);
    if(!payload)return false;std::memcpy(payload.get(),source,static_cast<std::size_t>(length));
    std::unique_ptr<TXCACHE> node(new TXCACHE);node->info=*info;node->info.format=shape.format;
    node->info.data=payload.get();node->size=static_cast<int>(length);
    _cachelist.push_back(checksum);node->it=--_cachelist.end();
    try{_cache.emplace(checksum._checksum,node.get());}catch(...){_cachelist.erase(node->it);throw;}
    payload.release();node.release();_totalSize+=length;
    while(_totalSize>_cacheLimit){
      const auto oldest=_cachelist.begin();const auto range=_cache.equal_range(*oldest);auto victim=range.first;
      while(victim!=range.second&&victim->second->it!=oldest)++victim;
      if(victim==range.second)std::terminate(); // Internal ownership invariant.
      auto* entry=victim->second;_totalSize-=entry->size;_cache.erase(victim);_cachelist.erase(oldest);free(entry->info.data);delete entry;
    }
    return true;
  }catch(...){return false;}
}

bool TxMemoryCache::get(Checksum checksum,N64FormatSize n64FmtSz,GHQTexInfo* info){
  if(!info||!checksum)return false;auto found=find(checksum,n64FmtSz);if(found==_cache.end())return false;
  auto* entry=found->second;GHQTexInfo candidate=entry->info;
  if(candidate.format&GL_TEXFMT_GZ){
    if(!_gzdest0||!_gzdest1)return false;auto* destination=candidate.data==_gzdest0?_gzdest1:_gzdest0;
    const auto expected=om_n64_texture_bytes(om_n64_texture_shape(candidate));if(!expected||expected>_gzdestLen)return false;
    uLongf actual=_gzdestLen;
    if(uncompress(destination,&actual,candidate.data,entry->size)!=Z_OK||actual!=expected)return false;
    candidate.data=destination;candidate.format&=~GL_TEXFMT_GZ;
  }
  _cachelist.splice(_cachelist.end(),_cachelist,entry->it);*info=candidate;return true;
}

bool TxMemoryCache::save(const wchar_t* path,const wchar_t* filename,int config){
  try{
    if(_cache.empty()||!path||!filename||_cachePath!=path||(!_filename.empty()&&_filename!=filename))return false;
    const auto target=om_n64_cache_path(path,filename);if(target.empty())return false;
    om_n64_texture::Gzip output(target.c_str(),true);if(!output)return false;
    const std::int32_t header=config;
    if((!_isOldVersion&&!output.field_out(om_n64_texture::version))||!output.field_out(header))return false;
    std::uint64_t expanded=0;
    for(const auto& item:_cache){
      const auto& entry=*item.second;const auto shape=om_n64_texture_shape(entry.info);const auto length=static_cast<std::uint32_t>(entry.size);
      std::uint64_t decoded=0;if(!om_n64_texture::valid_shape(shape,om_n64_texture_bytes,decoded)||decoded>om_n64_texture::maximum_bytes-expanded||
        !om_n64_texture::valid_payload(shape,entry.info.data,length,om_n64_texture_bytes))return false;
      expanded+=decoded;
      if(!output.field_out(item.first)||!om_n64_texture::shape_out(output,shape,length,_isOldVersion)||!output.put(entry.info.data,length))return false;
    }
    if(!output.close())return false;_filename=filename;_fullPath=target;return true;
  }catch(...){return false;}
}

bool TxMemoryCache::load(const wchar_t* path,const wchar_t* filename,int config,bool force){
  try{
    if(!path||!filename||_cachePath!=path||(!_filename.empty()&&_filename!=filename))return false;
    auto target=om_n64_cache_path(path,filename);if(target.empty())return false;
    om_n64_texture::Image image;if(!om_n64_texture::read_gzip(target.c_str(),config,force,om_n64_texture_bytes,image))return false;
    TxMemoryCache candidate(_options,path,_evictionEnabled?_cacheLimit:0,nullptr);candidate._isOldVersion=image.legacy;
    for(auto& record:image.records){auto info=om_n64_texture_info(record);info.data=record.data.data();
      if(!candidate.add(Checksum(record.checksum),&info,static_cast<int>(record.data.size())))return false;
    }
    tx_wstring spelling(filename); // Throwing work precedes the nonthrowing swap.
    _cache.swap(candidate._cache);_cachelist.swap(candidate._cachelist);std::swap(_totalSize,candidate._totalSize);
    _isOldVersion=image.legacy;_filename.swap(spelling);_fullPath.swap(target);return true;
  }catch(...){return false;}
}
