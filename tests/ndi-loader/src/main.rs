//! Native probe for a separately installed NDI runtime; no network discovery,
//! video/audio capture, settings changes or DLL copying.
fn main() {
    let sdk = nida_ndi_loader_checks::sdk::NdiSdk::load().expect("NDI runtime did not load");
    // SAFETY: the production loader resolved these lifecycle functions and owns
    // the library until after destroy; this probe creates no other NDI objects.
    unsafe {
        assert!((sdk.initialize)(), "NDI runtime initialization failed");
        println!("Loaded and initialized {}", sdk.runtime_version);
        (sdk.destroy)();
    }
}
