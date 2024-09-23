
use std::vec::Vec;
use std::rc::Rc;
use std::sync::Arc;

use rustfft;
use rustfft::num_complex::Complex32;
use rustfft::num_traits::Zero;

mod sweep;


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
        let fft_size = self.input_parameters.fft_size;
        let ifft_size = self.buffer.len();
        let half_size = ifft_size / 2;

        let param_center_bin = self.parameters.center_bin;
        // Convert bin "frequencies" to indexes to the FFT result vector.
        let center_bin = param_center_bin.rem_euclid(fft_size as isize) as usize;
        let first_bin = (param_center_bin - half_size as isize).rem_euclid(fft_size as isize) as usize;
        let last_bin  = (param_center_bin + half_size as isize).rem_euclid(fft_size as isize) as usize;

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


#[cfg(test)]
mod tests {
    use std::io::Write;
    use byteorder::{self, ByteOrder};

    use super::*;
    use sweep;

    #[test]
    fn test_ddc() {
        let mut fft_planner = rustfft::FftPlanner::new();
        let mut sweepgen = sweep::SweepGenerator::new(100000000);
        let input_parameters = DdcInputParameters {
            fft_size: 1000,
        };
        let output_parameters = DdcOutputParameters {
            center_bin: 300,
            // TODO: proper weights
            weights: Rc::<[f32]>::from([1.0f32; 64]),
        };
        let mut ddc = DdcInputProcessor::new(&mut fft_planner, input_parameters);
        let mut ddc_output = DdcOutputProcessor::new(&mut fft_planner, input_parameters, output_parameters);

        let n_new = ddc.new_input_samples_per_block();
        let n_overlapping = ddc.overlapping_input_samples_per_block();
        let n_total = n_new + n_overlapping;

        let mut input_buffer = vec![Complex32::zero(); n_total];

        // Write output to a file so it can be manually inspected.
        // The result is not automatically checked for anything for now.
        let mut output_file = std::fs::File::create("test_results/ddc_output.cf32").unwrap();

        for _ in 0..200000 {
            // Move overlapping part
            input_buffer.copy_within(n_new .. n_total, 0);
            // Add new samples after the overlapping part
            for sample in input_buffer[n_overlapping .. n_total].iter_mut() {
                *sample = sweepgen.sample();
            }

            let intermediate_result = ddc.process(&input_buffer[..]);

            let result = ddc_output.process(intermediate_result);

            for sample in result {
                // Write sample in little-endian interleaved format
                let mut buf = [0u8; 8];
                byteorder::LittleEndian::write_f32(&mut buf[0..4], sample.re);
                byteorder::LittleEndian::write_f32(&mut buf[4..8], sample.im);
                output_file.write_all(&buf[..]).unwrap();
            }
        }
    }
}
