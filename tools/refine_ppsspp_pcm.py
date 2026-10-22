"""Author one traceable source-only PCM refinement; never build or execute a core.

Fresh full preparations already contain these changes. This one-time transition
preserves the previous candidates and their receipts before editing source that
has neither been built nor qualified. It is not a runtime verification result.
"""
import json
from pathlib import Path
import shutil
from pinned_core_source import ROOT, owned, file_hash, source_inventory


SAS_BEFORE = '(u64)sas_.streamOffset > sas_.bufSize[sas_.curBuffer]'
SAS_AFTER = 'sas_.bufSize[sas_.curBuffer] > 0x7FFFFFFFu || (u64)sas_.streamOffset > sas_.bufSize[sas_.curBuffer]'
SIMPLE_EDITS = (
    ('\tframe_ = av_frame_alloc();', '\tframe_ = av_frame_alloc();\n\tif (!frame_) return;'),
    ('\t\treturn codec_ != 0;', '\t\treturn codec_ != 0 && codecCtx_ && frame_;'),
    ('#ifdef USE_FFMPEG\n\n\tif (codecOpen_) {', '#ifdef USE_FFMPEG\n\tif (!codecCtx_) return;\n\n\tif (codecOpen_) {'),
    ('\tcodecOpen_ = true;\n\treturn retval >= 0;', '\tcodecOpen_ = retval >= 0;\n\treturn codecOpen_;'),
    ('void FFmpegAudioDecoder::SetChannels(int channels) {', 'void FFmpegAudioDecoder::SetChannels(int channels) {\n\tif (channels != 1 && channels != 2) return;'),
    ('\t    (outbuf && !outputCapacityFrames) || (!outbuf && outputCapacityFrames)) return false;',
     '\t    (outbuf && !outputCapacityFrames) || (!outbuf && outputCapacityFrames)) return false;\n#ifdef USE_FFMPEG\n\tif (!frame_ || !codecCtx_) return false;\n#endif'),
)


def changed(value, edits):
    for before, after in edits:
        if value.count(before) != 1:
            raise ValueError('The exact uncompiled PCM source transition no longer matches')
        value = value.replace(before, after)
    return value


def main():
    workspaces = [ROOT / 'out/native-sources/ppsspp-v3', ROOT / 'out/native-sources/ppsspp-gl-v3', ROOT / 'out/web-cores/ppsspp-v3']
    history = owned(ROOT / 'out/authoring-history/ppsspp-pcm-bounds-v1')
    if history.exists():
        raise ValueError('Preserve the existing source-authoring transition; do not replay it')
    prepared = []
    for workspace in workspaces:
        owned(workspace)
        if any((workspace / name).exists() for name in ('build', 'build-source', 'build-receipt.json', 'product')):
            raise ValueError('Only uncompiled source-only projects may receive this transition')
        web = workspace.parent.name == 'web-cores'
        receipt = workspace / ('corresponding-source.json' if web else 'source-preparation.json')
        record = json.loads(receipt.read_text(encoding='utf-8'))
        if record.get('verification') != 'not_run' or record.get('revision') != 'a5275067b62433375bbde12921e96329a795dff0':
            raise ValueError('The unverified pinned source identity changed')
        if web:
            if record.get('source_only') is not True or source_inventory(workspace / 'source') != record['prepared_source_sha256']:
                raise ValueError('The original Web source-only inventory changed')
            for name, expected in record['recipe_sha256'].items():
                if file_hash(workspace / name) != expected:
                    raise ValueError('The retained Web producer recipe changed')
        elif record.get('build') != 'not_started' or record.get('configure') != 'not_started':
            raise ValueError('Native preparation has advanced past source authoring')
        edits = []
        for relative, replacements in [('Core/HLE/AtracCtx2.cpp', ((SAS_BEFORE, SAS_AFTER),)), ('Core/HW/SimpleAudioDec.cpp', SIMPLE_EDITS)]:
            path = workspace / 'source' / relative
            before = path.read_text(encoding='utf-8')
            edits.append((relative, path, changed(before, replacements)))
        prepared.append((workspace, receipt, record, web, edits))
    history.mkdir(parents=True)
    shutil.copy2(Path(__file__), history / 'source-authoring-tool.py')
    for index, (workspace, receipt, record, web, edits) in enumerate(prepared):
        prior = history / str(index)
        prior.mkdir()
        shutil.copy2(receipt, prior / receipt.name)
        changes = []
        for relative, path, value in edits:
            backup = prior / 'source' / relative
            backup.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(path, backup)
            old_hash = file_hash(path)
            path.write_text(value, encoding='utf-8', newline='\n')
            new_hash = file_hash(path)
            changes.append({'path': relative, 'before_sha256': old_hash, 'after_sha256': new_hash})
            if web:
                record['prepared_source_sha256'][relative] = new_hash
        adapter = workspace / 'ppsspp_audio.py'
        if adapter.exists():
            shutil.copy2(adapter, prior / 'ppsspp_audio.py')
        shutil.copy2(ROOT / 'tools/ppsspp_audio.py', adapter)
        if web:
            record['recipe_sha256']['ppsspp_audio.py'] = file_hash(adapter)
        transition = {'schema_version': 1, 'kind': 'uncompiled_pcm_source_authoring_v1', 'source_only': True,
            'verification': 'not_run', 'reason': 'Bound restored signed SAS buffer offsets and propagate failed decoder allocations',
            'prior_receipt': str((prior / receipt.name).resolve()), 'prior_receipt_sha256': file_hash(prior / receipt.name),
            'before_sources': str((prior / 'source').resolve()), 'changes': changes,
            'audio_adapter_sha256': file_hash(adapter), 'authoring_tool_sha256': file_hash(Path(__file__))}
        transition_path = workspace / 'source-authoring-transition.json'
        transition_path.write_text(json.dumps(transition, indent=2) + '\n', encoding='utf-8')
        record['source_authoring_transition_sha256'] = file_hash(transition_path)
        if web:
            record['auxiliary_sha256']['source-authoring-transition.json'] = file_hash(transition_path)
        receipt.write_text(json.dumps(record, indent=2) + '\n', encoding='utf-8')
    print('Authored bounded PCM source refinements with retained before-images and source receipts. No build, runtime or test was started.')


if __name__ == '__main__':
    main()
