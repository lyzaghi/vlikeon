// OmniPlay bounded memory-file-backed GLideNHQ storage. Persisted records stay
// on an owned readonly snapshot. New records remain bounded pending images;
// save serializes a full private replacement and commits its index last.
class TxFileStorage:public TxCacheImpl {
  struct Entry {
    om_n64_texture::IndexEntry index;
    std::shared_ptr<std::vector<unsigned char>> pending;
  };
  using Key=om_n64_texture::Key;
  using Entries=std::map<Key,Entry>;
  uint32 _options;tx_wstring _cachePath,_filename;std::string _fullPath;
  dispInfoFuncExt _callback;Entries _entries;
  std::unique_ptr<om_n64_texture::File> _snapshot;
  uint64 _totalSize=0,_expanded=0;bool _legacy=false,_dirty=false;
  uint8* _gzdest0=nullptr;uint8* _gzdest1=nullptr;uint32 _gzdestLen=0;
  std::vector<unsigned char> _read;

  Entries::const_iterator find(Checksum checksum,N64FormatSize format)const{
    const auto key=Key(checksum._checksum,_legacy?0:format.formatsize());return _entries.find(key);
  }
  bool data(const Entry& entry,std::vector<unsigned char>& output){
    if(entry.pending){output=*entry.pending;return true;}
    if(!_snapshot)return false;const auto metadata=_legacy?21u:23u;
    output.resize(entry.index.length);
    return _snapshot->seek(entry.index.offset+metadata)&&_snapshot->bytes(output.data(),entry.index.length);
  }
public:
  TxFileStorage(uint32 options,const wchar_t* path,dispInfoFuncExt callback):_options(options),_cachePath(path?path:L""),_callback(callback){
    _gzdest0=TxMemBuf::getInstance()->get(0);_gzdest1=TxMemBuf::getInstance()->get(1);
    _gzdestLen=std::min(TxMemBuf::getInstance()->size_of(0),TxMemBuf::getInstance()->size_of(1));
    if(!_gzdest0||!_gzdest1||!_gzdestLen)_options&=~(GZ_TEXCACHE|GZ_HIRESTEXCACHE);
  }
  ~TxFileStorage()override=default;
  bool add(Checksum checksum,GHQTexInfo* info,int dataSize=0)override{
    try{
      if(!checksum||!info||!info->data||dataSize<0||find(checksum,info->n64_format_size)!=_entries.end()||_entries.size()>=om_n64_texture::maximum_records)return false;
      auto shape=om_n64_texture_shape(*info);std::uint64_t expanded=0;
      if(!om_n64_texture::valid_shape(shape,om_n64_texture_bytes,expanded)||expanded>om_n64_texture::maximum_bytes-_expanded)return false;
      const bool packed=(shape.format&GL_TEXFMT_GZ)!=0;if(packed&&!dataSize)return false;
      std::uint64_t length=dataSize?static_cast<unsigned>(dataSize):expanded;
      if(!om_n64_texture::valid_payload(shape,info->data,static_cast<std::size_t>(length),om_n64_texture_bytes))return false;
      const uint8* source=info->data;
      if(!packed&&(_options&(GZ_TEXCACHE|GZ_HIRESTEXCACHE))&&_gzdest0&&_gzdest1){
        auto* destination=source==_gzdest0?_gzdest1:_gzdest0;uLongf actual=_gzdestLen;
        if(compress2(destination,&actual,source,static_cast<uLong>(length),1)==Z_OK){source=destination;length=actual;shape.format|=GL_TEXFMT_GZ;}
      }
      const auto metadata=_legacy?21u:23u;
      const auto overhead=(_legacy?12ull:16ull)+4+std::uint64_t(_entries.size()+1)*(metadata+16);
      if(overhead>om_n64_texture::maximum_file_bytes||_totalSize>om_n64_texture::maximum_file_bytes-overhead||length>om_n64_texture::maximum_file_bytes-overhead-_totalSize)return false;
      Entry entry;entry.index.shape=shape;entry.index.length=static_cast<std::uint32_t>(length);
      entry.pending=std::make_shared<std::vector<unsigned char>>(source,source+length);
      if(!_entries.emplace(Key(checksum._checksum,_legacy?0:shape.n64_format_size),std::move(entry)).second)return false;
      _totalSize+=length;_expanded+=expanded;_dirty=true;return true;
    }catch(...){return false;}
  }
  bool get(Checksum checksum,N64FormatSize format,GHQTexInfo* output)override{
    try{
      if(!output||!checksum)return false;const auto found=find(checksum,format);if(found==_entries.end())return false;
      std::vector<unsigned char> payload;if(!data(found->second,payload))return false;
      const auto& shape=found->second.index.shape;
      if(!om_n64_texture::valid_payload(shape,payload.data(),payload.size(),om_n64_texture_bytes))return false;
      om_n64_texture::Record record;record.shape=shape;GHQTexInfo candidate=om_n64_texture_info(record);
      if(shape.format&GL_TEXFMT_GZ){
        const auto expected=om_n64_texture_bytes(shape);if(!_gzdest0||expected>_gzdestLen)return false;
        uLongf actual=_gzdestLen;if(uncompress(_gzdest0,&actual,payload.data(),static_cast<uLong>(payload.size()))!=Z_OK||actual!=expected)return false;
        candidate.data=_gzdest0;candidate.format&=~GL_TEXFMT_GZ;
      }else{_read.swap(payload);candidate.data=_read.data();}
      *output=candidate;return true;
    }catch(...){return false;}
  }
  bool save(const wchar_t* path,const wchar_t* filename,int config)override{
    try{
      if(!path||!filename||_cachePath!=path||(!_filename.empty()&&_filename!=filename)||_entries.empty())return false;
      auto target=om_n64_cache_path(path,filename);if(target.empty())return false;
      tx_wstring spelling(filename);om_n64_texture::File output(target.c_str(),true);if(!output)return false;
      const std::int32_t settings=config;std::int64_t indexOffset=0;
      if((!_legacy&&!output.field_out(om_n64_texture::version))||!output.field_out(settings)||!output.field_out(indexOffset))return false;
      Entries replacement;std::vector<unsigned char> payload;
      for(const auto& item:_entries){
        if(!data(item.second,payload)||!om_n64_texture::valid_payload(item.second.index.shape,payload.data(),payload.size(),om_n64_texture_bytes))return false;
        const auto position=output.tell();if(position<0)return false;Entry entry;entry.index=item.second.index;entry.index.offset=static_cast<std::uint64_t>(position);
        if(!om_n64_texture::shape_out(output,entry.index.shape,entry.index.length,_legacy)||!output.put(payload.data(),entry.index.length))return false;
        replacement.emplace(item.first,std::move(entry));
      }
      indexOffset=output.tell();if(indexOffset<0)return false;const auto count=static_cast<std::int32_t>(replacement.size());
      if(!output.field_out(count))return false;
      for(const auto& item:replacement){const auto packed=item.second.index.offset|(std::uint64_t(_legacy?0:item.first.second)<<48);
        if(!output.field_out(item.first.first)||!output.field_out(packed))return false;
      }
      if(!output.seek(_legacy?4:8)||!output.field_out(indexOffset)||!output.close())return false;
      std::unique_ptr<om_n64_texture::File> snapshot(new om_n64_texture::File(target.c_str(),false));if(!*snapshot)return false;
      // A reopen fault can follow an already successful publication. Preserve
      // the former runtime snapshot/pending records and report failure; retry
      // serializes the same full logical cache, never a partial index.
      _snapshot.swap(snapshot);_entries.swap(replacement);_filename.swap(spelling);_fullPath.swap(target);_dirty=false;return true;
    }catch(...){return false;}
  }
  bool load(const wchar_t* path,const wchar_t* filename,int config,bool force)override{
    try{
      if(!path||!filename||_cachePath!=path||(!_filename.empty()&&_filename!=filename))return false;
      auto target=om_n64_cache_path(path,filename);if(target.empty())return false;
      std::unique_ptr<om_n64_texture::File> snapshot(new om_n64_texture::File(target.c_str(),false));if(!*snapshot)return false;
      om_n64_texture::Index index;if(!om_n64_texture::read_index(*snapshot,config,force,om_n64_texture_bytes,index))return false;
      Entries replacement;std::uint64_t expanded=0;
      for(const auto& item:index.records){Entry entry;entry.index=item.second;replacement.emplace(item.first,std::move(entry));expanded+=om_n64_texture_bytes(item.second.shape);}
      tx_wstring spelling(filename);_snapshot.swap(snapshot);_entries.swap(replacement);_filename.swap(spelling);_fullPath.swap(target);
      _totalSize=index.stored;_expanded=expanded;_legacy=index.legacy;_dirty=false;return true;
    }catch(...){return false;}
  }
  void clear()override{_entries.clear();_snapshot.reset();_read.clear();_totalSize=_expanded=0;_legacy=false;_dirty=false;}
  bool del(Checksum)override{return false;}
  bool isCached(Checksum checksum,N64FormatSize format)const override{return find(checksum,format)!=_entries.end();}
  bool empty()const override{return _entries.empty();}
  uint64 size()const override{return _entries.size();}
  uint64 totalSize()const override{return _totalSize;}
  uint64 cacheLimit()const override{return om_n64_texture::maximum_file_bytes;}
  uint32 getOptions()const override{return _options;}
  void setOptions(uint32 options)override{_options=options;}
};
