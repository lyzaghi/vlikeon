"""Build-time admission for an actual atlas and its retained source inventory."""
import json
from pathlib import Path
from pinned_core_source import file_hash, source_inventory

POLICY = 'ppsspp_atlas_committed_v1'


def admit_generated(path, core_revision, font_sources):
    if not 0 < path.stat().st_size <= 1024 * 1024:
        raise ValueError('The generated atlas receipt exceeds its bound')
    value = json.loads(path.read_text(encoding='utf-8'))
    if value.get('kind') != 'ppsspp_open_font_atlas' or value.get('source_policy') != POLICY or value.get('core_revision') != core_revision or value.get('font_sources') != font_sources or value.get('generated') is not True or value.get('source_only') is not False:
        raise ValueError('The atlas requires actual outputs from the selected committed source producer')
    source = path.parent / 'source'
    if Path(value['corresponding_source']).resolve(strict=True) != source.resolve(strict=True):
        raise ValueError('The atlas source inventory does not belong to its build project')
    evidence = path.parent / 'source-evidence.json'
    identity = value.get('source_evidence', {})
    if not 0 < evidence.stat().st_size <= 16 * 1024 * 1024 or evidence.stat().st_size != identity.get('bytes') or file_hash(evidence) != identity.get('sha256'):
        raise ValueError('The retained atlas source evidence differs from its generated receipt')
    record = json.loads(evidence.read_text(encoding='utf-8'))
    if record.get('source_policy') != POLICY or record.get('core_revision') != core_revision or record.get('font_sources') != font_sources or source_inventory(source) != record.get('compiled_source_sha256'):
        raise ValueError('The generated atlas corresponding source changed after generation')
    if file_hash(path.parent / 'source-preparation.json') != record.get('source_preparation_sha256'):
        raise ValueError('The atlas preparation receipt changed after generation')
    assets = value.get('assets', [])
    if len(assets) != 2 or {item['name'] for item in assets} != {'ppge_atlas.zim', 'ppge_atlas.meta'}:
        raise ValueError('The atlas output set differs from the fixed product selection')
    for item in assets:
        asset = source / item['name']
        if Path(item['source']).resolve(strict=True) != asset.resolve(strict=True) or not 0 < item['bytes'] <= 32 * 1024 * 1024 or asset.stat().st_size != item['bytes'] or file_hash(asset) != item['sha256']:
            raise ValueError('The generated atlas asset differs from its source/build receipt')
    return value
