use std::{env,path::PathBuf,process::Command};
fn main(){
    println!("cargo:rerun-if-changed=native/macos_xpc.mm");
    println!("cargo:rerun-if-changed=native/macos_xpc.h");
    if env::var("CARGO_CFG_TARGET_OS").as_deref()!=Ok("macos"){return;}
    let output=PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"));
    let target=env::var("CARGO_CFG_TARGET_ARCH").expect("Cargo target architecture");
    let arch=match target.as_str(){"aarch64"=>"arm64","x86_64"=>"x86_64",_=>panic!("Unsupported macOS native architecture")};
    let object=output.join("macos_xpc.o");let archive=output.join("libomniplay_macos_xpc.a");
    let compiled=Command::new("xcrun").args(["--sdk","macosx","clang++","-std=c++20","-fobjc-arc","-fblocks","-fstack-protector-strong","-mmacosx-version-min=11.0","-arch",arch,"-c","native/macos_xpc.mm","-o"]).arg(&object).status().expect("The selected Xcode compiler is required");
    assert!(compiled.success(),"macOS XPC bridge compilation failed");
    let packed=Command::new("xcrun").args(["--sdk","macosx","ar","rcs"]).arg(&archive).arg(&object).status().expect("The selected Xcode archiver is required");assert!(packed.success(),"macOS XPC bridge archive failed");
    println!("cargo:rustc-link-search=native={}",output.display());println!("cargo:rustc-link-lib=static=omniplay_macos_xpc");
    println!("cargo:rustc-link-lib=framework=Foundation");println!("cargo:rustc-link-lib=framework=Security");println!("cargo:rustc-link-lib=c++");
}
