use nih_plug::prelude::*;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use std::f32::consts::PI;
use std::sync::Arc;

mod editor;

const FFT_FRAME_SIZE: usize = 2048;
const OSAMP: usize = 16;
const STEP_SIZE: usize = FFT_FRAME_SIZE / OSAMP; // 128
const FFT_BINS: usize = FFT_FRAME_SIZE / 2 + 1; // 1025

/// Pure DSP struct for single-channel FFT-based phase vocoder pitch shifting.
pub struct PitchShifterDsp {
    factor: f32,
    freq_per_bin: f32,
    expct: f32,
    g_rover: usize,

    window: [f32; FFT_FRAME_SIZE],
    g_in_fifo: [f32; FFT_FRAME_SIZE],
    windowed_input: [f32; FFT_FRAME_SIZE],
    g_fftworksp: [Complex<f32>; FFT_BINS],
    g_last_phase: [f32; FFT_BINS],
    g_sum_phase: [f32; FFT_BINS],
    g_output_accum: [f32; 2 * FFT_FRAME_SIZE],
    g_ana_freq: [f32; FFT_FRAME_SIZE],
    g_ana_magn: [f32; FFT_FRAME_SIZE],
    g_syn_freq: [f32; FFT_FRAME_SIZE],
    g_syn_magn: [f32; FFT_FRAME_SIZE],
    g_ifft_output: [f32; FFT_FRAME_SIZE],

    fft_scratch: Vec<Complex<f32>>,
    ifft_scratch: Vec<Complex<f32>>,

    real_fft: Arc<dyn RealToComplex<f32>>,
    real_ifft: Arc<dyn ComplexToReal<f32>>,
}

impl PitchShifterDsp {
    pub fn new(semitones: i32, sample_rate: f32) -> Self {
        let expct = 2.0 * PI * STEP_SIZE as f32 / FFT_FRAME_SIZE as f32;

        let mut real_planner = RealFftPlanner::<f32>::new();
        let real_fft = real_planner.plan_fft_forward(FFT_FRAME_SIZE);
        let real_ifft = real_planner.plan_fft_inverse(FFT_FRAME_SIZE);
        let fft_scratch = real_fft.make_scratch_vec();
        let ifft_scratch = real_ifft.make_scratch_vec();

        let mut window = [0.0f32; FFT_FRAME_SIZE];
        for i in 0..FFT_FRAME_SIZE {
            window[i] = -0.5 * (2.0 * PI * i as f32 / FFT_FRAME_SIZE as f32).cos() + 0.5;
        }

        Self {
            factor: 2.0_f32.powf(semitones as f32 / 12.0),
            freq_per_bin: sample_rate / FFT_FRAME_SIZE as f32,
            expct,
            g_rover: FFT_FRAME_SIZE - STEP_SIZE,
            window,
            g_in_fifo: [0.0; FFT_FRAME_SIZE],
            windowed_input: [0.0; FFT_FRAME_SIZE],
            g_fftworksp: [Complex { re: 0.0, im: 0.0 }; FFT_BINS],
            g_last_phase: [0.0; FFT_BINS],
            g_sum_phase: [0.0; FFT_BINS],
            g_output_accum: [0.0; 2 * FFT_FRAME_SIZE],
            g_ana_freq: [0.0; FFT_FRAME_SIZE],
            g_ana_magn: [0.0; FFT_FRAME_SIZE],
            g_syn_freq: [0.0; FFT_FRAME_SIZE],
            g_syn_magn: [0.0; FFT_FRAME_SIZE],
            g_ifft_output: [0.0; FFT_FRAME_SIZE],
            fft_scratch,
            ifft_scratch,
            real_fft,
            real_ifft,
        }
    }

    pub fn set_pitch_shift(&mut self, semitones: i32) {
        self.factor = 2.0_f32.powf(semitones as f32 / 12.0);
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.freq_per_bin = sample_rate / FFT_FRAME_SIZE as f32;
    }

    pub fn reset(&mut self) {
        self.g_rover = FFT_FRAME_SIZE - STEP_SIZE;
        self.g_in_fifo.fill(0.0);
        self.windowed_input.fill(0.0);
        self.g_fftworksp.fill(Complex { re: 0.0, im: 0.0 });
        self.g_last_phase.fill(0.0);
        self.g_sum_phase.fill(0.0);
        self.g_output_accum.fill(0.0);
        self.g_ana_freq.fill(0.0);
        self.g_ana_magn.fill(0.0);
        self.g_syn_freq.fill(0.0);
        self.g_syn_magn.fill(0.0);
        self.g_ifft_output.fill(0.0);
    }

