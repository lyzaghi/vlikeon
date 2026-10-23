#include "bounded_samples.hpp"
#include <atomic>
#include <cstdlib>
#include <iostream>
#include <thread>
#include <utility>

void require(bool value, const char *message) {
  if (!value) { std::cerr << message << '\n'; std::exit(1); }
}
struct Sample {
  static inline std::atomic<int> live = 0;
  int value = -1;
  explicit Sample(int id) : value(id) { ++live; }
  Sample(const Sample&) = delete;
  Sample& operator=(const Sample&) = delete;
  Sample(Sample&& other) noexcept : value(std::exchange(other.value,-1)) {}
  Sample& operator=(Sample&& other) noexcept {
    if (this != &other) { if (value >= 0) --live; value = std::exchange(other.value,-1); }
    return *this;
  }
  ~Sample() { if (value >= 0) --live; }
};
using Mailbox = vlikeon::capture::BoundedSamples<Sample,2,8>;
int main() {
  {
    Mailbox queue;
    require(!queue.push(Sample(0),false) && Sample::live == 0,"Pre-start sample leaked");
    require(queue.open() && !queue.open(),"Duplicate start was admitted");
    for (int id=0; id<10000; ++id) queue.push(Sample(id),false);
    require(Sample::live==2,"Backlogged video ownership exceeded two");
    for (int id=0; id<10000; ++id) queue.push(Sample(id),true);
    require(Sample::live==10,"Backlogged combined ownership exceeded ten");
    auto video=queue.take(false),audio=queue.take(true);
    require(video && video->value==9998 && audio && audio->value==9992,"Oldest live samples not evicted independently");
    require(Sample::live==10,"Moving out a sample changed ownership");
    queue.close();
    require(Sample::live==2,"Close retained queued samples or invalidated consumer ownership");
    require(!queue.take(false) && !queue.take(true),"Stopped queue returned a sample");
    require(!queue.open() && !queue.push(Sample(10001),true),"Late callback reopened a stopped queue");
    video.reset();audio.reset();require(Sample::live==0,"Consumer samples leaked");
    queue.close();
  }
  for (int run=0; run<100; ++run) {
    Mailbox queue; require(queue.open(),"Start failed");
    std::atomic<bool> ready=false,closed=false;
    std::thread producer([&]{ ready=true;for(int n=0;n<2000;++n)queue.push(Sample(n),n%2!=0);while(!closed.load())std::this_thread::yield();require(!queue.push(Sample(3000),false),"Producer published after close"); });
    while(!ready.load())std::this_thread::yield();
    queue.close();closed=true;producer.join();
    require(!queue.take(false)&&!queue.take(true)&&Sample::live==0,"Concurrent close leaked media or delivered late sample");
  }
  std::cout << "Bounded ownership, eviction, consumer retention and 100 concurrent-close runs passed.\n";
}
