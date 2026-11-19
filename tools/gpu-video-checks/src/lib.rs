//! CPU-only tests of the production colour parameters and resource ownership.
//! This harness never creates a GPU device or runs an application.
#![allow(dead_code)]

#[path = "../../../src/internal/video/delivery.rs"]
mod delivery;
#[path = "../../../src/internal/video/frame_mailbox.rs"]
mod frame_mailbox;
#[path = "../../../src/internal/video/gpu_colour.rs"]
mod gpu_colour;
#[path = "../../../src/internal/video/gpu_pool.rs"]
mod gpu_pool;
#[path = "../../../src/internal/video/hdr.rs"]
mod hdr;

#[test]
fn production_video_shader_validates_without_a_gpu() {
    let source = include_str!("../../../src/internal/video/gpu_video.wgsl");
    let module = naga::front::wgsl::parse_str(source)
        .unwrap_or_else(|error| panic!("{}", error.emit_to_string(source)));
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .unwrap();
}
