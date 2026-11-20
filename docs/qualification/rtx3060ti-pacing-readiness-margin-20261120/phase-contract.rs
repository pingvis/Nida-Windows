#[path = "../../src/internal/video/cadence.rs"]
mod cadence;
use std::time::{Duration, Instant};

#[test]
fn command_wakes_cover_every_frame_phase_and_boundary_jitter() {
    let start = Instant::now();
    let interval = Duration::from_nanos(16_666_667);
    for phase in 0..=1000 {
        for jitter_ns in [-1000_i64, 0, 1000] {
            let offset = 16_666_667_i64 * phase / 1000 + jitter_ns;
            let wake = if offset < 0 {
                start - Duration::from_nanos(offset.unsigned_abs())
            } else {
                start + Duration::from_nanos(offset as u64)
            };
            let mut deadline = start + interval;
            cadence::on_wake(&mut deadline, interval, wake);
            let expected = if wake < start + interval { start + interval } else { start + interval * 2 };
            assert_eq!(deadline, expected, "phase={phase}, jitter={jitter_ns}");
            assert_eq!(cadence::next_wait(deadline, wake), expected - wake);
        }
    }
}
