// Authored for item 16. These use the production pool and real Rust threads.
// They do not establish actor/job exit, actual pipe cancellation or disk safety.
use super::*;
use std::{sync::mpsc,time::{Duration,Instant}};

fn collected(pool:&WorkerPool,expected:usize)->bool{
    let until=Instant::now()+Duration::from_secs(3);
    loop{pool.collect();if pool.used.load(Ordering::Acquire)==expected{return true;}
        if Instant::now()>=until{return false;}std::thread::sleep(Duration::from_millis(1));}
}
#[test]
fn reservations_are_finite_and_unused_prelaunch_capacity_is_returned(){
    let pool=Arc::new(WorkerPool::new());let mut held=Vec::new();
    for _ in 0..LIMIT{held.push(pool.reserve().expect("reserve"));}
    let error=pool.reserve().err().expect("finite admission");assert_eq!(error.code,ErrorCode::ResourceLimit);assert_eq!(pool.used.load(Ordering::Acquire),LIMIT);
    held.pop();assert_eq!(pool.used.load(Ordering::Acquire),LIMIT-1);
    held.push(pool.reserve().expect("unused reservation reusable"));drop(held);assert_eq!(pool.used.load(Ordering::Acquire),0);
}
#[test]
fn dropping_a_live_worker_never_joins_or_refunds_it(){
    let pool=Arc::new(WorkerPool::new());let mut held=Vec::new();for _ in 1..LIMIT{held.push(pool.reserve().expect("reserve"));}
    let(entered,observed)=mpsc::channel();let(release,gate)=mpsc::channel();
    let worker=pool.reserve().expect("last slot").spawn(move||{let _=entered.send(());let _=gate.recv();}).expect("spawn");
    let running=observed.recv_timeout(Duration::from_secs(2)).is_ok();
    let(closed,closure)=mpsc::channel();let closer=std::thread::spawn(move||{drop(worker);let _=closed.send(());});
    let close_returned=closure.recv_timeout(Duration::from_secs(1)).is_ok();
    let still_charged=pool.used.load(Ordering::Acquire)==LIMIT;
    let refused=pool.reserve().is_err(); // May collect completed entries, never this live one.
    let _=release.send(());let close_joined=closer.join().is_ok();let retired=collected(&pool,LIMIT-1);
    // Release the gates before assertions, so a broken join implementation
    // cannot leave test threads permanently waiting after a failed assertion.
    assert!(running&&close_returned&&still_charged&&refused&&close_joined&&retired);
    let replacement=pool.reserve().expect("joined worker capacity reusable");drop(replacement);drop(held);assert_eq!(pool.used.load(Ordering::Acquire),0);
}
#[test]
fn worker_panic_is_joined_before_its_slot_is_refunded(){
    let pool=Arc::new(WorkerPool::new());let worker=pool.reserve().expect("reserve").spawn(||panic!("qualification control worker panic")).expect("spawn");
    drop(worker);assert!(collected(&pool,0));assert!(pool.retired.lock().unwrap_or_else(|error|error.into_inner()).iter().all(Option::is_none));
}
#[test]
fn independent_pools_do_not_borrow_each_others_capacity(){
    let left=Arc::new(WorkerPool::new());let right=Arc::new(WorkerPool::new());let mut held=Vec::new();for _ in 0..LIMIT{held.push(left.reserve().expect("reserve"));}
    assert!(left.reserve().is_err());let right_slot=right.reserve().expect("separate test owner");assert_eq!(left.used.load(Ordering::Acquire),LIMIT);assert_eq!(right.used.load(Ordering::Acquire),1);drop(right_slot);drop(held);
}
#[test]
fn a_single_slot_pool_retains_a_dropped_owner_until_its_thread_ends(){
    let pool=Arc::new(WorkerPool::with_limit(1));let(release,gate)=mpsc::channel();
    let owner=pool.reserve().expect("single reservation").spawn(move||{let _=gate.recv();}).expect("spawn");
    drop(owner);let busy=pool.pending();let refused=pool.reserve().is_err();
    let _=release.send(());let retired=collected(&pool,0);
    assert!(busy&&refused&&retired);assert!(!pool.pending());
    drop(pool.reserve().expect("retired capacity reusable"));
}
#[test]
fn recovery_owner_replacement_cannot_detach_and_replace_a_live_collector(){
    // Production singleton, not a new private test pool. Run this filter with
    // one test thread in item 16; all gates are released before assertions.
    let(release,gate)=mpsc::channel();let(published,result)=mpsc::channel();
    let owner=NativeRecoveryWorker::spawn(move||{let _=published.send(());let _=gate.recv();}).expect("collector");
    let result_published=result.recv_timeout(Duration::from_secs(2)).is_ok();
    let still_owned=!owner.has_ended();drop(owner);
    let busy=NativeRecoveryWorker::pending();let replacement=NativeRecoveryWorker::spawn(||{});
    let refused=replacement.as_ref().err().is_some_and(|error|error.code==ErrorCode::Busy);
    drop(replacement);let _=release.send(());let retired=collected(&RECOVERY_WORKERS,0);
    assert!(result_published&&still_owned&&busy&&refused&&retired);
    let next=NativeRecoveryWorker::spawn(||{}).expect("ended collector reusable");drop(next);assert!(collected(&RECOVERY_WORKERS,0));
}
#[test]
fn losing_the_pool_record_cannot_guess_a_refund_from_later_external_join(){
    let pool=Arc::new(WorkerPool::new());let(release,gate)=mpsc::channel();
    let mut owner=pool.reserve().expect("reserve").spawn(move||{let _=gate.recv();}).expect("spawn");
    // Qualification-only ownership-loss injection. Keep the actual handle in
    // the test so its OS thread can still be joined during test cleanup.
    let mut record=owner.record.take().expect("owned record");let thread=record.thread.take().expect("held thread");drop(record);drop(owner);
    let _=release.send(());let joined=thread.join().is_ok();pool.collect();assert!(joined);assert_eq!(pool.used.load(Ordering::Acquire),1);
    let mut held=Vec::new();for _ in 1..LIMIT{held.push(pool.reserve().expect("remaining capacity"));}assert!(pool.reserve().is_err());drop(held);assert_eq!(pool.used.load(Ordering::Acquire),1);
    // The deliberately lost permit/counter metadata remains charged until this
    // qualification process exits. Do not repair it by editing the counter.
}

