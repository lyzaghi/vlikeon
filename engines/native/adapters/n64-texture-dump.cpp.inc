// Pinned texture dump shape/naming, with bounded scratch-only publication.
boolean TxFilter::dmptx(uint8* source,int width,int height,int rowStridePixel,
    ColorFormat format,N64FormatSize n64FmtSz,Checksum checksum,boolean strong)
{
    om_n64_texture::TexturePool::Use use(om_n64_texture::pool());
    if (!use || !_initialized || !source || !_txImage || !_txQuantize || !(_options&DUMP_TEX) ||
        width<=0 || height<=0 || rowStridePixel<width || rowStridePixel>_maxwidth || height>_maxheight ||
        TxUtil::sizeofTx(rowStridePixel,height,format)<=0 || _dumpPath.empty() || _ident.empty()) return 0;
    try {
        if (format!=graphics::internalcolorFormat::RGBA8) {
            if (!_txQuantize->quantize(source,_tex1,rowStridePixel,height,format,graphics::internalcolorFormat::RGBA8)) return 0;
            source=_tex1;
        }
        const tx_wstring directory=_dumpPath+L"/"+_ident+(strong?L"/GLideNHQ_strong_crc":L"/GLideNHQ");
        std::string parent;if (!om_n64_utf8(directory.c_str(),parent) || om_n64_mkdir(parent.c_str())!=0) return 0;
        std::wostringstream leaf;leaf<<_ident<<L"#"<<std::uppercase<<std::hex<<std::setfill(L'0')<<std::setw(8)<<checksum._texture
            <<L"#"<<unsigned(n64FmtSz._format)<<L"#"<<unsigned(n64FmtSz._size);
        if (n64FmtSz._format==2) leaf<<L"#"<<std::setw(8)<<checksum._palette<<L"_ciByRGBA.png";
        else leaf<<L"_all.png";
        const auto path=om_n64_cache_path(directory.c_str(),leaf.str().c_str());if (path.empty()) return 0;
        struct Writer {
            OmniN64File* file;
            ~Writer(){if(file){om_n64_abort(file);om_n64_fclose(file);}}
        } writer{om_n64_fopen(path.c_str(),"wb")};
        if (!writer.file || !_txImage->writePNG(source,writer.file,width,height,rowStridePixel*4,graphics::internalcolorFormat::RGBA8)) return 0;
        auto* file=writer.file;writer.file=nullptr;return om_n64_fclose(file)==0?1:0;
    } catch (...) {return 0;}
}
