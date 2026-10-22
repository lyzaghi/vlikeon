"""Pinned native N64 memory-I/O adaptations. Source preparation only.

These are explicit source transforms, not a test or an enabled product route.
Remaining peripheral/state write closure and exact package admission remain due.
"""
import re


def replace(path,before,after,count=1):
    raw=path.read_text(encoding='utf-8')
    if raw.count(before)!=count:raise ValueError('Pinned N64 memory-I/O anchor changed: '+path.as_posix())
    path.write_text(raw.replace(before,after),encoding='utf-8',newline='\n')


def region(path,begin,end,transform):
    raw=path.read_text(encoding='utf-8')
    if raw.count(begin)!=1:raise ValueError('Pinned N64 memory-I/O method changed: '+path.as_posix())
    at=raw.index(begin);stop=raw.index(end,at)
    path.write_text(raw[:at]+transform(raw[at:stop])+raw[stop:],encoding='utf-8',newline='\n')


def stdio(raw,names):
    for name in names:raw=re.sub(r'\b'+name+r'\b','OmniN64File'if name=='FILE'else 'om_n64_'+name,raw)
    return raw


def configuration(source):
    core=source/'libretro/libretro.c'
    replace(core,'#include "libretro_private.h"','#include "libretro_private.h"\n#include "n64-memory-io.h"\nstatic int omniplay_memory_core_live=0;\nstatic int omniplay_memory_startup_ready=0;')
    def copy_file(_):
        return '''static int copy_file(char *ininame,char *fileName)
{
    const char* filename=ConfigGetSharedDataFilepath(fileName);
    OmniN64File* file=om_n64_fopen(filename,"wb");
    int wrote,closed;
    if (!file) return 0;
    wrote=om_n64_fputs(ininame,file)==0;
    closed=om_n64_fclose(file)==0;
    return wrote && closed;
}

'''
    region(core,'void copy_file(char * ininame, char * fileName)','void retro_init(void)',copy_file)
    region(core,'void retro_init(void)\n{','    struct retro_log_callback log;',lambda _:'''void retro_init(void)
{
    if (omniplay_memory_core_live) {environ_cb(RETRO_ENVIRONMENT_SHUTDOWN,NULL);return;}
    omniplay_memory_core_live=1;
    omniplay_memory_startup_ready=0;
    if (!om_n64_files_installed() || om_n64_mkdir("scratch/Mupen64plus")!=0 || !copy_file(inifile,"mupen64plus.ini"))
    {environ_cb(RETRO_ENVIRONMENT_SHUTDOWN,NULL);return;}

''')
    replace(core,'game_thread = co_create(65536 * sizeof(void*) * 16, (void (*)(void))EmuThreadFunction);',
        'game_thread = co_create(65536 * sizeof(void*) * 16, (void (*)(void))EmuThreadFunction);\n        if (!game_thread) {environ_cb(RETRO_ENVIRONMENT_SHUTDOWN,NULL);return;}')
    startup='m64p_error ret = CoreStartup(FRONTEND_API_VERSION, "scratch", "scratch", NULL, n64DebugCallback, 0, n64StateCallback);'
    replace(core,startup,startup+'\n    omniplay_memory_startup_ready=ret==M64ERR_SUCCESS;')
    replace(core,'void retro_deinit(void)\n{','static void omniplay_upstream_deinit(void)\n{')
    replace(core,'bool retro_load_game(const struct retro_game_info *game)\n{','bool retro_load_game(const struct retro_game_info *game)\n{\n    if (!omniplay_memory_startup_ready || !game) return false;')
    replace(core,'void retro_run (void)\n{','void retro_run (void)\n{\n    if (!omniplay_memory_startup_ready || !load_game_successful) return;')
    with core.open('a',encoding='utf-8',newline='\n')as out:
        out.write('''
/* OmniPlay: retain the table through the real guest/cache shutdown boundary. */
void retro_deinit(void)
{
    if (!omniplay_memory_core_live) return;
    if (game_thread && co_active()==game_thread) {environ_cb(RETRO_ENVIRONMENT_SHUTDOWN,NULL);return;}
    if (omniplay_memory_startup_ready) omniplay_upstream_deinit();
    if (game_thread) {co_delete(game_thread);game_thread=NULL;}
    load_game_successful=false;
    omniplay_memory_startup_ready=0;
    omniplay_memory_core_live=0;
}
RETRO_API int omniplay_install_writable_files_v1(const struct OmniWritableFilesV1* table)
{
    if (omniplay_memory_core_live) return 0;
    return om_n64_files_install(table);
}
''')
    exports=source/'libretro/link.T'
    replace(exports,'global: retro_*;','global: retro_*; omniplay_install_writable_files_v1;')

    config=source/'custom/mupen64plus-core/api/config.c'
    replace(config,'#include <libretro.h>','#include <libretro.h>\n#include "n64-memory-io.h"')
    def writer(raw):
        raw=stdio(raw,['FILE','fopen','fprintf','fclose'])
        return raw.replace('    om_n64_fclose(fPtr);\n    return M64ERR_SUCCESS;',
            '    return om_n64_fclose(fPtr)==0 ? M64ERR_SUCCESS : M64ERR_FILES;')
    region(config,'static m64p_error write_configlist_file(void)','/* ----------------------------------------------------------- */',writer)
    region(config,'EXPORT const char * CALL ConfigGetSharedDataFilepath(const char *filename)','EXPORT const char * CALL ConfigGetUserConfigPath(void)',lambda _:'''EXPORT const char * CALL ConfigGetSharedDataFilepath(const char *filename)
{
    static char systemDir[2048];
    char* base=NULL;
    size_t size=0,leaf=0;
    int count;
    if (!filename) return NULL;
    while (leaf<=255 && filename[leaf]) ++leaf;
    if (!leaf || leaf>255 || strchr(filename,'/') || strchr(filename,'\\\\') || strchr(filename,':') || !strcmp(filename,".") || !strcmp(filename,"..")) return NULL;
    if (!strcmp(filename,"mupen64plus.ini")) return "scratch/Mupen64plus/mupen64plus.ini";
    if (!strcmp(filename,"GLideN64.custom.ini")) return "scratch/Mupen64plus/GLideN64.custom.ini";
    /* Other assets, including fonts, retain the readonly system-directory path. */
    if (!environ_cb(RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY,&base) || !base) return NULL;
    while (size<=1536 && base[size]) ++size;
    if (!size || size>1536) return NULL;
    count=snprintf(systemDir,sizeof(systemDir),"%s%sMupen64plus/%s",base,(base[size-1]=='/' || base[size-1]=='\\\\')?"":"/",filename);
    return count<0 || (size_t)count>=sizeof(systemDir) ? NULL : systemDir;
}

''')
    for name in ['Config','Data','Cache']:
        replace(config,'EXPORT const char * CALL ConfigGetUser'+name+'Path(void)\n{\n  return "";\n}',
            'EXPORT const char * CALL ConfigGetUser'+name+'Path(void)\n{\n  return "scratch/Mupen64plus";\n}')

    custom=source/'custom/GLideN64/mupenplus/Config_mupenplus.cpp'
    replace(custom,'#include <algorithm>','#include <algorithm>\n#include <cctype>\n#include <type_traits>\n#include <utility>\n#include "n64-memory-text.hpp"')
    def settings(raw):
        at=raw.index('\t\tini_line l = ini_parse_line(&line);')
        tail=raw.index('\t\tif (internal) {',at)
        parser=raw[at:tail].replace('config.','candidate.')
        return '''void LoadCustomSettings(bool internal)
{
    try {
        std::string contents;
        if (internal) contents.assign(customini,sizeof(customini)-1);
        else if (!om_n64_text::read(ConfigGetSharedDataFilepath("GLideN64.custom.ini"),4u*1024u*1024u,contents)) return;
        Config candidate=config;
        static_assert(std::is_nothrow_move_assignable<Config>::value,"Config commit must not throw");
        std::string myString=replaceChars(RSP.romname);
        std::transform(myString.begin(),myString.end(),myString.begin(),[](unsigned char c){return static_cast<char>(std::toupper(c));});
        bool found=false;
        std::size_t position=0;
        while (position<contents.size()) {
            const auto end=contents.find('\\n',position);
            char* line=&contents[position];
            if (end!=std::string::npos) contents[end]=0;
            position=end==std::string::npos ? contents.size() : end+1;
'''+parser+'''
        }
        config=std::move(candidate);
    } catch (...) { return; }
}

'''
    region(custom,'void LoadCustomSettings(bool internal)','extern "C" void Config_LoadConfig()',settings)


def rom_database(source):
    path=source/'mupen64plus-core/src/main/rom.c'
    replace(path,'#include "rom.h"','#include "rom.h"\n#include "n64-memory-io.h"')
    replace(path,'static _romdatabase g_romdatabase;','static _romdatabase g_romdatabase;\nstatic int omniplay_romdb_resolve_oom=0;')
    replace(path,'            entry->entry.goodname = strdup(ref->goodname);',
        '            entry->entry.goodname = strdup(ref->goodname);\n            if (!entry->entry.goodname) omniplay_romdb_resolve_oom=1;')
    replace(path,'            if (ref->cheats)\n                entry->entry.cheats = strdup(ref->cheats);',
        '            if (ref->cheats) {\n                entry->entry.cheats = strdup(ref->cheats);\n                if (!entry->entry.cheats) omniplay_romdb_resolve_oom=1;\n            }')
    def reader(raw):
        raw=stdio(raw,['FILE','fopen','fgets','fclose'])
        raw=raw.replace('void romdatabase_open(void)','int romdatabase_open(void)')
        raw=raw.replace('    int counter, value, lineno;','    int counter, value, lineno;\n    unsigned entries=0;')
        raw=raw.replace('    g_romdatabase.have_database = 1;','    g_romdatabase.have_database = 1;\n    omniplay_romdb_resolve_oom=0;')
        raw=raw.replace('if(g_romdatabase.have_database)\n        return;','if(g_romdatabase.have_database)\n        return 1;')
        raw=raw.replace('        return;','        return 0;')
        raw=raw.replace('counter < 255','counter < 256')
        raw=raw.replace('            *next_search = (romdatabase_search*) malloc(sizeof(romdatabase_search));',
            '            if (++entries>65536) goto omniplay_romdb_failed;\n            *next_search = (romdatabase_search*) malloc(sizeof(romdatabase_search));\n            if (!*next_search) goto omniplay_romdb_failed;')
        raw=raw.replace('                search->entry.goodname = strdup(l.value);',
            '                char* name=strdup(l.value);\n                if (!name) goto omniplay_romdb_failed;\n                free(search->entry.goodname);search->entry.goodname=name;')
        raw=raw.replace('                    search->entry.refmd5 = (md5_byte_t*)malloc(16*sizeof(md5_byte_t));',
            '                    md5_byte_t* reference=(md5_byte_t*)malloc(16*sizeof(md5_byte_t));\n                    if (!reference) goto omniplay_romdb_failed;\n                    free(search->entry.refmd5);search->entry.refmd5=reference;')
        raw=raw.replace('                if (len1 == 0 && len2 > 0)\n                    search->entry.cheats = strdup(l.value);',
            '                if (len1 == 0 && len2 > 0) {\n                    char* cheats=strdup(l.value);\n                    if (!cheats) goto omniplay_romdb_failed;\n                    free(search->entry.cheats);search->entry.cheats=cheats;\n                }')
        raw=raw.replace('                        DebugMessage(M64MSG_WARNING, "ROM Database: Failed to append cheat");',
            '                        goto omniplay_romdb_failed;')
        return raw.replace('    om_n64_fclose(fPtr);\n    romdatabase_resolve();',
            '''    if (om_n64_ferror(fPtr)) goto omniplay_romdb_failed;
    if (om_n64_fclose(fPtr)!=0) {fPtr=NULL;goto omniplay_romdb_failed;}
    fPtr=NULL;
    romdatabase_resolve();
    if (omniplay_romdb_resolve_oom) goto omniplay_romdb_failed;
    return 1;
omniplay_romdb_failed:
    if (fPtr) om_n64_fclose(fPtr);
    romdatabase_close();
    return 0;''')
    region(path,'void romdatabase_open(void)','void romdatabase_close(void)',reader)
    replace(source/'mupen64plus-core/src/main/rom.h','void romdatabase_open(void);','int romdatabase_open(void);')
    frontend=source/'mupen64plus-core/src/api/frontend.c'
    replace(frontend,'    romdatabase_open();','    if (!romdatabase_open()) {release_mem_base(g_mem_base);g_mem_base=NULL;return M64ERR_FILES;}')


def shader_storage(source,adapters):
    selected=source/'GLideN64/src/Graphics/OpenGLContext/GLSL'
    (selected/'glsl_ShaderStorage.cpp').write_text((adapters/'n64-shader-storage.cpp.inc').read_text(encoding='utf-8'),encoding='utf-8',newline='\n')
    binary=selected/'glsl_CombinerProgramImpl.cpp'
    replace(binary,'#include <algorithm>','#include <algorithm>\n#include "n64-shader-cache.hpp"')
    replace(binary,'\tif (binaryLength < 1)','\tif (binaryLength < 1 || static_cast<unsigned>(binaryLength)>om_n64_shader::maximum_binary)')
    replace(binary,'\tGLenum binaryFormat;','\tconst GLint capacity=binaryLength;\n\tGLenum binaryFormat;')
    replace(binary,'\tif (opengl::Utils::isGLError())','\tif (opengl::Utils::isGLError() || binaryLength<1 || binaryLength>capacity)')
    replace(source/'GLideN64/src/Combiner.cpp','\tconst u32 numCycles = cycleType + 1;',
        '\tconst u32 numCycles = cycleType == G_CYC_1CYCLE ? 1U : 2U; /* Two physical stage slots, also for cache fallback keys. */')


