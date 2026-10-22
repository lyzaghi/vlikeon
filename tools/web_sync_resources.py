"""Select an explicitly approved crypto candidate for the web source bundle."""
import hashlib
import json
from pathlib import Path
import re
from build_web_sync import source_inventory,encoded
from web_sync_artifact import admitted_package

ROOT=Path(__file__).resolve().parents[1]
FIELDS={'schema_version','enabled','artifact','artifact_sha256','source_inventory_sha256','distribution_approved','verification_approved'}
DISABLED={'schema_version':1,'enabled':False,'protocol':1,'wasm_path':None,'wasm_bytes':0,'wasm_sha256':None,'distribution_approved':False}

def resources(regular):
    policy=regular(ROOT/'security/web-sync-channel.json',ROOT)
    if policy.stat().st_size>8192:raise ValueError('Browser crypto product policy exceeds its bound')
    value=json.loads(policy.read_text(encoding='utf-8'))
    if set(value)!=FIELDS or type(value['schema_version'])is not int or value['schema_version']!=1 or any(type(value[name])is not bool for name in ['enabled','distribution_approved','verification_approved']):raise ValueError('Browser crypto product policy is malformed')
    if not value['enabled']:
        if any(value[name]is not None for name in ['artifact','artifact_sha256','source_inventory_sha256']) or value['distribution_approved'] or value['verification_approved']:raise ValueError('Disabled browser crypto cannot reference a binary or approval')
        return {'web/sync-crypto.json':encoded(DISABLED)}
    if not value['distribution_approved'] or not value['verification_approved']:raise ValueError('Browser crypto requires explicit distribution and final verification approval')
    relative=value['artifact']
    if not isinstance(relative,str) or len(relative)>256 or not re.fullmatch(r'out/web-sync/[A-Za-z0-9_.-]+/package',relative) or any(part in {'.','..'} for part in relative.split('/')):raise ValueError('Browser crypto needs one confined prepared package')
    directory=ROOT/relative
    if not directory.resolve().is_relative_to(ROOT.resolve()):raise ValueError('Browser crypto package escaped the workspace')
    for path in [directory,*directory.parents]:
        if path==ROOT:break
        if path.is_symlink() or getattr(path,'is_junction',lambda:False)():raise ValueError('Browser crypto package ancestry cannot be linked')
    artifact,receipt=admitted_package(directory,regular);project=directory.parent;preparation=regular(project/'preparation-receipt.json',project)
    if preparation.stat().st_size>8*1024*1024:raise ValueError('Browser crypto preparation receipt exceeds its bound')
    before=json.loads(preparation.read_text(encoding='utf-8'));inventory=source_inventory(project/'source');identity=hashlib.sha256(encoded(inventory)).hexdigest()
    if before.get('source_only')is not True or before.get('source_inventory')!=inventory or receipt.get('preparation_receipt_sha256')!=hashlib.sha256(preparation.read_bytes()).hexdigest() or receipt.get('prepared_source_inventory_sha256')!=identity or receipt.get('cargo_lock_sha256')!=hashlib.sha256(regular(project/'source/Cargo.lock',project).read_bytes()).hexdigest():raise ValueError('Browser crypto corresponding source or preparation identity changed')
    if value['artifact_sha256']!=receipt['wasm_sha256'] or value['source_inventory_sha256']!=identity:raise ValueError('Browser crypto approval refers to another artifact/source candidate')
    config={'schema_version':1,'enabled':True,'protocol':1,'wasm_path':'sync-crypto.wasm','wasm_bytes':receipt['wasm_bytes'],'wasm_sha256':receipt['wasm_sha256'],'distribution_approved':True}
    return {'web/sync-crypto.json':encoded(config),'web/sync-crypto.wasm':artifact}
