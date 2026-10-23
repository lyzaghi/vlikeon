#![cfg_attr(all(windows,not(debug_assertions)),windows_subsystem="windows")]
fn main(){
    let args:Vec<_>=std::env::args_os().collect();if args.len()!=2{std::process::exit(2);}
    let result=if args[1]=="--stdio-input-v1"{omniplay_stream_input::run_worker()}
        else if args[1]=="--stdio-gamepad-v2"{omniplay_stream_input::run_gamepad_worker()}
        else{2};
    // A private reader may still be blocked in stdin. Each worker attempts its
    // own teardown; gamepad exit code 3 explicitly retains unknown cleanup.
    // Process exit closes pipes/handles but is not a device-removal receipt.
    std::process::exit(result);
}
