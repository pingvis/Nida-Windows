#![allow(dead_code)]
#[path = "../../src/internal/video/command_channel.rs"]
mod command_channel;
use std::time::{Duration, Instant};
#[repr(C)]
#[derive(Default)]
struct FileTime { low: u32, high: u32 }
impl FileTime { fn value(&self) -> u64 { (u64::from(self.high) << 32) | u64::from(self.low) } }
#[link(name="kernel32")]
unsafe extern "system" {
    fn GetCurrentProcess() -> *mut std::ffi::c_void;
    fn GetProcessTimes(process:*mut std::ffi::c_void, creation:*mut FileTime, exit:*mut FileTime, kernel:*mut FileTime, user:*mut FileTime) -> i32;
    fn QueryProcessCycleTime(process:*mut std::ffi::c_void, cycles:*mut u64) -> i32;
}
fn cpu() -> (u64,u64) {
    let (mut creation,mut exit,mut kernel,mut user)=(FileTime::default(),FileTime::default(),FileTime::default(),FileTime::default());
    let mut cycles=0;
    unsafe {
        assert_ne!(GetProcessTimes(GetCurrentProcess(),&mut creation,&mut exit,&mut kernel,&mut user),0);
        assert_ne!(QueryProcessCycleTime(GetCurrentProcess(),&mut cycles),0);
    }
    (kernel.value()+user.value(),cycles)
}
fn measure(name:&str,mut wait:impl FnMut()) {
    let before=cpu();
    let start=Instant::now();
    let mut wakes=0;
    while start.elapsed()<Duration::from_secs(5) { wait(); wakes+=1; }
    let wall=start.elapsed().as_secs_f64();
    let after=cpu();
    let cpu_ms=(after.0-before.0) as f64/10000.0;
    let cycles=after.1-before.1;
    println!("{{\"primitive\":\"{name}\",\"wall_s\":{wall},\"wakes\":{wakes},\"cpu_ms\":{cpu_ms},\"cpu_core_percent\":{},\"cycles\":{cycles},\"cycles_per_wake\":{}}}",cpu_ms/(wall*10.0),cycles/wakes);
}
fn main() {
    assert!(command_channel::NativeWait::new().is_some());
    let duration=Duration::from_secs_f64(1.0/60.0);
    for pass in 0..3 {
        let (_tx,rx)=std::sync::mpsc::channel::<()>();
        measure(&format!("mpsc-{pass}"),||assert_eq!(rx.recv_timeout(duration),Err(std::sync::mpsc::RecvTimeoutError::Timeout)));
        let (_tx,rx)=command_channel::channel::<()>();
        measure(&format!("native-{pass}"),||assert_eq!(rx.recv_timeout(duration),Err(std::sync::mpsc::RecvTimeoutError::Timeout)));
    }
}