def fiber_lifecycle(source,adapters):
    (source/'libretro-common/libco/fiber.c').write_text((adapters/'n64-fiber-win.c.inc').read_text(encoding='utf-8'),encoding='utf-8',newline='\n')
    core=source/'libretro/libretro.c'
    replace(core,'#include "n64-memory-io.h"',
        '#include "n64-memory-io.h"\n#ifdef _WIN32\nint om_n64_co_retire_thread(void);\n#endif')
    replace(core,'    if (game_thread) {co_delete(game_thread);game_thread=NULL;}',
        '    if (game_thread) {co_delete(game_thread);game_thread=NULL;}\n#ifdef _WIN32\n    if (!om_n64_co_retire_thread()) {environ_cb(RETRO_ENVIRONMENT_SHUTDOWN,NULL);return;}\n#endif')


def texture_memory(source,adapters):
    path=source/'GLideN64/src/GLideNHQ/TxCache.cpp'
    replace(path,'#include <fstream>','#include <fstream>\n#include <cstring>\n#include <exception>\n#include <utility>\n#include "n64-memory-path.hpp"\n#include "n64-texture-cache.hpp"')
    replace(path,', _cacheLimit(cacheLimit)',', _cacheLimit(cacheLimit ? std::min<uint64>(cacheLimit,om_n64_texture::maximum_bytes) : om_n64_texture::maximum_bytes)')
    replace(path,'\tuint64 _totalSize;','\tuint64 _totalSize;\n\tbool _evictionEnabled;')
    replace(path,'\t, _totalSize(0U)','\t, _totalSize(0U)\n\t, _evictionEnabled(cacheLimit!=0)')
    region(path,'TxMemoryCache::TxMemoryCache(uint32 options,','TxMemoryCache::~TxMemoryCache()',
        lambda raw:raw.replace('\tif (_options & (GZ_TEXCACHE | GZ_HIRESTEXCACHE)) {','\t{ /* Even forced legacy compressed imports need bounded decode buffers. */'))
    methods=(adapters/'n64-texture-memory.cpp.inc').read_text(encoding='utf-8')
    add=methods.index('bool TxMemoryCache::get(');save=methods.index('bool TxMemoryCache::save(')
    region(path,'bool TxMemoryCache::add(Checksum checksum, GHQTexInfo* info, int dataSize)',
        'TxMemoryCache::Cache::const_iterator TxMemoryCache::find(',lambda _:methods[:add])
    region(path,'bool TxMemoryCache::get(Checksum checksum, N64FormatSize n64FmtSz, GHQTexInfo *info)',
        'bool TxMemoryCache::save(',lambda _:methods[add:save])
    region(path,'bool TxMemoryCache::save(', 'bool TxMemoryCache::del(',lambda _:methods[save:]+'\n')
    region(path,'void TxMemoryCache::buildFullPath()', 'static om_n64_texture::Shape om_n64_texture_shape(',
        lambda _:'''void TxMemoryCache::buildFullPath()
{
    _fullPath=om_n64_cache_path(_cachePath.c_str(),_filename.c_str());
}

''')
    region(path,'class TxFileStorage : public TxCacheImpl','/************************** TxCache *************************************/',
        lambda _:(adapters/'n64-texture-storage.cpp.inc').read_text(encoding='utf-8')+'\n')
    # Only mutable cache storage moves. Readonly texture-pack/data paths retain
    # their existing system-directory authority and are not global-intercepted.
    api=source/'custom/GLideN64/mupenplus/CommonAPIImpl_mupenplus.cpp'
    replace(api,'void PluginAPI::GetUserCachePath(wchar_t * _strPath)\n{\n\tgetRetroArchDir(_strPath);\n}',
        'void PluginAPI::GetUserCachePath(wchar_t * _strPath)\n{\n\tif (_strPath) wcscpy(_strPath,L"scratch/Mupen64plus");\n}')


def texture_pool(source,adapters):
    base=source/'GLideN64/src/GLideNHQ'
    header=base/'TxUtil.h'
    replace(header,'\tuint8 *_tex[2];\n\tuint32 _size[2];\n\tstd::vector< std::vector<uint32> > _bufs;','\t/* Owned storage is in the bounded pool; no untracked vector capacity. */')
    util=base/'TxUtil.cpp'
    replace(util,'#include "TxUtil.h"','#include "TxUtil.h"\n#include "n64-texture-pool.hpp"\n#include "n64-texture-cache.hpp"')
    region(util,'TxMemBuf::TxMemBuf()','void setTextureFormat(',
        lambda _:(adapters/'n64-texture-pool-core.cpp.inc').read_text(encoding='utf-8')+'\n')
    region(util,'int\nTxUtil::sizeofTx(int width, int height, ColorFormat format)','uint64\nTxUtil::checksum64(',lambda _:'''int
TxUtil::sizeofTx(int width,int height,ColorFormat format)
{
    if (width<=0 || height<=0 || width>16384 || height>16384) return 0;
    std::uint64_t stride=0;
    if (format==graphics::internalcolorFormat::COLOR_INDEX8) stride=1;
    else if (format==graphics::internalcolorFormat::RGBA4 || format==graphics::internalcolorFormat::RGB5_A1 || format==graphics::internalcolorFormat::RGB8) stride=2;
    else if (format==graphics::internalcolorFormat::RGBA8) stride=4;
    const auto bytes=std::uint64_t(width)*static_cast<unsigned>(height)*stride;
    return !bytes || bytes>om_n64_texture::maximum_bytes ? 0 : static_cast<int>(bytes);
}

''')
    replace(util,'\tint dataSize = sizeofTx(width, height, format);','\tint dataSize = sizeofTx(width, height, format);\n\tif (!src || dataSize<=0) return 0;')
    filt=base/'TxFilter.cpp'
    replace(filt,'#include "TxFilter.h"','#include "TxFilter.h"\n#include "n64-texture-pool.hpp"')
    # The pinned constructor reads some fields in a stale re-init shortcut;
    # initialize every field and let the single export owner govern re-init.
    replace(filt,': _tex1(nullptr)\n\t, _tex2(nullptr)\n\t, _txQuantize(nullptr)\n\t, _txTexCache(nullptr)\n\t, _txHiResLoader(nullptr)\n\t, _txImage(nullptr)',
        ': _numcore(1)\n\t, _tex1(nullptr)\n\t, _tex2(nullptr)\n\t, _maxwidth(0)\n\t, _maxheight(0)\n\t, _maxbpp(0)\n\t, _options(0)\n\t, _cacheSize(0)\n\t, _txQuantize(nullptr)\n\t, _txTexCache(nullptr)\n\t, _txHiResLoader(nullptr)\n\t, _txImage(nullptr)\n\t, _initialized(0)\n\t, _ownsPool(false)')
    region(filt,'\t/* HACKALERT: the emulator misbehaves and sometimes forgets to shutdown */',
        '\t/* shamelessness :P',lambda _:'''    if (texCachePath==nullptr || texDumpPath==nullptr || texPackPath==nullptr || maxwidth<=0 || maxheight<=0 || maxbpp<=0 || maxbpp>32 || cachesize<0) return;
    try {
''')
    replace(filt,'\tif (_tex1 && _tex2)\n\t\t_initialized = 1;','\tif (_tex1 && _tex2)\n\t\t_initialized = 1;\n    } catch (...) {clear();throw;}')
    replace(filt,'\t_numcore = TxUtil::getNumberofProcessors();','\t_numcore = 1; /* The pinned source has one actual filter lane. */')
    replace(filt,'\tif (TxMemBuf::getInstance()->init(_maxwidth, _maxheight)) {','\tif (TxMemBuf::getInstance()->init(_maxwidth, _maxheight)) {\n        _ownsPool=true;')
    replace(filt,'\t\t\t_tex2 = TxMemBuf::getInstance()->get(1);\n\t}','\t\t\t_tex2 = TxMemBuf::getInstance()->get(1);\n\t} else return;')
    region(filt,'void TxFilter::clear()','TxFilter::~TxFilter()',lambda _:'''void TxFilter::clear()
{
    _initialized=0;
    delete _txHiResLoader;_txHiResLoader=nullptr;
    delete _txTexCache;_txTexCache=nullptr;
    if (_ownsPool) {TxMemBuf::getInstance()->shutdown();_ownsPool=false;}_tex1=nullptr;_tex2=nullptr;
    delete _txImage;_txImage=nullptr;
    delete _txQuantize;_txQuantize=nullptr;
}

''')
    for begin in [
        'TxFilter::filter(uint8 *src, int srcwidth, int srcheight, ColorFormat srcformat, uint64 g64crc, N64FormatSize n64FmtSz, GHQTexInfo *info)',
        'TxFilter::hirestex(uint64 g64crc, Checksum r_crc64, uint16 *palette, N64FormatSize n64FmtSz, GHQTexInfo *info)',
        'TxFilter::dmptx(uint8 *src, int width, int height, int rowStridePixel,\n\t\t\t\tColorFormat gfmt, N64FormatSize n64FmtSz, Checksum r_crc64, boolean isStrongCrc)']:
        replace(filt,begin+'\n{',begin+'\n{\n    om_n64_texture::TexturePool::Use poolUse(om_n64_texture::pool());\n    if (!poolUse) return 0;')
    replace(filt,'TxFilter::reloadhirestex()\n{','TxFilter::reloadhirestex()\n{\n    om_n64_texture::TexturePool::Use poolUse(om_n64_texture::pool());\n    if (!poolUse || !_initialized || !_txHiResLoader) return 0;')
    replace(filt,'TxFilter::dumpcache()\n{','TxFilter::dumpcache()\n{\n    om_n64_texture::TexturePool::Use poolUse(om_n64_texture::pool());\n    if (!poolUse || !_initialized || !_txTexCache) return;')
    replace(filt,'TxFilter::hirestex(uint64 g64crc, Checksum r_crc64, uint16 *palette, N64FormatSize n64FmtSz, GHQTexInfo *info)\n{',
        'TxFilter::hirestex(uint64 g64crc, Checksum r_crc64, uint16 *palette, N64FormatSize n64FmtSz, GHQTexInfo *info)\n{\n    if (!_initialized || !info) return 0;')
    replace(filt,'\tuint8 *texture = src;','    if (!_initialized || !src || !info || srcwidth<=0 || srcheight<=0 || srcwidth>_maxwidth || srcheight>_maxheight || TxUtil::sizeofTx(srcwidth,srcheight,srcformat)<=0) return 0;\n\tuint8 *texture = src;')
    replace(filt,'((_options & (FILTER_MASK|ENHANCEMENT_MASK)) ||','((_options & (FILTER_MASK|ENHANCEMENT_MASK|DEPOSTERIZE)) ||')
    replace(filt,'\t\t\tfilter |= _options & DEPOSTERIZE;','\t\t\tfilter |= _options & DEPOSTERIZE;\n            if (!num_filters && (filter & DEPOSTERIZE)) num_filters=1;')
    # Replace only the dormant no-op worker branch: the selected core already
    # reports one processor. Do not advertise a newly invented parallel route.
    region(filt,'\t\t\t\tunsigned int numcore = _numcore;', '\n\t\t\t\tif (filter & ENHANCEMENT_MASK)',lambda _:'''                if (!filter_8888((uint32*)_texture,srcwidth,srcheight,(uint32*)_tmptex,filter,0)) return 0;
''')
    filter_header=base/'TxFilter.h'
    replace(filter_header,'  boolean _initialized;','  boolean _initialized;\n  bool _ownsPool;')
    replace(filter_header,'  ~TxFilter();','  ~TxFilter();\n  bool ready() const { return _initialized!=0; }')
    ops=base/'TextureFilters.cpp'
    replace(ops,'#include "TxUtil.h"','#include "TxUtil.h"\n#include "n64-texture-pool.hpp"')
    def checked(raw):
        raw=raw.replace('void filter_8888(','bool filter_8888(',1)
        raw=raw.replace(' {\n\tif (filter & DEPOSTERIZE)', ''' {
    om_n64_texture::TexturePool::Use poolUse(om_n64_texture::pool());
    if (!poolUse || !src || !dest || srcwidth<4 || srcheight<4 || srcwidth>4096 || srcheight>4096 || threadId!=0) return false;
    unsigned factor=1;
    switch (filter & ENHANCEMENT_MASK) {
    case NO_ENHANCEMENT:break;
    case BRZ3X_ENHANCEMENT:factor=3;break;
    case HQ4X_ENHANCEMENT:case BRZ4X_ENHANCEMENT:factor=4;break;
    case BRZ5X_ENHANCEMENT:factor=5;break;
    case BRZ6X_ENHANCEMENT:factor=6;break;
    case BRZ2X_ENHANCEMENT:case HQ2X_ENHANCEMENT:case HQ2XS_ENHANCEMENT:case LQ2X_ENHANCEMENT:case LQ2XS_ENHANCEMENT:case X2SAI_ENHANCEMENT:case X2_ENHANCEMENT:factor=2;break;
    default:return false;
    }
    const auto poolShape=om_n64_texture::pool().stats();
    if (srcwidth>poolShape.width/factor || srcheight>poolShape.height/factor) return false;
\tif (filter & DEPOSTERIZE)''',1)
        raw=raw.replace('\t\tconst auto bufSize = srcwidth * srcheight;','\t\tom_n64_texture::TexturePool::Work workspace(om_n64_texture::pool());\n\t\tif (!workspace) return false;\n\t\tconst auto bufSize = srcwidth * srcheight;')
        raw=raw.replace('\t\t\tsrc = tex;\n\t\t}','\t\t\tsrc = tex;\n\t\t} else return false;')
        raw=raw.replace('return;','return true;')
        # src may be a workspace pointer while the enhancement/smooth operation
        # runs; keep Work alive through the entire helper, not only deposterize.
        raw=raw.replace('\tif (filter & DEPOSTERIZE) {\n\t\tom_n64_texture::TexturePool::Work workspace(om_n64_texture::pool());\n\t\tif (!workspace) return false;',
            '    om_n64_texture::TexturePool::Work workspace(om_n64_texture::pool());\n    if (!workspace) return false;\n\tif (filter & DEPOSTERIZE) {')
        at=raw.rfind('}');return raw[:at]+'    if (filter & DEPOSTERIZE) {memcpy(dest,src,std::size_t(srcwidth)*srcheight*4);return true;}\n    return false;\n'+raw[at:]
    raw=ops.read_text(encoding='utf-8');at=raw.index('void filter_8888(')
    ops.write_text(raw[:at]+checked(raw[at:]),encoding='utf-8',newline='\n')
    replace(base/'TextureFilters.h','void filter_8888(','bool filter_8888(')
    exports=base/'TxFilterExport.cpp'
    replace(exports,'#include "TxFilter.h"','#include "TxFilter.h"\n#include <memory>\n#include "n64-texture-pool.hpp"')
    replace(exports,'  txFilter = new TxFilter(maxwidth, maxheight, maxbpp, options, cachesize,\n\t  txCachePath, txDumpPath, texPackPath, ident, callback);\n\n  return 1;',
        '  try {\n    std::unique_ptr<TxFilter> candidate(new TxFilter(maxwidth,maxheight,maxbpp,options,cachesize,txCachePath,txDumpPath,texPackPath,ident,callback));\n    if (!candidate->ready()) return 0;\n    txFilter=candidate.release();return 1;\n  } catch (...) {return 0;}')
    replace(exports,'  if (txFilter) delete txFilter;','  if (txFilter) {\n    if (!om_n64_texture::pool().request_retirement()) return;\n    delete txFilter;\n  }')
    core=source/'libretro/libretro.c'
    replace(core,'#include "n64-memory-io.h"','#include "n64-memory-io.h"\nint om_n64_texture_pool_retired(void);')
    replace(core,'    if (omniplay_memory_core_live) return 0;','    if (omniplay_memory_core_live || !om_n64_texture_pool_retired()) return 0;')
    cache=base/'TxCache.cpp'
    replace(cache,'#include "n64-texture-cache.hpp"','#include "n64-texture-cache.hpp"\n#include "n64-texture-pool.hpp"')


