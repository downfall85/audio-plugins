use nih_plug::prelude::*;
use std::sync::Arc;

mod editor;

/// Pure DSP struct for the noise gate — extracted for unit testing.
pub struct NoiseGateDsp {
    sample_rate: f32,
    threshold_linear: f32,
    attack_coeff: f32,
    release_coeff: f32,
    envelope: f32,
}

impl NoiseGateDsp {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            threshold_linear: nih_plug::util::db_to_gain(-40.0),
            attack_coeff: Self::attack_coeff(sample_rate, 5.0),
            release_coeff: Self::release_coeff(sample_rate, 100.0),
            envelope: 0.0,
        }
    }

    fn attack_coeff(sample_rate: f32, attack_ms: f32) -> f32 {
        1.0 - (-1.0 / (sample_rate * attack_ms * 0.001)).exp()
    }

    fn release_coeff(sample_rate: f32, release_ms: f32) -> f32 {
        (-1.0 / (sample_rate * release_ms * 0.001)).exp()
    }

    pub fn set_threshold(&mut self, threshold_linear: f32) {
        self.threshold_linear = threshold_linear;
    }

    pub fn set_attack_ms(&mut self, attack_ms: f32) {
        self.attack_coeff = Self::attack_coeff(self.sample_rate, attack_ms);
    }

    pub fn set_release_ms(&mut self, release_ms: f32) {
        self.release_coeff = Self::release_coeff(self.sample_rate, release_ms);
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }

    /// Returns the gate gain (0.0 or 1.0) for the given input sample.
    pub fn process(&mut self, sample: f32) -> f32 {
        let level = sample.abs();
        if level >= self.envelope {
            self.envelope = self.envelope * (1.0 - self.attack_coeff) + level * self.attack_coeff;
        } else {
            self.envelope *= self.release_coeff;
        }
        if self.envelope > self.threshold_linear {
            1.0
        } else {
            0.0
        }
    }

    pub fn reset(&mut self) {
        self.envelope = 0.0;
    }

    pub fn envelope(&self) -> f32 {
        self.envelope
    }
}

#[derive(Params)]
pub struct NoiseGateParams {
    #[persist = "editor-state"]
    editor_state: Arc<nih_plug_iced::IcedState>,

    #[id = "threshold"]
    pub threshold: FloatParam,

    #[id = "attack_ms"]
    pub attack_ms: FloatParam,

    #[id = "release_ms"]
    pub release_ms: FloatParam,
}

impl Default for NoiseGateParams {
    fn default() -> Self {
        Self {
            editor_state: editor::default_state(),
            threshold: FloatParam::new(
                "Threshold",
                -40.0,
                FloatRange::Linear { min: -80.0, max: 0.0 },
            )
            .with_unit(" dB")
            .with_smoother(SmoothingStyle::Linear(5.0)),
            attack_ms: FloatParam::new(
                "Attack",
                5.0,
                FloatRange::Linear { min: 0.1, max: 50.0 },
            )
            .with_unit(" ms")
            .with_smoother(SmoothingStyle::Linear(5.0)),
            release_ms: FloatParam::new(
                "Release",
                100.0,
                FloatRange::Linear { min: 10.0, max: 500.0 },
            )
            .with_unit(" ms")
            .with_smoother(SmoothingStyle::Linear(5.0)),
        }
    }
}

pub struct NoiseGate {
    params: Arc<NoiseGateParams>,
    sample_rate: f32,
    /// Per-channel envelope followers.
    envelopes: Vec<f32>,
}

impl Default for NoiseGate {
    fn default() -> Self {
        Self {
            params: Arc::new(NoiseGateParams::default()),
            sample_rate: 1.0,
            envelopes: vec![0.0; 2],
        }
    }
}

impl Plugin for NoiseGate {
    const NAME: &'static str = "Noise Gate";
    const VENDOR: &'static str = "Audio Plugins";
    const URL: &'static str = "https://github.com/example/audio-plugins";
    const EMAIL: &'static str = "example@example.com";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: NonZeroU32::new(2),
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::None;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(self.params.clone(), self.params.editor_state.clone())
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate;
        let num_channels = 2;
        self.envelopes.resize(num_channels, 0.0);
        true
    }

    fn reset(&mut self) {
        for env in self.envelopes.iter_mut() {
            *env = 0.0;
        }
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let sample_rate = self.sample_rate;

        // Process sample by sample across all channels
        for mut channel_samples in buffer.iter_samples() {
            // Grab per-sample smoothed values
            let threshold_db = self.params.threshold.smoothed.next();
            let threshold_linear = nih_plug::util::db_to_gain(threshold_db);
            let attack_ms = self.params.attack_ms.smoothed.next();
            let release_ms = self.params.release_ms.smoothed.next();
            let attack_coeff = 1.0 - (-1.0_f32 / (sample_rate * attack_ms * 0.001)).exp();
            let release_coeff = (-1.0_f32 / (sample_rate * release_ms * 0.001)).exp();

            for (ch, sample) in channel_samples.iter_mut().enumerate() {
                // Ensure we have an envelope entry for this channel.
                if ch >= self.envelopes.len() {
                    self.envelopes.resize(ch + 1, 0.0);
                }

                let level = sample.abs();
                let env = &mut self.envelopes[ch];
                if level >= *env {
                    *env = *env * (1.0 - attack_coeff) + level * attack_coeff;
                } else {
                    *env *= release_coeff;
                }

                let gain = if *env > threshold_linear { 1.0_f32 } else { 0.0_f32 };
                *sample *= gain;
            }
        }

        ProcessStatus::Normal
    }
}

