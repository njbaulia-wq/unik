//! FluxCut Audio Subsystem.
//!
//! Provides audio waveform downsampling, multi-track mixing, volume envelopes,
//! mute handling, and fade transitions.

pub mod mixer;
pub mod monitor;
pub mod waveform;

pub use mixer::{AudioMixer, AudioSourceSlice};
pub use monitor::AudioMonitor;
pub use waveform::{WaveformBar, WaveformDownsampler};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_waveform_downsampling() {
        let samples = vec![0.1, -0.2, 0.5, -0.8, 0.3, -0.4, 0.9, -0.1];
        let bars = WaveformDownsampler::downsample(&samples, 2);
        assert_eq!(bars.len(), 2);
        assert!(bars[0].max >= 0.5);
        assert!(bars[1].max >= 0.9);
    }

    #[test]
    fn test_audio_mixer_mute_and_fades() {
        let slice = AudioSourceSlice {
            volume: 0.8,
            muted: false,
            fade_in_sec: 1.0,
            fade_out_sec: 1.0,
            duration_sec: 5.0,
        };

        // Halfway through fade-in
        let gain_start = AudioMixer::compute_gain(&slice, 0.5);
        assert!((gain_start - 0.4).abs() < 0.01);

        // In middle (full volume)
        let gain_mid = AudioMixer::compute_gain(&slice, 2.5);
        assert!((gain_mid - 0.8).abs() < 0.01);

        // Halfway through fade-out
        let gain_end = AudioMixer::compute_gain(&slice, 4.5);
        assert!((gain_end - 0.4).abs() < 0.01);

        // Muted
        let mut muted_slice = slice.clone();
        muted_slice.muted = true;
        assert_eq!(AudioMixer::compute_gain(&muted_slice, 2.5), 0.0);
    }

    #[test]
    fn test_audio_buffer_mixing_and_limiting() {
        let track1 = vec![0.6f32; 10];
        let track2 = vec![0.6f32; 10];
        let mut out = vec![0.0f32; 10];

        AudioMixer::mix_buffers(&[(&track1, 1.0), (&track2, 1.0)], &mut out);
        // Total would be 1.2 without limiter, but with limiter it must be <= 1.0
        for sample in out {
            assert!(sample <= 1.0);
            assert!(sample > 0.6);
        }
    }
}
