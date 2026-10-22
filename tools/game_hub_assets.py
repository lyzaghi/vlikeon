"""Shared, explicit source inventory for the desktop, Android and iOS bundles."""
import hashlib
import json
import os
from pathlib import Path
import re
from streaming_config import configuration
from web_firmware_policy import admit as admit_firmware_policy
from web_disc_policy import admit as admit_disc_policy
from web_saved_files_policy import admit as admit_saved_files_policy
from web_core_options_policy import admit as admit_core_options_policy
from web_portable_files_policy import admit as admit_portable_files_policy
from web_memory_policy import admit as admit_memory_policy
from web_hardware_policy import admit as admit_hardware_policy
from web_thread_policy import admit as admit_thread_policy
from web_system_assets_policy import admit as admit_system_assets_policy

ROOT=Path(__file__).resolve().parents[1]
BASE=ROOT/'apps/game-hub'
EXTENSIONS={'.html','.css','.js','.json','.wasm','.txt'}
CORE_NUMERIC=r'(?:0|[1-9][0-9]*)'
CORE_PRERELEASE=r'(?:0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*)'
CORE_VERSION=re.compile(CORE_NUMERIC+r'\.'+CORE_NUMERIC+r'\.'+CORE_NUMERIC+r'(?:-'+CORE_PRERELEASE+r'(?:\.'+CORE_PRERELEASE+r')*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?')

def safe_relative(value):
    if not isinstance(value,str) or not value or len(value)>512 or not all(re.fullmatch(r'[A-Za-z0-9_.-]+',segment) and segment not in {'.','..'} for segment in value.split('/')):
        raise ValueError('A bundled resource path is not a safe relative path')
    return value

def regular(path,base):
    if path.is_symlink() or getattr(path,'is_junction',lambda:False)() or not path.is_file() or not path.resolve().is_relative_to(base.resolve()):
        raise ValueError('Bundled resources must be regular files inside their source root')
    return path

