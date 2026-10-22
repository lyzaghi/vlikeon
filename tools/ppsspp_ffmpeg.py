"""Adapt the selected PPSSPP callers to the exact modern decoder source/API."""
import shutil
from ffmpeg_source import PIN, prepare as prepare_ffmpeg
from pinned_core_source import ROOT, owned,file_hash
from ppsspp_audio import adapt as adapt_audio


def replace(path,before,after,count=1):
    value=path.read_text(encoding='utf-8')
    if value.count(before)!=count:raise ValueError('Pinned modern-decoder adaptation anchor changed: '+path.name)
    path.write_text(value.replace(before,after),encoding='utf-8',newline='\n')


def install_source(work,source,git='git'):
    decoder,record=prepare_ffmpeg(work/'modern-ffmpeg',git)
    shutil.copy2(ROOT/'security/ffmpeg-source.lock.json',work/'modern-ffmpeg/ffmpeg-source.lock.json')
    record['source_lock_sha256']=file_hash(work/'modern-ffmpeg/ffmpeg-source.lock.json')
    target=owned(source/'ffmpeg')
    if target.exists():
        if target.is_symlink() or any(target.iterdir()):raise ValueError('The old decoder source must not be copied into the current product tree')
        target.rmdir()
    target.parent.mkdir(parents=True,exist_ok=True);decoder.rename(target)
    return record


