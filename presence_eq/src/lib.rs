use nih_plug::prelude::*;
use std::sync::Arc;

mod editor;

// ---------------------------------------------------------------------------
// Biquad filter — direct form II transposed
// ---------------------------------------------------------------------------

/// Second-order IIR filter state (per channel).
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

/// Biquad coefficients.
#[derive(Clone, Copy, Default)]
pub struct BiquadCoeffs {
    pub b0: f32,
    pub b1: f32,
    pub b2: f32,
    pub a1: f32,
    pub a2: f32,
}

impl BiquadCoeffs {
    /// 2nd-order Butterworth high-pass filter.
    pub fn highpass(freq: f32, sample_rate: f32) -> Self {
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        // Q = 1/sqrt(2) for Butterworth
        let alpha = sin_w0 / (2.0_f32.sqrt());

        let b0 = (1.0 + cos_w0) / 2.0;
        let b1 = -(1.0 + cos_w0);
        let b2 = (1.0 + cos_w0) / 2.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;

        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }

    /// 2nd-order Butterworth low-pass filter.
    pub fn lowpass(freq: f32, sample_rate: f32) -> Self {
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

        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }

    /// Peaking EQ bell boost/cut (RBJ cookbook).
    pub fn peaking(freq: f32, gain_db: f32, q: f32, sample_rate: f32) -> Self {
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let a_amp = 10.0_f32.powf(gain_db / 40.0); // sqrt of linear gain
        let alpha = sin_w0 / (2.0 * q);

        let b0 = 1.0 + alpha * a_amp;
        let b1 = -2.0 * cos_w0;
        let b2 = 1.0 - alpha * a_amp;
        let a0 = 1.0 + alpha / a_amp;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha / a_amp;

        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }
}

// ---------------------------------------------------------------------------
// Per-channel DSP chain — extracted for unit testing
// ---------------------------------------------------------------------------

pub struct PresenceEqDsp {
    hpf: BiquadState,
    bell: BiquadState,
    lpf: BiquadState,
}

impl PresenceEqDsp {
    pub fn new() -> Self {
        Self {
            hpf: BiquadState::default(),
            bell: BiquadState::default(),
            lpf: BiquadState::default(),
        }
    }

    pub fn reset(&mut self) {
        self.hpf.reset();
        self.bell.reset();
        self.lpf.reset();
    }

    #[inline]
    pub fn process(
        &mut self,
        input: f32,
        hpf_c: &BiquadCoeffs,
        bell_c: &BiquadCoeffs,
        lpf_c: &BiquadCoeffs,
        output_gain: f32,
    ) -> f32 {
        let x = self.hpf.process(input, hpf_c.b0, hpf_c.b1, hpf_c.b2, hpf_c.a1, hpf_c.a2);
        let x = self.bell.process(x, bell_c.b0, bell_c.b1, bell_c.b2, bell_c.a1, bell_c.a2);
        let x = self.lpf.process(x, lpf_c.b0, lpf_c.b1, lpf_c.b2, lpf_c.a1, lpf_c.a2);
        x * output_gain
    }
}

// ---------------------------------------------------------------------------
// Plugin params
// ---------------------------------------------------------------------------

#[derive(Params)]
pub struct PresenceEqParams {
    #[persist = "editor-state"]
    editor_state: Arc<nih_plug_iced::IcedState>,

    /// High-pass cutoff frequency (Hz) — rolls off sub-bass.
    #[id = "hp_freq"]
    pub hp_freq: FloatParam,

    /// Low-pass cutoff frequency (Hz) — rolls off harshness.
    #[id = "lp_freq"]
    pub lp_freq: FloatParam,

    /// Center frequency of the mid bell boost (Hz).
    #[id = "mid_freq"]
    pub mid_freq: FloatParam,

    /// Bell boost amount (dB).
    #[id = "mid_gain"]
    pub mid_gain: FloatParam,

    /// Bell Q — bandwidth. Higher = narrower boost.
    #[id = "mid_q"]
    pub mid_q: FloatParam,

