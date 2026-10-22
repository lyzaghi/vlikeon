// OmniPlay adaptation of the pinned GLideN64 shader storage implementation.
// Retains its key versions 4/5 and binary layout. All cache I/O is memory only.
#include "n64-memory-streams.hpp"
#include "n64-shader-cache.hpp"
#include <algorithm>
#include <functional>
#include <iomanip>
#include <memory>
#include <sstream>
#include <Graphics/CombinerProgram.h>
#include <Graphics/Context.h>
#include <Graphics/OpenGLContext/opengl_Utils.h>
#include <Types.h>
#include <Log.h>
#include <RSP.h>
#include <Combiner.h>
#include <DisplayLoadProgress.h>
#include <Config.h>
#include "glsl_Utils.h"
#include "glsl_ShaderStorage.h"
#include "glsl_CombinerProgramImpl.h"
#include "glsl_CombinerProgramUniformFactoryAccurate.h"
#include "glsl_CombinerProgramUniformFactoryFast.h"

using namespace glsl;
static_assert(sizeof(u32)==4&&sizeof(u64)==8&&sizeof(int)==4&&sizeof(GLenum)==4&&sizeof(GLint)==4,
  "Pinned native shader cache layout changed");
namespace {
std::string getStorageFileName(const opengl::GLInfo& info,const char* extension){
  const char* directory="scratch/Mupen64plus/shaders";
  if(om_n64_mkdir(directory)!=0)return std::string();
  std::ostringstream name;name<<directory<<"/GLideN64."<<std::hex
    <<static_cast<u32>(std::hash<std::string>()(RSP.romname))
    <<(info.isGLESX?".GLES.":".OpenGL.")<<extension;
  return name.str();
}
bool driverStrings(std::string& renderer,std::string& version){
  return om_n64_shader::driver_text(reinterpret_cast<const char*>(glGetString(GL_RENDERER)),renderer)&&
    om_n64_shader::driver_text(reinterpret_cast<const char*>(glGetString(GL_VERSION)),version);
}
void remember(graphics::Combiners& combiners,std::unique_ptr<graphics::CombinerProgram> program){
  if(!program)return;const auto key=program->getKey();program->update(true);
  if(combiners.emplace(key,program.get()).second)program.release();
}
struct ProgramOwner {
  GLuint id;explicit ProgramOwner(GLuint value):id(value){}
  ~ProgramOwner(){if(id)glDeleteProgram(id);}
  ProgramOwner(const ProgramOwner&)=delete;ProgramOwner& operator=(const ProgramOwner&)=delete;
};
std::unique_ptr<graphics::CombinerProgram> fromRecord(const om_n64_shader::Record& record,
    CombinerKey& key,CombinerProgramUniformFactory& factory,opengl::CachedUseProgram* use){
  CombinerInputs inputs(record.inputs);ProgramOwner program(glCreateProgram());if(!program.id)return nullptr;
  glsl::Utils::locateAttributes(program.id,key.isRectKey(),inputs.usesTexture());
  glProgramBinary(program.id,static_cast<GLenum>(record.format),record.binary.data(),static_cast<GLint>(record.binary.size()));
  if(!glsl::Utils::checkProgramLinkStatus(program.id,true))return nullptr;
  UniformGroups uniforms;factory.buildUniforms(program.id,inputs,key,uniforms);
  std::unique_ptr<graphics::CombinerProgram> result(new CombinerProgramImpl(key,program.id,use,inputs,std::move(uniforms)));
  program.id=0;return result;
}
}

bool ShaderStorage::_saveCombinerKeys(const graphics::Combiners& combiners)const{
  try{
    if(combiners.size()>om_n64_shader::maximum_records)return false;
    std::vector<u64> keys;keys.reserve(combiners.size());for(const auto& item:combiners)keys.push_back(item.first.getMux());
    std::sort(keys.begin(),keys.end());OmniN64Output output(getStorageFileName(m_glinfo,"keys"));if(!output)return false;
    output<<"0x"<<std::hex<<std::setfill('0')<<std::setw(8)<<m_keysFormatVersion<<"\n"
      <<"0x"<<std::setw(8)<<keys.size()<<"\n";
    for(const auto key:keys)output<<"0x"<<std::setw(16)<<key<<"\n";
    output.close();return !output.fail();
  }catch(...){return false;}
}

