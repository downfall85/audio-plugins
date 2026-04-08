use nih_plug::prelude::*;
use std::sync::Arc;

mod editor;

// Comb filter lengths at 44100 Hz (Freeverb standard).
const COMB_TUNING_L: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const COMB_R_OFFSET: usize = 23;
// Allpass filter lengths at 44100 Hz.
const ALLPASS_TUNING: [usize; 4] = [556, 441, 341, 225];

pub struct CombFilter {
    buffer: Vec<f32>,
    pos: usize,
    filterstore: f32,
}

impl CombFilter {
    pub fn new(len: usize) -> Self {
        Self {
            buffer: vec![0.0; len],
            pos: 0,
            filterstore: 0.0,
        }
    }

    pub fn resize(&mut self, len: usize) {
        self.buffer.resize(len, 0.0);
        // Trim if shorter
        self.buffer.truncate(len);
        self.pos = self.pos.min(len.saturating_sub(1));
    }

    pub fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.filterstore = 0.0;
        self.pos = 0;
    }

    pub fn process(&mut self, input: f32, room_size: f32, damp: f32) -> f32 {
        if self.buffer.is_empty() {
            return input;
        }
        let output = self.buffer[self.pos];
        self.filterstore = output * (1.0 - damp) + self.filterstore * damp;
        self.buffer[self.pos] = input + self.filterstore * room_size;
        self.pos = (self.pos + 1) % self.buffer.len();
        output
    }
}

pub struct AllpassFilter {
    buffer: Vec<f32>,
    pos: usize,
}

impl AllpassFilter {
    pub fn new(len: usize) -> Self {
        Self {
            buffer: vec![0.0; len],
            pos: 0,
        }
    }

    pub fn resize(&mut self, len: usize) {
        self.buffer.resize(len, 0.0);
        self.buffer.truncate(len);
        self.pos = self.pos.min(len.saturating_sub(1));
    }

    pub fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.pos = 0;
    }

    pub fn process(&mut self, input: f32) -> f32 {
        if self.buffer.is_empty() {
            return input;
        }
        let bufout = self.buffer[self.pos];
        let output = -input + bufout;
        self.buffer[self.pos] = input + bufout * 0.5;
        self.pos = (self.pos + 1) % self.buffer.len();
        output
    }
}

/// Per-channel Freeverb DSP.
pub struct FreeverbChannel {
    pub combs: Vec<CombFilter>,
    pub allpasses: Vec<AllpassFilter>,
}

impl FreeverbChannel {
    pub fn new(sample_rate: f32, right_channel: bool) -> Self {
        let offset = if right_channel { COMB_R_OFFSET } else { 0 };
        let sr_scale = sample_rate / 44100.0;

        let combs = COMB_TUNING_L
            .iter()
            .map(|&len| {
                let scaled = ((len + offset) as f32 * sr_scale).round() as usize;
                CombFilter::new(scaled.max(1))
            })
            .collect();

        let allpasses = ALLPASS_TUNING
            .iter()
            .map(|&len| {
                let scaled = (len as f32 * sr_scale).round() as usize;
                AllpassFilter::new(scaled.max(1))
            })
            .collect();

        Self { combs, allpasses }
    }

    pub fn resize(&mut self, sample_rate: f32, right_channel: bool) {
        let offset = if right_channel { COMB_R_OFFSET } else { 0 };
        let sr_scale = sample_rate / 44100.0;

        for (i, comb) in self.combs.iter_mut().enumerate() {
            let base_len = COMB_TUNING_L[i] + offset;
            let scaled = (base_len as f32 * sr_scale).round() as usize;
            comb.resize(scaled.max(1));
        }

        for (i, ap) in self.allpasses.iter_mut().enumerate() {
            let scaled = (ALLPASS_TUNING[i] as f32 * sr_scale).round() as usize;
            ap.resize(scaled.max(1));
        }
    }

    pub fn reset(&mut self) {
        for c in self.combs.iter_mut() {
            c.reset();
        }
        for a in self.allpasses.iter_mut() {
            a.reset();
        }
    }

    pub fn process(&mut self, input: f32, room_size: f32, damp: f32) -> f32 {
        // Sum 8 combs in parallel
        let comb_out: f32 = self
            .combs
            .iter_mut()
            .map(|c| c.process(input, room_size, damp))
            .sum::<f32>()
            / 8.0;

        // Chain 4 allpass in series
        let mut ap_out = comb_out;
        for ap in self.allpasses.iter_mut() {
            ap_out = ap.process(ap_out);
        }
        ap_out
    }
}

#[derive(Params)]
pub struct ReverbParams {
    #[persist = "editor-state"]
    editor_state: Arc<nih_plug_iced::IcedState>,

    #[id = "room_size"]
    pub room_size: FloatParam,

    #[id = "damping"]
    pub damping: FloatParam,

    #[id = "wet"]
    pub wet: FloatParam,
}

impl Default for ReverbParams {
    fn default() -> Self {
        Self {
            editor_state: editor::default_state(),
            room_size: FloatParam::new(
                "Room Size",
                0.5,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(5.0)),
            damping: FloatParam::new(
                "Damping",
                0.5,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(5.0)),
            wet: FloatParam::new(
                "Wet",
                0.33,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(5.0)),
        }
    }
}

pub struct Reverb {
    params: Arc<ReverbParams>,
    sample_rate: f32,
    channels: [FreeverbChannel; 2],
}

impl Default for Reverb {
    fn default() -> Self {
        Self {
            params: Arc::new(ReverbParams::default()),
            sample_rate: 44100.0,
            channels: [
                FreeverbChannel::new(44100.0, false),
                FreeverbChannel::new(44100.0, true),
            ],
        }
    }
}

