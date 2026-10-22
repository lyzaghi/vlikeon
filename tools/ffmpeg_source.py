"""Pinned modern decoder source and explicit, finite product build policies."""
import json
from pinned_core_source import ROOT, file_hash, materialize, source_inventory

PIN='946fcce07b6dcd0331c8cc609192aeff5e1924f8'
VERSION='9.0.2'
SELECTED=('COPYING.GPLv2','COPYING.GPLv3','COPYING.LGPLv2.1','COPYING.LGPLv3','CREDITS','LICENSE.md','README.md','RELEASE',
    'Makefile','configure','ffbuild','compat','doc','tests','tools','fftools','libavcodec','libavdevice','libavfilter','libavformat','libavutil','libswresample','libswscale')
LIBRARIES=('avformat','avcodec','swresample','swscale','avutil')
DECODERS=('h264','aac','atrac3','atrac3p','mp3')
DEMUXERS=('mpegps','mpegts','oma','mp3','aac')
PARSERS=('h264','aac','mpegaudio')


def prepare(work,git='git'):
    lock=json.loads((ROOT/'security/ffmpeg-source.lock.json').read_text(encoding='utf-8'))
    if lock.get('revision')!=PIN or lock.get('version')!=VERSION:raise ValueError('The decoder source lock differs from this producer')
    source,archive=materialize('ffmpeg',PIN,work,git,SELECTED)
    archive_hash=file_hash(archive);archive.unlink()
    for name in ('fftools/Makefile','tools/Makefile','ffbuild/common.mak','ffbuild/library.mak','doc/Makefile','doc/examples/Makefile','tests/Makefile'):
        if not (source/name).is_file():raise ValueError('A selected FFmpeg makefile source input is absent: '+name)
    if (source/'RELEASE').read_text(encoding='utf-8').strip()!=VERSION:raise ValueError('The committed FFmpeg release identity changed')
    return source,{'schema_version':1,'version':VERSION,'revision':PIN,'source_archive_sha256':archive_hash,'selected_committed_paths':SELECTED,
        'prepared_source_sha256':source_inventory(source),'decoders':DECODERS,'demuxers':DEMUXERS,'parsers':PARSERS,
        'verification':'not_run','distribution_approved':False}


def configure_policy():
    # Disable component autodetection and all URL protocols: callers supply their
    # own checked packet/custom-AVIO bytes, never network/file URLs to FFmpeg.
    return ['--disable-autodetect','--disable-everything','--enable-static','--disable-shared','--disable-programs','--disable-doc','--disable-debug',
        '--disable-network','--disable-avdevice','--disable-avfilter','--disable-encoders','--disable-muxers','--disable-hwaccels','--disable-devices',
        '--disable-protocols','--enable-avcodec','--enable-avformat','--enable-avutil','--enable-swresample','--enable-swscale',
        '--enable-decoder='+','.join(DECODERS),'--enable-demuxer='+','.join(DEMUXERS),'--enable-parser='+','.join(PARSERS),
        '--disable-gpl','--disable-nonfree','--disable-version3','--disable-asm','--disable-inline-asm','--disable-x86asm']
