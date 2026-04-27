use atomic_float::AtomicF32;
use nih_plug::prelude::*;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

mod editor;

const ANALYSIS_LEN: usize = 4096;
const ANALYSIS_INTERVAL: usize = 512;
const SILENCE_THRESHOLD_RMS: f32 = 0.001; // ~-60 dB
const AUTOCORR_PEAK_THRESHOLD: f32 = 0.8;
const NO_DETECTION: u8 = 255;

// --- Pure pitch-detection DSP (extracted for testing) ---

pub struct PitchDetector {
    sample_rate: f32,
    ring_buffer: Vec<f32>,
    write_pos: usize,
    analysis_buf: Vec<f32>,
}

impl PitchDetector {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            ring_buffer: vec![0.0; ANALYSIS_LEN],
            write_pos: 0,
            analysis_buf: vec![0.0; ANALYSIS_LEN],
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }

    pub fn reset(&mut self) {
        self.ring_buffer.fill(0.0);
        self.write_pos = 0;
    }

    pub fn push_sample(&mut self, sample: f32) {
        self.ring_buffer[self.write_pos] = sample;
        self.write_pos = (self.write_pos + 1) % ANALYSIS_LEN;
    }

    /// Returns `Some((frequency_hz, nearest_midi_note, cents_offset))` or `None` when silent.
    pub fn analyze(&mut self, reference_a4: f32) -> Option<(f32, i32, f32)> {
        // Unwrap ring buffer into contiguous analysis_buf (oldest sample first)
        let start = self.write_pos;
        for i in 0..ANALYSIS_LEN {
            self.analysis_buf[i] = self.ring_buffer[(start + i) % ANALYSIS_LEN];
        }

        // Silence check via RMS
        let rms = (self.analysis_buf.iter().map(|s| s * s).sum::<f32>() / ANALYSIS_LEN as f32)
            .sqrt();
        if rms < SILENCE_THRESHOLD_RMS {
            return None;
        }

        let sr = self.sample_rate;
        let min_lag = (sr / 1200.0).max(1.0) as usize;
        let max_lag = ((sr / 25.0) as usize).min(ANALYSIS_LEN / 2 - 1);

        if min_lag + 1 >= max_lag {
            return None;
        }

        let buf = &self.analysis_buf;

        // Step 1: Compute normalized autocorrelation (NSDF) for all lags in range.
        // For a pure sine, NSDF(lag) = cos(2π * f * lag / sr), which decreases from its initial
        // value, hits a trough at half the period, then rises to the first true peak at the
        // fundamental period. Scanning for the first lag above a fixed threshold would
        // prematurely accept a high-but-decreasing value near min_lag. Instead we collect all
        // values, find the global maximum, then pick the FIRST local maximum that exceeds
        // 0.8 * global_max — the McLeod Pitch Method key-maximum strategy.
        let lag_count = max_lag - min_lag + 1;
        let mut nsdf = vec![0.0f32; lag_count];

        for (idx, lag) in (min_lag..=max_lag).enumerate() {
            let n = ANALYSIS_LEN - lag;
            let mut sum_xy = 0.0f32;
            let mut sum_xx = 0.0f32;
            let mut sum_yy = 0.0f32;
            for i in 0..n {
                let x = buf[i];
                let y = buf[i + lag];
                sum_xy += x * y;
                sum_xx += x * x;
                sum_yy += y * y;
            }
            let denom = (sum_xx * sum_yy).sqrt();
            nsdf[idx] = if denom > 1e-10 { sum_xy / denom } else { 0.0 };
        }

        // Step 2: Global maximum and threshold.
        let global_max = nsdf.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        if global_max < AUTOCORR_PEAK_THRESHOLD {
            return None;
        }
        let threshold = AUTOCORR_PEAK_THRESHOLD * global_max;

        // Step 3: Find the first local maximum that exceeds the threshold.
        let mut best_idx = None;
        for i in 1..lag_count - 1 {
            if nsdf[i] > nsdf[i - 1] && nsdf[i] >= nsdf[i + 1] && nsdf[i] > threshold {
                best_idx = Some(i);
                break;
            }
        }

        let best_idx = best_idx?;
        let best_lag = min_lag + best_idx;

        // Step 4: Parabolic interpolation for sub-sample accuracy.
        let y1 = nsdf[best_idx - 1];
        let y2 = nsdf[best_idx];
        let y3 = nsdf[best_idx + 1];
        let denom = y1 - 2.0 * y2 + y3;
        let interpolated_lag = if denom.abs() > 1e-10 {
            best_lag as f32 - 0.5 * (y3 - y1) / denom
        } else {
            best_lag as f32
        };

        let frequency = sr / interpolated_lag;
        let (midi_note, cents) = cents_from_nearest(frequency, reference_a4);
        Some((frequency, midi_note, cents))
    }
}

