// Pinned TxMemBuf interface delegates to the actual bounded pool implementation.
TxMemBuf::TxMemBuf(){}
// The pool owns storage and is explicitly retired by TxFilter::clear. Avoid
// cross-singleton destructor order at DLL teardown; failed retirement exits
// the actor before any library/static destructor is allowed to run.
TxMemBuf::~TxMemBuf(){}
boolean TxMemBuf::init(int width,int height){return om_n64_texture::pool().init(width,height)?1:0;}
void TxMemBuf::shutdown(){(void)om_n64_texture::pool().shutdown();}
uint8* TxMemBuf::get(uint32 slot){return om_n64_texture::pool().shared(slot);}
uint32 TxMemBuf::size_of(uint32 slot){return om_n64_texture::pool().size(slot);}
uint32* TxMemBuf::getThreadBuf(uint32 lane,uint32 slot,uint32 count){return om_n64_texture::pool().workspace(lane,slot,count);}
