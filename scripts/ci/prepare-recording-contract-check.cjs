// Generates a small test crate from Nida's actual value types, recording plan,
// and plan tests. This does not replace building/testing the full application.
// Run from the repository root; see docs/windows-gpu-recording.md.
const fs = require('fs'), path = require('path');
const root = process.cwd(), out = path.join(root, 'target/performance-investigation/nvenc-harness');
fs.mkdirSync(path.join(out,'src'),{recursive:true});
fs.writeFileSync(path.join(out,'Cargo.toml'), `[package]\nname="nida-recording-contract-check"\nversion="0.0.0"\nedition="2021"\n[dependencies]\nserde={version="1",features=["derive"]}\nserde_json="1"\nutoipa="5"\nlog="0.4"\n`);
const source=fs.readFileSync('src/internal/renderer/subprocess.rs','utf8').replace(/\r\n/g,'\n');
const readback=fs.readFileSync('src/internal/renderer/readback.rs','utf8').replace(/\r\n/g,'\n');
function between(text,start,end) {
  const first=text.indexOf(start), last=text.indexOf(end,first);
  if(first<0 || last<0) throw new Error('Source layout changed; update harness boundaries: '+start);
  return text.slice(first,last);
}
const rb=between(readback,'pub enum ReadbackFormat','\nimpl ReadbackFormat');
const block=between(source,'struct RecordingPlan','#[derive(Debug, Clone, Copy, Default)]\n// Each flag');
const tests=between(source,'    fn ten_bit_request()','    #[test]\n    fn hap_family_ten_bit_request');
const header=`#![allow(dead_code)]\n#[path=${JSON.stringify(path.join(root,'src/engine/value/render.rs').replaceAll('\\','/'))}]\nmod render;\nuse render::*;\nuse std::process::{Command,Stdio};\nuse std::sync::atomic::Ordering;\n#[derive(Debug,Clone,Copy,PartialEq,Eq)]\n${rb}\n#[derive(Debug,Clone)]\n`;
const hardware=`
#[test]
fn real_nvenc_dual_1080p50_and_main10() {
  warm_recording_encoders();
  assert!(nvenc_recording_available("h264_nvenc","nv12"));
  assert!(nvenc_recording_available("hevc_nvenc","nv12"));
  assert!(nvenc_recording_available("hevc_nvenc","p010le"));
  let folder=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../nvenc-check-files");
  std::fs::create_dir_all(&folder).unwrap();
  let mut children=Vec::new();
  for (encoder,pixel,input) in [("h264_nvenc","nv12","nv12"),("hevc_nvenc","nv12","nv12"),("hevc_nvenc","p010le","x2bgr10le")] {
    let file=folder.join(format!("{encoder}-{pixel}.mp4"));
    let mut cmd=Command::new("ffmpeg");
    cmd.args(["-hide_banner","-loglevel","error","-y","-re","-f","lavfi","-i", &format!("testsrc2=size=1920x1080:rate=50,format={input}"),"-frames:v","150"])
       .args(nvenc_recording_args(encoder,pixel)).arg(&file).stdin(Stdio::null()).stdout(Stdio::null());
    children.push((cmd.spawn().unwrap(),file,pixel));
  }
  for (mut child,file,pixel) in children {
    assert!(child.wait().unwrap().success(),"{file:?}");
    let probe=Command::new("ffprobe").args(["-v","error","-show_entries","stream=codec_name,profile,pix_fmt,r_frame_rate,nb_frames","-of","json"]).arg(file).output().unwrap();
    assert!(probe.status.success());
    let info:serde_json::Value=serde_json::from_slice(&probe.stdout).unwrap();
    let stream=&info["streams"][0];
    assert_eq!(stream["r_frame_rate"],"50/1"); assert_eq!(stream["nb_frames"],"150");
    if pixel=="p010le" { assert_eq!(stream["profile"],"Main 10"); }
    println!("{}",info);
  }
}
`;
fs.writeFileSync(path.join(out,'src/lib.rs'),header+block+'\n#[cfg(test)] mod contract_tests { use super::*;\n'+tests+hardware+'\n}\n');
console.log('Prepared source-extracted recording checks: ' + path.relative(root,out));
