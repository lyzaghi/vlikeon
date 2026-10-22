#include "n64-texture-pool.hpp"
namespace om_n64_texture {TexturePool& pool(){static TexturePool value;return value;}}
extern "C" int om_n64_texture_pool_retired(void){return om_n64_texture::pool().retired()?1:0;}
