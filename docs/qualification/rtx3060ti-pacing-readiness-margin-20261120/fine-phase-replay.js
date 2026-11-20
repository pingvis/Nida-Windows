// CPU-only counterfactual: observed publications, ideal independent 60-Hz consumer.
// This does not model OS wake correlation, mutex blocking or physical presentation.
const fs = require('node:fs');
const crypto = require('node:crypto');
const path = process.argv[2];
if (!path) throw new Error('Usage: node fine-phase-replay.js trace.jsonl');
const raw = fs.readFileSync(path);
const rows = raw.toString('utf8').replace(/^\uFEFF/, '').trim().split(/\r?\n/)
  .map(JSON.parse).sort((a,b) => a[0]-b[0]);
const begin = rows.find(r => r[2] === 'measurement_begin')[0];
const end = rows.find(r => r[2] === 'measurement_end')[0];
const publications = rows.filter(r => r[2] === 'publish' && r[0] >= begin && r[0] <= end).map(r => r[0]);
const sweep = [];
for (let phase = 0; phase < 1e9/60; phase += 25000) {
  let cursor = 0, previous = 0, fresh = 0, ticks = 0;
  for (let tick = begin+phase; tick < end; tick = begin+phase+(++ticks)*1e9/60) {
    while (cursor < publications.length && publications[cursor] <= tick) cursor++;
    if (cursor > previous) fresh++;
    previous = cursor;
  }
  sweep.push({phase_ms:phase/1e6, fresh, ticks, fps:fresh/((end-begin)/1e9)});
}
const byFps = [...sweep].sort((a,b) => a.fps-b.fps);
console.log(JSON.stringify({model_only:true, input:path,
  input_sha256:crypto.createHash('sha256').update(raw).digest('hex'),
  assumptions:'Independent ideal consumer; ignores wake correlation, mutex blocking and scanout.',
  worst:byFps[0], best:byFps.at(-1), phases:sweep.length,
  below_599:sweep.filter(s => s.fps < 59.9).length, sweep}, null, 2));