def adapt(source,native=False):
    helper=source/'Core/OmniPlayFFmpeg.h';shutil.copy2(ROOT/'engines/native/adapters/OmniPlayFFmpeg.h',helper)
    for name in ('Core/HW/MediaEngine.cpp','Core/HW/AvcDecoder.cpp'):
        path=source/name
        replace(path,'#include "Core/FFMPEGCompat.h"','#include "Core/FFMPEGCompat.h"\n#include "Core/OmniPlayFFmpeg.h"')
    simple=source/'Core/HW/SimpleAudioDec.cpp';media=source/'Core/HW/MediaEngine.cpp';avc=source/'Core/HW/AvcDecoder.cpp'
    # The parent's SimpleAudioDec compatibility header is inside extern "C".
    # Keep our C++ owners/templates after that block, under USE_FFMPEG.
    replace(simple,'}\n#include "Core/Config.h"','}\n#include "Core/OmniPlayFFmpeg.h"\n#include "Core/Config.h"')
    replace(simple,'avcodec_open2(codecCtx_, codec_, &opts)','(OmniPlayCodecLimits(codecCtx_), avcodec_open2(codecCtx_, codec_, &opts))')
    replace(media,'avcodec_open2(m_pCodecCtx, pCodec, &opt)','(OmniPlayCodecLimits(m_pCodecCtx), avcodec_open2(m_pCodecCtx, pCodec, &opt))')
    replace(avc,'avcodec_open2(codecCtx_, codec, nullptr)','(OmniPlayCodecLimits(codecCtx_), avcodec_open2(codecCtx_, codec, nullptr))')
    # avcodec_open2 applies its dictionary after context fields: a "threads=0"
    # option would otherwise undo a pre-open single-thread bound.
    replace(media,'av_dict_set(&opt, "threads", "0", 0);','av_dict_set(&opt, "threads", "1", 0);')
    replace(media,'\t\tAVCodecContext *m_pCodecCtx = avcodec_alloc_context3(pCodec);','\t\tAVCodecContext *m_pCodecCtx = avcodec_alloc_context3(pCodec);\n\t\tif (!m_pCodecCtx) return false;')
    replace(media,'\t\t\tWARN_LOG_REPORT(Log::ME, "Failed to prepare context parameters: %08x", paramResult);\n\t\t\treturn false;',
        '\t\t\tWARN_LOG_REPORT(Log::ME, "Failed to prepare context parameters: %08x", paramResult);\n\t\t\tavcodec_free_context(&m_pCodecCtx);\n\t\t\treturn false;')
    replace(media,'\t\tif (openResult < 0) {\n\t\t\treturn false;','\t\tif (openResult < 0) {\n\t\t\tavcodec_free_context(&m_pCodecCtx);\n\t\t\treturn false;')
    before='\tAVPacket packet;\n\tav_init_packet(&packet);'
    owner='\tOmniPlayPacket owner;\n\tif (!owner.value) return false;\n\tAVPacket &packet = *owner.value;'
    replace(media,before,owner)
    replace(simple,before+'\n\tpacket.data = (uint8_t *)(inbuf);\n\tpacket.size = inbytes;',
        '\tif (inbytes < 0 || inbytes > 16 * 1024 * 1024 || (inbytes && !inbuf)) return false;\n'+owner+
        '\n\tif (inbytes && av_new_packet(&packet, inbytes) < 0) return false;\n\tif (inbytes) memcpy(packet.data, inbuf, inbytes);')
    text=avc.read_text(encoding='utf-8')
    if text.count(before)!=2:raise ValueError('The selected AVC packet ownership changed')
    text=text.replace(before,owner,1).replace(before,'\tOmniPlayPacket owner;\n\tif (!owner.value) return;\n\tAVPacket &packet = *owner.value;',1)
    avc.write_text(text,encoding='utf-8',newline='\n')
    replace(avc,'if (!codecCtx_ || !frame_ || !data || size <= 0)', 'if (!codecCtx_ || !frame_ || !data || size <= 0 || size > 16 * 1024 * 1024)')
    replace(avc,'if (!codecCtx_ || !data || size <= 0)', 'if (!codecCtx_ || !data || size <= 0 || size > 16 * 1024 * 1024)')
    replace(avc,'\tAVPacket &packet = *owner.value;\n\tif (data) {','\tAVPacket &packet = *owner.value;\n\tif (size < 0 || size > 16 * 1024 * 1024) return;\n\tif (data) {')
    replace(avc,'\t\tav_parser_parse2(parser_, parserCtx_, &out, &outSize, data, size, AV_NOPTS_VALUE, AV_NOPTS_VALUE, 0);',
        '\t\tpacketBuf_.resize((size_t)size + AV_INPUT_BUFFER_PADDING_SIZE);\n\t\tmemcpy(packetBuf_.data(), data, size);\n\t\tmemset(packetBuf_.data() + size, 0, AV_INPUT_BUFFER_PADDING_SIZE);\n\t\tav_parser_parse2(parser_, parserCtx_, &out, &outSize, packetBuf_.data(), size, AV_NOPTS_VALUE, AV_NOPTS_VALUE, 0);')
    # Keep custom AVIO ownership in the caller. FFmpeg may replace its input
    # buffer, so release its current buffer after closing the format context.
    replace(media,'\tu8 *tempbuf = (u8*)av_malloc(m_bufSize);\n\n\tm_pFormatCtx = avformat_alloc_context();\n\tm_pIOContext = avio_alloc_context(tempbuf, m_bufSize, 0, (void*)this, &MpegReadbuffer, nullptr, nullptr);\n\tm_pFormatCtx->pb = m_pIOContext;',
'''\tif (m_bufSize < 1 || m_bufSize > 16 * 1024 * 1024) return false;
\tu8 *tempbuf = (u8*)av_malloc(m_bufSize);
\tif (!tempbuf) return false;
\tm_pFormatCtx = avformat_alloc_context();
\tif (!m_pFormatCtx) { av_free(tempbuf); return false; }
\tm_pFormatCtx->max_streams = 64;
\tm_pFormatCtx->max_probe_packets = 512;
\tm_pFormatCtx->max_analyze_duration = 2000000;
\tm_pIOContext = avio_alloc_context(tempbuf, m_bufSize, 0, (void*)this, &MpegReadbuffer, nullptr, nullptr);
\tif (!m_pIOContext) { av_free(tempbuf); avformat_free_context(m_pFormatCtx); m_pFormatCtx = nullptr; return false; }
\tm_pFormatCtx->pb = m_pIOContext;
\tm_pFormatCtx->flags |= AVFMT_FLAG_CUSTOM_IO;''')
    replace(media,'\tif (m_pIOContext && m_pIOContext->buffer)\n\t\tav_free(m_pIOContext->buffer);\n\tif (m_pIOContext)\n\t\tav_free(m_pIOContext);','')
    replace(media,'\t\tav_dict_free(&open_opt);\n\t\treturn false;','\t\tav_dict_free(&open_opt);\n\t\tcloseContext();\n\t\treturn false;')
    replace(media,'\tif (m_pFormatCtx)\n\t\tavformat_close_input(&m_pFormatCtx);','\tif (m_pFormatCtx)\n\t\tavformat_close_input(&m_pFormatCtx);\n\tif (m_pIOContext) { av_freep(&m_pIOContext->buffer); avio_context_free(&m_pIOContext); }')
    if native:
        shutil.copy2(ROOT/'engines/native/adapters/ppsspp-ffmpeg-import.cmake',source/'ext/ffmpeg-build/CMakeLists.txt')
        with (source/'Core/CMakeLists.txt').open('a',encoding='utf-8') as target:
            target.write('\n# The explicitly pinned FFmpeg 9 headers return const codec descriptors.\ntarget_compile_definitions(Core PRIVATE HAVE_LIBAVCODEC_CONST_AVCODEC=1)\n')
    else:
        replace(source/'libretro/Makefile','COREFLAGS += -DUSE_FFMPEG','COREFLAGS += -DUSE_FFMPEG -DHAVE_LIBAVCODEC_CONST_AVCODEC=1')
    pcm=adapt_audio(source)
    return {'revision':PIN,'pcm':pcm,'codec_threads':1,'maximum_packet_bytes':16*1024*1024,'maximum_video_pixels':1920*1080,'maximum_audio_frame_samples':8192,
        'maximum_demux_streams':64,'maximum_probe_packets':512,'maximum_analyze_us':2000000,'packet_api':'av_packet_alloc_raii','custom_avio_ownership':True,'verification':'not_run'}
