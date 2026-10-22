// SPDX-License-Identifier: GPL-3.0-or-later
// Adapted from melonDS DS, Copyright 2023 Jesse Talavera-Greenberg.
// This class is for one non-reentrant Worker with no pthreads. Unexpected
// recursive acquisition terminates the guest instead of pretending to wait.
#ifndef MELONDS_DS_THREADS_HPP
#define MELONDS_DS_THREADS_HPP
#include <cstdlib>
namespace retro {
class slock {
public:
    slock() = default;
    slock(const slock&) = delete;
    slock(slock&&) = delete;
    slock& operator=(const slock&) = delete;
    slock& operator=(slock&&) = delete;
    ~slock() noexcept { if(held) std::abort(); }
    void lock() noexcept { if(held) std::abort(); held = true; }
    void unlock() noexcept { if(!held) std::abort(); held = false; }
    bool try_lock() noexcept { if(held) return false; held = true; return true; }
private:
    bool held = false;
};
}
#endif
