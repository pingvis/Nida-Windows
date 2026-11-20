use std::{sync::mpsc, time::{Duration, Instant}};
#[link(name="winmm")]
unsafe extern "system" { fn timeBeginPeriod(p:u32)->u32; fn timeEndPeriod(p:u32)->u32; }
fn main() {
    let (tx, rx) = mpsc::channel::<()>();
    for timer in [false, true, false] {
        if timer { assert_eq!(unsafe { timeBeginPeriod(1) }, 0); }
        for (kind, seconds) in [("sleep",0.001),("sleep",1.0/60.0),("recv",1.0/60.0)] {
            for index in 0..100 {
                let duration=Duration::from_secs_f64(seconds);
                let start=Instant::now();
                if kind=="recv" { assert!(matches!(rx.recv_timeout(duration),Err(mpsc::RecvTimeoutError::Timeout))); }
                else { std::thread::sleep(duration); }
                println!("{{\"timer_1ms\":{timer},\"kind\":\"{kind}\",\"requested_s\":{seconds},\"elapsed_s\":{},\"index\":{index}}}",start.elapsed().as_secs_f64());
            }
        }
        if timer { assert_eq!(unsafe { timeEndPeriod(1) }, 0); }
    }
    drop(tx);
}
