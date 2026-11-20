use std::{sync::{mpsc, Condvar, Mutex}, time::{Duration, Instant}};
#[link(name="winmm")]
unsafe extern "system" { fn timeBeginPeriod(p:u32)->u32; fn timeEndPeriod(p:u32)->u32; }
fn main() {
    let (tx, rx) = mpsc::channel::<()>();
    let condvar = Condvar::new();
    let mutex = Mutex::new(());
    for (phase, timer) in [false, true, false].into_iter().enumerate() {
        if timer { assert_eq!(unsafe { timeBeginPeriod(1) }, 0); }
        for (kind, seconds) in [("sleep",0.001),("sleep",1.0/60.0),("recv",1.0/60.0),("condvar",1.0/60.0)] {
            for index in 0..100 {
                let duration=Duration::from_secs_f64(seconds);
                let start=Instant::now();
                match kind {
                    "recv" => assert!(matches!(rx.recv_timeout(duration),Err(mpsc::RecvTimeoutError::Timeout))),
                    "condvar" => { let (_guard,result)=condvar.wait_timeout(mutex.lock().unwrap(),duration).unwrap(); assert!(result.timed_out()); }
                    _ => std::thread::sleep(duration),
                }
                println!("{{\"phase\":{phase},\"timer_1ms\":{timer},\"kind\":\"{kind}\",\"requested_s\":{seconds},\"elapsed_s\":{},\"index\":{index}}}",start.elapsed().as_secs_f64());
            }
        }
        if timer { assert_eq!(unsafe { timeEndPeriod(1) }, 0); }
    }
    drop(tx);
}
