// Counterfactual only: fixed measured publications and consumer starts, one clip.
const fs=require('node:fs'),crypto=require('node:crypto');
const file=process.argv[2]; if(!file)throw Error('Pass original app-seek-phase-0.trace.jsonl');
const raw=fs.readFileSync(file);const rows=raw.toString('utf8').replace(/^\uFEFF/,'').trim().split(/\r?\n/).map(JSON.parse).sort((a,b)=>a[0]-b[0]);
const begin=rows.find(r=>r[2]==='encode'&&r[3]>=47.002)[0],end=begin+8e9;
const pubs=rows.filter(r=>r[2]==='publish').map(r=>r[0]);
const takes=rows.filter(r=>r[2]==='take_ready'&&r[0]>=begin&&r[0]<=end);
const upper=t=>{let l=0,h=pubs.length;while(l<h){let m=(l+h)>>>1;if(pubs[m]<=t)l=m+1;else h=m;}return l;};
const sweep=[0,.25,.5,1].map(ms=>{let previous=upper(begin-1),fresh=0,waits=[];for(const r of takes){let current=upper(r[0]);if(current<=previous&&current<pubs.length&&pubs[current]<=r[0]+ms*1e6){waits.push((pubs[current]-r[0])/1e6);current++;}if(current>previous)fresh++;previous=current;}return{budget_ms:ms,fresh,empty:takes.length-fresh,waits:waits.length,max_wait_ms:Math.max(0,...waits)};});
const actual=takes.filter(r=>r[3]===1).length;
if(sweep[0].fresh!==actual)throw Error('Zero-wait replay does not reproduce original ownership');
console.log(JSON.stringify({model_only:true,input:file,input_sha256:crypto.createHash('sha256').update(raw).digest('hex'),roi_pts:47.002,seconds:8,takes:takes.length,actual_fresh_takes:actual,sweep,limits:'Fixed producer and tick starts; ignores scheduling feedback, changed GPU lifetime timing and scanout. Must verify in actual app.'},null,2));
