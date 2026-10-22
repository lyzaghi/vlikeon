"""Prepare/build the pinned native PS1 software core; never run cores or tests.

--prepare-only materializes exact committed source and adaptations without Make,
compiler or linker execution. --build-prepared uses that retained source later.
"""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
from build_beetle_psx_web import materialize, file_hash, source_inventory
from psx_source_adaptations import include_all_card_data, native_card_io, native_memory_cards

ROOT=Path(__file__).resolve().parents[1]
PIN="df2b53b306ba52d2116f8e8cc93b9ee33d239be7"
VERSION="0.9.44.1+omniplay.native3"
ADAPTER=ROOT/"engines/native/adapters/beetle-save-io.c.inc"
MEMORY_INPUTS={name:ROOT/"engines/native/adapters"/name for name in ["beetle-memory-files.h","beetle-memory-files.c","beetle-memory-frontio.c.inc"]}
MEMORY_INPUTS["writable_files_v1.h"]=ROOT/"engines/native/src/writable_files_v1.h"


def prepare(output,platform,git):
    source,archive=materialize(output,git)
    include_all_card_data(source)
    for name,path in MEMORY_INPUTS.items():shutil.copy2(path,source/name)
    native_memory_cards(source,ROOT/"engines/native/adapters")
    native_card_io(source,ADAPTER)
    options=source/"libretro_options.h";version=options.read_text(encoding="utf-8")
    anchor='#define MEDNAFEN_CORE_SETVERSION "0.9.44.1"'
    if version.count(anchor)!=1:raise ValueError("The pinned PS1 version anchor changed")
    options.write_text(version.replace(anchor,'#define MEDNAFEN_CORE_SETVERSION "'+VERSION+'"'),encoding="utf-8",newline="\n")
    makefile=source/"Makefile";raw=makefile.read_text(encoding="utf-8")
    if platform=="win":raw+="\n# OmniPlay: fixed GNU runtime dependencies, no PATH-resolved helper DLL.\nLDFLAGS += -static -static-libgcc -static-libstdc++\n"
    makefile.write_text(raw,encoding="utf-8",newline="\n")
    adaptations={"schema_version":1,"revision":PIN,"version":VERSION,"platform":platform,
      "changes":["software_renderer","no_lightrec_or_dynamic_code","no_direct_cdrom_device","static_gnu_runtime_on_windows","snapshots_always_include_memory_card_bytes","pause_joins_card_writer_and_lazy_frame_restart","checked_all_card_barrier","combined_memory_file_progress_and_disc_snapshots","bounded_frontend_memory_card_table","no_file_card_os_write_fallback","read_failure_retains_card_bytes","card_swap_joins_older_queued_writes"],
      "upstream_openbios":"MIT; original PCSX-Redux implementation; retained separately from manufacturer firmware"}
    (source/"OMNIPLAY_ADAPTATIONS.json").write_text(json.dumps(adaptations,indent=2)+"\n",encoding="utf-8")
    shutil.copy2(ADAPTER,output/ADAPTER.name)
    shutil.copy2(ROOT/"engines/policies/beetle-options.json",output/"core-options-policy.json")
    for name in ["build_beetle_psx.py","build_beetle_psx_web.py","psx_source_adaptations.py","web_core_options_policy.py"]:
        shutil.copy2(ROOT/"tools"/name,output/name)
    record={**adaptations,"source_only":True,"verification":"not_run","distribution_approved":False,
      "pause_barrier":True,"file_io_protocol":1,"filesystem_save_memory":True,"writable_files_protocol":1,
      "upstream_source_sha256":file_hash(archive),"adapter_sha256":file_hash(ADAPTER),
      "memory_adapter_sha256":{name:file_hash(path) for name,path in MEMORY_INPUTS.items()},
      "core_options_policy_sha256":file_hash(output/"core-options-policy.json"),
      "prepared_source_sha256":source_inventory(source),
      "remaining":["exact toolchain/build provenance","compiled dependency licenses","core I/O and file lifecycle evidence","region firmware and multi-disc/card compatibility","actual containment/quota/device performance and release evidence"]}
    (output/"corresponding-source.json").write_text(json.dumps(record,indent=2)+"\n",encoding="utf-8")
    return source,record


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prepare-only",action="store_true")
    parser.add_argument("--build-prepared",action="store_true")
    parser.add_argument("--project",type=Path)
    for name in ["make","cc","cxx","ar"]:parser.add_argument("--"+name,type=Path)
    parser.add_argument("--platform",choices=["win","unix","osx"],default="win")
    parser.add_argument("--parallel",type=int,default=8)
    parser.add_argument("--git",default="git")
    args=parser.parse_args()
    if not 1<=args.parallel<=32:raise ValueError("Build parallelism exceeds its bound")
    if args.prepare_only and args.build_prepared:raise ValueError("Choose preparation or a later prepared-source build")
    if not args.prepare_only and any(getattr(args,name)is None for name in ["make","cc","cxx","ar"]):raise ValueError("Explicit compiler/archive/Make paths are required in build mode")
    output=args.project.absolute()if args.project else ROOT/"out/native-cores/beetle-psx"
    if output.resolve()!=output.absolute()or not output.resolve().is_relative_to((ROOT/"out").resolve()):raise ValueError("PS1 source/build project must stay under the ordinary workspace output tree")
    if args.build_prepared:
        if output.resolve()!=output.absolute() or not output.resolve().is_relative_to(ROOT):raise ValueError("Prepared source must stay in its owned workspace tree")
        source=output/"source";record=json.loads((output/"corresponding-source.json").read_text(encoding="utf-8"))
        if record.get("revision")!=PIN or record.get("version")!=VERSION or record.get("platform")!=args.platform or record.get("source_only")is not True or record.get("writable_files_protocol")!=1 or source_inventory(source)!=record.get("prepared_source_sha256"):raise ValueError("Prepared PS1 source differs from its retained receipt")
    else:source,record=prepare(output,args.platform,args.git)
    if args.prepare_only:
        print("Authored pinned native PS1 source and checked card-barrier adaptation. No build, compiler, core or test was started.")
        return
    if file_hash(output/"core-options-policy.json")!=record["core_options_policy_sha256"]:raise ValueError("Prepared core options changed")
    command=[args.make,"-f","Makefile",f"platform={args.platform}",f"CC={args.cc}",f"CXX={args.cxx}",f"AR={args.ar}",
      "HAVE_HW=0","HAVE_OPENGL=0","HAVE_VULKAN=0","HAVE_LIGHTREC=0","THREADED_RECOMPILER=0","HAVE_CDROM=0","HAVE_CHD=1",
      'GIT_VERSION=" unknown"',"EXTRA_FLAGS=-DHAVE_NO_LANGEXTRA",f"-j{args.parallel}"]
    subprocess.run([str(value) for value in command],cwd=source,check=True)
    suffix={"win":".dll","unix":".so","osx":".dylib"}[args.platform];library=source/("mednafen_psx_libretro"+suffix)
    if not library.is_file():raise ValueError("The PS1 build did not produce its expected core library")
    receipt={"schema_version":1,"core":"beetle-psx","revision":PIN,"version":VERSION,"library":str(library.resolve()),"corresponding_source":str(source.resolve()),
      "library_sha256":file_hash(library),"pause_barrier":True,"file_io_protocol":1,"filesystem_save_memory":True,"writable_files_protocol":1,
      "adapter_sha256":record["adapter_sha256"],"core_options_policy_sha256":record["core_options_policy_sha256"],
      "memory_adapter_sha256":record["memory_adapter_sha256"],
      "verification":"not_run","distribution_review_required":["compiled_third_party_notices","complete_corresponding_source","actual_core_barrier_and_device_evidence"],
      "firmware":[{"role":role,"filename":name,"allowed_bytes":[524288],"required_group":"ps1_bios"} for role,name in [("ps1_bios_jp","scph5500.bin"),("ps1_bios","scph5501.bin"),("ps1_bios_eu","scph5502.bin")]]}
    (output/"build-receipt.json").write_text(json.dumps(receipt,indent=2)+"\n",encoding="utf-8")
    print("Built an unapproved native PS1 candidate. No game, application or test was run.")


if __name__=="__main__":main()