#[cfg(windows)]
struct TlsGate{entered:mpsc::Sender<()>,release:mpsc::Receiver<()>}
#[cfg(windows)]
impl Drop for TlsGate{fn drop(&mut self){let _=self.entered.send(());let _=self.release.recv();}}
#[cfg(windows)]
std::thread_local!{static TLS_GATE:std::cell::RefCell<Option<TlsGate>>=const{std::cell::RefCell::new(None)};}

#[cfg(windows)]
#[test]
fn windows_tls_cleanup_must_end_before_admission_can_reclaim_a_slot(){
    let pool=Arc::new(WorkerPool::new());let mut held=Vec::new();for _ in 1..LIMIT{held.push(pool.reserve().expect("reserve"));}
    // Start the probe before intentionally blocking TLS cleanup. Creating a
    // new OS thread while cleanup is stalled can itself serialize on Windows.
    let(begin,start)=mpsc::channel();let(probed,probe)=mpsc::channel();let(ready,probe_ready)=mpsc::channel();let probing=pool.clone();
    let collector=std::thread::spawn(move||{let _=std::thread::current().id();let _=ready.send(());if start.recv().is_err(){return;}let refused=probing.reserve().is_err();let _=probed.send(refused&&probing.used.load(Ordering::Acquire)==LIMIT);});
    let probe_running=probe_ready.recv_timeout(Duration::from_secs(2)).is_ok();
    let(entered,observed)=mpsc::channel();let(release,gate)=mpsc::channel();
    let worker=pool.reserve().expect("last slot").spawn(move||{TLS_GATE.with(|value|*value.borrow_mut()=Some(TlsGate{entered,release:gate}));}).expect("spawn");
    drop(worker);let tls_running=observed.recv_timeout(Duration::from_secs(2)).is_ok();
    let _=begin.send(());
    let nonblocking_refusal=probe.recv_timeout(Duration::from_secs(1)).ok();
    let _=release.send(());let collector_joined=collector.join().is_ok();let retired=collected(&pool,LIMIT-1);
    assert!(probe_running&&tls_running&&collector_joined&&retired);assert_eq!(nonblocking_refusal,Some(true));
    drop(held);assert_eq!(pool.used.load(Ordering::Acquire),0);
}
