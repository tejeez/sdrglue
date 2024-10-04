//! Filter design

use crate::{Sample, sample_consts};
use super::fir;

/// Design taps for FirCf32Sym using windowed sinc method.
pub fn design_fir_lowpass(
    sample_rate: f64,
    cutoff: f64,
    half_length: usize,
) -> fir::SymmetricRealTaps {
    let sinc_freq = (std::f64::consts::PI * 2.0 * cutoff / sample_rate) as Sample;
    let window_freq = sample_consts::PI / half_length as Sample;

    let mut halftaps = (0..half_length).map(|i| {
        let t = i as Sample + 0.5;
        let sinc_phase = t * sinc_freq;
        sinc_phase.sin() / sinc_phase * (1.0 + (t * window_freq).cos())
    }).collect::<Vec<Sample>>();

    // Normalize to unity gain at DC.
    // Scale by 0.5 because this is only half of the impulse response
    // and filter has each tap twice.
    let scaling = 0.5 / halftaps.iter().sum::<Sample>();
    for value in halftaps.iter_mut() {
        *value *= scaling;
    }

    fir::convert_symmetric_real_taps(&halftaps[..])
}