def texture_png_and_logs(source,adapters):
    base=source/'GLideN64/src/GLideNHQ'
    header=base/'TxImage.h'
    replace(header,'#include <png.h>','#include <png.h>\n#include "n64-memory-png.h"')
    replace(header,'boolean writePNG(uint8* src, FILE* fp,','boolean writePNG(uint8* src, OmniN64File* fp,')
    image=base/'TxImage.cpp'
    region(image,'boolean\nTxImage::writePNG(', 'boolean\nTxImage::getBMPInfo(',lambda _:'''boolean
TxImage::writePNG(uint8* source,OmniN64File* file,int width,int height,int stride,ColorFormat format)
{
    if (!file) return 0;
    if (format!=graphics::internalcolorFormat::RGBA8 || width<=0 || height<=0 || width>4096 || height>4096 || stride<width*4 || stride>4096*4) {om_n64_abort(file);return 0;}
    const auto bytes=std::uint64_t(height-1)*static_cast<unsigned>(stride)+static_cast<unsigned>(width)*4;
    return om_n64_png_encode(file,source,bytes,width,height,stride,4,0,OM_N64_PNG_CODEC_LIMIT)?1:0;
}

''')
    # The C writer owns all setjmp/error cleanup; no C++ stack frame or FILE
    # stream is substituted underneath a readonly PNG/BMP decoder.
    replace(image,'#include "TxImage.h"','#include "TxImage.h"\n#include <cstdint>')
    filt=base/'TxFilter.cpp'
    replace(filt,'#include "n64-texture-pool.hpp"','#include "n64-texture-pool.hpp"\n#include "n64-memory-path.hpp"\n#include <sstream>\n#include <iomanip>')
    region(filt,'boolean\nTxFilter::dmptx(', 'boolean\nTxFilter::reloadhirestex()',
        lambda _:(adapters/'n64-texture-dump.cpp.inc').read_text(encoding='utf-8')+'\n')
    dbg_header=base/'TxDbg.h'
    replace(dbg_header,'  FILE* _dbgfile;','  /* Native diagnostics keep no retained OS or memory stream. */')
    dbg=base/'TxDbg.cpp';raw=dbg.read_text(encoding='utf-8');stop=raw.index('#define DBG_LEVEL 80')
    dbg.write_text(raw[:stop]+(adapters/'n64-texture-debug.cpp.inc').read_text(encoding='utf-8'),encoding='utf-8',newline='\n')
    (source/'GLideN64/src/Log.cpp').write_text((adapters/'n64-native-log.cpp.inc').read_text(encoding='utf-8'),encoding='utf-8',newline='\n')


def persistent_storage(source,adapters):
    base=source/'mupen64plus-core/src'
    storage=base/'backends/file_storage.c'
    raw=storage.read_text(encoding='utf-8');at=raw.index('int open_file_storage(')
    storage.write_text(raw[:at]+(adapters/'n64-file-storage.c.inc').read_text(encoding='utf-8'),encoding='utf-8',newline='\n')
    replace(base/'backends/file_storage.h','    int first_access;','    int first_access;\n    int write_blocked, write_failed, loaded_existing;')
    util=base/'main/util.c'
    region(util,'file_status_t read_from_file(', '/**********************\n   Byte swap utilities',
        lambda _:(adapters/'n64-util-save.c.inc').read_text(encoding='utf-8')+'\n')
    main=base/'main/main.c'
    replace(main,'#include <assert.h>','#include <assert.h>\n#include <errno.h>\n#include "n64-save-files.h"')
    region(main,'static char *get_gb_ram_path(', 'const char *get_savestatepath(',lambda _:'''static char *get_gb_ram_path(const void* rom,size_t bytes,unsigned int port)
{
    md5_state_t state;md5_byte_t digest[16];char hex[33];unsigned int i;
    if(!rom || !bytes || bytes>OM_N64_SAVE_MAX_BYTES || port<1 || port>4)return NULL;
    md5_init(&state);md5_append(&state,(const md5_byte_t*)rom,(int)bytes);md5_finish(&state,digest);
    for(i=0;i<16;++i)snprintf(hex+i*2,3,"%02x",(unsigned int)digest[i]);
    return formatstr("save/N64/tpak/%s-p%u.ram",hex,port);
}

''')
    region(main,'static void init_gb_ram(', 'static void release_gb_ram(',
        lambda _:(adapters/'n64-transferpak-ram.c.inc').read_text(encoding='utf-8')+'\n')
    # New file-storage flags must never be read from uninitialized DD objects.
    # DD destination selection/import/format handling still has its own open
    # completion gate; utility writes already refuse host destinations.
    replace(main,'struct file_storage* fstorage = malloc(sizeof(struct file_storage));','struct file_storage* fstorage = calloc(1,sizeof(struct file_storage));')
    replace(main,'struct file_storage* fstorage_save = malloc(sizeof(struct file_storage));','struct file_storage* fstorage_save = calloc(1,sizeof(struct file_storage));')


def dd_storage(source,adapters):
    base=source/'mupen64plus-core/src'
    core=source/'libretro/libretro.c'
    replace(core,'#include "n64-memory-io.h"','#include "n64-memory-io.h"\n#include "n64-dd-storage.h"')
    replace(core,'static int omniplay_memory_startup_ready=0;','static int omniplay_memory_startup_ready=0;\nstatic int omniplay_upstream_deinit_finished=0;')
    replace(core,'    omniplay_memory_core_live=1;','    omniplay_memory_core_live=1;\n    omniplay_upstream_deinit_finished=0;')
    replace(core,'    if (omniplay_memory_startup_ready) omniplay_upstream_deinit();',
        '    if (omniplay_memory_startup_ready && !omniplay_upstream_deinit_finished) {omniplay_upstream_deinit();omniplay_upstream_deinit_finished=1;}\n    if (!om_n64_dd_quiescent()) {environ_cb(RETRO_ENVIRONMENT_SHUTDOWN,NULL);return;}')
    replace(core,'    if (omniplay_memory_core_live || !om_n64_texture_pool_retired()) return 0;',
        '    if (omniplay_memory_core_live || !om_n64_texture_pool_retired() || !om_n64_dd_quiescent()) return 0;')
    disk=base/'device/dd/disk.c';raw=disk.read_text(encoding='utf-8');at=raw.index('static uint8_t* storage_disk_data(')
    disk.write_text(raw[:at]+'#include "n64-dd-format.c.inc"\n#include "n64-dd-geometry.c.inc"\n',encoding='utf-8',newline='\n')
    header=base/'device/dd/disk.h'
    replace(header,'void GenerateLBAToPhysTable(struct dd_disk* disk);','int GenerateLBAToPhysTable(struct dd_disk* disk);')
    replace(header,'unsigned int head, unsigned int track, unsigned int block, unsigned int sector);','unsigned int head, unsigned int track, unsigned int block, unsigned int sector, size_t requested);')
    replace(header,'scan_and_expand_disk_format(uint8_t* data, size_t size,','scan_and_expand_disk_format(uint8_t* data, size_t* size,')
    rom=base/'main/rom.c'
    replace(rom,'scan_and_expand_disk_format(fstorage->data, fstorage->size,','scan_and_expand_disk_format(fstorage->data, &fstorage->size,')
    main=base/'main/main.c'
    replace(main,'#include "n64-save-files.h"','#include "n64-save-files.h"\n#include "n64-dd-storage.h"')
    region(main,'const char *get_dd_disk_save_path(', 'const char *get_savestatefilename(',lambda _:'')
    region(main,'static void load_dd_rom(', 'struct gb_cart_data\n',lambda _:(adapters/'n64-dd-main.c.inc').read_text(encoding='utf-8')+'\n')
    region(main,'    /* try to load DD disk first, if that succeeds, pass the region to load_dd_rom */',
        '    /* setup pif channel devices */',lambda _:'''    /* A supplied but unreadable/corrupt disk/save/IPL is an admission error,
     * never a silent cartridge-only fallback that can overwrite progress. */
    {
        int dd_result=load_dd_disk(&dd_disk,&dd_idisk);
        if(dd_result<0){if(environ_cb)environ_cb(RETRO_ENVIRONMENT_SHUTDOWN,NULL);return M64ERR_INPUT_INVALID;}
        if(dd_result>0){
            dd_rtc_iclock=&g_iclock_ctime_plus_delta;
            load_dd_rom((uint8_t*)mem_base_u32(g_mem_base,MM_DD_ROM),&dd_rom_size,&dd_disk.region);
            if(!dd_rom_size){om_n64_dd_discard_unstarted(&dd_disk);if(environ_cb)environ_cb(RETRO_ENVIRONMENT_SHUTDOWN,NULL);return M64ERR_INPUT_INVALID;}
        }else dd_rom_size=0;
        if(g_rom_size==0 && dd_rom_size==0){close_dd_disk(&dd_disk);return M64ERR_INPUT_INVALID;}
    }

''')
    controller=base/'device/dd/dd_controller.c'
    replace(controller,'    uint8_t* sector_base = get_sector_base(dd->disk, head, track, block, sector);',
        '''    uint8_t* sector_base = NULL;
    if (dd->regs[DD_ASIC_HOST_SECBYTE] < sizeof(dd->ds_buf))
        sector_base = get_sector_base(dd->disk, head, track, block, sector, (size_t)dd->regs[DD_ASIC_HOST_SECBYTE]+1);
''')
    replace(controller,'    size_t length = zone_sec_size[dd->bm_zone];',
        '    if (dd->bm_zone>=16) {dd->regs[DD_ASIC_BM_STATUS_CTL]|=DD_BM_STATUS_MICRO;return;}\n    size_t length = zone_sec_size[dd->bm_zone];')
    replace(controller,'void dd_update_bm(void* opaque)\n{\n    struct dd_controller* dd = (struct dd_controller*)opaque;',
        'void dd_update_bm(void* opaque)\n{\n    struct dd_controller* dd = (struct dd_controller*)opaque;\n    if (!dd->disk || !dd->idisk) {dd->regs[DD_ASIC_BM_STATUS_CTL]&=~DD_BM_STATUS_RUNNING;return;}')


