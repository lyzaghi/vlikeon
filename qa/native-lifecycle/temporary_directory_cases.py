"""Actual worker/core observation of private TEMP/TMP bootstrap; no sandbox proof."""
import argparse
import json
import os
from pathlib import Path
from run_cases import Worker, digest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--worker', type=Path, required=True)
    parser.add_argument('--core', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if os.name != 'nt':
        raise ValueError('Windows qualification only')
    worker_path, core = args.worker.resolve(strict=True), args.core.resolve(strict=True)
    output = args.output.absolute()
    if output.exists():
        raise ValueError('Choose a fresh retained output')
    output.mkdir(parents=True)
    report = {'verification': 'running', 'worker_sha256': digest(worker_path),
              'core_sha256': digest(core), 'cases': [],
              'scope': 'direct worker and fixture DLL, Win32 and shared CRT environment; no AppContainer/core filesystem/device proof'}
    try:
        for name, directory_name in [('ascii', 'private scratch'), ('unicode', 'scratch Việt 日本')]:
            scratch, previous = output / directory_name, output / (name + '-os-temporary')
            scratch.mkdir(); previous.mkdir()
            actor = Worker(worker_path, core, output / name, {
                'OMNIPLAY_PRIVATE_SCRATCH': str(scratch), 'OMNIPLAY_EXPECT_PRIVATE_TEMP': '1',
                'TEMP': str(previous), 'TMP': str(previous)})
            row = {'name': name, 'passed': False}; report['cases'].append(row)
            try:
                hello, _ = actor.okay('hello')
                if hello.get('library_version') != '1.0.0' or hello.get('library_name') != 'OmniPlay lifecycle qualification':
                    raise AssertionError('core did not observe matching Win32/wide/narrow CRT temporary environment')
                actor.okay('stop')
                row['exit_code'] = actor.process.wait(timeout=5)
                if row['exit_code'] != 0:
                    raise AssertionError('worker shutdown failed')
                row['passed'] = True
            finally:
                actor.close()
        actor = Worker(worker_path, core, output / 'relative', {'OMNIPLAY_PRIVATE_SCRATCH': 'relative-scratch'})
        row = {'name': 'relative-refused', 'passed': False}; report['cases'].append(row)
        try:
            row['exit_code'] = actor.process.wait(timeout=5)
            if row['exit_code'] != 2:
                raise AssertionError('relative private scratch was admitted')
            row['passed'] = True
        finally:
            actor.close()
        report['verification'] = 'passed'
    except BaseException:
        report['verification'] = 'failed'
        raise
    finally:
        (output / 'evidence.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')


if __name__ == '__main__':
    main()
