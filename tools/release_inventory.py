"""Check shipped-file attribution and accidental private content, not legal approval.

The corresponding-source declaration is an operator-reviewed input. This checker
cannot establish license compatibility, authenticity of approvals, vulnerability
absence, code signatures or complete build dependency provenance.
"""
import hashlib
import json
from pathlib import Path, PurePosixPath
import re

MAX_DECLARATION=4*1024*1024
MAX_SCAN=8*1024*1024
SECRET_PATTERNS=(
    re.compile(rb'-----BEGIN (?:RSA |EC |OPENSSH |DSA |ENCRYPTED )?PRIVATE KEY-----'),
    re.compile(rb'\bgh[pousr]_[A-Za-z0-9]{36,255}\b'),
    re.compile(rb'\bgithub_pat_[A-Za-z0-9_]{60,255}\b'),
    re.compile(rb'\bsk-proj-[A-Za-z0-9_-]{40,255}\b'),
)
PRIVATE_NAMES={'.git','.svn','.env','user-data','node_modules','secrets',
               'cookies','login data','credentials.json','id_rsa','id_ed25519'}

def file_digest(path):
    digest=hashlib.sha256()
    with path.open('rb') as source:
        for block in iter(lambda:source.read(65536),b''):digest.update(block)
    return digest.hexdigest()

def relative(value):
    if not isinstance(value,str) or not value or len(value)>240 or '\\' in value or ':' in value:return False
    path=PurePosixPath(value)
    return bool(path.parts) and not path.is_absolute() and path.as_posix()==value and all(part not in {'.','..'} for part in path.parts)

def checked_file(base,name,maximum=None):
    if not relative(name):raise ValueError('unsafe_relative_path')
    path=base/name
    for current in (path,*path.parents):
        if current==base:break
        if current.is_symlink() or getattr(current,'is_junction',lambda:False)():raise ValueError('linked_input')
    if not path.is_file() or not path.resolve().is_relative_to(base.resolve()):raise ValueError('missing_regular_input')
    if maximum is not None and path.stat().st_size>maximum:raise ValueError('input_size_limit')
    return path

def check_release_tree(base,entries):
    """Return bounded findings without ever including matched secret bytes."""
    base=Path(base)
    issues=[]
    def issue(code,path=None):
        if len(issues)<1024:issues.append({'code':code,**({'path':path} if path else {})})
    inventory={};folded=set()
    for entry in entries:
        name=entry.get('path')
        if not relative(name):issue('unsafe_package_path');continue
        if name.casefold() in folded:issue('duplicate_package_path',name);continue
        folded.add(name.casefold())
        inventory[name]=entry
        parts=PurePosixPath(name).parts
        if any(part.casefold() in PRIVATE_NAMES or part.casefold().startswith('.env.') for part in parts):issue('private_content_path',name)
        try:
            path=checked_file(base,name)
            # A 1-KiB overlap catches credential markers spanning chunk boundaries.
            # Scan all bytes; the per-file read is bounded, including binary data.
            with path.open('rb') as source:
                tail=b''
                while block:=source.read(MAX_SCAN):
                    window=tail+block
                    if any(pattern.search(window) for pattern in SECRET_PATTERNS):issue('credential_marker',name);break
                    tail=window[-1024:]
        except (ValueError,OSError):issue('unreadable_package_input',name)
    required={'LICENSE','NOTICE','licenses/third_party_notices.txt','licenses/source_offer.json'}
    for name in sorted(required-inventory.keys()):issue('missing_license_file',name)
    if 'licenses/source_offer.json' not in inventory:return issues
    try:
        path=checked_file(base,'licenses/source_offer.json',MAX_DECLARATION)
        def unique(pairs):
            value={}
            for key,item in pairs:
                if key in value:raise ValueError('duplicate_declaration_key')
                value[key]=item
            return value
        declaration=json.loads(path.read_text(encoding='utf-8'),object_pairs_hook=unique)
        components=declaration.get('components')
        if declaration.get('schema_version')!=1 or not isinstance(components,list) or not 1<=len(components)<=2048:raise ValueError('invalid_declaration')
    except (ValueError,TypeError,AttributeError,OSError,UnicodeError):issue('invalid_source_declaration');return issues
    covered=set();identities=set();license_files=set(required)
    for component in components:
        if not isinstance(component,dict):issue('invalid_component');continue
        identity=tuple(component.get(key) for key in ('name','revision'))
        if not all(isinstance(component.get(key),str) and 0<len(component[key])<=1024 for key in ('name','revision','license')):issue('invalid_component_identity');continue
        if identity in identities:issue('duplicate_component');continue
        identities.add(identity)
        if component.get('distribution_approved') is not True:issue('component_review_missing')
        paths=component.get('artifact_paths');notices=component.get('notice_paths')
        if not isinstance(paths,list) or not paths or len(paths)>100000:issue('component_payload_inventory_missing');continue
        for name in paths:
            if not isinstance(name,str) or name not in inventory:issue('component_payload_missing');continue
            if name in covered:issue('ambiguous_component_attribution',name)
            covered.add(name)
        if not isinstance(notices,list) or not 1<=len(notices)<=128:issue('component_notice_inventory_missing')
        else:
            for name in notices:
                try:
                    if not isinstance(name,str) or name not in inventory or not checked_file(base,name,MAX_DECLARATION).stat().st_size:raise ValueError('notice_missing')
                    license_files.add(name)
                except (ValueError,OSError):issue('component_notice_missing')
        required_source=component.get('corresponding_source_required')
        if type(required_source) is not bool:issue('source_obligation_not_declared')
        if required_source is True:
            name=component.get('source_archive_path');expected=component.get('source_archive_sha256')
            try:
                if not isinstance(expected,str) or not re.fullmatch('[0-9a-f]{64}',expected) or not isinstance(name,str) or name not in inventory:raise ValueError('source_missing')
                source=checked_file(base,name)
                if source.stat().st_size==0 or file_digest(source)!=expected:raise ValueError('source_digest_mismatch')
            except (ValueError,OSError):issue('corresponding_source_missing_or_mismatched')
    # Declarations and license texts identify components but are not payload.
    for name in sorted(inventory.keys()-covered):
        if name not in license_files:issue('unattributed_payload',name)
    return issues
