// Only graphics enum values are fixture services. The separately compiled
// TxCache.cpp, bounded pool/TxMemBuf, adapters/store and codec are actual code.
#include <Graphics/Parameters.h>

namespace graphics {
namespace internalcolorFormat {
InternalColorFormatParam COLOR_INDEX8(0x80e5U),RGBA4(0x8056U),RGB5_A1(0x8057U),RGB8(0x8051U),RGBA8(0x8058U);
}
namespace colorFormat {ColorFormatParam RGBA(0x1908U),RED_GREEN_BLUE(0x1907U);}
namespace datatype {
DatatypeParam UNSIGNED_BYTE(0x1401U),UNSIGNED_SHORT_5_6_5(0x8363U),UNSIGNED_SHORT_4_4_4_4(0x8033U),UNSIGNED_SHORT_5_5_5_1(0x8034U);
}
}

