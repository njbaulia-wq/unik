//! Waveform amplitude downsampler for responsive timeline visual rendering.

use serde::{Deserialize, Serialize};

/// Downsampled audio waveform bucket for rendering.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WaveformBar {
    pub min: f32,
    pub max: f32,
    pub rms: f32,
}

pub struct WaveformDownsampler;

impl WaveformDownsampler {
    /// Downsample raw amplitude samples into a target count of visual bars.
    pub fn downsample(samples: &[f32], target_bars: usize) -> Vec<WaveformBar> {
        if samples.is_empty() || target_bars == 0 {
            return Vec::new();
        }

        let chunk_size = ((samples.len() as f64) / (target_bars as f64)).max(1.0);
        let mut bars = Vec::with_capacity(target_bars);

        for i in 0..target_bars {
            let start = (i as f64 * chunk_size).floor() as usize;
            let end = (((i + 1) as f64 * chunk_size).ceil() as usize).min(samples.len());

            if start >= samples.len() {
                break;
            }

            let slice = &samples[start..end];
            if slice.is_empty() {
                continue;
            }

            let mut min = 0.0f32;
            let mut max = 0.0f32;
            let mut sum_sq = 0.0f64;

            for &sample in slice {
                if sample < min {
                    min = sample;
                }
                if sample > max {
                    max = sample;
                }
                sum_sq += (sample as f64) * (sample as f64);
            }

            let rms = (sum_sq / slice.len() as f64).sqrt() as f32;
            bars.push(WaveformBar { min, max, rms });
        }

        bars
    }
}
