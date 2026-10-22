#![cfg_attr(all(windows,not(debug_assertions)),windows_subsystem="windows")]
fn main(){
    let args:Vec<_>=std::env::args_os().collect();if args.len()!=2||args[1]!="--stdio-input-v1"{std::process::exit(2);}
    let result=omniplay_stream_input::run_worker();
    // The private reader may still be blocked in stdin. TargetInput has already
    // released its tracked state; process exit closes that reader's handles.
    std::process::exit(result);
}
