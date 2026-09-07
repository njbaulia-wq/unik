//! Software audio mixer with volume envelope, mute, fade calculations, and clipping limiter.

/// Audio slice configuration for mixing.
#[derive(Debug, Clone)]
pub struct AudioSourceSlice {
    pub volume: f32,
    pub muted: bool,
    pub fade_in_sec: f32,
    pub fade_out_sec: f32,
    pub duration_sec: f32,
}

impl Default for AudioSourceSlice {
    fn default() -> Self {
        Self {
            volume: 1.0,
            muted: false,
            fade_in_sec: 0.0,
            fade_out_sec: 0.0,
            duration_sec: 0.0,
        }
    }
}

pub struct AudioMixer;

impl AudioMixer {
    /// Calculate the instantaneous gain multiplier at a specific time within a clip slice.
    pub fn compute_gain(slice: &AudioSourceSlice, local_time_sec: f32) -> f32 {
        if slice.muted || slice.volume <= 0.0001 {
            return 0.0;
        }

        let mut gain = slice.volume;

        // Apply fade-in envelope
        if slice.fade_in_sec > 0.001 && local_time_sec < slice.fade_in_sec {
            let fade_factor = (local_time_sec / slice.fade_in_sec).clamp(0.0, 1.0);
            gain *= fade_factor;
        }

        // Apply fade-out envelope
        if slice.fade_out_sec > 0.001 && local_time_sec > (slice.duration_sec - slice.fade_out_sec)
        {
            let time_from_end = (slice.duration_sec - local_time_sec).max(0.0);
            let fade_factor = (time_from_end / slice.fade_out_sec).clamp(0.0, 1.0);
            gain *= fade_factor;
        }

        gain
    }

    /// Mix multiple audio channels/buffers into a master output buffer.
    pub fn mix_buffers(tracks: &[(&[f32], f32)], output: &mut [f32]) {
        for sample in output.iter_mut() {
            *sample = 0.0;
        }

        for (track_buf, track_vol) in tracks {
            if *track_vol <= 0.0001 {
                continue;
            }
            let len = track_buf.len().min(output.len());
            for i in 0..len {
                output[i] += track_buf[i] * track_vol;
            }
        }

        // Soft limiter to prevent clipping outside [-1.0, 1.0]
        for sample in output.iter_mut() {
            if *sample > 1.0 {
                *sample = (1.0 - (-*sample).exp()).min(1.0);
            } else if *sample < -1.0 {
                *sample = -((1.0 - (sample.abs()).exp()).min(1.0));
            }
        }
    }
}
