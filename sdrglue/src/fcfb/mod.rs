
use std::vec::Vec;
use std::rc::Rc;
use std::sync::Arc;

use rustfft;
use crate::num_complex::Complex32;
use crate::num_traits::Zero;

mod sweep;


#[derive(Copy, Clone)]
pub struct AnalysisInputParameters {
    pub fft_size: usize,
}

#[derive(Copy, Clone)]
pub struct AnalysisInputBlockSize {
    /// Number of new input samples in each input block.
    pub new:     usize,
    /// Number of overlapping samples between consecutive blocks.
    /// The first "overlap" samples of a block
    /// should be the same as the last samples of the previous block.
    /// Total number of samples in each input block
    /// is equal to the sum of "new" and "overlap".
    pub overlap: usize,
}

pub struct AnalysisInputBuffer {
    size: AnalysisInputBlockSize,
    buffer: Vec<Complex32>,
}

impl AnalysisInputBuffer {
    pub fn new(size: AnalysisInputBlockSize) -> Self {
        Self {
            size,
            buffer: vec![Complex32::zero(); size.new + size.overlap],
        }
    }

    /// Prepare buffer for a new input block.
    /// Return a slice for writing new input samples.
    pub fn prepare_for_new_samples(&mut self) -> &mut [Complex32] {
        // Move overlapping part from the end of the previous block to the beginning
        self.buffer.copy_within(self.size.new .. self.size.new + self.size.overlap, 0);
        // Return slice for writing new samples
        &mut self.buffer[self.size.overlap .. self.size.new + self.size.overlap]
    }

    /// Return a slice which can be passed to the process() method of a filter bank.
    pub fn buffer(&self) -> &[Complex32] {
        &self.buffer[..]
    }
}


pub struct AnalysisIntermediateResult {
    fft_result: Vec<Complex32>,
}

/// Fast-convolution analysis filter bank.
pub struct AnalysisInputProcessor {
    parameters: AnalysisInputParameters,
    fft_plan: Arc<dyn rustfft::Fft<f32>>,
    result: AnalysisIntermediateResult,
}

impl AnalysisInputProcessor {
    pub fn new(
        fft_planner: &mut rustfft::FftPlanner<f32>,
        parameters: AnalysisInputParameters,
    ) -> Self {
        Self {
            parameters,
            fft_plan: fft_planner.plan_fft_forward(parameters.fft_size),
            result: AnalysisIntermediateResult {
                fft_result: vec![Complex32::zero(); parameters.fft_size],
            }
        }
    }

    pub fn input_block_size(&self) -> AnalysisInputBlockSize {
        // Fixed overlap factor of 50% for now
        AnalysisInputBlockSize {
            new: self.parameters.fft_size / 2,
            overlap: self.parameters.fft_size / 2,
        }
    }

    pub fn make_input_buffer(&self) -> AnalysisInputBuffer {
        AnalysisInputBuffer::new(self.input_block_size())
    }

    /// Input samples should overlap between consequent blocks.
    /// The first "overlap" samples of a block
    /// should be the same as the last samples of the previous block.
    /// The numbers of samples are returned by the input_block_size()
    /// method, returning an AnalysisInputBlockSize struct.
    ///
    /// Caller may implement overlap in any way it wants, for example,
    /// by giving overlapping slices from a larger buffer,
    /// or by copying the end of the previous block
    /// to the beginning of the current block.
    /// The latter can be done using the AnalysisInputBuffer struct
    /// which can be constructed using the make_input_buffer() method.
    pub fn process(
        &mut self,
        input: &[Complex32],
    ) -> &AnalysisIntermediateResult {
        self.result.fft_result.copy_from_slice(input);
        self.fft_plan.process(&mut self.result.fft_result[..]);
        &self.result
    }
}

#[derive(Clone)]
pub struct AnalysisOutputParameters {
    pub center_bin: isize,
    pub weights: Rc<[f32]>,
}

pub struct AnalysisOutputProcessor {
    input_parameters: AnalysisInputParameters,
    parameters: AnalysisOutputParameters,
    ifft_plan: Arc<dyn rustfft::Fft<f32>>,
    buffer: Vec<Complex32>,
}