    /// Post-EQ output gain trim (dB).
    #[id = "output_gain"]
    pub output_gain: FloatParam,
}

impl Default for PresenceEqParams {
    fn default() -> Self {
        Self {
            editor_state: editor::default_state(),
            hp_freq: FloatParam::new(
                "HP Freq",
                80.0,
                FloatRange::Skewed { min: 50.0, max: 120.0, factor: FloatRange::skew_factor(-1.0) },
            )
            .with_unit(" Hz")
            .with_smoother(SmoothingStyle::Linear(10.0)),
            lp_freq: FloatParam::new(
                "LP Freq",
                7500.0,
                FloatRange::Skewed { min: 4000.0, max: 7500.0, factor: FloatRange::skew_factor(-1.0) },
            )
            .with_unit(" Hz")
            .with_smoother(SmoothingStyle::Linear(10.0)),
            mid_freq: FloatParam::new(
                "Mid Freq",
                300.0,
                FloatRange::Skewed { min: 100.0, max: 700.0, factor: FloatRange::skew_factor(-1.0) },
            )
            .with_unit(" Hz")
            .with_smoother(SmoothingStyle::Linear(10.0)),
            mid_gain: FloatParam::new(
                "Mid Gain",
                3.0,
                FloatRange::Linear { min: 0.0, max: 12.0 },
            )
            .with_unit(" dB")
            .with_smoother(SmoothingStyle::Linear(5.0)),
            mid_q: FloatParam::new(
                "Mid Q",
                1.0,
                FloatRange::Skewed { min: 0.5, max: 4.0, factor: FloatRange::skew_factor(1.0) },
            )
            .with_smoother(SmoothingStyle::Linear(5.0)),
            output_gain: FloatParam::new(
                "Output Gain",
                0.0,
                FloatRange::Linear { min: -12.0, max: 12.0 },
            )
            .with_unit(" dB")
            .with_smoother(SmoothingStyle::Linear(5.0)),
        }
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct PresenceEq {
    params: Arc<PresenceEqParams>,
    sample_rate: f32,
    channels: Vec<PresenceEqDsp>,
    // Cached coefficients — recomputed when params change.
    hpf_coeffs: BiquadCoeffs,
    bell_coeffs: BiquadCoeffs,
    lpf_coeffs: BiquadCoeffs,
}

impl Default for PresenceEq {
    fn default() -> Self {
        let sr = 44100.0;
        let params = PresenceEqParams::default();
        Self {
            hpf_coeffs: BiquadCoeffs::highpass(params.hp_freq.default_plain_value(), sr),
            bell_coeffs: BiquadCoeffs::peaking(
                params.mid_freq.default_plain_value(),
                params.mid_gain.default_plain_value(),
                params.mid_q.default_plain_value(),
                sr,
            ),
            lpf_coeffs: BiquadCoeffs::lowpass(params.lp_freq.default_plain_value(), sr),
            params: Arc::new(params),
            sample_rate: sr,
            channels: vec![PresenceEqDsp::new(), PresenceEqDsp::new()],
        }
    }
}

impl Plugin for PresenceEq {
    const NAME: &'static str = "Presence EQ";
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
        self.channels = vec![PresenceEqDsp::new(), PresenceEqDsp::new()];
        self.recompute_coeffs();
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
            // Read smoothed param values once per sample-frame
            let hp_freq = self.params.hp_freq.smoothed.next();
            let lp_freq = self.params.lp_freq.smoothed.next();
            let mid_freq = self.params.mid_freq.smoothed.next();
            let mid_gain = self.params.mid_gain.smoothed.next();
            let mid_q = self.params.mid_q.smoothed.next();
            let output_gain_db = self.params.output_gain.smoothed.next();

            self.hpf_coeffs = BiquadCoeffs::highpass(hp_freq, self.sample_rate);
            self.bell_coeffs = BiquadCoeffs::peaking(mid_freq, mid_gain, mid_q, self.sample_rate);
            self.lpf_coeffs = BiquadCoeffs::lowpass(lp_freq, self.sample_rate);
            let output_gain = 10.0_f32.powf(output_gain_db / 20.0);

            for (ch_idx, sample) in channel_samples.iter_mut().enumerate() {
                while ch_idx >= self.channels.len() {
                    self.channels.push(PresenceEqDsp::new());
                }
                *sample = self.channels[ch_idx].process(
                    *sample,
                    &self.hpf_coeffs,
                    &self.bell_coeffs,
                    &self.lpf_coeffs,
                    output_gain,
                );
            }
        }

        ProcessStatus::Normal
    }
}