impl Vst3Plugin for NoiseGate {
    const VST3_CLASS_ID: [u8; 16] = *b"NoiseGatePlg0000";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Dynamics];
}

nih_export_vst3!(NoiseGate);

#[cfg(test)]
mod tests {
    use super::*;
    use nih_plug::util;

    fn make_gate(sample_rate: f32, threshold_db: f32, attack_ms: f32, release_ms: f32) -> NoiseGateDsp {
        let mut gate = NoiseGateDsp::new(sample_rate);
        gate.set_threshold(util::db_to_gain(threshold_db));
        gate.set_attack_ms(attack_ms);
        gate.set_release_ms(release_ms);
        gate
    }

    #[test]
    fn test_gate_opens_above_threshold() {
        let mut gate = make_gate(44100.0, -40.0, 0.1, 100.0);
        let input = util::db_to_gain(-20.0); // 20 dB above threshold
        let gain = gate.process(input);
        // After one sample with a fast attack, envelope should be above threshold → gate open
        assert!(gain > 0.0, "gate should be opening (got {})", gain);
    }

    #[test]
    fn test_gate_closes_below_threshold() {
        let mut gate = make_gate(44100.0, -40.0, 0.1, 100.0);
        // Prime the envelope above threshold
        for _ in 0..1000 {
            gate.process(util::db_to_gain(-20.0));
        }
        // Now feed silence for long enough for release
        let mut last_gain = 1.0;
        for _ in 0..100_000 {
            last_gain = gate.process(0.0);
        }
        assert_eq!(last_gain, 0.0, "gate should be fully closed after long silence");
    }

    #[test]
    fn test_attack_is_gradual() {
        // With a slow attack the envelope should NOT immediately reach the input level
        let mut gate = make_gate(44100.0, -40.0, 50.0, 100.0); // 50 ms attack
        let input = util::db_to_gain(-20.0);
        gate.process(input); // one sample
        // envelope after one sample must be less than the input level
        assert!(
            gate.envelope() < input,
            "envelope should still be below input after 1 sample (gradual attack)"
        );
    }

    #[test]
    fn test_release_is_gradual() {
        let mut gate = make_gate(44100.0, -40.0, 0.1, 500.0); // 500 ms release
        // Prime envelope
        for _ in 0..5000 {
            gate.process(util::db_to_gain(-20.0));
        }
        let env_before = gate.envelope();
        gate.process(0.0); // one silent sample
        let env_after = gate.envelope();
        // Envelope should have decayed a tiny bit but not to zero
        assert!(env_after > 0.0, "envelope should not instantly drop to zero");
        assert!(env_after < env_before, "envelope should be decaying");
    }

    #[test]
    fn test_silence_produces_zero() {
        // Gate closed → multiply by 0 → output zero
        let mut gate = make_gate(44100.0, -40.0, 0.1, 100.0);
        // Envelope starts at 0, which is below threshold → gate is closed
        let gain = gate.process(util::db_to_gain(-20.0));
        // With a very fast attack (0.1 ms) but the envelope starts at 0, after 1 sample
        // the envelope may or may not exceed the threshold depending on values.
        // What matters: if envelope <= threshold → gain = 0.
        // For this test we verify the closed state explicitly:
        let mut gate2 = make_gate(44100.0, -40.0, 50.0, 100.0); // slow attack
        let gain2 = gate2.process(0.0); // silence
        assert_eq!(gain2, 0.0, "silence with gate closed should yield zero gain");
    }

    #[test]
    fn test_coefficients_recomputed_at_48000() {
        let sr = 48000.0_f32;
        let mut gate = NoiseGateDsp::new(sr);
        gate.set_threshold(nih_plug::util::db_to_gain(-40.0));
        gate.set_attack_ms(5.0);
        gate.set_release_ms(100.0);
        // Verify the envelope advances differently than at 44100 Hz
        let input = nih_plug::util::db_to_gain(-20.0);
        gate.process(input);
        let env_48k = gate.envelope();

        let mut gate_44k = NoiseGateDsp::new(44100.0);
        gate_44k.set_threshold(nih_plug::util::db_to_gain(-40.0));
        gate_44k.set_attack_ms(5.0);
        gate_44k.set_release_ms(100.0);
        gate_44k.process(input);
        let env_44k = gate_44k.envelope();

        // At higher sample rate the per-sample coefficient is smaller → envelope rises more slowly
        assert!(
            env_48k < env_44k,
            "at 48kHz one sample should advance the envelope less than at 44.1kHz"
        );
    }
}