def capture_owner(source,adapters):
    base=source/'mupen64plus-core/src'
    core=source/'libretro/libretro.c'
    replace(core,'#include "n64-dd-storage.h"','#include "n64-dd-storage.h"\n#include "n64-core-owner.h"\nstatic int om_n64_frame_pending;')
    region(core,'void retro_init(void)\n{','    struct retro_log_callback log;',lambda _:'''void retro_init(void)
{
    if(!om_n64_owner_begin()){if(om_n64_owner_thread() && environ_cb)environ_cb(RETRO_ENVIRONMENT_SHUTDOWN,NULL);return;}
    if(omniplay_memory_core_live){om_n64_owner_admission_failed();return;}
    omniplay_memory_core_live=1;omniplay_upstream_deinit_finished=0;omniplay_memory_startup_ready=0;
    retro_thread=NULL;game_thread=NULL;om_n64_frame_pending=0;
    if(!om_n64_native_peripherals_quiet() || !om_n64_files_installed() || om_n64_mkdir("scratch/Mupen64plus")!=0 || !copy_file(inifile,"mupen64plus.ini"))
    {om_n64_owner_admission_failed();if(environ_cb)environ_cb(RETRO_ENVIRONMENT_SHUTDOWN,NULL);return;}

''')
    replace(core,'        if (!game_thread) {environ_cb(RETRO_ENVIRONMENT_SHUTDOWN,NULL);return;}',
        '        if (!game_thread || !om_n64_owner_bind(retro_thread,game_thread)) {om_n64_owner_admission_failed();environ_cb(RETRO_ENVIRONMENT_SHUTDOWN,NULL);return;}')
    region(core,'static void* EmuThreadFunction(void* param)','static void reinit_gfx_plugin(void)',lambda _:'''static void EmuThreadFunction(void)
{
    m64p_error result;
    initializing=false;
    result=CoreDoCommand(M64CMD_EXECUTE,0,NULL);
    if(!om_n64_owner_ended(game_thread,result==M64ERR_SUCCESS))_Exit(1);
    if(result!=M64ERR_SUCCESS && environ_cb)environ_cb(RETRO_ENVIRONMENT_SHUTDOWN,NULL);
    co_switch(retro_thread);
    /* The checked root deletes this finished guest. Never return from a
     * Windows fiber entry or execute a second guest after retirement. */
    _Exit(1);
}

''')
    replace(core,'EnableThreadedRenderer = !strcmp(var.value, "True") ? 1 : 0;',
        'EnableThreadedRenderer = 0; /* This native package owns one cooperative guest. */')
    region(core,'bool retro_load_game(const struct retro_game_info *game)','void retro_unload_game(void)',lambda raw:
        raw.replace('    load_game_successful = true;','    if(!om_n64_owner_thread() || co_active()!=retro_thread || !om_n64_owner_loaded(retro_thread))return false;\n    load_game_successful = true;'))
    replace(core,'    if (!omniplay_memory_startup_ready || !game) return false;',
        '    if (!om_n64_owner_thread() || !omniplay_memory_startup_ready || !game || !retro_thread || co_active()!=retro_thread || load_game_successful || om_n64_owner_started()) return false;')
    replace(core,'    game_data = malloc(game->size);\n    memcpy(game_data, game->data, game->size);',
        '    if (!game->data || !game->size || game->size>512u*1024u*1024u) return false;\n    game_data = malloc(game->size);\n    if (!game_data) return false;\n    memcpy(game_data,game->data,game->size);')
    region(core,'void retro_unload_game(void)','void retro_reset (void)',lambda _:'')
    region(core,'bool retro_serialize(void *data, size_t size)','//Needed to be able to detach controllers',lambda _:'')
    region(core,'void retro_return(void)','/* OmniPlay: retain the table',lambda _:'')
    region(core,'/* OmniPlay: retain the table', 'RETRO_API int omniplay_install_writable_files_v1',lambda _:'')
    raw=core.read_text(encoding='utf-8');at=raw.index('RETRO_API int omniplay_install_writable_files_v1')
    core.write_text(raw[:at]+(adapters/'n64-core-barrier.c.inc').read_text(encoding='utf-8'),encoding='utf-8',newline='\n')
    replace(core,'    if (!initializing) CoreDoCommand(M64CMD_RESET, 0, (void*)0);',
        '    if (om_n64_owner_thread() && retro_thread && co_active()==retro_thread && om_n64_owner_ready(retro_thread)) CoreDoCommand(M64CMD_RESET,0,NULL);')
    exports=source/'libretro/link.T'
    replace(exports,'omniplay_install_writable_files_v1;',
        'omniplay_install_writable_files_v1; omniplay_finish_prepare; omniplay_pause_guest; omniplay_flush_io_checked;')
    header=base/'backends/file_storage.h'
    replace(header,'void close_file_storage(struct file_storage* storage);',
        'int close_file_storage(struct file_storage* storage);\nint om_n64_file_storage_flush(struct file_storage* storage);\nvoid om_n64_file_storage_discard_unstarted(struct file_storage* storage);')
    main=base/'main/main.c'
    replace(main,'#include "n64-dd-storage.h"','#include "n64-dd-storage.h"\n#include "n64-core-owner.h"')
    region(main,'struct gb_cart_data\n{','static struct gb_cart_data l_gb_carts_data',
        lambda _:'#include "n64-gb-storage-owner.h"\n\n')
    replace(main,'static struct gb_cart_data l_gb_carts_data[GAME_CONTROLLERS_COUNT];',
        'static struct gb_cart_data l_gb_carts_data[GAME_CONTROLLERS_COUNT];\n'+(adapters/'n64-main-storage-owner.c.inc').read_text(encoding='utf-8'))
    replace(main,'    close_file_storage(&data->ram_fstorage);\n\n    memset(&data->ram_fstorage, 0, sizeof(data->ram_fstorage));',
        '    if (close_file_storage(&data->ram_fstorage))\n        memset(&data->ram_fstorage,0,sizeof(data->ram_fstorage));')
    replace(main,'    /* reset gb_cart_data */\n    memset(data, 0, sizeof(*data));',
        '    /* Refuse a cart swap until the old RAM is actually published. */\n    if (!close_file_storage(&data->ram_fstorage)) {om_n64_owner_admission_failed();return;}\n    close_file_storage(&data->rom_fstorage);\n    memset(data,0,sizeof(*data));')
    replace(main,'    if (open_rom_file_storage(&data->rom_fstorage, rom_filename) != file_ok) {',
        '    if (open_rom_file_storage(&data->rom_fstorage, rom_filename) != file_ok) {\n        om_n64_owner_admission_failed();')
    replace(main,'    if (rom_filename == NULL || strlen(rom_filename) == 0) {',
        '    if (rom_filename==NULL && g_media_loader.get_gb_cart_rom==NULL && retro_transferpak_rom_path) om_n64_owner_admission_failed();\n    if (rom_filename == NULL || strlen(rom_filename) == 0) {')
    replace(main,'    struct dd_disk dd_disk;','    static struct dd_disk dd_disk; /* Stable through failed-close retry after guest exit. */')
    replace(main,'    m64p_error failure_rval;','    m64p_error failure_rval=M64ERR_INTERNAL;')
    replace(main,'    memset(&dd_disk, 0, sizeof(dd_disk));',
        '    if (!om_n64_native_peripherals_quiet()) return M64ERR_INVALID_STATE;\n    memset(&dd_disk,0,sizeof(dd_disk));')
    region(main,'    /**\n     * Actually never returns.', '\n    return M64ERR_SUCCESS;',lambda _:'')
    gb=base/'device/gb/gb_cart.c'
    replace(gb,'#include "gb_cart.h"','#include "gb_cart.h"\n#include "n64-core-owner.h"')
    replace(gb,'error_release_ram:\n','error_release_ram:\n    om_n64_owner_admission_failed();\n')
    replace(gb,'error_release_rom:\n','error_release_rom:\n    om_n64_owner_admission_failed();\n')
    replace(gb,'    if (type->extra_devices & GED_CAMERA) {\n        init_pocket_cam(',
        '    if (type->extra_devices & GED_CAMERA) {\n        if (!iram_storage || !ram_storage) goto error_release_ram;\n        init_pocket_cam(')
    pure=base/'device/r4300/pure_interp.c'
    replace(pure,'void run_pure_interpreter(struct r4300_core* r4300)',
        '#include "n64-core-owner.h"\nvoid run_pure_interpreter(struct r4300_core* r4300)')
    replace(pure,'   *r4300_pc(r4300) = r4300->cp0.last_addr = r4300->start_address;',
        '   *r4300_pc(r4300) = r4300->cp0.last_addr = r4300->start_address;\n   if (!om_n64_guest_initial_checkpoint()) {*r4300_stop(r4300)=1;return;}')
    cpu=base/'device/r4300/r4300_core.c'
    replace(cpu,'void run_r4300(struct r4300_core* r4300)',
        '#include "n64-core-owner.h"\nvoid run_r4300(struct r4300_core* r4300)')
    replace(cpu,'        run_cached_interpreter(r4300);',
        '        if (!om_n64_guest_initial_checkpoint()) *r4300_stop(r4300)=1;\n        run_cached_interpreter(r4300);')
    interrupt=base/'device/r4300/interrupt.c'
    replace(interrupt,'void gen_interrupt(struct r4300_core* r4300)',
        '#include "n64-core-owner.h"\nvoid gen_interrupt(struct r4300_core* r4300)')
    # This method is the final method in the pinned source. Place the boundary
    # after event/job processing, never halfway through the VI callback.
    raw=interrupt.read_text(encoding='utf-8');at=raw.index('void gen_interrupt(struct r4300_core* r4300)');stop=raw.index('\n}',at)
    interrupt.write_text(raw[:stop]+'\n    if (!r4300->cp0.interrupt_unsafe_state) om_n64_guest_interrupt_checkpoint();\n'+raw[stop:],encoding='utf-8',newline='\n')
    states=base/'main/savestates.c'
    replace(states,'    save = malloc(sizeof(*save));','    save = calloc(1,sizeof(*save));')
    replace(states,'        main_message(M64MSG_STATUS, OSD_BOTTOM_LEFT, "Savestate is not a valid Mupen64plus savestate.");\n        return 0;',
        '        main_message(M64MSG_STATUS,OSD_BOTTOM_LEFT,"Savestate is not a valid Mupen64plus savestate.");\n#ifdef USE_SDL\n        SDL_UnlockMutex(savestates_lock);\n#else\n        pthread_mutex_unlock(&savestates_lock);\n#endif\n        return 0;')
    raw=states.read_text(encoding='utf-8');at=raw.index('void savestates_init(void)')
    states.write_text(raw[:at]+(adapters/'n64-snapshot-lifecycle.c.inc').read_text(encoding='utf-8'),encoding='utf-8',newline='\n')
    replace(base/'main/savestates.h','void savestates_init(void);','int savestates_init(void);')
    frontend=base/'api/frontend.c'
    replace(frontend,'    workqueue_init();',
        '    if (!savestates_init()) {romdatabase_close();release_mem_base(g_mem_base);g_mem_base=NULL;return M64ERR_NO_MEMORY;}\n    workqueue_init();')


