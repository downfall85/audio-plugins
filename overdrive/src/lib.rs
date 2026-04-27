use nih_plug::prelude::*;
use std::sync::Arc;

mod editor;

// ---------------------------------------------------------------------------
// Biquad filter — direct form II transposed
// ---------------------------------------------------------------------------

#[derive(Clone, Default)]
struct BiquadState {
    s1: f32,
    s2: f32,
}

impl BiquadState {
    fn reset(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
    }

    #[inline]
    fn process(&mut self, x: f32, b0: f32, b1: f32, b2: f32, a1: f32, a2: f32) -> f32 {
        let y = b0 * x + self.s1;
        self.s1 = b1 * x - a1 * y + self.s2;
        self.s2 = b2 * x - a2 * y;
        y
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct BiquadCoeffs {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

impl BiquadCoeffs {
    fn highpass(freq: f32, sample_rate: f32) -> Self {
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0_f32.sqrt());
        let b0 = (1.0 + cos_w0) / 2.0;
        let b1 = -(1.0 + cos_w0);
        let b2 = (1.0 + cos_w0) / 2.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;
        Self { b0: b0 / a0, b1: b1 / a0, b2: b2 / a0, a1: a1 / a0, a2: a2 / a0 }
    }

    fn lowpass(freq: f32, sample_rate: f32) -> Self {
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0_f32.sqrt());
        let b0 = (1.0 - cos_w0) / 2.0;
        let b1 = 1.0 - cos_w0;
        let b2 = (1.0 - cos_w0) / 2.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;
        Self { b0: b0 / a0, b1: b1 / a0, b2: b2 / a0, a1: a1 / a0, a2: a2 / a0 }
    }
}

// ---------------------------------------------------------------------------
// Pure DSP — extracted for unit testing
// ---------------------------------------------------------------------------

pub struct OverdriveDsp {
    /// Fixed high-pass at 100 Hz: removes sub-bass before clipping to avoid
    /// low-frequency intermodulation and mud.
    pre_hp: BiquadState,
    /// Variable low-pass (Tone): shapes brightness after clipping.
    tone_lp: BiquadState,
}

impl OverdriveDsp {
    pub fn new() -> Self {
        Self {
            pre_hp: BiquadState::default(),
            tone_lp: BiquadState::default(),
        }
    }

    pub fn reset(&mut self) {
        self.pre_hp.reset();
        self.tone_lp.reset();
    }

    /// Process one sample through the full overdrive chain.
    ///
    /// Signal flow:
    ///   Input → HP filter (100 Hz) → drive gain → tanh soft-clip → LP tone filter → output gain
    ///   Mixed with dry signal according to `mix` (0.0 = full dry, 1.0 = full wet).
    #[inline]
    pub fn process(
        &mut self,
        input: f32,
        drive_linear: f32,
        hp_coeffs: &BiquadCoeffs,
        lp_coeffs: &BiquadCoeffs,
        output_gain: f32,
        mix: f32,
    ) -> f32 {
        // Remove low-end before clipping to keep the saturation musical
        let filtered = self.pre_hp.process(
            input,
            hp_coeffs.b0, hp_coeffs.b1, hp_coeffs.b2,
            hp_coeffs.a1, hp_coeffs.a2,
        );

        // Drive into soft clipper — tanh produces smooth, tube-like odd harmonics
        let clipped = (filtered * drive_linear).tanh();

        // Tone control rolls off harshness post-clip
        let toned = self.tone_lp.process(
            clipped,
            lp_coeffs.b0, lp_coeffs.b1, lp_coeffs.b2,
            lp_coeffs.a1, lp_coeffs.a2,
        );

        // Output gain trim + dry/wet blend
        let wet = toned * output_gain;
        input * (1.0 - mix) + wet * mix
    }
}

// ---------------------------------------------------------------------------
// Plugin params
// ---------------------------------------------------------------------------

#[derive(Params)]
pub struct OverdriveParams {
    #[persist = "editor-state"]
    editor_state: Arc<nih_plug_iced::IcedState>,

    /// Amount of gain pushed into the soft clipper (dB).
    /// Low values add gentle warmth; high values saturate heavily.
    #[id = "drive"]
    pub drive: FloatParam,