impl Plugin for Reverb {
    const NAME: &'static str = "Reverb";
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
        self.channels[0].resize(buffer_config.sample_rate, false);
        self.channels[1].resize(buffer_config.sample_rate, true);
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
            let room_size = self.params.room_size.smoothed.next();
            let damp = self.params.damping.smoothed.next();
            let wet = self.params.wet.smoothed.next();

            for (ch, sample) in channel_samples.iter_mut().enumerate() {
                let ch_idx = ch.min(1);
                let reverb_out = self.channels[ch_idx].process(*sample, room_size, damp);
                *sample = *sample * (1.0 - wet) + reverb_out * wet;
            }
        }

        // Reverb tail: longest comb buffer length
        let max_comb = COMB_TUNING_L[7] + COMB_R_OFFSET;
        let sr_scale = self.sample_rate / 44100.0;
        let tail_samples = (max_comb as f32 * sr_scale * 10.0) as u32; // generous tail
        ProcessStatus::Tail(tail_samples)
    }
}

impl Vst3Plugin for Reverb {
    const VST3_CLASS_ID: [u8; 16] = *b"GuitarReverbPlg0";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Reverb];
}

nih_export_vst3!(Reverb);

#[cfg(test)]
mod tests {
    use super::*;

    fn make_reverb_channel(sample_rate: f32) -> FreeverbChannel {
        FreeverbChannel::new(sample_rate, false)
    }

    fn process_with_wet(
        channel: &mut FreeverbChannel,
        input: f32,
        room_size: f32,
        damp: f32,
        wet: f32,
    ) -> f32 {
        let reverb_out = channel.process(input, room_size, damp);
        input * (1.0 - wet) + reverb_out * wet
    }

    #[test]
    fn test_wet_zero_is_dry() {
        let mut ch = make_reverb_channel(44100.0);
        let input = 0.5_f32;
        let out = process_with_wet(&mut ch, input, 0.5, 0.5, 0.0);
        assert!(
            (out - input).abs() < 1e-6,
            "wet=0 should output the dry signal unchanged"
        );
    }

    #[test]
    fn test_wet_one_dry_gone() {
        let mut ch = make_reverb_channel(44100.0);
        // Feed impulse then silence; with wet=1 and room_size>0 there will be reverb output
        // but the dry signal itself should not be directly present.
        // At t=0 the comb buffers are empty → output = 0 (no dry component)
        let out = process_with_wet(&mut ch, 1.0, 0.5, 0.5, 1.0);
        // At the very first sample the comb output is 0 (buffers are empty), so output = 0.
        assert!(
            (out).abs() < 1e-6,
            "wet=1 at t=0 should have no output (empty buffers): got {}",
            out
        );
    }

    #[test]
    fn test_reverb_adds_tail() {
        let mut ch = make_reverb_channel(44100.0);
        // Feed one impulse
        process_with_wet(&mut ch, 1.0, 0.8, 0.5, 1.0);
        // Feed silence and check that we get non-zero output (the reverb tail)
        let mut has_tail = false;
        for _ in 0..5000 {
            let out = process_with_wet(&mut ch, 0.0, 0.8, 0.5, 1.0);
            if out.abs() > 1e-6 {
                has_tail = true;
                break;
            }
        }
        assert!(has_tail, "reverb should produce a tail after impulse");
    }

    #[test]
    fn test_room_size_zero_kills_tail() {
        let mut ch = make_reverb_channel(44100.0);
        // Feed impulse with room_size=0 (no feedback — comb buffers play out once then go silent).
        process_with_wet(&mut ch, 1.0, 0.0, 0.5, 1.0);
        // With room_size=0 there is no feedback, but the allpass buffers decay by 0.5 per
        // cycle. Allow enough cycles for all allpass buffers (longest = 556 at 44100 Hz) to
        // fully decay — 20000 samples gives >30 full passes through allpass[0].
        let drain_samples = 20_000_usize;
        for _ in 0..drain_samples {
            process_with_wet(&mut ch, 0.0, 0.0, 0.5, 1.0);
        }
        // Now the buffers should be silent — no feedback loop possible with room_size=0.
        let mut energy_after_drain = 0.0_f32;
        for _ in 0..500 {
            let out = process_with_wet(&mut ch, 0.0, 0.0, 0.5, 1.0);
            energy_after_drain += out * out;
        }
        assert!(
            energy_after_drain < 1e-6,
            "room_size=0 should have no energy after buffer has drained, energy={}",
            energy_after_drain
        );
    }

    #[test]
    fn test_output_is_stable() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut ch = make_reverb_channel(44100.0);
        // Deterministic pseudo-noise via hashing
        let mut hasher = DefaultHasher::new();
        for i in 0_u64..1000 {
            i.hash(&mut hasher);
            let bits = hasher.finish();
            // Map to [-1, 1]
            let noise = (bits as f32 / u64::MAX as f32) * 2.0 - 1.0;
            let out = process_with_wet(&mut ch, noise, 0.8, 0.5, 0.5);
            assert!(
                out.is_finite(),
                "output must be finite at sample {}, got {}",
                i,
                out
            );
        }
    }

    #[test]
    fn test_reverb_buffer_lengths_scale() {
        let ch_44k = FreeverbChannel::new(44100.0, false);
        let ch_48k = FreeverbChannel::new(48000.0, false);

        let expected_44k = COMB_TUNING_L[0]; // 1116
        let expected_48k = (COMB_TUNING_L[0] as f32 * 48000.0 / 44100.0).round() as usize;

        assert_eq!(ch_44k.combs[0].buffer.len(), expected_44k);
        assert_eq!(ch_48k.combs[0].buffer.len(), expected_48k);
        assert_ne!(
            expected_44k, expected_48k,
            "buffer lengths should differ between sample rates"
        );
    }
}