def snapshot_pointer_bounds(source,adapters):
    base=source/'mupen64plus-core/src'
    interrupt=base/'device/r4300/interrupt.c'
    region(interrupt,'int save_eventqueue_infos(', 'void init_interrupt(',lambda _:'#include "n64-event-queue.c.inc"\n\n')
    # Freeze exact upstream scheduler/pool bodies for the later focused fixture.
    # The fixture does not replace the actual CPU/guest acceptance obligations.
    raw=interrupt.read_text(encoding='utf-8')
    scheduler=raw[raw.index('static struct node* alloc_node('):raw.index('unsigned int add_random_interrupt_time(')]
    scheduler+=raw[raw.index('void add_interrupt_event_count('):raw.index('void remove_interrupt_event(')]
    scheduler+=raw[raw.index('unsigned int* get_event('):raw.index('int get_next_event_type(')]
    cp0=(base/'device/r4300/cp0.c').read_text(encoding='utf-8')
    accessors=''
    for begin,end in [('uint32_t* r4300_cp0_regs(', 'uint64_t* r4300_cp0_latch('),('unsigned int* r4300_cp0_next_interrupt(', 'int check_cop1_unusable(')]:
        accessors+=cp0[cp0.index(begin):cp0.index(end,cp0.index(begin))]
    (source/'n64-event-queue-scheduler.c.inc').write_text(accessors+scheduler+'\n#include "n64-event-queue.c.inc"\n',encoding='utf-8',newline='\n')
    header=base/'device/r4300/interrupt.h'
    replace(header,'#include <stdint.h>','#include <stdint.h>\n#include <stddef.h>')
    replace(header,'int save_eventqueue_infos(const struct cp0* cp0, char *buf);','int save_eventqueue_infos(const struct cp0* cp0,char* buf,size_t bytes);')
    replace(header,'void load_eventqueue_infos(struct cp0* cp0, const char *buf);','int load_eventqueue_infos(struct cp0* cp0,const char* buf,size_t bytes);')
    states=base/'main/savestates.c'
    replace(states,'#include "workqueue.h"','#include "workqueue.h"\n#include "n64-snapshot-bounds.h"\n#include "n64-dd-storage.h"')
    replace(states,'enum { GB_CART_FINGERPRINT_OFFSET = 0x134 };', '''enum { GB_CART_FINGERPRINT_OFFSET = 0x134 };
typedef char om_n64_snapshot_pinned_layout[(RDRAM_MAX_SIZE==0x800000 && SP_MEM_SIZE==0x2000 && PIF_RAM_SIZE==64 && PIF_CHANNELS_COUNT==5 && GAME_CONTROLLERS_COUNT==4 && GB_CART_FINGERPRINT_SIZE==28 && MBC3_RTC_REGS_COUNT==5 && POCKET_CAM_REGS_COUNT==54)?1:-1];''')
    # These adapters select only the synchronous fixed-size libretro entry.
    region(states,'    unsigned char using_tlb_data[4];','    // Parse savestate',lambda raw:
        raw.replace('    uint32_t* cp0_regs = r4300_cp0_regs(&dev->r4300.cp0);', '''    uint32_t* cp0_regs = r4300_cp0_regs(&dev->r4300.cp0);
#ifdef __LIBRETRO__
    uint32_t admitted_disk_id;
    if(!om_n64_snapshot_pointer_preflight(data,OM_N64_SNAPSHOT_BYTES))return 0;
    if(dev->dd.rom_size>0 && dev->dd.idisk!=NULL && !om_n64_dd_snapshot_id(dev->dd.disk,&admitted_disk_id))return 0;
#endif'''))
    replace(states,'    load_eventqueue_infos(&dev->r4300.cp0, queue);','    if(!load_eventqueue_infos(&dev->r4300.cp0,queue,sizeof(queue))){free(savestateData);return 0;}')
    replace(states,'    load_eventqueue_infos(&dev->r4300.cp0, buffer);','    if(!load_eventqueue_infos(&dev->r4300.cp0,buffer,20))return 0;')
    replace(states,'    save_eventqueue_infos(&dev->r4300.cp0, queue);', '''    if(!save_eventqueue_infos(&dev->r4300.cp0,queue,sizeof(queue))){
        free(save->filepath);free(save);StateChanged(M64CORE_STATE_SAVECOMPLETE,0);return 0;
    }''')
    replace(states,'''            uint32_t* current_disk_id = ((dev->dd.rom_size > 0) && dev->dd.idisk != NULL)
                ? (uint32_t*)(dev->dd.idisk->data(dev->dd.disk) + DD_DISK_ID_OFFSET)
                : NULL;

            if (current_disk_id != NULL && *current_disk_id == disk_id) {''', '''            uint32_t current_disk_id=0;
            int has_disk_id=(dev->dd.rom_size>0 && dev->dd.idisk!=NULL && om_n64_dd_snapshot_id(dev->dd.disk,&current_disk_id));
            if (has_disk_id && current_disk_id==disk_id) {''')
    replace(states,'''    uint32_t* disk_id = ((dev->dd.rom_size > 0) && dev->dd.idisk != NULL)
        ? (uint32_t*)(dev->dd.idisk->data(dev->dd.disk) + DD_DISK_ID_OFFSET)
        : NULL;

    if (disk_id == NULL) {''', '''    uint32_t disk_id=0;
    int has_disk_id=(dev->dd.rom_size>0 && dev->dd.idisk!=NULL);
    if(has_disk_id && !om_n64_dd_snapshot_id(dev->dd.disk,&disk_id)){
        free(save->data);free(save->filepath);free(save);StateChanged(M64CORE_STATE_SAVECOMPLETE,0);return 0;
    }
    if (!has_disk_id) {''')
    replace(states,'PUTDATA(curr, uint32_t, *disk_id);','PUTDATA(curr, uint32_t, disk_id);')
    replace(states,'    queue_work(&save->work);', '''#ifdef __LIBRETRO__
    if(!om_n64_snapshot_pointer_preflight(save->data,save->size)){
        free(save->data);free(save->filepath);free(save);StateChanged(M64CORE_STATE_SAVECOMPLETE,0);return 0;
    }
#endif
    queue_work(&save->work);''')
    replace(states,'Current GB cart: %s. Expected GB cart : %s','Current GB cart: %.28s. Expected GB cart : %.28s',count=2)
    replace(states,'''            if (offset >= 0) {
                setup_pif_channel(&dev->pif.channels[i], dev->pif.ram + offset);''','''            if (offset>=0 && offset<PIF_RAM_SIZE) {
                setup_pif_channel(&dev->pif.channels[i],dev->pif.ram+offset,PIF_RAM_SIZE-(size_t)offset);''',count=2)
    replace(states,'''       PUTDATA(curr, int8_t, (dev->pif.channels[i].tx == NULL)
               ? (int8_t)-1
               : (int8_t)(dev->pif.channels[i].tx - dev->pif.ram));''', '''       /* An inactive/reset channel may point at a separate dummy buffer.
        * Compare identities; subtracting unrelated pointers is undefined. */
       int8_t offset=-1;size_t at;
       for(at=0;at<PIF_RAM_SIZE;++at)if(dev->pif.channels[i].tx==dev->pif.ram+at){offset=(int8_t)at;break;}
       PUTDATA(curr,int8_t,offset);''')
    pif=base/'device/pif/pif.c'
    replace(pif,'#include "pif.h"','#include "pif.h"\n#include "n64-snapshot-bounds.h"')
    replace(pif,'    channel->rx_buf = NULL;','    channel->rx_buf = NULL;\n    channel->capacity=0;')
    region(pif,'size_t setup_pif_channel(', 'void init_pif(',lambda _:'''size_t setup_pif_channel(struct pif_channel* channel,uint8_t* buf,size_t bytes)
{
    size_t extent=om_n64_pif_command_extent(buf,bytes);
    if(!extent){disable_pif_channel(channel);return 0;}
    channel->tx=buf;channel->rx=buf+1;channel->tx_buf=buf+2;
    channel->rx_buf=buf+2+(buf[0]&0x3f);channel->capacity=extent;
    post_setup_channel(channel);
    return extent;
}

''')
    replace(pif,'    /* reset Tx/Rx just in case */', '''    if(!om_n64_pif_command_extent(channel->tx,channel->capacity)
       || channel->rx!=channel->tx+1 || channel->tx_buf!=channel->tx+2
       || channel->rx_buf!=channel->tx+2+(*channel->tx&0x3f)){
        disable_pif_channel(channel);return;
    }
    /* reset Tx/Rx just in case */''')
    replace(pif,'setup_pif_channel(&pif->channels[k], dummy_reset_buffer[k]);','setup_pif_channel(&pif->channels[k],dummy_reset_buffer[k],sizeof(dummy_reset_buffer[k]));')
    replace(pif,'if ((i + 2) >= PIF_RAM_SIZE)', 'if (PIF_RAM_SIZE-i<2)')
    replace(pif,'            i += setup_pif_channel(&pif->channels[k++], &pif->ram[i]);', '''            {
                size_t extent=setup_pif_channel(&pif->channels[k++],&pif->ram[i],PIF_RAM_SIZE-i);
                if(!extent){i=PIF_RAM_SIZE;break;}
                i+=extent;
            }''')
    region(pif,'void setup_channels_format(', 'static void process_cic_challenge(',lambda raw:raw.replace(
        '    /* Zilmar-Spec plugin expect a call with control_id = -1 when RAM processing is done */',
        '    while(k<PIF_CHANNELS_COUNT)disable_pif_channel(&pif->channels[k++]);\n    /* Zilmar-Spec plugin expect a call with control_id = -1 when RAM processing is done */'))
    header=base/'device/pif/pif.h'
    replace(header,'#include <stdint.h>','#include <stdint.h>\n#include <stddef.h>')
    replace(header,'    uint8_t* rx_buf;','    uint8_t* rx_buf;\n    size_t capacity;')
    replace(header,'size_t setup_pif_channel(struct pif_channel* channel, uint8_t* buf);','size_t setup_pif_channel(struct pif_channel* channel,uint8_t* buf,size_t bytes);')


def tlb_snapshot_bounds(source,adapters):
    base=source/'mupen64plus-core/src'
    tlb=base/'device/r4300/tlb.c'
    region(tlb,'void tlb_unmap(', 'uint32_t virtual_to_physical_address(',lambda _:'#include "n64-tlb-memory.c.inc"\n\n')
    raw=tlb.read_text(encoding='utf-8')
    (source/'n64-tlb-actual.c.inc').write_text(raw[raw.index('void poweron_tlb('):raw.index('uint32_t virtual_to_physical_address(')],encoding='utf-8',newline='\n')
    instructions=base/'device/r4300/mips_instructions.def'
    replace(instructions,'static void TLBWrite(struct r4300_core* r4300, unsigned int idx)\n{', '''static void TLBWrite(struct r4300_core* r4300, unsigned int idx)
{
    /* WIRED>=32 permits a six-bit RANDOM result in the pinned interpreter.
     * Ignore nonexistent entries, as TLBWI/TLBR already do, before dereference. */
    if(idx>=32)return;''')
    states=base/'main/savestates.c'
    replace(states,'MBC3_RTC_REGS_COUNT==5 && POCKET_CAM_REGS_COUNT==54', 'MBC3_RTC_REGS_COUNT==5 && POCKET_CAM_REGS_COUNT==54 && CP0_REGS_COUNT==32')


def transferpak_snapshot_bounds(source,adapters):
    base=source/'mupen64plus-core/src'
    gb=base/'device/gb/gb_cart.c'
    region(gb,'static void read_rom(', 'static void set_ram_enable(',lambda _:'#include "n64-gb-storage-access.c.inc"\n\n')
    raw=gb.read_text(encoding='utf-8');at=raw.index('int read_gb_cart(struct gb_cart*')
    gb.write_text(raw[:at],encoding='utf-8',newline='\n')
    replace(gb,'((gb_cart->ram_bank & 0x7) * 0x2000)', '((gb_cart->ram_bank & ((gb_cart->extra_devices & GED_RUMBLE)?0x7:0xf)) * 0x2000)')
    replace(gb,'((gb_cart->ram_bank & 0x07)* 0x2000)', '((gb_cart->ram_bank & ((gb_cart->extra_devices & GED_RUMBLE)?0x7:0xf)) * 0x2000)')
    header=base/'device/gb/gb_cart.h'
    replace(header,'    uint8_t* ram;', '    uint8_t* ram;\n    size_t ram_bytes;')
    replace(gb,'    /* setup sensor regs */', '    if(!cam->ram || cam->ram_bytes<sizeof(tiles))return;\n    /* setup sensor regs */')
    replace(gb,'if (cam->ivcap->grab_image(cam->vcap, bgr) != M64ERR_SUCCESS)', 'if (!cam->ivcap || !cam->ivcap->grab_image || cam->ivcap->grab_image(cam->vcap,bgr)!=M64ERR_SUCCESS)')
    replace(gb,'static void init_pocket_cam(struct pocket_cam* cam, uint8_t* ram, void* vcap, const struct video_capture_backend_interface* ivcap)', 'static void init_pocket_cam(struct pocket_cam* cam,uint8_t* ram,size_t bytes,void* vcap,const struct video_capture_backend_interface* ivcap)')
    replace(gb,'    cam->ram = ram;', '    cam->ram=ram;cam->ram_bytes=bytes;')
    replace(gb,'        if (!iram_storage || !ram_storage) goto error_release_ram;', '        if (!iram_storage || !ram_storage || iram_storage->size(ram_storage)<4096) goto error_release_ram;')
    replace(gb,'init_pocket_cam(&cam, iram_storage->data(ram_storage), vcap, ivcap);', 'init_pocket_cam(&cam,iram_storage->data(ram_storage),iram_storage->size(ram_storage),vcap,ivcap);')
    replace(gb,'        if (ram_size != 0)', '''        /* Camera output occupies 4096 bytes. Reject its bad header before
         * opening RAM, so failed admission cannot publish a default image. */
        if((type->extra_devices & GED_CAMERA) && ram_size<4096)goto error_release_rom;
        if (ram_size != 0)''')
    # This callback is used only while initializing a cartridge, not for its
    # eventual normal retirement. Pass an explicit no-publication discard.
    main=base/'main/main.c'
    replace(main,'static void release_gb_ram(void* opaque)', '''static void discard_gb_ram(void* opaque)
{
    struct gb_cart_data* data=(struct gb_cart_data*)opaque;
    om_n64_file_storage_discard_unstarted(&data->ram_fstorage);
}

static void release_gb_ram(void* opaque)''')
    replace(main,'init_gb_ram, release_gb_ram,', 'init_gb_ram, discard_gb_ram,',count=2)
    replace(header,'void init_gb_cart(struct gb_cart* gb_cart,', '''/* release_ram is startup-failure cleanup only: discard the unstarted RAM
 * without publishing defaults. Normal dirty retirement is owned by main. */
void init_gb_cart(struct gb_cart* gb_cart,''')
    pak=base/'device/controllers/paks/transferpak.c'
    replace(pak,'#include "transferpak.h"', '#include "transferpak.h"\n#include "n64-core-owner.h"')
    replace(pak,'    DebugMessage(M64MSG_VERBOSE, "tpak read: %04x", address);', '''    if(!om_n64_owner_access() || !data || !size || size>32)return;
    memset(data,0xff,size);
    if(!tpk || tpk->bank>3)return;
    DebugMessage(M64MSG_VERBOSE, "tpak read: %04x", address);''')
    replace(pak,'    uint8_t value = data[size-1];', '''    uint8_t value;
    if(!om_n64_owner_access() || !tpk || !data || !size || size>32 || tpk->bank>3)return;
    value=data[size-1];''')


def mbc3_rtc_snapshot(source,adapters):
    base=source/'mupen64plus-core/src'
    rtc=base/'device/gb/mbc3_rtc.c'
    raw=rtc.read_text(encoding='utf-8');at=raw.index('/* TODO: halt bit is not implemented */')
    rtc.write_text(raw[:at]+'#include "n64-mbc3-rtc.c.inc"\n',encoding='utf-8',newline='\n')
    replace(base/'device/gb/mbc3_rtc.h','    unsigned int latch;', '    unsigned int latch; /* Last full latch-command byte. */\n    uint8_t latched_valid; /* Independent of the last command. */')
    states=base/'main/savestates.c'
    replace(states,'static const int savestate_latest_version = 0x00010900;  /* 1.9 */', '''#ifndef __LIBRETRO__
#error OmniPlay v1.10 snapshots require the selected fixed-size native libretro profile
#endif
static const int savestate_latest_version = 0x00010a00;  /* 1.10: independent RTC latch validity */''')
    replace(states,'CP0_REGS_COUNT==32', 'CP0_REGS_COUNT==32 && DD_ASIC_REGS_COUNT==19')
    replace(states,'    unsigned char using_tlb_data[4];', '    unsigned char using_tlb_data[4];\n    uint8_t om_n64_rtc_flags[GAME_CONTROLLERS_COUNT]={0};')
    replace(states,'    if(!om_n64_snapshot_pointer_preflight(data,OM_N64_SNAPSHOT_BYTES))return 0;', '''    if(!om_n64_snapshot_pointer_preflight(data,OM_N64_SNAPSHOT_BYTES))return 0;
    if(((const uint8_t*)data)[10]==10 && !om_n64_snapshot_rtc_flags(data,OM_N64_SNAPSHOT_BYTES,om_n64_rtc_flags))return 0;''')
    replace(states,'dev->transferpaks[i].gb_cart->rtc.latch = rtc_latch;', '''dev->transferpaks[i].gb_cart->rtc.latch=rtc_latch;
                        dev->transferpaks[i].gb_cart->rtc.latched_valid=(version>=0x00010a00)?om_n64_rtc_flags[i]:(rtc_latch?1:0);''',count=2)
    replace(states,'            *r4300_cp2_latch(&dev->r4300.cp2) = GETDATA(curr, uint64_t);', '''            *r4300_cp2_latch(&dev->r4300.cp2) = GETDATA(curr, uint64_t);
            if(version>=0x00010a00){
                /* Already staged before mutation and applied only at each
                 * matched cartridge restore, before any PIF post-setup call. */
                curr+=GAME_CONTROLLERS_COUNT*sizeof(uint32_t);
            }''')
    replace(states,'    PUTDATA(curr, uint64_t, *r4300_cp2_latch((struct cp2*)&dev->r4300.cp2));', '''    PUTDATA(curr, uint64_t, *r4300_cp2_latch((struct cp2*)&dev->r4300.cp2));
    /* v1.10 retains the v1.9 body and appends one bounded flag per GB port
     * inside the existing fixed-size extra-state block. */
    for(i=0;i<GAME_CONTROLLERS_COUNT;++i){
        uint32_t valid=dev->transferpaks[i].gb_cart?dev->transferpaks[i].gb_cart->rtc.latched_valid:0;
        PUTDATA(curr,uint32_t,valid);
    }''')


