#pragma once
#include "wire.hpp"
#include <memory>

namespace omni::presentation {
// Separate trusted-owner socket. The core's media channel cannot request an
// owner focus operation or supply an executable/path/activation command here.
class Owner{
  struct State;std::shared_ptr<State> state_;
public:
  Owner()=default;
  static Owner gated(Session,std::uint32_t expected_parent);
  bool stop_requested()const;
  bool take_focus();
};
}
