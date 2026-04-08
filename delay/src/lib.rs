use nih_plug::prelude::*;
use std::sync::Arc;

mod editor;

/// Pure DSP struct for a single-channel delay line — extracted for unit testing.
pub struct DelayDsp {
    buffer: Vec<f32>,
    write_pos: usize,
    sample_rate: f32,
}

impl DelayDsp {
    pub fn new(sample_rate: f32) -> Self {
        let max_samples = (sample_rate * 2.0) as usize;
        Self {
            buffer: vec![0.0; max_samples],
            write_pos: 0,
            sample_rate,
        }
    }

    pub fn resize(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        let max_samples = (sample_rate * 2.0) as usize;
        self.buffer.resize(max_samples, 0.0);
        self.write_pos = 0;
    }

    pub fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.write_pos = 0;
    }

    /// Returns output after applying delay, feedback, and mix.
    pub fn process(&mut self, input: f32, delay_samples: usize, feedback: f32, mix: f32) -> f32 {
        let buf_size = self.buffer.len();
        let delay_samples = delay_samples.min(buf_size - 1);
        let read_pos = (self.write_pos + buf_size - delay_samples) % buf_size;

        let delayed = self.buffer[read_pos];
        self.buffer[self.write_pos] = input + delayed * feedback;
        self.write_pos = (self.write_pos + 1) % buf_size;

        input * (1.0 - mix) + delayed * mix
    }

    pub fn delay_samples_for_ms(&self, time_ms: f32) -> usize {
        (time_ms * self.sample_rate / 1000.0) as usize
    }
}

#[derive(Params)]
pub struct DelayParams {
    #[persist = "editor-state"]
    editor_state: Arc<nih_plug_iced::IcedState>,

    #[id = "time_ms"]
    pub time_ms: FloatParam,

    #[id = "feedback"]
    pub feedback: FloatParam,

    #[id = "mix"]
    pub mix: FloatParam,
}

impl Default for DelayParams {
    fn default() -> Self {
        Self {
            editor_state: editor::default_state(),
            time_ms: FloatParam::new(
                "Time",
                300.0,
                FloatRange::Linear { min: 1.0, max: 2000.0 },
            )
            .with_unit(" ms")
            .with_smoother(SmoothingStyle::Linear(5.0)),
            feedback: FloatParam::new(
                "Feedback",
                40.0,
                FloatRange::Linear { min: 0.0, max: 95.0 },
            )
            .with_unit(" %")
            .with_smoother(SmoothingStyle::Linear(5.0)),
            mix: FloatParam::new(
                "Mix",
                50.0,
                FloatRange::Linear { min: 0.0, max: 100.0 },
            )
            .with_unit(" %")
            .with_smoother(SmoothingStyle::Linear(5.0)),
        }
    }
}

pub struct Delay {
    params: Arc<DelayParams>,
    sample_rate: f32,
    /// Per-channel delay lines.
    delay_lines: Vec<DelayDsp>,
}

impl Default for Delay {
    fn default() -> Self {
        Self {
            params: Arc::new(DelayParams::default()),
            sample_rate: 44100.0,
            delay_lines: vec![DelayDsp::new(44100.0), DelayDsp::new(44100.0)],
        }
    }
}

impl Plugin for Delay {
    const NAME: &'static str = "Delay";
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
        self.delay_lines = vec![
            DelayDsp::new(buffer_config.sample_rate),
            DelayDsp::new(buffer_config.sample_rate),
        ];
        true
    }

    fn reset(&mut self) {
        for line in self.delay_lines.iter_mut() {
            line.reset();
        }
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let sample_rate = self.sample_rate;

        for mut channel_samples in buffer.iter_samples() {
            let time_ms = self.params.time_ms.smoothed.next();
            let feedback = self.params.feedback.smoothed.next() / 100.0;
            let mix = self.params.mix.smoothed.next() / 100.0;
            let delay_samples = (time_ms * sample_rate / 1000.0) as usize;

            for (ch, sample) in channel_samples.iter_mut().enumerate() {
                if ch >= self.delay_lines.len() {
                    self.delay_lines.push(DelayDsp::new(sample_rate));
                }
                *sample = self.delay_lines[ch].process(*sample, delay_samples, feedback, mix);
            }
        }

        // Tail: max delay + some feedback decay headroom
        let max_delay_samples = (sample_rate * 2.0) as u32;
        let feedback_tail = (self.params.feedback.value() / 100.0 * 5.0 * sample_rate) as u32;
        ProcessStatus::Tail(max_delay_samples + feedback_tail)
    }
}

