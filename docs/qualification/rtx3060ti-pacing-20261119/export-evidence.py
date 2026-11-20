"""Export small, reproducible pacing evidence; keep full traces/binaries local."""
import collections
import hashlib
import json
import math
from pathlib import Path
import runpy
import shutil

root = next(p for p in Path(__file__).resolve().parents if (p / 'Cargo.toml').exists())
local = root / 'target/gpu-pacing-20261119'
out = root / 'docs/qualification/rtx3060ti-pacing-20261119'
out.mkdir(parents=True, exist_ok=True)
analyze = runpy.run_path(str(root / 'scripts/analyze-video-pacing.py'))['analyze']

def read(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))

def save(name, value):
    (out / name).write_text(json.dumps(value, indent=2) + '\n', encoding='utf-8')

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest().upper()

def rows(path):
    return [json.loads(line) for line in path.read_text(encoding='utf-8-sig').splitlines() if line.strip()]

bench = []
analyses = {}
index = []
examples = []
for path in sorted(local.glob('*.log')):
    records = [json.loads(line[15:]) for line in path.read_text(encoding='utf-8-sig').splitlines()
               if line.startswith('VIDEO_PLAYBACK ')]
    if not records:
        continue
    record = records[-1]
    bench.append({'run': path.stem, 'fresh_fps': record['delivered_per_clip'][0] / record['wall_s'],
                  'record': record, 'manifest': read(Path(str(path) + '.manifest.json'))})
    trace = Path(str(path) + '.trace.jsonl')
    if trace.exists():
        data = rows(trace)
        result = analyze(data)
        analyses[path.stem] = result
        (local / (path.stem + '.analysis.json')).write_text(json.dumps(result, indent=2) + '\n')
        index.append({'run': path.stem, 'path': str(trace.relative_to(root)).replace('\\', '/'),
                      'sha256': sha(trace), 'bytes': trace.stat().st_size})
        begin = next(r[0] for r in data if r[2] == 'measurement_begin')
        end = next(r[0] for r in data if r[2] == 'measurement_end')
        # Deterministic first examples, not hand-picked best/worst intervals.
        for label, predicate in [
            ('empty', lambda r: r[2] == 'take' and r[3] == 0),
            ('selection_skip', lambda r: r[2] == 'select' and r[4] == 1),
            ('overwrite', lambda r: r[2] == 'publish' and r[4] == 1),
        ]:
            event = next((r for r in data if begin < r[0] < end and predicate(r)), None)
            if event:
                examples.append({'run': path.stem, 'kind': label, 'event': event,
                                 'rows': [r for r in data if event[0] - 20e6 <= r[0] <= event[0] + 20e6]})
save('benchmark-runs.json', bench)
save('trace-analyses.json', analyses)
save('trace-index.json', index)
save('trace-examples.json', examples)

apps = []
for name in ['app-headless-1080', 'app-headless-4k', 'app-windowed-1080', 'app-windowed-visible']:
    manifest = local / (name + '.manifest.json')
    if not manifest.exists():
        continue
    trace = local / (name + '.trace.jsonl')
    start = 40 if 'windowed' in name else 5
    result = analyze(rows(trace), seconds=60, pts_start=start)
    (local / (name + '.analysis.json')).write_text(json.dumps(result, indent=2) + '\n')
    snapshots = []
    for path in sorted(local.glob(name + '.state-*.json'), key=lambda p: int(p.stem.rsplit('-', 1)[1])):
        state = read(path)
        videos = [d['video_playback'] for c in state['mixer']['channels'] for d in c['decks']
                  if d.get('video_playback')]
        snapshots.append({'file': path.name, 'videos': videos})
    apps.append({'run': name, 'manifest': read(manifest), 'pts_start': start,
                 'measured_seconds': 60, 'analysis': result, 'snapshots': snapshots,
                 'trace_sha256': sha(trace),
                 'visibility': ('normal launch; screenshot verified in warmup; no UI automation during measured interval'
                                if name == 'app-windowed-visible' else
                                'unverified; activation attempt failed, excluded from foreground comparison'
                                if name == 'app-windowed-1080' else 'headless')})