def rtc_battery_storage(source,adapters):
    base=source/'mupen64plus-core/src'
    main=base/'main/main.c'
    replace(main,'void main_change_gb_cart(int control_id)\n{', '''#include "n64-gb-rtc-bind.c.inc"
void main_change_gb_cart(int control_id)
{
    if(!om_n64_owner_access() || control_id<0 || control_id>=GAME_CONTROLLERS_COUNT)return;''')
    replace(main,'    if (!close_file_storage(&data->ram_fstorage)) {om_n64_owner_admission_failed();return;}', '    if (!om_n64_gb_retire_storage(data)) {om_n64_owner_admission_failed();return;}')
    replace(main,'    if (close_file_storage(&data->ram_fstorage))\n        memset(&data->ram_fstorage,0,sizeof(data->ram_fstorage));', '    if (om_n64_gb_retire_storage(data))\n        memset(&data->ram_fstorage,0,sizeof(data->ram_fstorage));')
    replace(main,'            data->gbcam_backend, data->igbcam_backend);', '''            data->gbcam_backend, data->igbcam_backend);
    if(!om_n64_bind_gb_clock(data,gb_cart)){change_gb_cart(tpk,NULL);return;}''')
    replace(main,'                            l_gb_carts_data[i].gbcam_backend, l_gb_carts_data[i].igbcam_backend);', '''                            l_gb_carts_data[i].gbcam_backend, l_gb_carts_data[i].igbcam_backend);
                    om_n64_bind_gb_clock(&l_gb_carts_data[i],&g_dev.gb_carts[i]);''')
    replace(main,'Inserting GB cart %s into transferpak %u', 'Inserting GB cart %.16s into transferpak %u')
    states=base/'main/savestates.c'
    replace(states,'#include "n64-snapshot-bounds.h"', '#include "n64-snapshot-bounds.h"\n#include "n64-rtc-storage.h"')
    replace(states,'memcpy(dev->transferpaks[i].gb_cart->rtc.latched_regs, rtc_latched_regs, MBC3_RTC_REGS_COUNT);', '''memcpy(dev->transferpaks[i].gb_cart->rtc.latched_regs, rtc_latched_regs, MBC3_RTC_REGS_COUNT);
                        om_n64_rtc_storage_changed(&dev->transferpaks[i].gb_cart->rtc);''',count=2)
    header=base/'device/gb/mbc3_rtc.h'
    replace(header,'Days Counter is 16 bit wide', 'Low byte of the 9-bit day counter')
    replace(header,'halt (0: stop timer, 1: active timer)', 'halt (0: running, 1: stopped)')


def peripheral_memory_bounds(source,adapters):
    base=source/'mupen64plus-core/src'
    flash=base/'device/cart/flashram.c';raw=flash.read_text(encoding='utf-8');at=raw.index('static void flashram_command(')
    flash.write_text(raw[:at]+'#include "n64-flash-memory.c.inc"\n',encoding='utf-8',newline='\n')
    si=base/'device/rcp/si/si_controller.c'
    region(si,'static void copy_pif_rdram(', 'static void dma_si_write(',lambda _:'')
    replace(si,'static int validate_dma(struct si_controller* si, uint32_t reg)\n{', '''#include "n64-si-copy.c.inc"
static int validate_dma(struct si_controller* si, uint32_t reg)
{
    if(!om_n64_owner_access() || !si)return 0;''')
    region(si,'static int validate_dma(', 'static void dma_si_write(',lambda raw:raw.replace('    return 1;', '''    if(!om_n64_si_memory_ready(si)){si->regs[SI_STATUS_REG]|=SI_STATUS_DMA_ERROR;return 0;}
    return 1;'''))
    replace(si,'\n    copy_pif_rdram(si);', '\n    if(!copy_pif_rdram(si)){si->dma_dir=SI_NO_DMA;return;}')
    # Freeze actual methods for later focused qualification. Scheduler/PIF/MI
    # callbacks are explicit fixture stubs, not emulated CPU/device execution.
    raw=si.read_text(encoding='utf-8')
    fragment=raw[raw.index('#include "n64-si-copy.c.inc"'):raw.index('void read_si_regs(')]
    fragment+=raw[raw.index('void si_end_of_dma_event('):]
    (source/'n64-si-actual.c.inc').write_text(fragment,encoding='utf-8',newline='\n')
    dd=base/'device/dd/dd_controller.c'
    region(dd,'static void read_C2(', 'static uint8_t* seek_sector(',lambda _:'#include "n64-dd-c2.c.inc"\n\n')
    replace(dd,'        if ((dd->regs[DD_ASIC_HOST_SECBYTE] + 1) != zone_sec_size[dd->bm_zone]) {', '''        if(dd->bm_zone>=16){dd->regs[DD_ASIC_BM_STATUS_CTL]|=DD_BM_STATUS_MICRO;return;}
        if ((dd->regs[DD_ASIC_HOST_SECBYTE] + 1) != zone_sec_size[dd->bm_zone]) {''')
    states=base/'main/savestates.c'
    replace(states,'    uint32_t admitted_disk_id;', '    uint32_t admitted_disk_id=0;')
    replace(states,'    if(dev->dd.rom_size>0 && dev->dd.idisk!=NULL && !om_n64_dd_snapshot_id(dev->dd.disk,&admitted_disk_id))return 0;', '''    if(dev->dd.rom_size>0 && dev->dd.idisk!=NULL && !om_n64_dd_snapshot_id(dev->dd.disk,&admitted_disk_id))return 0;
    if(!om_n64_snapshot_devices_valid(data,OM_N64_SNAPSHOT_BYTES,dev->dd.rom_size>0 && dev->dd.idisk!=NULL,admitted_disk_id))return 0;''')
    replace(states,'    if(!om_n64_snapshot_pointer_preflight(save->data,save->size)){', '    if(!om_n64_snapshot_pointer_preflight(save->data,save->size) || !om_n64_snapshot_devices_valid(save->data,save->size,has_disk_id,disk_id)){')
    replace(states,'DD_ASIC_REGS_COUNT==19', 'DD_ASIC_REGS_COUNT==19 && SI_NO_DMA==0 && SI_DMA_READ==1 && SI_DMA_WRITE==2 && SP_DMA_READ==0 && SP_DMA_WRITE==1 && FLASHRAM_MODE_PAGE_PROGRAM==5')


def pi_framebuffer_memory(source,adapters):
    base=source/'mupen64plus-core/src'
    pi=base/'device/rcp/pi/pi_controller.c'
    region(pi,'int validate_pi_request(', 'void init_pi(',lambda _:'#include "n64-pi-memory.c.inc"\n\n')
    raw=pi.read_text(encoding='utf-8')
    fragment=raw[raw.index('#include "n64-pi-memory.c.inc"'):raw.index('void read_pi_regs(')]
    (source/'n64-pi-actual.c.inc').write_text(fragment,encoding='utf-8',newline='\n')
    fb=base/'device/rcp/rdp/fb.c'
    region(fb,'static osal_inline size_t fb_buffer_size(', 'void init_fb(',lambda _:'#include "n64-framebuffer-memory.c.inc"\n\n')
    replace(base/'device/rcp/rdp/fb.h','void pre_framebuffer_read(struct fb* fb, uint32_t address);',
        'void pre_framebuffer_read(struct fb* fb, uint32_t address);\nvoid pre_framebuffer_read_range(struct fb* fb, uint32_t address, uint32_t length);')
    replace(fb,'    pre_framebuffer_read(fb, address);\n    read_rdram_dram(fb->rdram, address, value);', '''    uint32_t physical;
    if(!om_n64_owner_access() || !value)return;
    *value=0;
    if(!om_n64_fb_range(fb,address&~UINT32_C(3),4,&physical))return;
    pre_framebuffer_read(fb,physical);
    read_rdram_dram(fb->rdram,physical,value);''')
    replace(fb,'    write_rdram_dram(fb->rdram, address, value, mask);', '''    uint32_t physical;
    if(!om_n64_owner_access() || !mask || !om_n64_fb_range(fb,address&~UINT32_C(3),4,&physical))return;
    address=physical;
    write_rdram_dram(fb->rdram,address,value,mask);''')
    raw=fb.read_text(encoding='utf-8');at=raw.index('void protect_framebuffers(')
    fb.write_text(raw[:at]+'#include "n64-framebuffer-mapping.c.inc"\n',encoding='utf-8',newline='\n')
    graphics=source/'GLideN64/src/FrameBufferInfo.cpp'
    replace(graphics,'#include <assert.h>', '#include <assert.h>\n#include "n64-framebuffer-cache.hpp"')
    region(graphics,'\tFBInfo::BufferSearchResult FBInfo::_findBuffer(', '\tvoid FBInfo::Write(',lambda _:'''\tFBInfo::BufferSearchResult FBInfo::_findBuffer(const BuffersArray& _buffers, const FrameBuffer* _buf) const
\t{
\t\treturn om_n64_find_framebuffer(_buffers,_buf);
\t}

''')
    replace(graphics,'if (!findRes.first)\n\t\t\tm_writeBuffers[findRes.second]', 'if (!findRes.first && findRes.second < m_writeBuffers.size())\n\t\t\tm_writeBuffers[findRes.second]')
    replace(graphics,'if (!findRes.first)\n\t\t\tm_readBuffers[findRes.second]', 'if (!findRes.first && findRes.second < m_readBuffers.size())\n\t\t\tm_readBuffers[findRes.second]')


def cartridge_dma_memory(source,adapters):
    base=source/'mupen64plus-core/src'
    sram=base/'device/cart/sram.c';raw=sram.read_text(encoding='utf-8');at=raw.index('void format_sram(')
    sram.write_text(raw[:at]+'#include "n64-sram-memory.c.inc"\n',encoding='utf-8',newline='\n')
    rom=base/'device/cart/cart_rom.c';raw=rom.read_text(encoding='utf-8');at=raw.index('void read_cart_rom(')
    rom.write_text(raw[:at]+'#include "n64-cart-rom-memory.c.inc"\n',encoding='utf-8',newline='\n')
    dd=base/'device/dd/dd_controller.c';raw=dd.read_text(encoding='utf-8');at=raw.index('void read_dd_rom(')
    dd.write_text(raw[:at]+'#include "n64-dd-dma-memory.c.inc"\n',encoding='utf-8',newline='\n')
    cart=base/'device/cart/cart.c'
    replace(cart,'#include "cart.h"', '#include "cart.h"\n#include "n64-core-owner.h"')
    region(cart,'void read_cart_dom2(', 'unsigned int cart_dom2_dma_read(',lambda raw:raw.replace(
        '    struct cart* cart = (struct cart*)opaque;',
        '    struct cart* cart = (struct cart*)opaque;\n    if(!om_n64_owner_access() || !cart)return;'))
    raw=cart.read_text(encoding='utf-8');at=raw.index('unsigned int cart_dom2_dma_read(')
    tail=raw[at:].replace('    struct cart* cart = (struct cart*)opaque;',
        '    struct cart* cart = (struct cart*)opaque;\n    if(!om_n64_owner_access() || !cart)return 0x1000;')
    cart.write_text(raw[:at]+tail,encoding='utf-8',newline='\n')
    (source/'n64-cart-dma-actual.c.inc').write_text(tail,encoding='utf-8',newline='\n')