impl AnalysisOutputProcessor {
    pub fn new(
        fft_planner: &mut rustfft::FftPlanner<f32>,
        input_parameters: AnalysisInputParameters,
        parameters: AnalysisOutputParameters,
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
        intermediate_result: &AnalysisIntermediateResult,
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


/// Design raised cosine weights for a given IFFT size,
/// passband width and transition band width (given as number of bins).
/// Use None for default values.
fn raised_cosine_weights(
    ifft_size: usize,
    passband_bins: Option<usize>,
    transition_bins: Option<usize>,
) -> Rc<[f32]> {
    // I am not sure if it this would work correctly for an odd size,
    // but an overlap factor of 1/2 requires an even IFFT size anyway,
    // so check for that.
    // Maybe returning an error instead of panicing with invalid values
    // would be better though.
    assert!(ifft_size % 2 == 0);

    let default_max_transition = 15;
    let transition_bins_ = transition_bins.unwrap_or(default_max_transition.min(ifft_size/2 - 1));
    let passband_half = passband_bins.unwrap_or(ifft_size - 2 - 2*transition_bins_) / 2 + 1;

    assert!(passband_half + transition_bins_ <= ifft_size/2);

    let center = ifft_size / 2;

    let mut weights = vec![0.0f32; ifft_size];
    for i in 0 .. passband_half {
        weights[center + i] = 1.0;
        weights[center - i] = 1.0;
    }
    for i in 0 .. transition_bins_ {
        let v = 0.5 + 0.5 * (std::f32::consts::PI * (i+1) as f32 / (transition_bins_+1) as f32).cos();
        weights[center + (passband_half + i)] = v;
        weights[center - (passband_half + i)] = v;
    }

    Rc::<[f32]>::from(weights)
}


#[cfg(test)]
mod tests {
    use std::io::Write;
    use byteorder::{self, ByteOrder};

    use super::*;
    use sweep;

    #[test]
    fn test_analysis() {
        let mut fft_planner = rustfft::FftPlanner::new();
        let sweep_length = 1000000;
        let mut sweepgen = sweep::SweepGenerator::new(sweep_length);
        let input_parameters = AnalysisInputParameters {
            fft_size: 1000,
        };
        let output_parameters = AnalysisOutputParameters {
            center_bin: 300,
            weights: raised_cosine_weights(100, None, None),
        };
        let mut an = AnalysisInputProcessor::new(&mut fft_planner, input_parameters);
        let mut an_output = AnalysisOutputProcessor::new(&mut fft_planner, input_parameters, output_parameters);

        let mut input_buffer = an.make_input_buffer();

        // Write output to a file so it can be manually inspected.
        // The result is not automatically checked for anything for now.
        let mut output_file = std::fs::File::create("test_results/analysis_output.cf32").unwrap();

        for _ in 0..(sweep_length / (input_parameters.fft_size/2) as u64) {
            for sample in input_buffer.prepare_for_new_samples() {
                *sample = sweepgen.sample();
            }

            let intermediate_result = an.process(input_buffer.buffer());

            let result = an_output.process(intermediate_result);

            for sample in result {
                // Write sample in little-endian interleaved format
                let mut buf = [0u8; 8];
                byteorder::LittleEndian::write_f32(&mut buf[0..4], sample.re);
                byteorder::LittleEndian::write_f32(&mut buf[4..8], sample.im);
                output_file.write_all(&buf[..]).unwrap();
            }
        }
    }

    #[test]
    fn test_weights() {
        fn test(
            ifft_size: usize,
            passband_bins: Option<usize>,
            transition_bins: Option<usize>,
        ) {
            let weights = raised_cosine_weights(ifft_size, passband_bins, transition_bins);
            println!("{:?}", weights);
            // Check that "DC" bin is 1.0
            assert!(weights[ifft_size/2] == 1.0);
            // Check that it falls to zero at Nyquist frequency
            assert!(weights[0] == 0.0);
        }
        test(32, Some(9), Some(4));
        test(100, None, None);
        test(16, None, None);
    }
}