impl Vst3Plugin for Delay {
    const VST3_CLASS_ID: [u8; 16] = *b"GuitarDelayPlg00";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Delay];
}

nih_export_vst3!(Delay);

#[cfg(test)]
mod tests {
    use super::*;

    fn make_delay(sample_rate: f32) -> DelayDsp {
        DelayDsp::new(sample_rate)
    }

    #[test]
    fn test_output_delayed_by_correct_samples() {
        let mut d = make_delay(44100.0);
        let delay_samples = 10_usize;
        let pulse = 1.0_f32;

        // Feed a pulse at t=0, then silence
        let out0 = d.process(pulse, delay_samples, 0.0, 1.0);
        // For the first `delay_samples` outputs there's nothing in the buffer yet → should be 0
        assert_eq!(out0, 0.0, "no delayed signal yet at t=0");

        for i in 1..delay_samples {
            let out = d.process(0.0, delay_samples, 0.0, 1.0);
            assert_eq!(out, 0.0, "still no delayed signal at t={}", i);
        }

        // At t=delay_samples the pulse should arrive
        let out_delayed = d.process(0.0, delay_samples, 0.0, 1.0);
        assert!(
            (out_delayed - pulse).abs() < 1e-5,
            "pulse should appear after {} samples, got {}",
            delay_samples,
            out_delayed
        );
    }

    #[test]
    fn test_dry_signal_passthrough() {
        let mut d = make_delay(44100.0);
        let input = 0.5_f32;
        // mix=0 → output = input * 1.0 + delayed * 0.0
        let out = d.process(input, 100, 0.0, 0.0);
        assert!((out - input).abs() < 1e-6, "mix=0 should pass dry signal");
    }

    #[test]
    fn test_wet_only() {
        let mut d = make_delay(44100.0);
        let delay_samples = 5_usize;
        // mix=1.0 → output is only the delayed signal
        let out_first = d.process(1.0, delay_samples, 0.0, 1.0);
        // At t=0 there's no delayed content
        assert_eq!(out_first, 0.0, "wet-only: first output should be silent");

        for _ in 1..delay_samples {
            d.process(0.0, delay_samples, 0.0, 1.0);
        }
        let out_at_delay = d.process(0.0, delay_samples, 0.0, 1.0);
        assert!(
            (out_at_delay - 1.0).abs() < 1e-5,
            "wet-only: pulse should appear at delay time"
        );
    }

    #[test]
    fn test_feedback_causes_repetition() {
        let mut d = make_delay(44100.0);
        let delay_samples = 5_usize;
        let feedback = 0.5_f32;

        // Feed pulse
        d.process(1.0, delay_samples, feedback, 1.0);
        for _ in 1..delay_samples {
            d.process(0.0, delay_samples, feedback, 1.0);
        }
        // First echo
        let echo1 = d.process(0.0, delay_samples, feedback, 1.0);
        assert!(echo1 > 0.0, "first echo should be non-zero");

        // Advance to second echo
        for _ in 1..delay_samples {
            d.process(0.0, delay_samples, feedback, 1.0);
        }
        let echo2 = d.process(0.0, delay_samples, feedback, 1.0);
        assert!(echo2 > 0.0, "second echo (feedback) should be non-zero");
        assert!(echo2 < echo1, "second echo should be quieter due to feedback < 1");
    }

    #[test]
    fn test_zero_feedback_no_repetition() {
        let mut d = make_delay(44100.0);
        let delay_samples = 5_usize;

        d.process(1.0, delay_samples, 0.0, 1.0);
        for _ in 1..delay_samples {
            d.process(0.0, delay_samples, 0.0, 1.0);
        }
        let echo1 = d.process(0.0, delay_samples, 0.0, 1.0);
        // The pulse appears once
        assert!(echo1 > 0.0, "first echo still appears");

        // But no second echo with feedback=0
        for _ in 1..delay_samples {
            d.process(0.0, delay_samples, 0.0, 1.0);
        }
        let echo2 = d.process(0.0, delay_samples, 0.0, 1.0);
        assert_eq!(echo2, 0.0, "no second echo when feedback=0");
    }

    #[test]
    fn test_delay_length_scales_with_sample_rate() {
        let d = make_delay(48000.0);
        let delay_samples = d.delay_samples_for_ms(300.0);
        assert_eq!(
            delay_samples, 14400,
            "300 ms at 48000 Hz should be 14400 samples"
        );
    }
}
