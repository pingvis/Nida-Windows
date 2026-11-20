"""Export compact pacing evidence; keep binaries, pictures and full traces local."""
import argparse
import hashlib
import json
import re
import shutil
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--input', default='target/gpu-pacing-fix-20261120-qualified')
parser.add_argument('--output', default='docs/qualification/rtx3060ti-pacing-fix-20261120')
args = parser.parse_args()
root, output = Path(args.input), Path(args.output)
if not (root / 'overnight-complete.txt').exists():
    raise SystemExit('Qualification is incomplete; no evidence exported')
output.mkdir(parents=True, exist_ok=True)

def read(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))

def save(name, value):
    (output / name).write_text(json.dumps(value, indent=2) + '\n', encoding='utf8')

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest().upper()

def test_result(path):
    text = path.read_text(encoding='utf-8-sig')
    found = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored', text)
    if not found or any(int(failed) for _, failed, _ in found):
        raise ValueError(f'No passing test result: {path}')
    return [{'passed': int(p), 'failed': int(f), 'ignored': int(i)} for p, f, i in found]

benchmarks = []
for pacing in sorted(root.glob('*.log.pacing.json')):
    log = pacing.with_name(pacing.name.removesuffix('.pacing.json'))
    lines = [line[15:] for line in log.read_text(encoding='utf-8-sig').splitlines()
             if line.startswith('VIDEO_PLAYBACK ')]
    if len(lines) != 1:
        raise ValueError(f'Expected one benchmark record: {log}')
    record = json.loads(lines[0])
    for delivery in record['delivery_per_clip']:
        if delivery['downloaded_bytes'] or delivery['uploaded_bytes'] or delivery['peak_slots'] > 3 or delivery['fallback_reason']:
            raise ValueError(f'GPU-resident invariant failed: {log}')
    benchmarks.append({'name': log.stem, 'record': record,
                       'manifest': read(Path(str(log) + '.manifest.json')),
                       'diagnostic_manifest': read(pacing)})
save('benchmarks.json', benchmarks)

analyses = {p.name: read(p) for p in sorted(root.glob('*.analysis.json'))}
save('trace-analyses.json', analyses)
apps = []
for manifest in sorted(root.glob('app-*.manifest.json')):
    name = manifest.name.removesuffix('.manifest.json')
    samples = read(root / (name + '.samples.json'))
    apps.append({'name': name, 'manifest': read(manifest), 'samples': samples,
                 'visibility': 'API driven; no unobstructed foreground or scanout claim'})
save('apps.json', apps)
estimates = []
for app in apps:
    if 'untraced' not in app['name']:
        continue
    first = next(s for s in app['samples'] if s['index'] == 4)
    last = next(s for s in app['samples'] if s['index'] == 64)
    client_seconds = last['elapsed_s'] - first['elapsed_s']
    playhead_seconds = last['video']['position'] - first['video']['position']
    delta = last['video']['delivery']['gpu_frames'] - first['video']['delivery']['gpu_frames']
    estimates.append({'name': app['name'], 'client_seconds': client_seconds,
                      'playhead_seconds': playhead_seconds, 'encoded_counter_delta': delta,
                      'rate_per_client_seconds': delta / client_seconds,
                      'rate_per_playhead_seconds': delta / playhead_seconds,
                      'payload_discard_delta': last['video']['delivery']['dropped_frames'] - first['video']['delivery']['dropped_frames'],
                      'cpu_core_percent': (last['process_cpu_ms'] - first['process_cpu_ms']) / (client_seconds * 10) if 'process_cpu_ms' in last else None})
save('app-counter-estimates.json', {'estimates': estimates,
     'limits': 'Asynchronous worker snapshots and RPC endpoints; estimates are not exact per-picture or presentation intervals.'})

# Record acceptance separately from observations; retain failed evidence too.
checks = []
for item in benchmarks:
    record = item['record']
    if record['wall_s'] >= 59.9 and not item['diagnostic_manifest'].get('legacy_wait', False):
        fps = record['delivered_per_clip'][0] / record['wall_s']
        checks.append({'name': item['name'], 'kind': '60-second harness freshness',
                       'fresh_fps': fps, 'minimum': 59.9, 'passed': fps >= 59.9})
for name in ['app-headless-1080p60.analysis.json', 'app-headless-4k60.analysis.json',
             'app-windowed-final-occluded.analysis.json']:
    if name in analyses:
        result = analyses[name]
        checks.append({'name': name, 'kind': '60-second app freshness',
                       'fresh_fps': result['unique_encoded_pts'] / result['wall_s'],
                       'minimum': 59.9, 'passed': result['unique_encoded_pts'] / result['wall_s'] >= 59.9
                       and result['repeated_encoded_pts'] == 0 and result['backward_encoded_pts_steps'] == 0})
speed = analyses.get('app-headless-speed2.analysis.json')
if speed:
    copies = speed['events'].get('copy_complete', 0)
    checks.append({'name': '2x output cap', 'kind': 'bounded native copies',
                   'copy_count': copies, 'seconds': speed['wall_s'],
                   'maximum_count_with_boundary_allowance': speed['wall_s'] * 60 + 2,
                   'passed': copies <= speed['wall_s'] * 60 + 2})
phase_residuals = [name for name, result in analyses.items() if name.startswith('app-seek-phase-')
                   and (result['empty_takes'] or result['mailbox_replacements']
                        or result['selected_superseding_previous'] or result['repeated_encoded_pts'])]
