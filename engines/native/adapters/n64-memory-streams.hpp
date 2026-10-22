#pragma once
#include "n64-memory-io.h"
#include <cstdio>
#include <exception>
#include <istream>
#include <limits>
#include <ostream>
#include <streambuf>
#include <string>

// These scratch-only streams intentionally preserve the std::istream/ostream
// surface used by the pinned cache code. They are not std::fstream substitutes
// for arbitrary application code, and they never open an OS file.
class OmniN64Buffer final:public std::streambuf {
  OmniN64File* file_=nullptr;char lookahead_=0;bool input_=false;
protected:
  int_type underflow()override{
    if(!file_||!input_)return traits_type::eof();
    if(gptr()&&gptr()<egptr())return traits_type::to_int_type(*gptr());
    const auto byte=om_n64_fgetc(file_);if(byte==EOF)return traits_type::eof();
    lookahead_=static_cast<char>(byte);setg(&lookahead_,&lookahead_,&lookahead_+1);return traits_type::to_int_type(lookahead_);
  }
  std::streamsize xsgetn(char* data,std::streamsize bytes)override{
    if(!file_||!input_||bytes<0)return 0;std::streamsize done=0;
    if(bytes&&gptr()&&gptr()<egptr()){*data++=*gptr();gbump(1);++done;}
    if(done<bytes)done+=static_cast<std::streamsize>(om_n64_fread(data,1,static_cast<std::size_t>(bytes-done),file_));return done;
  }
  std::streamsize xsputn(const char* data,std::streamsize bytes)override{
    if(!file_||input_||bytes<0)return 0;return static_cast<std::streamsize>(om_n64_fwrite(data,1,static_cast<std::size_t>(bytes),file_));
  }
  int_type overflow(int_type byte=traits_type::eof())override{
    if(!file_||input_)return traits_type::eof();
    if(traits_type::eq_int_type(byte,traits_type::eof()))return sync()==0?traits_type::not_eof(byte):traits_type::eof();
    const auto value=traits_type::to_char_type(byte);return om_n64_fwrite(&value,1,1,file_)==1?byte:traits_type::eof();
  }
  int sync()override{return file_&&om_n64_fflush(file_)==0?0:-1;}
  pos_type seekoff(off_type offset,std::ios_base::seekdir direction,std::ios_base::openmode)override{
    if(!file_)return pos_type(off_type(-1));const auto cached=input_&&gptr()&&gptr()<egptr()?1:0;
    if(direction==std::ios_base::cur){if(offset<std::numeric_limits<off_type>::min()+cached){om_n64_abort(file_);return pos_type(off_type(-1));}offset-=cached;}
    if(offset<std::numeric_limits<long>::min()||offset>std::numeric_limits<long>::max()){om_n64_abort(file_);return pos_type(off_type(-1));}
    const auto origin=direction==std::ios_base::beg?SEEK_SET:direction==std::ios_base::cur?SEEK_CUR:direction==std::ios_base::end?SEEK_END:-1;
    if(om_n64_fseek(file_,static_cast<long>(offset),origin)!=0)return pos_type(off_type(-1));
    setg(nullptr,nullptr,nullptr);const auto position=om_n64_ftell(file_);return position<0?pos_type(off_type(-1)):pos_type(position);
  }
  pos_type seekpos(pos_type position,std::ios_base::openmode mode)override{return seekoff(off_type(position),std::ios_base::beg,mode);}
public:
  ~OmniN64Buffer()override{close();}
  bool open(const char* path,bool input){
    if(file_)return false;file_=om_n64_fopen(path,input?"rb":"wb");input_=input;setg(nullptr,nullptr,nullptr);return file_!=nullptr;
  }
  bool close(bool commit=true)noexcept{
    if(!file_)return true;if(!commit)om_n64_abort(file_);auto* old=file_;file_=nullptr;setg(nullptr,nullptr,nullptr);return om_n64_fclose(old)==0;
  }
  bool is_open()const noexcept{return file_!=nullptr;}
};

class OmniN64Input final:public std::istream {
  OmniN64Buffer buffer_;
public:
  OmniN64Input():std::istream(nullptr){rdbuf(&buffer_);clear();}
  explicit OmniN64Input(const char* path,std::ios_base::openmode mode=std::ios_base::in):OmniN64Input(){open(path,mode);}
  explicit OmniN64Input(const std::string& path,std::ios_base::openmode mode=std::ios_base::in):OmniN64Input(path.c_str(),mode){}
  ~OmniN64Input()override{buffer_.close();}
  void open(const char* path,std::ios_base::openmode mode=std::ios_base::in){
    if((mode&(std::ios_base::out|std::ios_base::app|std::ios_base::trunc))||!buffer_.open(path,true)){setstate(std::ios_base::failbit);return;}clear();if(mode&std::ios_base::ate)seekg(0,std::ios_base::end);
  }
  void open(const std::string& path,std::ios_base::openmode mode=std::ios_base::in){open(path.c_str(),mode);}
  void close(){if(!buffer_.close())setstate(std::ios_base::failbit);}
  bool is_open()const noexcept{return buffer_.is_open();}
};

class OmniN64Output final:public std::ostream {
  OmniN64Buffer buffer_;
public:
  OmniN64Output():std::ostream(nullptr){rdbuf(&buffer_);clear();}
  explicit OmniN64Output(const char* path,std::ios_base::openmode mode=std::ios_base::out):OmniN64Output(){open(path,mode);}
  explicit OmniN64Output(const std::string& path,std::ios_base::openmode mode=std::ios_base::out):OmniN64Output(path.c_str(),mode){}
  ~OmniN64Output()override{
#if __cplusplus >= 201703L
    buffer_.close(!fail()&&std::uncaught_exceptions()==0);
#else
    buffer_.close(!fail()&&!std::uncaught_exception());
#endif
  }
  void open(const char* path,std::ios_base::openmode mode=std::ios_base::out){
    // The selected cache code writes new complete images. Reject update/append
    // modes rather than silently weakening its atomic-publication contract.
    if((mode&(std::ios_base::in|std::ios_base::app|std::ios_base::ate))||!buffer_.open(path,false)){setstate(std::ios_base::failbit);return;}clear();
  }
  void open(const std::string& path,std::ios_base::openmode mode=std::ios_base::out){open(path.c_str(),mode);}
  void close(){if(!buffer_.close(!fail()))setstate(std::ios_base::failbit);}
  bool is_open()const noexcept{return buffer_.is_open();}
};
