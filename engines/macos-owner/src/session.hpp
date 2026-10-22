#pragma once
#include <xpc/xpc.h>

namespace omniplay::macos {
enum class StageStatus { ok, rejected, storage_failure };
StageStatus stage_session(xpc_object_t request);
StageStatus discard_session(xpc_object_t request);
}