save('acceptance.json', {'all_passed': all(c['passed'] for c in checks) and not phase_residuals,
                         'steady_and_speed_checks_passed': all(c['passed'] for c in checks),
                         'phase_windows_with_residual_loss': phase_residuals, 'checks': checks,
                         'limits': 'Fresh PTS before composition; no physical scanout qualification. Short seek-phase windows are exploratory.'})

before = read(Path('target/gpu-pacing-fix-20261120/settings-before.json'))
after = read(root / 'settings-after.json')
def settings(values):
    return {Path(v['Path']).name: v['Hash'] for v in values}
if settings(before) != settings(after):
    raise ValueError('Saved Nida settings changed')
save('settings-preserved.json', settings(after))

validation_names = ['cpu-checks.log', 'clippy.log', 'check-tests.log',
                    'adapter-regression.log', 'engine-stop-regression.log',
                    'video-regressions.log', 'interlaced.log', 'playlist-180s.log']
validation_names += sorted(p.name for p in root.glob('picture-*.log'))
if (root / 'retained-picture-regression.log').exists():
    validation_names.append('retained-picture-regression.log')
results = {}
for name in validation_names:
    path = root / name
    if name not in ('clippy.log', 'check-tests.log'):
        results[name] = test_result(path)
    shutil.copy2(path, output / name)
save('validation-results.json', results)
for name in ['binary-hashes.json', 'source-hashes.json', 'measurement-gpu-activity.csv',
             'build.ps1', 'run-measurements.ps1', 'run-app.ps1', 'run-app-phase-sweep.ps1',
             'run-validation.ps1', 'overnight.ps1', 'phase-contract.rs', 'phase-contract.log',
             'cadence-overrun-red.log', 'cpu-checks-green.log', 'build-test.log', 'build-app.log',
             'fine-phase-replay.js', 'wait-cpu-probe.rs', 'run-wait-cpu-probe.ps1',
             'wait-cpu-probe.jsonl', 'wait-cpu-probe.manifest.json', 'measurement-summary.json']:
    path = root / name
    if path.exists():
        shutil.copy2(path, output / name)
shutil.copy2(__file__, output / 'export-evidence.py')

history = {}
for folder in ['gpu-pacing-fix-20261120', 'gpu-pacing-fix-20261120-engine', 'gpu-pacing-fix-20261120-final']:
    directory = Path('target') / folder
    item = {'binary_hashes': read(directory / 'binary-hashes.json')}
    for name in ['harness-deadline-limitation.json', 'native-wait-probe.jsonl',
                 'app-windowed-worker-fix.analysis.json', 'app-windowed-worker-fix.engine-analysis.json',
                 'app-windowed-engine-fix-occluded.analysis.json', 'app-windowed-engine-fix-occluded.engine-analysis.json']:
        path = directory / name
        if path.exists():
            if name.endswith('.jsonl'):
                item[name] = [json.loads(line) for line in path.read_text(encoding='utf-8-sig').splitlines() if line.strip()]
            else:
                item[name] = read(path)
    if folder.endswith('-final'):
        item['status'] = 'Superseded by pending-picture retry and effective-rate cap guards'
        item['source_hashes'] = read(directory / 'source-hashes.json')
        item['environment_recovery'] = read(directory / 'queue-environment-recovery.json')
        for script in ['resume-phases.ps1', 'run-app-phase-sweep.ps1', 'run-app-speed-check.ps1']:
            shutil.copy2(directory / script, output / ('candidate3-' + script))
        item['analyses'] = {p.name: read(p) for p in sorted(directory.glob('*.analysis.json'))}
        item['benchmarks'] = []
        for log in sorted(directory.glob('*.log.pacing.json')):
            raw = log.with_name(log.name.removesuffix('.pacing.json'))
            records = [json.loads(line[15:]) for line in raw.read_text(encoding='utf-8-sig').splitlines() if line.startswith('VIDEO_PLAYBACK ')]
            item['benchmarks'].append({'name': raw.stem, 'record': records[0], 'diagnostic_manifest': read(log), 'manifest': read(Path(str(raw) + '.manifest.json'))})
    history[folder] = item
save('candidate-history.json', history)
fine_models = {p.name: read(p) for p in sorted(root.glob('*.fine-phase.json'))}
if fine_models:
    save('fine-phase-models.json', fine_models)
shutil.copy2(Path('target/gpu-pacing-fix-20261120/host.json'), output / 'host.json')
shutil.copy2(Path('docs/qualification/rtx3060ti-pacing-20261119/fixtures.json'), output / 'fixtures.json')

artifacts = []
for directory in [Path('target/gpu-pacing-fix-20261120'), Path('target/gpu-pacing-fix-20261120-engine'), Path('target/gpu-pacing-fix-20261120-final'), root]:
    for path in sorted(directory.glob('*.trace.jsonl')):
        artifacts.append({'local_path': path.as_posix(), 'bytes': path.stat().st_size, 'sha256': digest(path)})
save('local-trace-index.json', artifacts)
save('provenance.json', {'base': 'a5b98f1fe93b9f1e4f82214fc0cc83ce63b2c1bd',
                       'shader_sha256': digest(Path('src/internal/video/gpu_video.wgsl')),
                       'qualification_complete_utc': (root / 'overnight-complete.txt').read_text().strip(),
                       'source_snapshot_local': (root / 'source-snapshot').as_posix(),
                       'production_shader_override': False,
                       'physical_scanout_measured': False,
                       'unobstructed_foreground_verified': False})
print(f'Exported {len(benchmarks)} benchmark records, {len(apps)} app runs and {len(analyses)} analyses to {output}')