// --- Pure helper functions ---

pub fn frequency_to_midi(freq: f32, reference_a4: f32) -> f32 {
    69.0 + 12.0 * (freq / reference_a4).log2()
}

pub fn midi_to_note_name(midi: i32) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let octave = midi / 12 - 1;
    let name = NAMES[((midi % 12) + 12) as usize % 12];
    format!("{}{}", name, octave)
}

/// Returns `(nearest_midi_note, cents_offset)` where cents is in -50..+50.
pub fn cents_from_nearest(freq: f32, reference_a4: f32) -> (i32, f32) {
    let midi_f = frequency_to_midi(freq, reference_a4);
    let nearest = midi_f.round() as i32;
    let cents = (midi_f - nearest as f32) * 100.0;
    (nearest, cents)
}

// --- Params ---

#[derive(Params)]
pub struct GuitarTunerParams {
    #[persist = "editor-state"]
    editor_state: Arc<nih_plug_iced::IcedState>,

    #[id = "reference_pitch"]
    pub reference_pitch: FloatParam,
}

impl Default for GuitarTunerParams {
    fn default() -> Self {
        Self {
            editor_state: editor::default_state(),
            reference_pitch: FloatParam::new(
                "A4 Reference",
                440.0,
                FloatRange::Linear {
                    min: 430.0,
                    max: 450.0,
                },
            )
            .with_unit(" Hz")
            .with_smoother(SmoothingStyle::None),
        }
    }
}

// --- Plugin ---

pub struct GuitarTuner {
    params: Arc<GuitarTunerParams>,
    sample_rate: f32,
    pitch_detector: PitchDetector,
    samples_since_analysis: usize,

    // Shared with editor
    detected_freq: Arc<AtomicF32>,
    detected_cents: Arc<AtomicF32>,
    detected_note: Arc<AtomicU8>,
}

impl Default for GuitarTuner {
    fn default() -> Self {
        Self {
            params: Arc::new(GuitarTunerParams::default()),
            sample_rate: 44100.0,
            pitch_detector: PitchDetector::new(44100.0),
            samples_since_analysis: 0,
            detected_freq: Arc::new(AtomicF32::new(0.0)),
            detected_cents: Arc::new(AtomicF32::new(0.0)),
            detected_note: Arc::new(AtomicU8::new(NO_DETECTION)),
        }
    }
}

impl Plugin for GuitarTuner {
    const NAME: &'static str = "Guitar Tuner";
    const VENDOR: &'static str = "Audio Plugins";
    const URL: &'static str = "https://github.com/downfall85/audio-plugins";
    const EMAIL: &'static str = "example@example.com";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: NonZeroU32::new(2),
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::None;
    const SAMPLE_ACCURATE_AUTOMATION: bool = false;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(
            self.params.clone(),
            self.params.editor_state.clone(),
            self.detected_freq.clone(),
            self.detected_cents.clone(),
            self.detected_note.clone(),
        )
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate;
        self.pitch_detector.set_sample_rate(buffer_config.sample_rate);
        true
    }

    fn reset(&mut self) {
        self.pitch_detector.reset();
        self.samples_since_analysis = 0;
        self.detected_freq.store(0.0, Ordering::Relaxed);
        self.detected_cents.store(0.0, Ordering::Relaxed);
        self.detected_note.store(NO_DETECTION, Ordering::Relaxed);
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let reference_a4 = self.params.reference_pitch.value();

        for channel_samples in buffer.iter_samples() {
            // Mono-mix all channels for analysis
            let num_channels = channel_samples.len();
            let mono: f32 = if num_channels > 0 {
                channel_samples.into_iter().map(|s| *s).sum::<f32>() / num_channels as f32
            } else {
                0.0
            };

            self.pitch_detector.push_sample(mono);
            self.samples_since_analysis += 1;

            if self.samples_since_analysis >= ANALYSIS_INTERVAL {
                self.samples_since_analysis = 0;

                match self.pitch_detector.analyze(reference_a4) {
                    Some((freq, midi_note, cents)) => {
                        self.detected_freq.store(freq, Ordering::Relaxed);
                        self.detected_cents.store(cents, Ordering::Relaxed);
                        self.detected_note
                            .store(midi_note.clamp(0, 127) as u8, Ordering::Relaxed);
                    }
                    None => {
                        self.detected_freq.store(0.0, Ordering::Relaxed);
                        self.detected_cents.store(0.0, Ordering::Relaxed);
                        self.detected_note.store(NO_DETECTION, Ordering::Relaxed);
                    }
                }
            }
        }

        ProcessStatus::Normal
    }
}

