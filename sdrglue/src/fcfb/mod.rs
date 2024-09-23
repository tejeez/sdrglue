
use std::vec::Vec;
use std::rc::Rc;
use std::sync::Arc;

use rustfft;
use rustfft::num_complex::Complex32;
use rustfft::num_traits::Zero;


#[derive(Copy, Clone)]
pub struct DdcInputParameters {
    pub fft_size: usize,
}

pub struct DdcIntermediateResult {
    fft_result: Vec<Complex32>,
}

/// Fast-convolution filter bank for digital down-conversion.
pub struct DdcInputProcessor {
    parameters: DdcInputParameters,
    fft_plan: Arc<dyn rustfft::Fft<f32>>,
    result: DdcIntermediateResult,
}

impl DdcInputProcessor {
    pub fn new(
        fft_planner: &mut rustfft::FftPlanner<f32>,
        parameters: DdcInputParameters,
    ) -> Self {
        Self {
            parameters,
            fft_plan: fft_planner.plan_fft_forward(parameters.fft_size),
            result: DdcIntermediateResult {
                fft_result: vec![Complex32::zero(); parameters.fft_size],
            }
        }
    }

    pub fn new_input_samples_per_block(&self) -> usize {
        // Fixed overlap factor of 50% for now
        self.parameters.fft_size / 2
    }

    pub fn overlapping_input_samples_per_block(&self) -> usize {
        // Fixed overlap factor of 50% for now
        self.parameters.fft_size / 2
    }

    /// Input samples should overlap between consequent blocks.
    ///
    /// The total length of an input block is given by
    /// new_input_samples_per_block() + overlapping_input_samples_per_block().
    /// The first overlapping_input_samples_per_block() samples of a block
    /// should be the same as the last samples of the previous block.
    ///
    /// Caller may implement overlap in any way it wants, for example,
    /// by giving overlapping slices from a larger buffer,
    /// or by copying the end of the previous block
    /// to the beginning of the current block.
    pub fn process(
        &mut self,
        input: &[Complex32],
    ) -> &DdcIntermediateResult {
        self.result.fft_result.copy_from_slice(input);
        self.fft_plan.process(&mut self.result.fft_result[..]);
        &self.result
    }
}

#[derive(Clone)]
pub struct DdcOutputParameters {
    pub center_bin: isize,
    pub weights: Rc<[f32]>,
}

pub struct DdcOutputProcessor {
    input_parameters: DdcInputParameters,
    parameters: DdcOutputParameters,
    ifft_plan: Arc<dyn rustfft::Fft<f32>>,
    buffer: Vec<Complex32>,
}

impl DdcOutputProcessor {
    pub fn new(
        fft_planner: &mut rustfft::FftPlanner<f32>,
        input_parameters: DdcInputParameters,
        parameters: DdcOutputParameters,
    ) -> Self {
        let ifft_size = parameters.weights.len();
        Self {
            input_parameters,
            parameters: parameters.clone(),
            ifft_plan: fft_planner.plan_fft_inverse(ifft_size),
            buffer: vec![Complex32::zero(); ifft_size],
        }
    }

    pub fn process(
        &mut self,
        intermediate_result: &DdcIntermediateResult,
    ) -> &[Complex32] {
        let ifft_size = self.buffer.len();
        let half_size = ifft_size / 2;

        let param_center_bin = self.parameters.center_bin;
        // Convert bin "frequencies" to indexes to the FFT result vector.
        let center_bin = param_center_bin.rem_euclid(ifft_size as isize) as usize;
        let first_bin = (param_center_bin - half_size as isize).rem_euclid(ifft_size as isize) as usize;
        let last_bin  = (param_center_bin + half_size as isize).rem_euclid(ifft_size as isize) as usize;

        fn apply_weights(
            output: &mut [Complex32],
            weights: &[f32],
            input: &[Complex32],
        ) {
            for (out, (weight, in_)) in output.iter_mut().zip(weights.iter().zip(input.iter())) {
                *out = in_ * weight;
            }
        }
        // Negative output frequencies
        apply_weights(
            &mut self.buffer[half_size .. ifft_size],
            &self.parameters.weights[0 .. half_size],
            &intermediate_result.fft_result[first_bin .. center_bin]
        );
        // Positive output frequencies
        apply_weights(
            &mut self.buffer[0 .. half_size],
            &self.parameters.weights[half_size .. ifft_size],
            &intermediate_result.fft_result[center_bin .. last_bin]
        );

        self.ifft_plan.process(&mut self.buffer);

        // Fixed overlap factor of 50% for now
        &self.buffer[ifft_size/4 .. ifft_size/4 * 3]
    }
}
