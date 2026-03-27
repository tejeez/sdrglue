//! Filter design

use crate::dsp_types::*;
use std::rc::Rc;

/// Design taps for FirComplexSym using windowed sinc method.
pub fn design_fir_lowpass(
    sample_rate: f64,
    cutoff: f64,
    half_length: usize,
) -> Rc<[RealSample]> {
    let sinc_freq = (std::f64::consts::PI * 2.0 * cutoff / sample_rate) as RealSample;
    let window_freq = sample_consts::PI / half_length as RealSample;

    let mut halftaps = (0..half_length).map(|i| {
        let t = i as RealSample + 0.5;
        let sinc_phase = t * sinc_freq;
        sinc_phase.sin() / sinc_phase * (1.0 + (t * window_freq).cos())
    }).collect::<Vec<RealSample>>();

    // Normalize to unity gain at DC.
    // Scale by 0.5 because this is only half of the impulse response
    // and filter has each tap twice.
    let scaling = 0.5 / halftaps.iter().sum::<RealSample>();
    for value in halftaps.iter_mut() {
        *value *= scaling;
    }

    halftaps.into()
}