impl Vst3Plugin for GuitarTuner {
    const VST3_CLASS_ID: [u8; 16] = *b"GuitarTunerP0000";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[Vst3SubCategory::Analyzer];
}

nih_export_vst3!(GuitarTuner);

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn sine_samples(freq: f32, sample_rate: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (2.0 * PI * freq * i as f32 / sample_rate).sin())
            .collect()
    }

    fn detect(freq: f32, sample_rate: f32) -> Option<(f32, i32, f32)> {
        let mut detector = PitchDetector::new(sample_rate);
        for s in sine_samples(freq, sample_rate, ANALYSIS_LEN) {
            detector.push_sample(s);
        }
        detector.analyze(440.0)
    }

    #[test]
    fn test_detect_sine_440() {
        let result = detect(440.0, 44100.0);
        assert!(result.is_some(), "should detect 440 Hz");
        let (freq, _, _) = result.unwrap();
        assert!(
            (freq - 440.0).abs() < 1.0,
            "detected freq {} should be within 1 Hz of 440",
            freq
        );
    }

    #[test]
    fn test_detect_low_e() {
        let target = 82.41; // E2
        let result = detect(target, 44100.0);
        assert!(result.is_some(), "should detect E2 (82.41 Hz)");
        let (freq, _, _) = result.unwrap();
        assert!(
            (freq - target).abs() < 1.0,
            "detected freq {} should be within 1 Hz of {}", freq, target
        );
    }

    #[test]
    fn test_detect_high_e() {
        let target = 329.63; // E4
        let result = detect(target, 44100.0);
        assert!(result.is_some(), "should detect E4 (329.63 Hz)");
        let (freq, _, _) = result.unwrap();
        assert!(
            (freq - target).abs() < 1.0,
            "detected freq {} should be within 1 Hz of {}", freq, target
        );
    }

    #[test]
    fn test_silence_returns_none() {
        let mut detector = PitchDetector::new(44100.0);
        for _ in 0..ANALYSIS_LEN {
            detector.push_sample(0.0);
        }
        assert!(detector.analyze(440.0).is_none(), "silence should return None");
    }

    #[test]
    fn test_freq_to_midi() {
        let midi = frequency_to_midi(440.0, 440.0);
        assert!((midi - 69.0).abs() < 0.01, "A4 should be midi 69, got {}", midi);

        let midi_c4 = frequency_to_midi(261.63, 440.0);
        assert!((midi_c4 - 60.0).abs() < 0.1, "C4 should be near midi 60, got {}", midi_c4);
    }

    #[test]
    fn test_cents_exact() {
        let (_, cents) = cents_from_nearest(440.0, 440.0);
        assert!(cents.abs() < 0.01, "exact A4 should be 0 cents, got {}", cents);
    }

    #[test]
    fn test_cents_sharp() {
        // One semitone up from A4 is A#4 = 466.16 Hz. Halfway = ~452.9 Hz → ~+50 cents from A4
        let freq_sharp = 440.0 * 2.0f32.powf(10.0 / 1200.0); // +10 cents above A4
        let (_, cents) = cents_from_nearest(freq_sharp, 440.0);
        assert!(cents > 0.0, "frequency above A4 should have positive cents, got {}", cents);
        assert!((cents - 10.0).abs() < 0.5, "expected ~10 cents, got {}", cents);
    }

    #[test]
    fn test_cents_flat() {
        let freq_flat = 440.0 * 2.0f32.powf(-10.0 / 1200.0); // -10 cents below A4
        let (_, cents) = cents_from_nearest(freq_flat, 440.0);
        assert!(cents < 0.0, "frequency below A4 should have negative cents, got {}", cents);
        assert!((cents + 10.0).abs() < 0.5, "expected ~-10 cents, got {}", cents);
    }

    #[test]
    fn test_note_names() {
        assert_eq!(midi_to_note_name(69), "A4");
        assert_eq!(midi_to_note_name(40), "E2");
        assert_eq!(midi_to_note_name(64), "E4");
        assert_eq!(midi_to_note_name(60), "C4");
        assert_eq!(midi_to_note_name(45), "A2");
        assert_eq!(midi_to_note_name(50), "D3");
        assert_eq!(midi_to_note_name(55), "G3");
        assert_eq!(midi_to_note_name(59), "B3");
    }

    #[test]
    fn test_detection_at_48000() {
        let target = 440.0;
        let result = detect(target, 48000.0);
        assert!(result.is_some(), "should detect at 48000 Hz sample rate");
        let (freq, _, _) = result.unwrap();
        assert!(
            (freq - target).abs() < 1.0,
            "detected freq {} should be within 1 Hz of {} at 48kHz", freq, target
        );
    }
}