    /// Process a single sample. The output is delayed by `STEP_SIZE` samples (128).
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let in_fifo_latency = FFT_FRAME_SIZE - STEP_SIZE;

        // Read output from the accumulator before advancing the rover
        let out = self.g_output_accum[self.g_rover - in_fifo_latency];

        // Write input into the FIFO
        self.g_in_fifo[self.g_rover] = input;
        self.g_rover += 1;

        // When FIFO is full, process one FFT frame
        if self.g_rover >= FFT_FRAME_SIZE {
            self.g_rover = in_fifo_latency;
            self.process_fft_frame();

            // Shift output accumulator by one hop
            self.g_output_accum.copy_within(STEP_SIZE.., 0);
            self.g_output_accum[2 * FFT_FRAME_SIZE - STEP_SIZE..].fill(0.0);

            // Shift input FIFO by one hop
            self.g_in_fifo.copy_within(STEP_SIZE.., 0);
        }

        out
    }

    fn process_fft_frame(&mut self) {
        let fft_frame_size2 = FFT_FRAME_SIZE / 2;

        // Apply Hann window to FIFO
        for k in 0..FFT_FRAME_SIZE {
            self.windowed_input[k] = self.g_in_fifo[k] * self.window[k];
        }

        // Forward real FFT
        self.real_fft
            .process_with_scratch(
                &mut self.windowed_input,
                &mut self.g_fftworksp,
                &mut self.fft_scratch,
            )
            .unwrap();

        // Analysis: compute magnitude and true frequency per bin
        for k in 0..=fft_frame_size2 {
            let scale = if k == 0 || k == fft_frame_size2 {
                1.0
            } else {
                2.0
            };
            let magn = scale * self.g_fftworksp[k].re.hypot(self.g_fftworksp[k].im);
            let phase = self.g_fftworksp[k].im.atan2(self.g_fftworksp[k].re);

            let mut tmp = phase - self.g_last_phase[k];
            self.g_last_phase[k] = phase;

            // Subtract expected phase advance
            tmp -= k as f32 * self.expct;

            // Wrap to [-PI, PI]
            let mut qpd = (tmp / PI) as isize;
            if qpd >= 0 {
                qpd += qpd & 1;
            } else {
                qpd -= qpd & 1;
            }
            tmp -= PI * qpd as f32;

            // Compute true frequency
            tmp = OSAMP as f32 * tmp / (2.0 * PI);
            tmp = k as f32 * self.freq_per_bin + tmp * self.freq_per_bin;

            self.g_ana_magn[k] = magn;
            self.g_ana_freq[k] = tmp;
        }

        // Pitch shifting: remap bins by the shift factor
        self.g_syn_freq.fill(0.0);
        self.g_syn_magn.fill(0.0);

        for k in 0..=fft_frame_size2 {
            let index = (k as f32 * self.factor) as usize;
            if index <= fft_frame_size2 {
                self.g_syn_magn[index] += self.g_ana_magn[k];
                self.g_syn_freq[index] = self.g_ana_freq[k] * self.factor;
            }
        }

        // Synthesis: rebuild phases and construct complex spectrum
        for k in 0..=fft_frame_size2 {
            let magn = self.g_syn_magn[k];
            let mut tmp = self.g_syn_freq[k];

            tmp -= k as f32 * self.freq_per_bin;
            tmp /= self.freq_per_bin;
            tmp = 2.0 * PI * tmp / OSAMP as f32;
            tmp += k as f32 * self.expct;

            self.g_sum_phase[k] += tmp;

            // Keep accumulated phase in a reasonable range to prevent overflow
            if self.g_sum_phase[k] > 8.0 * PI {
                self.g_sum_phase[k] -= 8.0 * PI;
            } else if self.g_sum_phase[k] < -8.0 * PI {
                self.g_sum_phase[k] += 8.0 * PI;
            }

            let phase = self.g_sum_phase[k];
            self.g_fftworksp[k].re = magn * phase.cos();
            // DC and Nyquist bins must have zero imaginary part for a real IFFT
            if k == 0 || k == fft_frame_size2 {
                self.g_fftworksp[k].im = 0.0;
            } else {
                self.g_fftworksp[k].im = magn * phase.sin();
            }
        }

        // Inverse real FFT
        self.real_ifft
            .process_with_scratch(
                &mut self.g_fftworksp,
                &mut self.g_ifft_output,
                &mut self.ifft_scratch,
            )
            .unwrap();

        // Overlap-add with windowing and normalization
        let scale = 1.0 / (fft_frame_size2 * OSAMP) as f32;
        for k in 0..FFT_FRAME_SIZE {
            self.g_output_accum[k] += scale * self.window[k] * self.g_ifft_output[k];
        }
    }
}

