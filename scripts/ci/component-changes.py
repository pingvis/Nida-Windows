"""Select component jobs from the complete Git diff, without API pagination limits."""
import fnmatch
import json
import os
from pathlib import Path
import subprocess

FILTERS = {'decklink': ['src/internal/decklink/**',
              'src/internal/ndi/mod.rs',
              'src/internal/audio/mod.rs',
              'src/engine/value/render.rs',
              'src/engine/value/led.rs',
              'src/engine/value/warp.rs',
              'src/internal/scene/mod.rs',
              'src/internal/persistence/**',
              'src/app/state/outputs.rs',
              'src/usecases/ui/panels/outputs.rs',
              'vendor/decklink-sdk/**',
              'tests/decklink-native/**',
              'build.rs',
              'Cargo.toml',
              'Cargo.lock',
              'clippy.toml',
              '.github/actions/linux-ffmpeg/**',
              'scripts/ci/setup-linux-ffmpeg.py'],
 'ndi': ['src/internal/ndi/sdk.rs', 'src/internal/ndi/ffi.rs', 'tests/ndi-loader/**'],
 'powerpoint': ['src/internal/presentation/**',
                'src/app/state/presentations.rs',
                'src/usecases/ui/panels/add_input.rs',
                'src/usecases/ui/panels/file_drop.rs',
                'src/usecases/ui/panels/presentation.rs',
                'scripts/ci/test-windows-powerpoint.ps1',
                'scripts/ci/setup-windows-build.ps1'],
 'browser': ['browser/**',
             'scripts/ci/*browser*',
             'scripts/ci/*cef*',
             '.github/actions/windows-cef/**',
             'src/internal/html/**']}
COMMON = ['Cargo.toml',
 'Cargo.lock',
 'build.rs',
 'clippy.toml',
 '.cargo/**',
 '.github/workflows/component-checks.yml',
 'scripts/ci/component-changes.py',
 'scripts/ci/test-component-changes.py']

def select(paths):
    return {
        name: any(fnmatch.fnmatchcase(path, pattern)
                  for path in paths for pattern in patterns + COMMON)
        for name, patterns in FILTERS.items()
    }


def main():
    event = json.loads(Path(os.environ['GITHUB_EVENT_PATH']).read_text())
    kind = os.environ['GITHUB_EVENT_NAME']
    if kind == 'workflow_dispatch':
        selected = dict.fromkeys(FILTERS, True)
    else:
        if kind == 'pull_request':
            pr = event['pull_request']
            revisions = [pr['base']['sha'] + '...' + pr['head']['sha']]
        elif kind == 'push':
            before = event['before']
            revisions = [before, event['after']] if before.strip('0') else None
        else:
            raise ValueError('Unsupported workflow event: ' + kind)
        if revisions is None:
            selected = dict.fromkeys(FILTERS, True)
        else:
            diff = subprocess.check_output(
                ['git', 'diff', '--name-only', '--no-renames', '-z', *revisions, '--'])
            selected = select(diff.decode('utf-8', errors='surrogateescape').split('\0'))
    with open(os.environ['GITHUB_OUTPUT'], 'a', encoding='utf-8') as output:
        for name, enabled in selected.items():
            output.write(f'{name}={str(enabled).lower()}\n')
            print(f'{name}: {enabled}')


if __name__ == '__main__':
    main()