def color_buffer_memory(source,adapters):
    header=source/'GLideN64/src/BufferCopy/RDRAMtoColorBuffer.h'
    replace(header,'#include <vector>', '#include "n64-color-memory.hpp"')
    replace(header,'\tvoid addAddress(u32 _address, u32 _size);', '''\tvoid addAddress(u32 _address, u32 _size);
\tbool hasPendingRange(u32 begin,u32 end) const {if(end<begin || begin>=OmniN64DirtyBytes::capacity)return false;const auto last=end<OmniN64DirtyBytes::capacity?end:OmniN64DirtyBytes::capacity-1;return m_dirty.any(begin,static_cast<std::size_t>(last)-begin+1);}''')
    replace(header,'\tvoid reset();', '\tvoid reset();\n\tbool prepareImage(u32 width,u32 height);')
    replace(header,'\tstd::vector<u32> m_vecAddress;', '\tOmniN64DirtyBytes m_dirty;\n\tstd::size_t m_capacity=0;')
    cpp=source/'GLideN64/src/BufferCopy/RDRAMtoColorBuffer.cpp'
    replace(cpp,'#include <algorithm>', '#include <algorithm>\n#include <cstdlib>\n#include <new>\n#include "n64-core-owner.h"')
    replace(cpp,'void RDRAMtoColorBuffer::init()\n{', '''void RDRAMtoColorBuffer::init()
{
    if(!om_n64_owner_access())return;
    destroy();m_dirty.clear();''')
    replace(cpp,'\tm_pTexture = textureCache().addFrameBufferTexture(textureTarget::TEXTURE_2D);', '''\tm_pTexture = textureCache().addFrameBufferTexture(textureTarget::TEXTURE_2D);
    if(!m_pTexture){om_n64_owner_admission_failed();return;}
    if(fbTexFormats.colorFormatBytes!=(fbTexFormats.colorType==datatype::FLOAT?16u:4u)){destroy();om_n64_owner_admission_failed();return;}''')
    replace(cpp,'\tm_pbuf = (u8*)malloc(m_pTexture->textureBytes);', '''\tm_pbuf = static_cast<u8*>(std::malloc(m_pTexture->textureBytes));
    if(!m_pbuf){destroy();om_n64_owner_admission_failed();return;}
    m_capacity=m_pTexture->textureBytes;''')
    replace(cpp,'\tm_pTexture->width = 640;', '\tm_pTexture->width = 1;')
    replace(cpp,'\tm_pTexture->height = 580;', '\tm_pTexture->height = 1;')
    replace(cpp,'void RDRAMtoColorBuffer::destroy()\n{', '''void RDRAMtoColorBuffer::destroy()
{
    if(!om_n64_owner_access())return;
    reset();m_dirty.clear();m_capacity=0;''')
    region(cpp,'void RDRAMtoColorBuffer::addAddress(', 'static\nu32 RGBA16ToABGR32(',lambda _:'')
    region(cpp,'void RDRAMtoColorBuffer::_copyFromRDRAM(', '\tconst u32 cycleType = gDP.otherMode.cycleType;',lambda _:'''void RDRAMtoColorBuffer::_copyFromRDRAM(u32 _height, bool _fullAlpha)
{
#include "n64-color-upload.cpp.inc"

''')
    replace(cpp,'\tgDP.changed |= CHANGED_RENDERMODE | CHANGED_COMBINE;', '''\tgDP.changed |= CHANGED_RENDERMODE | CHANGED_COMBINE;
    m_dirty.clear(address,source_bytes);''')
    raw=cpp.read_text(encoding='utf-8');at=raw.index('void RDRAMtoColorBuffer::copyFromRDRAM(u32')
    cpp.write_text(raw[:at]+'#include "n64-color-buffer.cpp.inc"\n',encoding='utf-8',newline='\n')
    # Keep the exact production converters for independent CPU pixel fixtures.
    raw=cpp.read_text(encoding='utf-8');begin=raw.index('static\nu32 RGBA16ToABGR32(');end=raw.index('void RDRAMtoColorBuffer::_copyFromRDRAM(',begin)
    (source/'n64-color-converters.cpp.inc').write_text(raw[begin:end],encoding='utf-8',newline='\n')
    info=source/'GLideN64/src/FrameBufferInfo.cpp'
    replace(info,'#include "n64-framebuffer-cache.hpp"', '#include "n64-framebuffer-cache.hpp"\n#include "BufferCopy/RDRAMtoColorBuffer.h"')
    replace(info,'if (pBuffer == nullptr || _findBuffer(m_writeBuffers, pBuffer).first)', '''if (pBuffer == nullptr || _findBuffer(m_writeBuffers, pBuffer).first
            || RDRAMtoColorBuffer::get().hasPendingRange(pBuffer->m_startAddress,pBuffer->m_endAddress))''')


def rom_image_memory(source,adapters):
    path=source/'mupen64plus-core/src/main/rom.c'
    region(path,'static const uint8_t Z64_SIGNATURE[4]', 'm64p_error open_rom(',lambda _:'''#include "n64-rom-image.h"
#include "n64-core-owner.h"
typedef char om_n64_rom_image_types[(Z64IMAGE==0 && V64IMAGE==1 && N64IMAGE==2)?1:-1];
static int is_valid_rom(const unsigned char* image,unsigned int bytes)
{return om_n64_rom_image_format(image,bytes)>=0;}

''')
    replace(path,'    if (romimage == NULL || !is_valid_rom(romimage, size))',
        '    if (!om_n64_owner_access() || !is_valid_rom(romimage, size))')
    replace(path,'    swap_copy_rom((uint8_t*)mem_base_u32(g_mem_base, MM_CART_ROM), romimage, size, &imagetype);',
        '    if(!om_n64_rom_image_copy((uint8_t*)mem_base_u32(g_mem_base,MM_CART_ROM),size,romimage,size,&imagetype))return M64ERR_INPUT_INVALID;')
    replace(path,'DebugMessage(M64MSG_INFO, "Name: %s", ROM_HEADER.Name);',
        'DebugMessage(M64MSG_INFO, "Name: %.20s", ROM_HEADER.Name);')


def content_load_lifecycle(source,adapters):
    core=source/'libretro/libretro.c'
    region(core,'static bool emu_step_load_data()', 'static void emu_step_initialize(',lambda _:'#include "n64-content-load.c.inc"\n\n')
    def load(raw):
        start=raw.index('    char* gamePath;')
        raw=raw[:start]+'''    if(!om_n64_owner_admitted() || om_n64_content_rom_open || game_data
        || om_n64_rom_image_format(game->data,game->size)<0)return false;
    if(game->path){size_t bytes=0;while(bytes<=1024 && game->path[bytes])++bytes;if(bytes>1024)return false;}
'''+raw[start:]
        raw=re.sub(r'^( +)strcpy\(newPath, gamePath\);',
            lambda m:m[1]+'if(!newPath){om_n64_owner_admission_failed();return false;}\n'+m[0],raw,flags=re.MULTILINE)
        # Once setup starts, failure is retired through deinit rather than
        # replaying format_saved_memory/audio/context setup in the same session.
        at=raw.index('    // Init default vals')
        raw=raw[:at]+raw[at:].replace('return false;', '{om_n64_owner_admission_failed();return false;}')
        return raw
    region(core,'bool retro_load_game(const struct retro_game_info *game)', 'void retro_reset (void)',load)
    # Successful unload retires the exact frontend receipt. A close failure
    # leaves both the loaded flag and the receipt for conservative teardown.
    replace(core,'    cleanup_global_paths();load_game_successful=false;', '    om_n64_content_rom_open=0;cleanup_global_paths();load_game_successful=false;')
    replace(core,'        if(closed!=M64ERR_SUCCESS && closed!=M64ERR_INVALID_STATE){om_n64_core_failure();return;}', '''        if(closed!=M64ERR_SUCCESS && (om_n64_content_rom_open || closed!=M64ERR_INVALID_STATE)){om_n64_core_failure();return;}
        om_n64_content_rom_open=0;''')
    replace(core,'    retro_thread=NULL;omniplay_memory_startup_ready=0;omniplay_memory_core_live=0;', '''    cleanup_global_paths();free(game_data);game_data=NULL;game_size=0;
    retro_thread=NULL;omniplay_memory_startup_ready=0;omniplay_memory_core_live=0;''')


def audio_lifecycle(source,adapters):
    base=source/'custom/mupen64plus-core/plugin/audio_libretro'
    path=base/'audio_backend_libretro.c'
    region(path,'void deinit_audio_libretro(void)', 'static void aiDacrateChanged(',lambda _:'#include "n64-audio-lifecycle.c.inc"\n\n')
    replace(base/'audio_plugin.h','void init_audio_libretro(unsigned max_frames);',
        'void init_audio_libretro(unsigned max_frames);\nint om_n64_audio_is_ready(void);')
    replace(source/'libretro/libretro.c','    init_audio_libretro(audio_buffer_size);',
        '    init_audio_libretro(audio_buffer_size);\n    if(!om_n64_audio_is_ready()){om_n64_owner_admission_failed();return false;}')
    replace(path,'   GameFreq        = frequency;', '''   if(!om_n64_owner_access() || !om_n64_audio_ready || !frequency || frequency>0x1fffffff){om_n64_owner_admission_failed();return;}
   GameFreq        = frequency;''')
    replace(path,'   struct ai_controller* ai = (struct ai_controller*)user_data;\n   uint32_t saved_ai_dacrate', '''   struct ai_controller* ai = (struct ai_controller*)user_data;
   if(!om_n64_owner_access() || !om_n64_audio_ready || !ai || !ai->vi || !frequency || frequency>0x1fffffff){om_n64_owner_admission_failed();return;}
   uint32_t saved_ai_dacrate''')
    replace(path,'   uint32_t i;\n   int16_t *out', '''   if(!om_n64_owner_access() || !om_n64_audio_ready || !audio_batch_cb || !buffer || (size&3))return;
   uint32_t i;
   int16_t *out''')


def audio_output_queue(source,adapters):
    base=source/'custom/mupen64plus-core/plugin/audio_libretro'
    path=base/'audio_backend_libretro.c'
    replace(path,'#include "n64-audio-lifecycle.c.inc"', '#include "n64-audio-lifecycle.c.inc"\n#include "n64-audio-output.c.inc"')
    replace(base/'audio_plugin.h','int om_n64_audio_is_ready(void);',
        'int om_n64_audio_is_ready(void);\nint om_n64_audio_drain_pending(void);')
    replace(path,'''      while (output_frames)
      {
         size_t ret     = audio_batch_cb(out, output_frames);
         output_frames -= ret;
         out           += ret * 2;
      }''', '      if(!om_n64_audio_submit(out,output_frames))return;')
    replace(path,'''   while (data.output_frames)
   {
      size_t ret          = audio_batch_cb(out, data.output_frames);
      data.output_frames -= ret;
      out                += ret * 2;
   }''', '   if(!om_n64_audio_submit(out,data.output_frames))return;')
    # Queue storage must precede lifecycle because confirmed deinit cancels it.
    replace(path,'#include "n64-audio-lifecycle.c.inc"\n#include "n64-audio-output.c.inc"',
        '#include "n64-audio-output.h"\nstatic struct om_n64_audio_queue om_n64_audio_pending;\n#define OM_N64_AUDIO_CAN_RETIRE (!om_n64_audio_pending.busy)\n#define OM_N64_AUDIO_CANCEL() do {om_n64_audio_pending.head=om_n64_audio_pending.count=0;om_n64_audio_pending.fault=0;} while(0)\n#include "n64-audio-lifecycle.c.inc"\n#define OM_N64_AUDIO_PENDING_DEFINED 1\n#include "n64-audio-output.c.inc"')
    replace(source/'libretro/libretro.c','    libretro_swap_buffer=false;',
        '    if(om_n64_audio_drain_pending()!=1)return;\n    libretro_swap_buffer=false;')


def audio_callback_retirement(source,adapters):
    header=source/'custom/mupen64plus-core/plugin/audio_libretro/audio_plugin.h'
    replace(header,'int om_n64_audio_drain_pending(void);', '''int om_n64_audio_drain_pending(void);
int om_n64_audio_callback_idle(void);
int om_n64_audio_is_retired(void);''')
    core=source/'libretro/libretro.c'
    replace(core,'{return om_n64_owner_thread() && retro_thread && co_active()==retro_thread;}',
        '{return om_n64_owner_thread() && retro_thread && co_active()==retro_thread && om_n64_audio_callback_idle();}')
    replace(core,'    if(!om_n64_owner_thread() || !omniplay_memory_core_live)return;',
        '    if(!om_n64_owner_thread() || !omniplay_memory_core_live || !om_n64_audio_callback_idle())return;')
    replace(core,'    if(!om_n64_native_peripherals_quiet() || !om_n64_texture_pool_retired() || !om_n64_files_idle())return;',
        '    if(!om_n64_native_peripherals_quiet() || !om_n64_texture_pool_retired() || !om_n64_files_idle() || !om_n64_audio_is_retired())return;')
    replace(core,'    if(!om_n64_owner_quiescent() || omniplay_memory_core_live || !om_n64_texture_pool_retired() || !om_n64_native_peripherals_quiet())return 0;',
        '    if(!om_n64_owner_quiescent() || omniplay_memory_core_live || !om_n64_texture_pool_retired() || !om_n64_native_peripherals_quiet() || !om_n64_audio_is_retired())return 0;')
    replace(core,'if (om_n64_owner_thread() && retro_thread && co_active()==retro_thread && om_n64_owner_ready(retro_thread)) CoreDoCommand',
        'if (om_n64_owner_thread() && retro_thread && co_active()==retro_thread && om_n64_audio_callback_idle() && om_n64_owner_ready(retro_thread)) CoreDoCommand')
    replace(core,'void retro_set_audio_sample_batch(retro_audio_sample_batch_t cb) { audio_batch_cb = cb; }',
        'void retro_set_audio_sample_batch(retro_audio_sample_batch_t cb) { if(om_n64_audio_callback_idle())audio_batch_cb=cb; }')
    region(core,'bool retro_load_game_special(', 'void retro_init(void)',lambda raw:raw.replace(
        '    cleanup_global_paths();',
        '    if(!om_n64_owner_thread() || !om_n64_audio_callback_idle() || load_game_successful || om_n64_owner_started())return false;\n    cleanup_global_paths();',1))


