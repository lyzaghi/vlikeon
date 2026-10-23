"""Admit actual Git checkouts, not just gclient's submodule revision cache."""
import argparse
import ast
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import subprocess


def verify_dependency_inventory(workspace, git):
    workspace = Path(workspace).resolve(strict=True)
    entry = workspace / '.gclient_entries'
    data = entry.read_bytes()
    if len(data) > 4 * 1024 * 1024:
        raise ValueError('The gclient entry inventory exceeds its bound')
    body = ast.parse(data.decode('utf-8')).body
    if (len(body) != 1 or not isinstance(body[0], ast.Assign)
            or len(body[0].targets) != 1
            or not isinstance(body[0].targets[0], ast.Name)
            or body[0].targets[0].id != 'entries'):
        raise ValueError('Unexpected gclient entry inventory syntax')
    entries = ast.literal_eval(body[0].value)
    if not isinstance(entries, dict) or not 1 <= len(entries) <= 10000:
        raise ValueError('Invalid gclient entry inventory')
    environment = {**os.environ, 'GIT_NO_LAZY_FETCH': '1', 'GIT_TERMINAL_PROMPT': '0'}

    def capture(directory, *arguments):
        return subprocess.run(
            [str(git), '--no-optional-locks', '-c', 'core.autocrlf=false',
             '-C', str(directory), *arguments], env=environment,
            capture_output=True, text=True, timeout=90)

    checked = []
    for name, url in sorted(entries.items()):
        if not isinstance(name, str) or not isinstance(url, str):
            raise ValueError('Invalid gclient entry type')
        match = re.fullmatch(r'https://[^\s]+@([0-9a-f]{40})', url)
        if not match:
            continue  # CIPD/GS inputs and the separately checked root solution.
        relative = PurePosixPath(name)
        if (relative.is_absolute() or '\\' in name or ':' in name
                or any(part in {'', '.', '..'} for part in name.split('/'))):
            raise ValueError('A Git dependency escapes the owned workspace')
        directory = workspace.joinpath(*relative.parts)
        if not directory.resolve().is_relative_to(workspace):
            raise ValueError('A Git dependency escapes the owned workspace')
        for ancestor in [directory / '.git', directory, *directory.parents]:
            if ancestor.is_symlink() or getattr(ancestor, 'is_junction', lambda: False)():
                raise ValueError('Git dependency ancestry contains a link: ' + name)
            if ancestor == workspace:
                break
        if not (directory / '.git').is_dir():
            raise ValueError('A Git dependency has no ordinary checkout: ' + name)
        expected = match[1]
        head = capture(directory, 'rev-parse', '--verify', 'HEAD')
        if head.returncode or head.stdout.strip() != expected:
            raise ValueError('Git dependency HEAD differs from its pin: ' + name)
        difference = capture(directory, 'diff', '--quiet', '--no-ext-diff',
                             '--no-textconv', '--ignore-submodules=all', expected, '--')
        if difference.returncode:
            raise ValueError('Git dependency tracked source is incomplete or changed: ' + name)
        checked.append({'path': name, 'revision': expected})
    if not checked or entry.read_bytes() != data:
        raise ValueError('The Git dependency inventory is empty or changed during admission')
    return {'schema_version': 1, 'entries_sha256': hashlib.sha256(data).hexdigest(),
            'git_dependencies': checked, 'tracked_source_matches': True,
            'scope': 'Git dependencies only; CIPD/GS and product behavior are separate'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--workspace', type=Path, required=True)
    parser.add_argument('--git', type=Path, required=True)
    args = parser.parse_args()
    result = verify_dependency_inventory(args.workspace, args.git)
    print(json.dumps(result, sort_keys=True))


if __name__ == '__main__':
    main()
