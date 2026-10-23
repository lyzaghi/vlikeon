"""Synthetic release attribution/privacy cases; no real credentials or games."""
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest
import subprocess

sys.path.insert(0,str(Path(__file__).resolve().parents[2]/'tools'))
from release_inventory import check_release_tree

class ReleaseInventoryCases(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory(prefix='vlikeon-release-case-')
        self.addCleanup(self.temp.cleanup);self.base=Path(self.temp.name)
        for name in ['LICENSE','NOTICE','licenses/third_party_notices.txt','app.bin']:
            self.write(name,b'synthetic fixture\n')
        self.component={'name':'fixture','revision':'fixture-v1','license':'Apache-2.0',
            'distribution_approved':True,'corresponding_source_required':False,
            'artifact_paths':['app.bin'],'notice_paths':['LICENSE']}
        self.offer()
    def write(self,name,content):
        path=self.base/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(content)
    def offer(self):self.write('licenses/source_offer.json',json.dumps({'schema_version':1,'components':[self.component]}).encode())
    def findings(self):return check_release_tree(self.base,[{'path':p.relative_to(self.base).as_posix()} for p in self.base.rglob('*') if p.is_file()])
    def codes(self):return {item['code'] for item in self.findings()}
    def test_declared_fixture_is_admitted(self):self.assertEqual(self.findings(),[])
    def test_unattributed_binary_rejected(self):
        self.write('extra.dll',b'fixture');self.assertIn('unattributed_payload',self.codes())
    def test_license_directory_cannot_hide_payload(self):
        self.write('licenses/extra.dll',b'fixture');self.assertIn('unattributed_payload',self.codes())
    def test_unreviewed_component_rejected(self):
        self.component['distribution_approved']=False;self.offer();self.assertIn('component_review_missing',self.codes())
    def test_notice_must_exist(self):
        self.component['notice_paths']=['licenses/missing.txt'];self.offer();self.assertIn('component_notice_missing',self.codes())
    def test_source_hash_is_checked(self):
        self.write('sources/fixture.tar',b'not a real archive')
        self.component.update(corresponding_source_required=True,source_archive_path='sources/fixture.tar',source_archive_sha256='0'*64)
        self.component['artifact_paths'].append('sources/fixture.tar');self.offer()
        self.assertIn('corresponding_source_missing_or_mismatched',self.codes())
        self.component['source_archive_sha256']=hashlib.sha256(b'not a real archive').hexdigest();self.offer();self.assertEqual(self.findings(),[])
    def test_source_obligation_is_explicit(self):
        del self.component['corresponding_source_required'];self.offer();self.assertIn('source_obligation_not_declared',self.codes())
    def test_profile_is_rejected(self):
        self.write('user-data/Preferences',b'fixture');self.assertIn('private_content_path',self.codes())
    def test_marker_does_not_leak_into_findings(self):
        marker=b'-----BEGIN '+b'PRIVATE KEY-----'
        self.write('app.bin',marker);result=self.findings()
        self.assertIn('credential_marker',{item['code'] for item in result});self.assertNotIn('PRIVATE KEY',json.dumps(result))
    def test_duplicate_attribution_is_rejected(self):
        self.write('licenses/source_offer.json',json.dumps({'schema_version':1,'components':[self.component,{**self.component,'name':'another'}]}).encode())
        self.assertIn('ambiguous_component_attribution',self.codes())
    def test_duplicate_json_keys_rejected(self):
        self.write('licenses/source_offer.json',b'{"schema_version":1,"schema_version":1,"components":[]}')
        self.assertIn('invalid_source_declaration',self.codes())
    def test_case_collision_and_traversal_rejected(self):
        result=check_release_tree(self.base,[{'path':'app.bin'},{'path':'APP.BIN'},{'path':'../outside'}])
        self.assertIn('duplicate_package_path',{item['code'] for item in result});self.assertIn('unsafe_package_path',{item['code'] for item in result})
    def test_packager_uses_inventory_gate(self):
        self.write('broker.bin',b'synthetic broker')
        self.component['artifact_paths'].append('broker.bin');self.offer()
        meta=self.base.parent/(self.base.name+'-metadata')
        meta.mkdir();self.addCleanup(lambda:__import__('shutil').rmtree(meta))
        identity={'channel':'development','platform':'windows-x64','version':'0.1.0',
                  'chromium_revision':'a'*40,'minimum_data_schema':1,'maximum_data_schema':1,
                  'browser_entry':'app.bin','broker_entry':'broker.bin'}
        (meta/'identity.json').write_text(json.dumps(identity));(meta/'executables.json').write_text(json.dumps(['app.bin','broker.bin']))
        script=Path(__file__).resolve().parents[2]/'tools/package_release.py'
        args=[sys.executable,'-B',str(script),'--directory',str(self.base),'--identity',str(meta/'identity.json'),'--executables',str(meta/'executables.json'),'--output',str(meta/'fixture.oppk')]
        accepted=subprocess.run(args,capture_output=True,timeout=20)
        self.assertEqual(accepted.returncode,0,accepted.stderr.decode(errors='replace'))
        self.assertEqual((meta/'fixture.oppk').read_bytes()[:8],b'OPPK0001')
        (meta/'fixture.oppk').unlink()
        self.write('unattributed.bin',b'synthetic extra')
        rejected=subprocess.run(args,capture_output=True,timeout=20)
        self.assertNotEqual(rejected.returncode,0);self.assertIn(b'unattributed_payload',rejected.stderr)
        self.assertFalse((meta/'fixture.oppk').exists())

if __name__=='__main__':unittest.main(verbosity=2)