def collect(development,extension=False):
    configuration()
    sources={};cores=[];identities=set()
    for path in BASE.rglob('*'):
        relative=path.relative_to(BASE).as_posix()
        if path.is_file() and path.suffix in EXTENSIONS and not relative.startswith('cores/'):
            if not extension and relative in {'background.js','browser-service.js','component-context.js','component-ownership.js','manifest.json'}:continue
            sources[safe_relative(relative)]=regular(path,BASE)
    for path in (ROOT/'engines/web').glob('*.js'):
        relative='runtime/'+path.name
        if relative in sources:raise ValueError('Two source trees claim the same bundled runtime resource')
        sources[safe_relative(relative)]=regular(path,ROOT/'engines/web')
    for path in (ROOT/'engines/streaming').glob('*.js'):
        relative='streaming/'+path.name
        if relative in sources:raise ValueError('Two source trees claim the same streaming resource')
        sources[safe_relative(relative)]=regular(path,ROOT/'engines/streaming')
    sources['streaming/config.json']=regular(ROOT/'security/streaming-channel.json',ROOT/'security')
    for manifest in sorted((BASE/'cores').glob('*/manifest.json')):
        regular(manifest,BASE);core=json.loads(manifest.read_text(encoding='utf-8'))
        admit_firmware_policy(core)
        admit_disc_policy(core)
        admit_saved_files_policy(core)
        admit_core_options_policy(core)
        admit_portable_files_policy(core)
        admit_memory_policy(core)
        admit_hardware_policy(core)
        threads=admit_thread_policy(core)
        system_assets=admit_system_assets_policy(core)
        if 'libretro_result_protocol' in core and (type(core['libretro_result_protocol']) is not int or core['libretro_result_protocol'] not in {1,2} or (core.get('wasm_threads') is not False and threads is None)):raise ValueError('Core result bridges require an explicitly admitted execution policy')
        if core.get('schema_version')!=1 or not isinstance(core.get('id'),str) or not core['id'] or core['id'] in identities:raise ValueError('Core manifest identity is missing or duplicated')
        if not isinstance(core.get('core_version'),str) or len(core['core_version'])>256 or not CORE_VERSION.fullmatch(core['core_version']):raise ValueError('Core save metadata requires a supported semantic version, separate from its upstream library string')
        if core.get('security_revoked') is True:raise ValueError('A revoked core cannot be bundled')
        if not development and core.get('distribution_approved') is not True:raise ValueError('Release assembly requires explicitly approved core artifacts')
        directory=manifest.parent.relative_to(BASE).as_posix()+'/'
        for field,suffix in [('module','.js'),('wasm','.wasm')]:
            relative=safe_relative(core.get(field))
            if not relative.startswith(directory) or '/' in relative[len(directory):] or not relative.endswith(suffix):raise ValueError('Core artifacts must live beside their own manifest')
            sources[relative]=regular(BASE/relative,BASE)
        wasm=sources[core['wasm']]
        if wasm.stat().st_size>512*1024*1024 or wasm.stat().st_size!=core.get('wasm_bytes'):raise ValueError('Bundled core size differs from its build manifest')
        digest=hashlib.sha256()
        with wasm.open('rb') as stream:
            for block in iter(lambda:stream.read(256*1024),b''):digest.update(block)
        if digest.hexdigest()!=core.get('wasm_sha256'):raise ValueError('Bundled core integrity differs from its build manifest')
        if threads:
            relative=safe_relative(threads['worker_module'])
            if not relative.startswith(directory) or '/' in relative[len(directory):]:raise ValueError('Pthread code must live beside its own manifest')
            worker=regular(BASE/relative,BASE)
            if worker.stat().st_size!=core['pthread_worker_bytes'] or hashlib.sha256(worker.read_bytes()).hexdigest()!=core['pthread_worker_sha256']:raise ValueError('The pthread resource differs from its build inventory')
            sources[relative]=worker
        for item in system_assets:
            relative=safe_relative(item['resource']);asset=regular(BASE/relative,BASE)
            if asset.stat().st_size!=item['bytes'] or hashlib.sha256(asset.read_bytes()).hexdigest()!=item['sha256']:raise ValueError('Bundled system data differs from its build inventory')
            if relative in sources:raise ValueError('A system asset collides with another resource')
            sources[relative]=asset
        notices=sorted(manifest.parent.glob('LICENSE*.txt'))
        if not notices:raise ValueError('Every bundled core needs its accompanying license notices')
        for path in [manifest]+notices:sources[safe_relative(path.relative_to(BASE).as_posix())]=regular(path,BASE)
        identities.add(core['id']);cores.append(core)
    if not cores and not development:raise ValueError('Build and approve bundled cores before release assembly')
    return sources,cores

def prepare(destination,expected):
    """Remove stale generated files, including retired core binaries, before copy.

    The resolved target and every existing entry are confined to this workspace.
    No recursive directory deletion, junction traversal or source deletion occurs.
    """
    destination=Path(destination).absolute()
    if destination.resolve()!=destination or not destination.resolve().is_relative_to(ROOT.resolve()) or destination.resolve()==ROOT.resolve():raise ValueError('Generated assets must stay in their own unlinked workspace directory')
    paths=set(map(safe_relative,expected));existing=[]
    # Check directories before os.walk descends, including Windows junctions.
    for current,folders,files in os.walk(destination,topdown=True,followlinks=False):
        for name in folders+files:
            path=Path(current)/name
            if path.is_symlink() or getattr(path,'is_junction',lambda:False)() or not path.resolve().is_relative_to(destination):raise ValueError('Generated assets contain a link or path outside their owned directory')
            if not path.is_file() and not path.is_dir():raise ValueError('Generated assets contain a non-regular entry')
            existing.append(path)
    for path in existing:
        if path.is_file() and path.relative_to(destination).as_posix() not in paths:path.unlink()
    destination.mkdir(parents=True,exist_ok=True)
