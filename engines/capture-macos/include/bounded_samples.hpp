#pragma once
#include <cstddef>
#include <deque>
#include <mutex>
#include <optional>
#include <utility>

namespace vlikeon::capture {
// Own values rather than borrowed media pointers. Backlogged old samples are
// released on eviction; close atomically clears both media types and fences
// late producer callbacks. A stopped mailbox is never reopened.
template<class T, size_t VideoBound, size_t AudioBound>
class BoundedSamples {
  static_assert(VideoBound > 0 && AudioBound > 0);
  std::mutex mutex_;
  std::deque<T> video_, audio_;
  bool accepting_ = false, closed_ = false;
public:
  bool open() {
    std::lock_guard lock(mutex_);
    if (closed_ || accepting_) return false;
    accepting_ = true; return true;
  }
  void close() {
    std::lock_guard lock(mutex_);
    accepting_ = false; closed_ = true; video_.clear(); audio_.clear();
  }
  bool push(T sample, bool audio) {
    std::lock_guard lock(mutex_);
    if (!accepting_) return false;
    auto& queue = audio ? audio_ : video_;
    const size_t bound = audio ? AudioBound : VideoBound;
    if (queue.size() == bound) queue.pop_front();
    queue.push_back(std::move(sample)); return true;
  }
  std::optional<T> take(bool audio) {
    std::lock_guard lock(mutex_);
    auto& queue = audio ? audio_ : video_;
    if (!accepting_ || queue.empty()) return std::nullopt;
    T value = std::move(queue.front()); queue.pop_front(); return value;
  }
};
}