    /// Low-pass cutoff applied after clipping (Hz).
    /// Turn left for dark/warm, right for bright/biting.
    #[id = "tone"]
    pub tone: FloatParam,

    /// Post-saturation output level (dB). Use to compensate for the volume
    /// increase caused by high drive settings.
    #[id = "output"]
    pub output: FloatParam,

    /// Dry/wet blend (%). 0% = clean bypass, 100% = fully driven.
    #[id = "mix"]
    pub mix: FloatParam,
}

impl Default for OverdriveParams {
    fn default() -> Self {
        Self {
            editor_state: editor::default_state(),
            drive: FloatParam::new(
                "Drive",
                10.0,
                FloatRange::Linear { min: 0.0, max: 40.0 },
            )
            .with_unit(" dB")
            .with_smoother(SmoothingStyle::Linear(5.0)),
            tone: FloatParam::new(
                "Tone",
                3000.0,
                FloatRange::Skewed {
                    min: 500.0,
                    max: 8000.0,
                    factor: FloatRange::skew_factor(-1.0),
                },
            )
            .with_unit(" Hz")
            .with_smoother(SmoothingStyle::Linear(10.0)),
            output: FloatParam::new(
                "Output",
                -6.0,
                FloatRange::Linear { min: -20.0, max: 0.0 },
            )
            .with_unit(" dB")
            .with_smoother(SmoothingStyle::Linear(5.0)),
            mix: FloatParam::new(
                "Mix",
                100.0,
                FloatRange::Linear { min: 0.0, max: 100.0 },
            )
            .with_unit(" %")
            .with_smoother(SmoothingStyle::Linear(5.0)),
        }
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct Overdrive {
    params: Arc<OverdriveParams>,
    sample_rate: f32,
    channels: Vec<OverdriveDsp>,
    /// Cached 100 Hz HP coefficients — recomputed only on sample-rate change.
    hp_coeffs: BiquadCoeffs,
    /// Cached tone LP coefficients — recomputed each sample when Tone moves.
    lp_coeffs: BiquadCoeffs,
}

impl Default for Overdrive {
    fn default() -> Self {
        let sr = 44100.0;
        let params = OverdriveParams::default();
        Self {
            hp_coeffs: BiquadCoeffs::highpass(100.0, sr),
            lp_coeffs: BiquadCoeffs::lowpass(params.tone.default_plain_value(), sr),
            params: Arc::new(params),
            sample_rate: sr,
            channels: vec![OverdriveDsp::new(), OverdriveDsp::new()],
        }
    }
}

impl Plugin for Overdrive {
    const NAME: &'static str = "Overdrive";
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
        self.channels = vec![OverdriveDsp::new(), OverdriveDsp::new()];
        self.hp_coeffs = BiquadCoeffs::highpass(100.0, self.sample_rate);
        self.lp_coeffs = BiquadCoeffs::lowpass(self.params.tone.value(), self.sample_rate);
        true
    }

    fn reset(&mut self) {
        for ch in self.channels.iter_mut() {
            ch.reset();
        }
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        for mut channel_samples in buffer.iter_samples() {
            let drive_db = self.params.drive.smoothed.next();
            let drive_linear = nih_plug::util::db_to_gain(drive_db);
            let tone_freq = self.params.tone.smoothed.next();
            let output_db = self.params.output.smoothed.next();
            let output_gain = nih_plug::util::db_to_gain(output_db);
            let mix = self.params.mix.smoothed.next() / 100.0;

            self.lp_coeffs = BiquadCoeffs::lowpass(tone_freq, self.sample_rate);

            for (ch_idx, sample) in channel_samples.iter_mut().enumerate() {
                while ch_idx >= self.channels.len() {
                    self.channels.push(OverdriveDsp::new());
                }
                *sample = self.channels[ch_idx].process(
                    *sample,
                    drive_linear,
                    &self.hp_coeffs,
                    &self.lp_coeffs,
                    output_gain,
                    mix,
                );
            }
        }

        ProcessStatus::Normal
    }
}

impl Vst3Plugin for Overdrive {
    const VST3_CLASS_ID: [u8; 16] = *b"OverdrivePlg0000";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Distortion];
}

nih_export_vst3!(Overdrive);

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 44100.0;

