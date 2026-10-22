// Pinned native texture diagnostics: finite scratch log, no executable-dir file.
#include "TxDbg.h"
#include "n64-memory-io.h"
#include "n64-memory-path.hpp"
#include <cstdarg>
#include <cwchar>
#include <string>

TxDbg::TxDbg():_level(80){}
TxDbg::~TxDbg(){}
void TxDbg::output(const int level,const wchar_t* format,...)
{
    if(level>_level||!format)return;
    wchar_t text[4096];va_list args;va_start(args,format);
    const auto count=std::vswprintf(text,4096,format,args);va_end(args);
    if(count<0||count>=4096)return;
    try{
        std::string message;if(!om_n64_utf8(text,message,16384,4096))return;
        auto* file=om_n64_fopen("scratch/Mupen64plus/debug/glidenhq.dbg","ab");if(!file)return;
        const auto prior=om_n64_ftell(file);
        // Include prefix/newline slack. Full logs retain their prior image;
        // no automatic truncation and no persistent stream survive the call.
        if(prior<0||static_cast<std::uint64_t>(prior)>1024u*1024u||message.size()+32>1024u*1024u-static_cast<std::uint64_t>(prior))om_n64_abort(file);
        else if(om_n64_fprintf(file,"%d:\t%s\n",level,message.c_str())<0)om_n64_abort(file);
        (void)om_n64_fclose(file);
    }catch(...){return;}
}
