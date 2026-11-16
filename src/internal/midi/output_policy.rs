//! Only control outputs belong in automatic MIDI controller discovery.

pub(super) fn is_controller_output(name: &str) -> bool {
    // The Windows built-in GM synthesizer is a sound generator, not a feedback
    // port. Opening/resetting it on short-lived discovery workers crashed in
    // wdmaud's CMicrosoftGSWavetableSynth (both retained audio-test dumps).
    // Keep real controllers and virtual MIDI routing ports available.
    !cfg!(target_os = "windows") || !name.eq_ignore_ascii_case("Microsoft GS Wavetable Synth")
}

#[cfg(test)]
mod tests {
    use super::is_controller_output;

    #[test]
    fn windows_builtin_synth_is_not_opened_as_a_controller() {
        for name in [
            "Microsoft GS Wavetable Synth",
            "MICROSOFT GS WAVETABLE SYNTH",
        ] {
            assert_eq!(is_controller_output(name), !cfg!(target_os = "windows"));
        }
    }

    #[test]
    fn hardware_and_virtual_control_outputs_remain_available() {
        for name in [
            "APC MINI",
            "Tascam Model 12 MIDI Out",
            "loopMIDI Port",
            "ą MIDI",
        ] {
            assert!(is_controller_output(name), "{name}");
        }
    }
}
