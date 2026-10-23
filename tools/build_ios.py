"""Explicit macOS build recipe. Reserved until full authoring is complete."""
import argparse
import os
import json
import plistlib
from pathlib import Path
import platform
import subprocess

ROOT=Path(__file__).resolve().parents[1]

def run(values,env=None):subprocess.run([str(v) for v in values],cwd=ROOT,env=env,check=True)
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument("--cargo",required=True,type=Path);parser.add_argument("--xcodebuild",required=True,type=Path);parser.add_argument("--team");parser.add_argument("--configuration",choices=["Debug","Release"],default="Release");parser.add_argument("--archive",action="store_true")
    parser.add_argument("--development",action="store_true",help="Explicitly allow the development resource assembly; never archive it")
    parser.add_argument("--destination",choices=["device","simulator"],default="device")
    parser.add_argument("--export-compliance",choices=["exempt","non-exempt"],help="Reviewed export classification, required for archives; never inferred from code")
    args=parser.parse_args()
    if platform.system()!="Darwin":raise ValueError("The actual iOS build requires a Mac with the selected Apple SDK")
    if not (ROOT/"apps/ios/OmniPlay.xcodeproj/project.pbxproj").is_file():raise ValueError("Assemble the iOS resources/project first")
    if args.archive and not args.team:raise ValueError("A real signing team must be explicitly selected for an archive")
    if args.archive and (args.development or args.destination!="device" or args.export_compliance is None):raise ValueError("Archives require release resources, a device destination and an explicit reviewed export classification")
    receipt=json.loads((ROOT/'apps/ios/assembly-receipt.json').read_text(encoding='utf-8'))
    if not args.development and (receipt.get('development_only') is not False or not receipt.get('core_ids')):raise ValueError("Release builds require approved bundled cores and a release assembly")
    framework=ROOT/"apps/ios/Libraries/OmniPlayMobile.xcframework"
    if framework.exists():raise ValueError("Choose a fresh XCFramework output before this explicit build")
    archive=ROOT/"out/ios/OmniPlay.xcarchive"
    if args.archive and archive.exists():raise ValueError("Archive output already exists; preserve it and choose a fresh workspace")
    archive_info=ROOT/'out/ios/archive-Info.plist'
    if args.archive:
        if archive_info.exists():raise ValueError("Preserve the prior archive Info.plist and choose a fresh workspace")
        info=plistlib.loads((ROOT/'apps/ios/OmniPlay/Info.plist').read_bytes())
        info['ITSAppUsesNonExemptEncryption']=args.export_compliance=='non-exempt'
        archive_info.parent.mkdir(parents=True,exist_ok=True);archive_info.write_bytes(plistlib.dumps(info))
    for target in ["aarch64-apple-ios","aarch64-apple-ios-sim"]:run([args.cargo,"build","--locked","--release","--target",target,"-p","omniplay-mobile"])
    framework.parent.mkdir(parents=True,exist_ok=True)
    command=[args.xcodebuild,"-create-xcframework"]
    for target in ["aarch64-apple-ios","aarch64-apple-ios-sim"]:command += ["-library",ROOT/"target"/target/"release/libomniplay_mobile.a","-headers",ROOT/"platform/mobile/include"]
    run(command+["-output",framework])
    destination="generic/platform=iOS Simulator" if args.destination=="simulator" else "generic/platform=iOS"
    command=[args.xcodebuild,"-project",ROOT/"apps/ios/OmniPlay.xcodeproj","-scheme","OmniPlay","-configuration",args.configuration,"-destination",destination]
    if args.archive:command += ["-archivePath",archive,"archive",f"DEVELOPMENT_TEAM={args.team}",f"INFOPLIST_FILE={archive_info}"]
    else:command += ["build","CODE_SIGNING_ALLOWED=NO"]
    run(command)
    print("iOS build completed. Device/compatibility/security verification must be recorded separately.")

if __name__=="__main__":main()
