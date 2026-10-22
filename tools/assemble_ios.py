"""Author the iOS app resources and an Xcode project; never build or run tests.

No executable emulator is fetched at runtime. The app bundles approved WASM assets
from explicit source builds. --without-cores is development-only source assembly.
"""
import argparse
import hashlib
import json
import mimetypes
from pathlib import Path
import shutil
from game_hub_assets import collect,prepare
from streaming_config import configuration

ROOT=Path(__file__).resolve().parents[1]
APP=ROOT/"apps/ios"

def identity(name):return hashlib.sha256(("omniplay.ios."+name).encode()).hexdigest()[:24].upper()
def quote(value):return json.dumps(str(value),ensure_ascii=True)

def project():
    sources=sorted((APP/"OmniPlay").glob("*.swift"))
    if not sources:raise ValueError("iOS source files are missing")
    objects=[]
    def item(name,value):objects.append(f"{identity(name)} = {{{value}}};");return identity(name)
    files=[];builds=[]
    for path in sources:
        ref=item("file."+path.name,f"isa = PBXFileReference; lastKnownFileType = sourcecode.swift; path = {quote(path.name)}; sourceTree = \"<group>\";")
        files.append(ref);builds.append(item("build."+path.name,f"isa = PBXBuildFile; fileRef = {ref};"))
    privacy=item("file.privacy","isa = PBXFileReference; lastKnownFileType = text.xml; path = PrivacyInfo.xcprivacy; sourceTree = \"<group>\";")
    files.append(privacy);privacy_build=item("build.privacy",f"isa = PBXBuildFile; fileRef = {privacy};")
    assets=item("file.assets","isa = PBXFileReference; lastKnownFileType = folder; path = GameHub; sourceTree = \"<group>\";")
    asset_build=item("build.assets",f"isa = PBXBuildFile; fileRef = {assets};")
    framework=item("file.framework","isa = PBXFileReference; lastKnownFileType = wrapper.xcframework; path = Libraries/OmniPlayMobile.xcframework; sourceTree = \"<group>\";")
    framework_build=item("build.framework",f"isa = PBXBuildFile; fileRef = {framework};")
    product=item("file.product","isa = PBXFileReference; explicitFileType = wrapper.application; includeInIndex = 0; path = OmniPlay.app; sourceTree = BUILT_PRODUCTS_DIR;")
    group=item("group.sources",f"isa = PBXGroup; children = ({','.join(files)},); path = OmniPlay; sourceTree = \"<group>\";")
    products=item("group.products",f"isa = PBXGroup; children = ({product},); name = Products; sourceTree = \"<group>\";")
    main=item("group.root",f"isa = PBXGroup; children = ({group},{assets},{framework},{products},); sourceTree = \"<group>\";")
    source_phase=item("phase.sources",f"isa = PBXSourcesBuildPhase; buildActionMask = 2147483647; files = ({','.join(builds)},); runOnlyForDeploymentPostprocessing = 0;")
    resource_phase=item("phase.resources",f"isa = PBXResourcesBuildPhase; buildActionMask = 2147483647; files = ({privacy_build},{asset_build},); runOnlyForDeploymentPostprocessing = 0;")
    framework_phase=item("phase.frameworks",f"isa = PBXFrameworksBuildPhase; buildActionMask = 2147483647; files = ({framework_build},); runOnlyForDeploymentPostprocessing = 0;")
    target_configs=[];project_configs=[]
    for config in ("Debug","Release"):
        common='CLANG_ENABLE_MODULES = YES; SDKROOT = iphoneos; IPHONEOS_DEPLOYMENT_TARGET = 17.0; SWIFT_VERSION = 5.0; SWIFT_STRICT_CONCURRENCY = minimal;'
        project_configs.append(item("config.project."+config,f"isa = XCBuildConfiguration; name = {config}; buildSettings = {{{common}}};"))
        flags='"-lz","-lc++","-framework","Security","-framework","SystemConfiguration","-framework","Network","-framework","WebKit","-framework","UIKit","-framework","AVFAudio","-framework","BackgroundTasks"'
        settings=common+f''' PRODUCT_NAME = OmniPlay; PRODUCT_BUNDLE_IDENTIFIER = org.omniplay.browser;
          INFOPLIST_FILE = OmniPlay/Info.plist; GENERATE_INFOPLIST_FILE = NO;
          SWIFT_OBJC_BRIDGING_HEADER = "OmniPlay/OmniPlay-Bridging-Header.h";
          HEADER_SEARCH_PATHS = ("$(inherited)","$(SRCROOT)/../../platform/mobile/include",);
          OTHER_LDFLAGS = ("$(inherited)",{flags},); CODE_SIGN_STYLE = Automatic;
          TARGETED_DEVICE_FAMILY = "1,2"; SUPPORTED_PLATFORMS = "iphoneos iphonesimulator";
          ENABLE_BITCODE = NO; SWIFT_OPTIMIZATION_LEVEL = "{'-Onone' if config=='Debug' else '-O'}";
          SWIFT_ACTIVE_COMPILATION_CONDITIONS = "{'DEBUG' if config=='Debug' else ''}";
          LD_RUNPATH_SEARCH_PATHS = ("$(inherited)","@executable_path/Frameworks",);
        '''
        target_configs.append(item("config.target."+config,f"isa = XCBuildConfiguration; name = {config}; buildSettings = {{{settings}}};"))
    tc=item("list.target",f"isa = XCConfigurationList; buildConfigurations = ({','.join(target_configs)},); defaultConfigurationIsVisible = 0; defaultConfigurationName = Release;")
    pc=item("list.project",f"isa = XCConfigurationList; buildConfigurations = ({','.join(project_configs)},); defaultConfigurationIsVisible = 0; defaultConfigurationName = Release;")
    target=item("target",f"isa = PBXNativeTarget; buildConfigurationList = {tc}; buildPhases = ({source_phase},{framework_phase},{resource_phase},); buildRules = (); dependencies = (); name = OmniPlay; productName = OmniPlay; productReference = {product}; productType = \"com.apple.product-type.application\";")
    root=item("project",f"isa = PBXProject; attributes = {{LastUpgradeCheck = 1600;}}; buildConfigurationList = {pc}; compatibilityVersion = \"Xcode 14.0\"; developmentRegion = vi; knownRegions = (vi,en,Base,); mainGroup = {main}; productRefGroup = {products}; projectDirPath = \"\"; projectRoot = \"\"; targets = ({target},);")
    directory=APP/"OmniPlay.xcodeproj";directory.mkdir(parents=True,exist_ok=True)
    (directory/"project.pbxproj").write_text("// !$*UTF8*$!\n{archiveVersion = 1; classes = {}; objectVersion = 56; objects = {\n"+"\n".join(objects)+f"\n}}; rootObject = {root};}}\n",encoding="utf-8")
    schemes=directory/"xcshareddata/xcschemes";schemes.mkdir(parents=True,exist_ok=True)
    reference=f'<BuildableReference BuildableIdentifier="primary" BlueprintIdentifier="{target}" BuildableName="OmniPlay.app" BlueprintName="OmniPlay" ReferencedContainer="container:OmniPlay.xcodeproj"/>'
    scheme=f'''<?xml version="1.0" encoding="UTF-8"?>
<Scheme LastUpgradeVersion="1600" version="1.3"><BuildAction parallelizeBuildables="YES" buildImplicitDependencies="YES"><BuildActionEntries><BuildActionEntry buildForTesting="NO" buildForRunning="YES" buildForProfiling="YES" buildForArchiving="YES" buildForAnalyzing="YES">{reference}</BuildActionEntry></BuildActionEntries></BuildAction><LaunchAction buildConfiguration="Debug" selectedDebuggerIdentifier="Xcode.DebuggerFoundation.Debugger.LLDB" selectedLauncherIdentifier="Xcode.IDEFoundation.Launcher.LLDB" launchStyle="0" useCustomWorkingDirectory="NO" ignoresPersistentStateOnLaunch="NO" debugDocumentVersioning="YES" allowLocationSimulation="YES"><BuildableProductRunnable runnableDebuggingMode="0">{reference}</BuildableProductRunnable></LaunchAction><ProfileAction buildConfiguration="Release" shouldUseLaunchSchemeArgsEnv="YES" savedToolIdentifier="" useCustomWorkingDirectory="NO" debugDocumentVersioning="YES"><BuildableProductRunnable runnableDebuggingMode="0">{reference}</BuildableProductRunnable></ProfileAction><AnalyzeAction buildConfiguration="Debug"/><ArchiveAction buildConfiguration="Release" revealArchiveInOrganizer="YES"/></Scheme>
'''
    (schemes/"OmniPlay.xcscheme").write_text(scheme,encoding="utf-8")

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument("--without-cores",action="store_true");args=parser.parse_args()
    assets=APP/"GameHub";sources,cores=collect(args.without_cores)
    prepare(assets,set(sources)|{"cores/catalog.json","asset-index.json"})
    for relative,path in sources.items():
        target=assets/relative;target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(path,target)
    catalog=assets/"cores/catalog.json";catalog.parent.mkdir(parents=True,exist_ok=True);catalog.write_text(json.dumps({"schema_version":1,"cores":cores,"development_only":args.without_cores},indent=2)+"\n",encoding="utf-8")
    routes=set(sources)|{"cores/catalog.json"}
    mime={".js":"application/javascript; charset=utf-8",".wasm":"application/wasm",".css":"text/css; charset=utf-8",".html":"text/html; charset=utf-8",".json":"application/json; charset=utf-8",".txt":"text/plain; charset=utf-8"}
    streaming=configuration()
    inventory={"schema_version":1,"streaming_origins":streaming['relay_origins'] if streaming['enabled'] else [],"assets":[{"path":relative,"bytes":(assets/relative).stat().st_size,"mime":mime.get(Path(relative).suffix,"application/octet-stream")} for relative in sorted(routes)]}
    (assets/"asset-index.json").write_text(json.dumps(inventory,indent=2)+"\n",encoding="utf-8")
    project()
    (APP/"assembly-receipt.json").write_text(json.dumps({"schema_version":1,"development_only":args.without_cores,"core_ids":[c["id"] for c in cores],"resource_files":len(routes),"rust_build":"not_started","xcode_build":"not_started","verification":"not_run"},indent=2)+"\n",encoding="utf-8")
    print("iOS resources and Xcode project authored. No build, simulator or test was started.")

if __name__=="__main__":main()