bool ShaderStorage::saveShadersStorage(const graphics::Combiners& combiners)const{
  try{
    if(!_saveCombinerKeys(combiners))return false;
    if(gfxContext.isCombinerProgramBuilderObsolete()||!graphics::Context::ShaderProgramBinary)return true;
    std::string renderer,version;if(!driverStrings(renderer,version))return false;
    OmniN64Output output(getStorageFileName(m_glinfo,"shaders"),OmniN64Output::binary);if(!output)return false;
    const u32 options=graphics::CombinerProgram::getShaderCombinerOptionsBits();
    output.write(reinterpret_cast<const char*>(&m_formatVersion),sizeof(m_formatVersion));
    output.write(reinterpret_cast<const char*>(&options),sizeof(options));
    u32 length=static_cast<u32>(renderer.size());output.write(reinterpret_cast<const char*>(&length),sizeof(length));output.write(renderer.data(),length);
    length=static_cast<u32>(version.size());output.write(reinterpret_cast<const char*>(&length),sizeof(length));output.write(version.data(),length);
    const auto countPosition=output.tellp();u32 count=0;output.write(reinterpret_cast<const char*>(&count),sizeof(count));
    if(!output)return false;
    std::uint64_t total=16+renderer.size()+version.size()+sizeof(count);
    displayLoadProgress(L"SAVE COMBINER SHADERS %.1f%%",0.0f);
    for(const auto& item:combiners){
      std::vector<char> data;
      if(!item.second->getBinaryForm(data))continue;
      if(data.size()<20||data.size()>om_n64_shader::maximum_binary+20||total>om_n64_shader::maximum_image||
         data.size()>om_n64_shader::maximum_image-total){output.setstate(std::ios_base::failbit);break;}
      output.write(data.data(),static_cast<std::streamsize>(data.size()));if(!output)break;total+=data.size();++count;
      displayLoadProgress(L"SAVE COMBINER SHADERS %.1f%%",static_cast<f32>(count)*100.f/static_cast<f32>(combiners.size()));
    }
    if(output){output.seekp(countPosition);output.write(reinterpret_cast<const char*>(&count),sizeof(count));}
    output.close();displayLoadProgress(L"");return !output.fail();
  }catch(...){displayLoadProgress(L"");return false;}
}

bool ShaderStorage::_loadFromCombinerKeys(graphics::Combiners& combiners){
  try{
    OmniN64Input input(getStorageFileName(m_glinfo,"keys"));if(!input)return false;
    om_n64_shader::Keys keys;const bool read=om_n64_shader::read_keys(input,keys);input.close();
    if(!read||input.fail())return false;
    if(keys.legacy_hwl)GBI.setHWLSupported(keys.hwl);
    for(std::size_t n=0;n<keys.values.size();++n){
      CombinerKey key(keys.values[n],false);if(!keys.legacy_hwl)GBI.setHWLSupported(key.isHWLSupported());
      std::unique_ptr<graphics::CombinerProgram> program(Combiner_Compile(key));if(!program)return false;
      remember(combiners,std::move(program));displayLoadProgress(L"LOAD COMBINER SHADERS %.1f%%",static_cast<f32>(n+1)*100.f/static_cast<f32>(keys.values.size()));
    }
    displayLoadProgress(L"");if(opengl::Utils::isGLError())return false;
    return graphics::Context::ShaderProgramBinary?saveShadersStorage(combiners):true;
  }catch(...){displayLoadProgress(L"");return false;}
}

bool ShaderStorage::loadShadersStorage(graphics::Combiners& combiners){
  if(!graphics::Context::ShaderProgramBinary)return _loadFromCombinerKeys(combiners);
  try{
    std::string renderer,version;if(!driverStrings(renderer,version))return _loadFromCombinerKeys(combiners);
    OmniN64Input input(getStorageFileName(m_glinfo,"shaders"),OmniN64Input::binary);if(!input)return _loadFromCombinerKeys(combiners);
    om_n64_shader::Image image;const auto options=graphics::CombinerProgram::getShaderCombinerOptionsBits();
    const bool read=om_n64_shader::read_image(input,m_formatVersion,options,renderer,version,image);input.close();
    if(!read||input.fail())return _loadFromCombinerKeys(combiners);
    std::unique_ptr<CombinerProgramUniformFactory> factory;
    if(config.generalEmulation.enableInaccurateTextureCoordinates)factory.reset(new CombinerProgramUniformFactoryFast(m_glinfo));
    else factory.reset(new CombinerProgramUniformFactoryAccurate(m_glinfo));
    for(std::size_t n=0;n<image.records.size();++n){
      const auto& record=image.records[n];CombinerKey key(record.mux,false);
      auto program=fromRecord(record,key,*factory,m_useProgram);
      if(!program)program.reset(Combiner_Compile(key));if(!program)return false;
      remember(combiners,std::move(program));displayLoadProgress(L"LOAD COMBINER SHADERS %.1f%%",static_cast<f32>(n+1)*100.f/static_cast<f32>(image.records.size()));
    }
    displayLoadProgress(L"");return !opengl::Utils::isGLError();
  }catch(...){displayLoadProgress(L"");return false;}
}

ShaderStorage::ShaderStorage(const opengl::GLInfo& info,opengl::CachedUseProgram* use):m_glinfo(info),m_useProgram(use){}