#[derive(Params)]
pub struct PitchShifterParams {
    #[persist = "editor-state"]
    editor_state: Arc<nih_plug_iced::IcedState>,

    #[id = "semitones"]
    pub semitones: IntParam,
}

impl Default for PitchShifterParams {
    fn default() -> Self {
        Self {
            editor_state: editor::default_state(),
            semitones: IntParam::new("Semitones", 0, IntRange::Linear { min: -12, max: 12 })
                .with_unit(" st"),
        }
    }
}

pub struct PitchShifter {
    params: Arc<PitchShifterParams>,
    sample_rate: f32,
    dsp: Vec<PitchShifterDsp>,
}

impl Default for PitchShifter {
    fn default() -> Self {
        Self {
            params: Arc::new(PitchShifterParams::default()),
            sample_rate: 44100.0,
            dsp: vec![
                PitchShifterDsp::new(0, 44100.0),
                PitchShifterDsp::new(0, 44100.0),
            ],
        }
    }
}

impl Plugin for PitchShifter {
    const NAME: &'static str = "Pitch Shifter";
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
        context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate;
        let semitones = self.params.semitones.value();
        self.dsp = vec![
            PitchShifterDsp::new(semitones, buffer_config.sample_rate),
            PitchShifterDsp::new(semitones, buffer_config.sample_rate),
        ];
        context.set_latency_samples(STEP_SIZE as u32);
        true
    }

    fn reset(&mut self) {
        for dsp in &mut self.dsp {
            dsp.reset();
        }
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let semitones = self.params.semitones.value();
        for dsp in &mut self.dsp {
            dsp.set_pitch_shift(semitones);
        }

        for mut channel_samples in buffer.iter_samples() {
            for (ch, sample) in channel_samples.iter_mut().enumerate() {
                if let Some(dsp) = self.dsp.get_mut(ch) {
                    *sample = dsp.process_sample(*sample);
                }
            }
        }

        ProcessStatus::Normal
    }
}

impl Vst3Plugin for PitchShifter {
    const VST3_CLASS_ID: [u8; 16] = *b"PitchShifterP000";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::PitchShift];
}

nih_export_vst3!(PitchShifter);

#[cfg(test)]
mod tests {
    use super::*;

    fn make_dsp() -> PitchShifterDsp {
        PitchShifterDsp::new(0, 44100.0)
    }

    #[test]
    fn test_silence_produces_silence() {
        let mut dsp = make_dsp();
        for _ in 0..4096 {
            let out = dsp.process_sample(0.0);
            assert!(
                out.abs() < 1e-6,
                "silence input should produce silence, got {}",
                out
            );
        }
    }

    #[test]
    fn test_reset_clears_state() {
        let mut dsp = make_dsp();
        // Process some non-trivial input to dirty the state
        for i in 0..512 {
            let input = (2.0 * PI * 440.0 * i as f32 / 44100.0).sin();
            dsp.process_sample(input);
        }
        dsp.reset();
        // After reset, silence should produce silence
        for _ in 0..256 {
            let out = dsp.process_sample(0.0);
            assert!(
                out.abs() < 1e-6,
                "after reset, silence should produce silence, got {}",
                out
            );
        }
    }

    #[test]
    fn test_zero_semitones_bounded_output() {
        let mut dsp = make_dsp();
        let mut max_out = 0.0f32;
        for i in 0..8192 {
            let input = (2.0 * PI * 440.0 * i as f32 / 44100.0).sin();
            let out = dsp.process_sample(input);
            max_out = max_out.max(out.abs());
        }
        // A unit sine input should produce bounded output
        assert!(
            max_out < 2.0,
            "output should be bounded for unit sine input, got max {}",
            max_out
        );
        // After warmup (>128 samples latency), output should be non-trivial
        assert!(
            max_out > 0.1,
            "output should be non-zero for sine input after warmup, got max {}",
            max_out
        );
    }
}
