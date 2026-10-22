"""Explicit macOS build recipe. Reserved until full authoring is complete."""
import argparse
import os
from pathlib import Path
import platform
import subprocess

ROOT=Path(__file__).resolve().parents[1]

def run(values,env=None):subprocess.run([str(v) for v in values],cwd=ROOT,env=env,check=True)
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument("--cargo",required=True,type=Path);parser.add_argument("--xcodebuild",required=True,type=Path);parser.add_argument("--team");parser.add_argument("--configuration",choices=["Debug","Release"],default="Release");parser.add_argument("--archive",action="store_true");args=parser.parse_args()
    if platform.system()!="Darwin":raise ValueError("The actual iOS build requires a Mac with the selected Apple SDK")
    if not (ROOT/"apps/ios/OmniPlay.xcodeproj/project.pbxproj").is_file():raise ValueError("Assemble the iOS resources/project first")
    if args.archive and not args.team:raise ValueError("A real signing team must be explicitly selected for an archive")
    for target in ["aarch64-apple-ios","aarch64-apple-ios-sim"]:run([args.cargo,"build","--locked","--release","--target",target,"-p","omniplay-mobile"])
    framework=ROOT/"apps/ios/Libraries/OmniPlayMobile.xcframework"
    if framework.exists():raise ValueError("Choose a fresh XCFramework output before this explicit build")
    framework.parent.mkdir(parents=True,exist_ok=True)
    command=[args.xcodebuild,"-create-xcframework"]
    for target in ["aarch64-apple-ios","aarch64-apple-ios-sim"]:command += ["-library",ROOT/"target"/target/"release/libomniplay_mobile.a","-headers",ROOT/"platform/mobile/include"]
    run(command+["-output",framework])
    command=[args.xcodebuild,"-project",ROOT/"apps/ios/OmniPlay.xcodeproj","-scheme","OmniPlay","-configuration",args.configuration,"-destination","generic/platform=iOS"]
    if args.archive:command += ["-archivePath",ROOT/"out/ios/OmniPlay.xcarchive","archive",f"DEVELOPMENT_TEAM={args.team}"]
    else:command += ["build","CODE_SIGNING_ALLOWED=NO"]
    run(command)
    print("iOS build completed. Device/compatibility/security verification must be recorded separately.")

if __name__=="__main__":main()