save('app-runs.json', apps)

counter_apps = []
for manifest in sorted(local.glob('app-untraced-*.manifest.json')):
    name = manifest.name.removesuffix('.manifest.json')
    samples = read(local / (name + '.samples.json'))
    a = next(s for s in samples if s['index'] == 4)
    b = next(s for s in samples if s['index'] == 64)
    frames = b['video']['delivery']['gpu_frames'] - a['video']['delivery']['gpu_frames']
    elapsed = b['elapsed_s'] - a['elapsed_s']
    position = b['video']['position'] - a['video']['position']
    assert read(manifest)['trace'] is None
    counter_apps.append({'run': name, 'manifest': read(manifest), 'samples': samples,
                         'frames_delta': frames, 'client_elapsed_s': elapsed,
                         'playhead_elapsed_s': position, 'payload_fps_client': frames / elapsed,
                         'payload_fps_playhead': frames / position,
                         'scope': 'published-counter estimates with asynchronous endpoints; no PTS gap/unique-picture trace'})
save('untraced-app-controls.json', counter_apps)

for stem in ['timer-probe', 'timer-probe-condvar']:
    data = rows(local / (stem + '.jsonl'))
    groups = collections.defaultdict(list)
    per_phase = len(data) // 3
    for i, row in enumerate(data):
        groups[(row.get('phase', i // per_phase), row['timer_1ms'], row['kind'], row['requested_s'])].append(row['elapsed_s'] * 1000)
    summary = []
    for (phase, timer, kind, requested), values in groups.items():
        values.sort()
        summary.append({'phase': phase, 'timer_1ms': timer, 'kind': kind,
                        'requested_ms': requested * 1000, 'n': len(values),
                        'p50_ms': values[len(values)//2],
                        'p95_ms': values[math.ceil((len(values)-1)*.95)], 'max_ms': values[-1]})
    save(stem + '-summary.json', summary)
    shutil.copyfile(local / (stem + '.rs'), out / (stem + '.rs'))
    shutil.copyfile(local / (stem + '.jsonl'), out / (stem + '.jsonl'))

fixtures = []
for name in ['h264-1080p60', 'h264-4k60']:
    data = read(local / (name + '.continuous.verified.json'))
    data['sha256'] = data.pop('hash')['Hash']
    fixtures.append(data)
save('fixtures.json', fixtures)
for name in ['host.json', 'diagnostic-binaries.json', 'diagnostic-source-hashes.json',
             'prior-decomposition.json', 'prior-g14-decomposition.json',
             'cpu-checks.log', 'video-regressions.log', 'interlaced.log', 'playlist-180s.log']:
    shutil.copyfile(local / name, out / name)
for path in local.glob('picture-*.log'):
    shutil.copyfile(path, out / path.name)
for name in ['run-app.ps1', 'run-diagnostics.ps1', 'run-validation.ps1']:
    shutil.copyfile(local / name, out / name)

save('artifact-provenance.json', {
    'base_revision': 'a5b98f1fe93b9f1e4f82214fc0cc83ce63b2c1bd',
    'profile': 'opt-level=3; lto=thin; codegen-units=1; strip=symbols; panic=unwind',
    'features': 'no default features; screen-capture,windows-browser',
    'test_feature_note': 'test-fixtures is enabled through the existing self development dependency; not in app',
    'measured_source': 'target/gpu-pacing-20261119/source-snapshot/',
    'source_note': 'After all measurements, publish_decoded match was rewritten to equivalent if-let to satisfy Clippy single_match_else. Measured snapshot and binaries are preserved; not claimed as a rebuild of that lint-only edit.',
    'intermediate_build': 'intermediate-unmeasured-test.exe excluded: source edits overlapped Cargo library/test passes',
    'analyzer_sha256': sha(root / 'scripts/analyze-video-pacing.py'),
    'wrapper_sha256': sha(root / 'scripts/measure-video-pacing.ps1'),
    'final_video_mod_sha256': sha(root / 'src/internal/video/mod.rs'),
})
print(json.dumps({'benchmark_runs': len(bench), 'traced_runs': len(analyses), 'app_runs': len(apps), 'output': str(out)}))
