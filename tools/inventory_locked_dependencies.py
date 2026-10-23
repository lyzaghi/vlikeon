"""Create a review inventory from Cargo.lock, not an artifact SBOM or audit verdict."""
import argparse
import hashlib
import json
from pathlib import Path
import tomllib

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--lock',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    raw=args.lock.read_bytes();lock=tomllib.loads(raw.decode('utf-8'))
    packages=lock.get('package')
    if not isinstance(packages,list) or not 1<=len(packages)<=10000:raise ValueError('Invalid lock inventory')
    components=[];seen=set()
    for item in packages:
        name=item['name'];version=item['version'];source=item.get('source','workspace')
        if not all(isinstance(value,str) and 0<len(value)<=2048 for value in [name,version,source]):raise ValueError('Invalid package identity')
        identity=(name,version,source)
        if identity in seen:raise ValueError('Duplicate dependency identity')
        seen.add(identity)
        checksum=item.get('checksum')
        if source.startswith('registry+') and (not isinstance(checksum,str) or len(checksum)!=64 or any(c not in '0123456789abcdef' for c in checksum)):raise ValueError('Registry checksum missing')
        components.append({'name':name,'version':version,'source':source,'checksum_sha256':checksum,
                           'license_review':'pending','advisory_review':'pending','included_in_final_artifact':'not_determined'})
    report={'schema_version':1,'kind':'locked-source-review-inventory','lock_sha256':hashlib.sha256(raw).hexdigest(),
            'artifact_sbom':False,'release_approved':False,'scope':'All lockfile packages, including development, optional and platform-specific dependencies; not proof of shipped dependency closure.',
            'components':sorted(components,key=lambda item:(item['name'],item['version'],item['source']))}
    args.output.parent.mkdir(parents=True,exist_ok=True)
    with args.output.open('x',encoding='utf-8') as target:json.dump(report,target,indent=2);target.write('\n')
    print(json.dumps({'packages':len(components),'lock_sha256':report['lock_sha256'],'artifact_sbom':False}))

if __name__=='__main__':main()