impl PresenceEq {
    fn recompute_coeffs(&mut self) {
        let sr = self.sample_rate;
        self.hpf_coeffs = BiquadCoeffs::highpass(self.params.hp_freq.value(), sr);
        self.bell_coeffs = BiquadCoeffs::peaking(
            self.params.mid_freq.value(),
            self.params.mid_gain.value(),
            self.params.mid_q.value(),
            sr,
        );
        self.lpf_coeffs = BiquadCoeffs::lowpass(self.params.lp_freq.value(), sr);
    }
}

impl Vst3Plugin for PresenceEq {
    const VST3_CLASS_ID: [u8; 16] = *b"PresenceEQ_Plug0";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Eq];
}

nih_export_vst3!(PresenceEq);

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 44100.0;

    fn run_samples(dsp: &mut PresenceEqDsp, coeffs: (&BiquadCoeffs, &BiquadCoeffs, &BiquadCoeffs), n: usize, input: f32) -> f32 {
        let mut out = 0.0;
        for _ in 0..n {
            out = dsp.process(input, coeffs.0, coeffs.1, coeffs.2, 1.0);
        }
        out
    }

    // --- HPF ---

    #[test]
    fn hpf_passes_high_freq() {
        // 1kHz well above 80Hz HPF → should pass with ≈0dB (close to 1.0 amplitude)
        let c = BiquadCoeffs::highpass(80.0, SR);
        let bell = BiquadCoeffs::peaking(300.0, 0.0, 1.0, SR); // 0dB gain = transparent
        let lpf = BiquadCoeffs::lowpass(7500.0, SR);
        let mut dsp = PresenceEqDsp::new();

        // Drive with 1kHz sine, let it settle for 2000 samples, then measure amplitude
        let freq = 1000.0_f32;
        let mut peak = 0.0_f32;
        for i in 0..4000 {
            let x = (2.0 * std::f32::consts::PI * freq / SR * i as f32).sin();
            let y = dsp.process(x, &c, &bell, &lpf, 1.0);
            if i > 2000 {
                peak = peak.max(y.abs());
            }
        }
        // Expect close to 1.0 (within 3%)
        assert!(peak > 0.97, "HPF should pass 1kHz, got peak={}", peak);
    }

    #[test]
    fn hpf_attenuates_dc() {
        // DC (0 Hz) must be blocked by HPF
        let c = BiquadCoeffs::highpass(80.0, SR);
        let bell = BiquadCoeffs::peaking(300.0, 0.0, 1.0, SR);
        let lpf = BiquadCoeffs::lowpass(7500.0, SR);
        let mut dsp = PresenceEqDsp::new();

        let out = run_samples(&mut dsp, (&c, &bell, &lpf), 10000, 1.0);
        assert!(out.abs() < 0.01, "HPF must block DC, got {}", out);
    }

    // --- LPF ---

    #[test]
    fn lpf_passes_low_freq() {
        // 100Hz well below 7500Hz LPF
        let hpf = BiquadCoeffs::highpass(50.0, SR);
        let bell = BiquadCoeffs::peaking(300.0, 0.0, 1.0, SR);
        let c = BiquadCoeffs::lowpass(7500.0, SR);
        let mut dsp = PresenceEqDsp::new();

        let freq = 100.0_f32;
        let mut peak = 0.0_f32;
        for i in 0..4000 {
            let x = (2.0 * std::f32::consts::PI * freq / SR * i as f32).sin();
            let y = dsp.process(x, &hpf, &bell, &c, 1.0);
            if i > 2000 {
                peak = peak.max(y.abs());
            }
        }
        assert!(peak > 0.96, "LPF should pass 100Hz, got peak={}", peak);
    }

    #[test]
    fn lpf_attenuates_high_freq() {
        // 20kHz well above 7500Hz LPF → should be strongly attenuated
        let hpf = BiquadCoeffs::highpass(50.0, SR);
        let bell = BiquadCoeffs::peaking(300.0, 0.0, 1.0, SR);
        let c = BiquadCoeffs::lowpass(7500.0, SR);
        let mut dsp = PresenceEqDsp::new();

        let freq = 20000.0_f32;
        let mut peak = 0.0_f32;
        for i in 0..4000 {
            let x = (2.0 * std::f32::consts::PI * freq / SR * i as f32).sin();
            let y = dsp.process(x, &hpf, &bell, &c, 1.0);
            if i > 2000 {
                peak = peak.max(y.abs());
            }
        }
        assert!(peak < 0.1, "LPF should heavily attenuate 20kHz, got peak={}", peak);
    }

    // --- Bell ---

    #[test]
    fn bell_boosts_at_center_freq() {
        // A 6dB boost at 300Hz → amplitude at 300Hz should be ~2x (linear)
        let hpf = BiquadCoeffs::highpass(50.0, SR);
        let bell = BiquadCoeffs::peaking(300.0, 6.0, 1.0, SR);
        let lpf = BiquadCoeffs::lowpass(7500.0, SR);
        let mut dsp = PresenceEqDsp::new();

        let freq = 300.0_f32;
        let mut peak = 0.0_f32;
        for i in 0..10000 {
            let x = (2.0 * std::f32::consts::PI * freq / SR * i as f32).sin();
            let y = dsp.process(x, &hpf, &bell, &lpf, 1.0);
            if i > 5000 {
                peak = peak.max(y.abs());
            }
        }
        // 6dB ≈ 2.0x; allow ±5%
        assert!(
            peak > 1.9 && peak < 2.1,
            "6dB bell boost at 300Hz should give ~2.0x amplitude, got {}",
            peak
        );
    }

    #[test]
    fn bell_zero_gain_is_transparent() {
        // 0dB gain → output amplitude should be ≈1.0 at center freq
        let hpf = BiquadCoeffs::highpass(50.0, SR);
        let bell = BiquadCoeffs::peaking(300.0, 0.0, 1.0, SR);
        let lpf = BiquadCoeffs::lowpass(7500.0, SR);
        let mut dsp = PresenceEqDsp::new();

        let freq = 300.0_f32;
        let mut peak = 0.0_f32;
        for i in 0..10000 {
            let x = (2.0 * std::f32::consts::PI * freq / SR * i as f32).sin();
            let y = dsp.process(x, &hpf, &bell, &lpf, 1.0);
            if i > 5000 {
                peak = peak.max(y.abs());
            }
        }
        assert!(
            (peak - 1.0).abs() < 0.03,
            "0dB bell should be transparent at center freq, got {}",
            peak
        );
    }

    // --- Output gain ---

    #[test]
    fn output_gain_scales_correctly() {
        // +6dB output gain should double the amplitude
        let hpf = BiquadCoeffs::highpass(50.0, SR);
        let bell = BiquadCoeffs::peaking(300.0, 0.0, 1.0, SR);
        let lpf = BiquadCoeffs::lowpass(7500.0, SR);
        let gain = 10.0_f32.powf(6.0 / 20.0); // ≈2.0
        let mut dsp = PresenceEqDsp::new();

        let freq = 1000.0_f32;
        let mut peak = 0.0_f32;
        for i in 0..4000 {
            let x = (2.0 * std::f32::consts::PI * freq / SR * i as f32).sin();
            let y = dsp.process(x, &hpf, &bell, &lpf, gain);
            if i > 2000 {
                peak = peak.max(y.abs());
            }
        }
        assert!(
            (peak - gain).abs() < 0.05,
            "+6dB output gain should give ~{} amplitude, got {}",
            gain,
            peak
        );
    }
}