def ai_memory_source(source,adapters):
    path=source/'mupen64plus-core/src/device/rcp/ai/ai_controller.c'
    replace(path,'static uint32_t get_remaining_dma_length(', '#include "n64-ai-memory.c.inc"\n\nstatic uint32_t get_remaining_dma_length(')
    replace(path,'''            unsigned int diff = ai->fifo[0].length - ai->last_read;
            unsigned char *p = (unsigned char*)&ai->ri->rdram->dram[ai->fifo[0].address/4];
            ai->iaout->push_samples(ai->aout, p + diff, ai->last_read - *value);
            ai->last_read = *value;''', '            om_n64_ai_publish(ai,*value);')
    replace(path,'''        unsigned int diff = ai->fifo[0].length - ai->last_read;
        unsigned char *p = (unsigned char*)&ai->ri->rdram->dram[ai->fifo[0].address/4];
        ai->iaout->push_samples(ai->aout, p + diff, ai->last_read);
        ai->last_read = 0;''', '        om_n64_ai_publish(ai,0);')
    for signature in ['void read_ai_regs(void* opaque, uint32_t address, uint32_t* value)',
                      'void write_ai_regs(void* opaque, uint32_t address, uint32_t value, uint32_t mask)',
                      'void ai_end_of_dma_event(void* opaque)']:
        region(path,signature, '\n}',lambda raw:raw.replace('    struct ai_controller* ai = (struct ai_controller*)opaque;',
            '    struct ai_controller* ai = (struct ai_controller*)opaque;\n    if(!om_n64_owner_access() || !ai)return;'))
    audio=source/'custom/mupen64plus-core/plugin/audio_libretro/audio_backend_libretro.c'
    raw=audio.read_text(encoding='utf-8');at=raw.index('/* Abuse core & audio plugin implementation details')
    audio.write_text(raw[:at]+'#include "n64-audio-source.c.inc"\n',encoding='utf-8',newline='\n')


def ai_timing_admission(source,adapters):
    path=source/'mupen64plus-core/src/device/rcp/ai/ai_controller.c'
    region(path,'static unsigned int get_dma_duration(', 'static void fifo_push(',lambda _:'#include "n64-ai-dma.c.inc"\n\n')
    replace(path,'    unsigned int duration = get_dma_duration(ai) * ai->dma_modifier;',
        '    uint32_t duration;\n    if(!om_n64_ai_duration_for_request(ai,&duration))return;')
    replace(path,'    if (ai->fifo[0].duration == 0)', '''    if(!om_n64_owner_access() || !ai || !ai->mi || !ai->mi->r4300
        || ai->fifo[0].duration>INT32_MAX || ai->last_read>ai->fifo[0].length){
        om_n64_owner_admission_failed();return ai?ai->last_read:0;
    }
    if (ai->fifo[0].duration == 0)''')
    replace(path,'    remaining_dma_duration = *next_ai_event - cp0_regs[CP0_COUNT_REG];', '''    remaining_dma_duration = *next_ai_event - cp0_regs[CP0_COUNT_REG];
    if(remaining_dma_duration>ai->fifo[0].duration){om_n64_owner_admission_failed();return ai->last_read;}''')
    raw=path.read_text(encoding='utf-8');begin=raw.index('#include "n64-ai-dma.c.inc"');end=raw.index('static void fifo_pop(',begin)
    (source/'n64-ai-request-actual.c.inc').write_text(raw[begin:end],encoding='utf-8',newline='\n')


def sinc_output_admission(source,adapters):
    sinc=source/'libretro-common/audio/resampler/drivers/sinc_resampler.c'
    replace(sinc,'   uint32_t time;', '''   uint32_t time;
   size_t om_phase_elems,om_buffer_elems;
   resampler_process_t om_process;''')
    replace(sinc,'   if (!re)\n      return NULL;', '''   if (!re)
      return NULL;
   if(!(bandwidth_mod>0 && bandwidth_mod<=1.7976931348623157e308))goto error;''')
    replace(sinc,'   re->subphase_mask = (1 << re->subphase_bits) - 1;', '''   if(window_type==SINC_WINDOW_NONE || re->phase_bits>12 || re->subphase_bits>16)goto error;
   re->subphase_mask = (1 << re->subphase_bits) - 1;''')
    replace(sinc,'      re->taps = (unsigned)ceil(re->taps / bandwidth_mod);', '''      double requested=ceil(re->taps / bandwidth_mod);
      if(!(requested>=1 && requested<=4096))goto error;
      re->taps = (unsigned)requested;''')
    replace(sinc,'   re->main_buffer = (float*)memalign_alloc(128, sizeof(float) * elems);', '''   if(!re->taps || re->taps>4096 || elems>64u*1024u*1024u/sizeof(float))goto error;
   re->om_phase_elems=phase_elems;re->om_buffer_elems=elems;
   re->main_buffer = (float*)memalign_alloc(128, sizeof(float) * elems);''')
    replace(sinc,'   return re;\n\nerror:', '   re->om_process=sinc_resampler.process;\n   return re;\n\nerror:')
    with sinc.open('a',encoding='utf-8',newline='\n') as out:
        out.write('\n#include "n64-sinc-checked.c.inc"\n')
    audio=source/'custom/mupen64plus-core/plugin/audio_libretro/audio_backend_libretro.c'
    replace(audio,'#include "n64-audio-output.c.inc"', '#include "n64-audio-output.c.inc"\n#ifndef __APPLE__\nint om_n64_sinc_process_checked(void*,const retro_resampler_t*,struct resampler_data*,size_t);\n#endif')
    raw=audio.read_text(encoding='utf-8');start=raw.index('   /* Non-Apple path: sinc resampler with float conversion */');end=raw.index('\n#endif\n}',start)
    audio.write_text(raw[:start]+'#include "n64-audio-sinc.cpp.inc"'+raw[end:],encoding='utf-8',newline='\n')


def ai_fifo_publication(source,adapters):
    path=source/'mupen64plus-core/src/device/rcp/ai/ai_controller.c'
    region(path,'static void fifo_push(', 'void init_ai(',lambda _:'#include "n64-ai-fifo.c.inc"\n\n')
    raw=path.read_text(encoding='utf-8');begin=raw.index('#include "n64-ai-dma.c.inc"');end=raw.index('void init_ai(',begin)
    (source/'n64-ai-request-actual.c.inc').write_text(raw[begin:end],encoding='utf-8',newline='\n')


def event_insertion_acknowledgement(source,adapters):
    base=source/'mupen64plus-core/src/device/r4300'
    header=base/'interrupt.h'
    replace(header,'void add_interrupt_event(struct cp0* cp0, int type, unsigned int delay);',
        'void add_interrupt_event(struct cp0* cp0, int type, unsigned int delay);\nint om_n64_add_interrupt_checked(struct cp0* cp0,int type,unsigned int delay);')
    replace(base/'interrupt.c','void remove_interrupt_event(struct cp0* cp0)',
        '#include "n64-event-insert.c.inc"\n\nvoid remove_interrupt_event(struct cp0* cp0)')
    # The original snapshot fixture remains a preserved scheduler snapshot.
    # This new fixture selects the same actual scheduler plus the new receipt.
    original=source/'n64-event-queue-scheduler.c.inc'
    incoming=original.read_text(encoding='utf-8')
    (source/'n64-event-insert-actual.c.inc').write_text(incoming+'\n#include "n64-event-insert.c.inc"\n',encoding='utf-8',newline='\n')


def integrate(source,adapters):
    configuration(source);rom_database(source);shader_storage(source,adapters);fiber_lifecycle(source,adapters);texture_memory(source,adapters);texture_pool(source,adapters);texture_png_and_logs(source,adapters);persistent_storage(source,adapters);dd_storage(source,adapters);capture_owner(source,adapters);snapshot_pointer_bounds(source,adapters);tlb_snapshot_bounds(source,adapters);transferpak_snapshot_bounds(source,adapters);mbc3_rtc_snapshot(source,adapters);rtc_battery_storage(source,adapters);peripheral_memory_bounds(source,adapters);pi_framebuffer_memory(source,adapters);cartridge_dma_memory(source,adapters);color_buffer_memory(source,adapters);rom_image_memory(source,adapters);content_load_lifecycle(source,adapters);audio_lifecycle(source,adapters);audio_output_queue(source,adapters);audio_callback_retirement(source,adapters);ai_memory_source(source,adapters);ai_timing_admission(source,adapters);sinc_output_admission(source,adapters);ai_fifo_publication(source,adapters);event_insertion_acknowledgement(source,adapters)
    make=source/'Makefile.common'
    with make.open('a',encoding='utf-8',newline='\n')as out:
        out.write('\n# OmniPlay owned memory I/O: no general OS stdio interception.\nINCFLAGS += -I$(ROOT_DIR)\nSOURCES_C += $(ROOT_DIR)/n64-memory-png.c $(ROOT_DIR)/n64-dd-storage.c $(ROOT_DIR)/n64-rtc-storage.c\nSOURCES_CXX += $(ROOT_DIR)/n64-memory-io.cpp $(ROOT_DIR)/n64-memory-gzip.cpp $(ROOT_DIR)/n64-texture-pool.cpp $(ROOT_DIR)/n64-save-files.cpp $(ROOT_DIR)/n64-core-owner.cpp\n')
    return ['scratch_generated_configuration_and_checked_close','bounded_readonly_asset_path_join',
            'checked_rom_database_io_and_complete_index_reset','staged_optional_custom_settings_and_builtin_copy',
            'quiescent_memory_table_init_deinit_and_fiber_retirement',
            'checked_owned_windows_root_guest_fibers_and_thread_restoration',
            'memory_shader_key_binary_cache_and_full_pre_gl_admission',
            'bounded_driver_binary_capture_and_complete_image_publication',
            'memory_texture_gzip_checked_records_crc_and_transactional_cache_swap',
            'bounded_texture_residency_and_exception_safe_exact_lru_ownership',
            'memory_file_texture_index_complete_bounds_and_snapshot_backed_pending_records',
            'bounded_shared_workspace_texture_pool_and_checked_filter_failure',
            'truthful_filter_initialization_and_quiescent_pool_retirement',
            'scratch_png_texture_dump_checked_extent_codec_budget_and_error_cleanup',
            'bounded_scratch_texture_debug_and_format_safe_frontend_diagnostics',
            'separate_persistent_save_namespace_and_atomic_exact_read_publication',
            'actual_file_storage_retains_failed_dirty_ranges_and_rejects_bad_initial_read',
            'transferpak_rom_fingerprint_port_save_and_readonly_old_ram_import',
            'dd_bounded_transactional_format_expansion_and_actual_geometry_ranges',
            'dd_content_selected_memory_save_and_checked_readonly_legacy_import',
            'dd_exact_expanded_size_and_checked_descriptor_publication_and_ipl_bounds',
            'thread_fiber_owner_checked_prepare_pause_flush_and_snapshot_guest_checkpoints',
            'retained_peripheral_close_and_quiescent_restore_callback_detach_acknowledgement',
            'bounded_event_queue_pif_pointer_preflight_and_actual_format_dd_snapshot_id',
            'tlb_wire_fields_derived_ranges_lookup_admission_and_finite_top_page_walk',
            'interpreter_tlb_random_index_guard_before_any_entry_dereference',
            'transferpak_snapshot_selectors_bounded_before_restore_mutation',
            'gb_bounded_storage_access_full_width_ram_bank_offsets_and_camera_extent',
            'mbc3_bounded_signed_time_halt_carry_and_persistent_latched_reads',
            'm64p_v1_10_rtc_latch_flags_with_explicit_legacy_import_and_separate_core_identity',
            'content_port_rtc_battery_record_staged_boot_offline_time_and_checked_retirement',
            'flash_complete_storage_dram_ranges_and_si_full_block_admission',
            'dd_c2_sector_subtraction_bounds_and_snapshot_device_selector_preflight',
            'active_pif_command_requires_owned_opcode_byte',
            'pi_adjusted_full_dram_admission_before_dispatch_and_framebuffer_callbacks',
            'bounded_framebuffer_descriptors_page_notifications_and_mapping_lifecycle',
            'finite_six_slot_gliden64_framebuffer_identity_cache_without_copy_suppression',
            'sram_complete_backing_and_both_dma_spans_before_copy_and_dirty_notification',
            'cart_rom_widened_source_offsets_checked_words_eof_padding_and_nonzero_invalidation',
            'dd_complete_buffer_ipl_dma_spans_checked_words_and_wide_cycle_arithmetic',
            'cart_domain_dispatch_owner_check_before_save_type_selection',
            'exact_fixed_rdram_dirty_bitmap_without_retained_framebuffer_borrows',
            'bounded_color_pixel_geometry_cpu_buffers_and_driver_dimension_admission',
            'cross_framebuffer_pending_write_retention_and_full_identity_cache_readback_guard',
            'rom_image_boot_extent_word_alignment_profile_size_and_signature_admission',
            'rom_image_unaligned_byte_conversion_and_bounded_header_name_diagnostic',
            'frontend_early_rom_path_admission_and_checked_companion_allocations',
            'frontend_rom_open_receipt_staged_header_and_failed_close_retention',
            'audio_checked_partial_allocation_cleanup_and_explicit_frontend_readiness',
            'finite_audio_output_queue_bounded_consumer_calls_and_root_backpressure',
            'audio_callback_borrow_blocks_root_reentry_and_requires_retirement_before_detach',
            'ai_full_ram_publication_before_pointer_and_immutable_backend_source_copy',
            'ai_wide_checked_duration_fifo_request_and_delayed_carry_admission',
            'sinc_phase_prewrite_capacity_admission_and_bounded_input_chunking',
            'sinc_bounded_filter_tables_and_per_instance_selected_processor',
            'ai_staged_fifo_start_and_promotion_preserve_slots_on_admission_failure',
            'ai_event_pool_insertion_acknowledgement_before_fifo_publication_and_failure_stop']