    /// HP near DC and LP near Nyquist — effectively transparent filters for testing the clipper.
    fn bypass_coeffs() -> (BiquadCoeffs, BiquadCoeffs) {
        (BiquadCoeffs::highpass(10.0, SR), BiquadCoeffs::lowpass(20000.0, SR))
    }

    #[test]
    fn silence_in_silence_out() {
        let (hp, lp) = bypass_coeffs();
        let mut dsp = OverdriveDsp::new();
        let out = dsp.process(0.0, 1.0, &hp, &lp, 1.0, 1.0);
        assert_eq!(out, 0.0);
    }

    #[test]
    fn soft_clip_limits_output() {
        // tanh saturates → output never exceeds 1.0 regardless of drive
        let (hp, lp) = bypass_coeffs();
        let mut dsp = OverdriveDsp::new();
        let out = dsp.process(100.0, 1000.0, &hp, &lp, 1.0, 1.0);
        assert!(out.abs() <= 1.01, "soft clipper must limit output, got {}", out);
    }

    #[test]
    fn tanh_is_near_linear_for_small_signals() {
        // For small x, tanh(x) ≈ x within 0.01% — the soft clipper is transparent
        // at low levels. Test the clipping function directly, independent of filters.
        let x = 0.01_f32;
        let out = x.tanh();
        assert!(
            (out - x).abs() < 0.0001 * x + 1e-7,
            "tanh should be near-linear for small signals; tanh({})={}",
            x, out
        );
    }

    #[test]
    fn mix_zero_returns_dry_signal() {
        let (hp, lp) = bypass_coeffs();
        let mut dsp = OverdriveDsp::new();
        let input = 0.5_f32;
        let out = dsp.process(input, 100.0, &hp, &lp, 1.0, 0.0);
        assert!((out - input).abs() < 1e-6, "mix=0 must return exact dry signal, got {}", out);
    }

    #[test]
    fn high_drive_saturates_signal() {
        // At extreme drive the clipper should flatten the wave; output far from input
        let (hp, lp) = bypass_coeffs();
        let mut dsp = OverdriveDsp::new();
        let input = 0.5_f32;
        let out = dsp.process(input, 1000.0, &hp, &lp, 1.0, 1.0);
        // tanh(500) ≈ 1.0, very different from 0.5
        assert!(
            (out - input).abs() > 0.3,
            "high drive should saturate signal, got out={}",
            out
        );
    }

    #[test]
    fn output_gain_scales_wet_signal() {
        let (hp, lp) = bypass_coeffs();
        let mut dsp1 = OverdriveDsp::new();
        let mut dsp2 = OverdriveDsp::new();
        let input = 0.1_f32;
        let out1 = dsp1.process(input, 2.0, &hp, &lp, 1.0, 1.0);
        let out2 = dsp2.process(input, 2.0, &hp, &lp, 2.0, 1.0);
        assert!(
            (out2 - 2.0 * out1).abs() < 1e-5,
            "output gain should scale wet signal; out1={}, out2={}",
            out1, out2
        );
    }

    #[test]
    fn tone_lp_attenuates_high_freq() {
        // Drive a 10kHz sine through a 500Hz LP tone → strong attenuation
        let hp = BiquadCoeffs::highpass(10.0, SR);
        let lp = BiquadCoeffs::lowpass(500.0, SR);
        let mut dsp = OverdriveDsp::new();

        let freq = 10000.0_f32;
        let mut peak = 0.0_f32;
        for i in 0..4000 {
            let x = (2.0 * std::f32::consts::PI * freq / SR * i as f32).sin();
            let y = dsp.process(x, 1.0, &hp, &lp, 1.0, 1.0);
            if i > 2000 {
                peak = peak.max(y.abs());
            }
        }
        assert!(peak < 0.1, "500Hz LP must attenuate 10kHz, got peak={}", peak);
    }

    #[test]
    fn pre_hp_attenuates_dc() {
        // HP at 100Hz must block DC (0 Hz) before the clipper
        let hp = BiquadCoeffs::highpass(100.0, SR);
        let lp = BiquadCoeffs::lowpass(20000.0, SR);
        let mut dsp = OverdriveDsp::new();
        let mut out = 0.0_f32;
        for _ in 0..10_000 {
            out = dsp.process(1.0, 1.0, &hp, &lp, 1.0, 1.0);
        }
        assert!(out.abs() < 0.01, "HP filter must block DC, got {}", out);
    }
}
