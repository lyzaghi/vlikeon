"""Prepare pinned official portable CMake locally, without executing it.

No installer, registry or global PATH changes. Published checksums establish
download integrity against the distribution, not independent signing.
"""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import urllib.request
import zipfile

VERSION='3.31.6'
ORIGIN='https://cmake.org/files/v3.31/'
STEM=f'cmake-{VERSION}-windows-x86_64'
MAX_ARCHIVE=128*1024*1024
MAX_TOTAL=512*1024*1024

def fetch(url,destination,limit):
    size=0;digest=hashlib.sha256()
    with urllib.request.urlopen(url,timeout=60) as response,destination.open('xb') as output:
        while True:
            block=response.read(256*1024)
            if not block:break
            size+=len(block)
            if size>limit:raise ValueError('CMake download exceeds budget')
            digest.update(block);output.write(block)
    return size,digest.hexdigest()

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--project',required=True);args=parser.parse_args()
    workspace=Path(__file__).resolve().parents[1];project=Path(args.project).absolute()
    if not project.is_relative_to(workspace) or project==workspace:raise ValueError('Choose a fresh project-local directory')
    for ancestor in [project,*project.parents]:
        if ancestor.is_symlink() or getattr(ancestor,'is_junction',lambda:False)():raise ValueError('Linked preparation ancestry')
        if ancestor==workspace:break
    if project.exists():raise ValueError('Preserve previous preparation; use a fresh path')
    project.mkdir(parents=True);checksums=project/'SHA-256.txt';archive=project/(STEM+'.zip')
    fetch(ORIGIN+f'cmake-{VERSION}-SHA-256.txt',checksums,64*1024)
    expected=None
    for line in checksums.read_text('ascii').splitlines():
        fields=line.split()
        if len(fields)==2 and fields[1].lstrip('*')==archive.name:
            if expected is not None or not re.fullmatch('[0-9a-f]{64}',fields[0]):raise ValueError('Invalid checksum entry')
            expected=fields[0]
    if expected is None:raise ValueError('Portable archive absent from official checksum list')
    archive_size,observed=fetch(ORIGIN+archive.name,archive,MAX_ARCHIVE)
    if observed!=expected:raise ValueError('CMake archive integrity mismatch')
    output=project/'toolchain';output.mkdir();inventory={};aliases=set();total=0
    with zipfile.ZipFile(archive) as source:
        if len(source.infolist())>20000:raise ValueError('CMake archive entry budget')
        for entry in source.infolist():
            relative=PurePosixPath(entry.filename)
            if relative.is_absolute() or '\\' in entry.filename or len(relative.parts)>32:raise ValueError('Invalid archive path')
            parts=relative.parts
            if not parts or parts[0]!=STEM:raise ValueError('Unexpected CMake archive root')
            for part in parts:
                if part in {'.','..'} or part.endswith((' ','.')) or re.search(r'[\x00-\x1f:<>"|?*]',part):raise ValueError('Invalid archive component')
                if part.split('.')[0].upper() in {'CON','PRN','AUX','NUL',*(f'COM{i}' for i in range(1,10)),*(f'LPT{i}' for i in range(1,10))}:raise ValueError('Reserved archive component')
            if len(parts)<3 or parts[1] not in {'bin','share'} or entry.is_dir():continue
            kind=(entry.external_attr>>16)&0o170000
            if kind not in {0,0o100000}:raise ValueError('Nonordinary CMake entry')
            name=PurePosixPath(*parts[1:]).as_posix();alias=name.casefold()
            if alias in aliases or entry.file_size>64*1024*1024:raise ValueError('CMake alias/size rejected')
            aliases.add(alias);total+=entry.file_size
            if total>MAX_TOTAL:raise ValueError('CMake expansion budget')
            target=output.joinpath(*parts[1:]);target.parent.mkdir(parents=True,exist_ok=True)
            digest=hashlib.sha256();written=0
            with source.open(entry) as payload,target.open('xb') as destination:
                while True:
                    block=payload.read(256*1024)
                    if not block:break
                    written+=len(block)
                    if written>entry.file_size:raise ValueError('CMake payload extent')
                    digest.update(block);destination.write(block)
            if written!=entry.file_size:raise ValueError('Truncated CMake payload')
            inventory[name]={'bytes':written,'sha256':digest.hexdigest()}
    if not (output/'bin/cmake.exe').is_file() or not (output/'share'/f'cmake-{VERSION.rsplit(".",1)[0]}'/'Modules/CMake.cmake').is_file():raise ValueError('CMake component incomplete')
    receipt={'schema_version':1,'version':VERSION,'url':ORIGIN+archive.name,'archive_sha256':observed,'archive_bytes':archive_size,'inventory':inventory,'files':len(inventory),'bytes':total,'tool_execution':'not_started','configuration':'not_started','product_build':'not_started','tests':'not_run','global_path_modified':False,'registry_modified':False}
    (project/'preparation-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf-8')
    print(json.dumps({key:receipt[key] for key in ('version','files','bytes','configuration','product_build','tests')}))

if __name__=='__main__':main()
