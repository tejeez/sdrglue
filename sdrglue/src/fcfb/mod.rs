
use std::vec::Vec;
use std::rc::Rc;
use std::sync::Arc;

use rustfft;
use crate::{Sample, ComplexSample, sample_consts};
use crate::num_traits::Zero;

mod sweep;


// ------------------------------------------------
// Buffering helper for both analysis and synthesis
// ------------------------------------------------

#[derive(Copy, Clone)]
pub struct InputBlockSize {
    /// Number of new input samples in each input block.
    pub new:     usize,
    /// Number of overlapping samples between consecutive blocks.
    /// The first "overlap" samples of a block
    /// should be the same as the last samples of the previous block.
    /// Total number of samples in each input block
    /// is equal to the sum of "new" and "overlap".
    pub overlap: usize,
}

pub struct InputBuffer {
    size: InputBlockSize,
    buffer: Vec<ComplexSample>,
}

impl InputBuffer {
    pub fn new(size: InputBlockSize) -> Self {
        Self {
            size,
            buffer: vec![ComplexSample::ZERO; size.new + size.overlap],
        }
    }

    /// Prepare buffer for a new input block.
    /// Return a slice for writing new input samples.
    pub fn prepare_for_new_samples(&mut self) -> &mut [ComplexSample] {
        // Move overlapping part from the end of the previous block to the beginning
        self.buffer.copy_within(self.size.new .. self.size.new + self.size.overlap, 0);
        // Return slice for writing new samples
        &mut self.buffer[self.size.overlap .. self.size.new + self.size.overlap]
    }

    /// Return a slice which can be passed to the process() method of a filter bank.
    pub fn buffer(&self) -> &[ComplexSample] {
        &self.buffer[..]
    }
}


// ----------------------------------------
//           Analysis filter bank
// ----------------------------------------

#[derive(Copy, Clone)]
pub struct AnalysisInputParameters {
    pub fft_size: usize,
    pub input_sample_rate: f64,
    pub input_center_frequency: f64,
}


pub struct AnalysisIntermediateResult {
    fft_result: Vec<ComplexSample>,
}

/// Fast-convolution analysis filter bank.
pub struct AnalysisInputProcessor {
    parameters: AnalysisInputParameters,
    fft_plan: Arc<dyn rustfft::Fft<Sample>>,
    result: AnalysisIntermediateResult,
}

impl AnalysisInputProcessor {
    pub fn new(
        fft_planner: &mut rustfft::FftPlanner<Sample>,
        parameters: AnalysisInputParameters,
    ) -> Self {
        Self {
            parameters,
            fft_plan: fft_planner.plan_fft_forward(parameters.fft_size),
            result: AnalysisIntermediateResult {
                fft_result: vec![ComplexSample::ZERO; parameters.fft_size],
            }
        }
    }

    pub fn input_block_size(&self) -> InputBlockSize {
        // Fixed overlap factor of 50% for now
        InputBlockSize {
            new: self.parameters.fft_size / 2,
            overlap: self.parameters.fft_size / 2,
        }
    }

    pub fn make_input_buffer(&self) -> InputBuffer {
        InputBuffer::new(self.input_block_size())
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
        input: &[ComplexSample],
    ) -> &AnalysisIntermediateResult {
        self.result.fft_result.copy_from_slice(input);
        self.fft_plan.process(&mut self.result.fft_result[..]);
        &self.result
    }
}

#[derive(Clone)]
pub struct AnalysisOutputParameters {
    pub center_bin: isize,
    pub weights: Rc<[Sample]>,
}

impl AnalysisOutputParameters {
    /// Design analysis bank output parameters
    /// for a given output sample rate and frequency.
    pub fn for_frequency(
        analysis_in_params: AnalysisInputParameters,
        output_sample_rate: f64,
        output_center_frequency: f64,
        // TODO: add optional passband_width and transition_band_width if needed
    ) -> Self {
        let ifft_size = (
            output_sample_rate
            * analysis_in_params.fft_size as f64
            / analysis_in_params.input_sample_rate
        ).round() as usize;

        let center_bin = ((
            (output_center_frequency - analysis_in_params.input_center_frequency)
            * analysis_in_params.fft_size as f64
            / analysis_in_params.input_sample_rate
        ).round() as isize
        ).rem_euclid(analysis_in_params.fft_size as isize);

        Self {
            center_bin,
            weights: raised_cosine_weights(ifft_size, None, None),
        }
    }
}

pub struct AnalysisOutputProcessor {
    input_parameters: AnalysisInputParameters,
    parameters: AnalysisOutputParameters,
    ifft_plan: Arc<dyn rustfft::Fft<Sample>>,
    buffer: Vec<ComplexSample>,
}

impl AnalysisOutputProcessor {
    pub fn new(
        fft_planner: &mut rustfft::FftPlanner<Sample>,
        input_parameters: AnalysisInputParameters,
        parameters: AnalysisOutputParameters,
    ) -> Self {
        let ifft_size = parameters.weights.len();
        Self {
            input_parameters,
            parameters: parameters.clone(),
            ifft_plan: fft_planner.plan_fft_inverse(ifft_size),
            buffer: vec![ComplexSample::ZERO; ifft_size],
        }
    }

    pub fn process(
        &mut self,
        intermediate_result: &AnalysisIntermediateResult,
    ) -> &[ComplexSample] {
        assert!(intermediate_result.fft_result.len() == self.input_parameters.fft_size);

        let fft_size = self.input_parameters.fft_size;
        let ifft_size = self.buffer.len();
        let half_size = (ifft_size / 2) as isize;

        // This could probably be optimized a lot.
        // Now it computes each index using modulos which might be slow.
        for bin_number in -half_size .. half_size {
            let bin_index_in = (self.parameters.center_bin + bin_number).rem_euclid(fft_size as isize) as usize;
            let bin_index_out = bin_number.rem_euclid(ifft_size as isize) as usize;
            // Apply weight
            self.buffer[bin_index_out] = self.parameters.weights[bin_index_out] * intermediate_result.fft_result[bin_index_in];
        }

        self.ifft_plan.process(&mut self.buffer);

        // Fixed overlap factor of 50% for now
        &self.buffer[ifft_size/4 .. ifft_size/4 * 3]
    }

    pub fn new_with_frequency(
        fft_planner: &mut rustfft::FftPlanner<Sample>,
        analysis_in_params: AnalysisInputParameters,
        output_sample_rate: f64,
        output_center_frequency: f64,
    ) -> Self {
        AnalysisOutputProcessor::new(
            fft_planner,
            analysis_in_params,
            AnalysisOutputParameters::for_frequency(analysis_in_params, output_sample_rate, output_center_frequency),
        )
    }
}




// ----------------------------------------
//          Synthesis filter bank
// ----------------------------------------

#[derive(Copy, Clone)]
pub struct SynthesisOutputParameters {
    pub ifft_size: usize,
    /// Output sample rate of synthesis bank.
    pub sample_rate: f64,
    /// Output center frequency of synthesis bank.
    pub center_frequency: f64,
}

pub struct SynthesisOutputProcessor {
    parameters: SynthesisOutputParameters,
    ifft_plan: Arc<dyn rustfft::Fft<Sample>>,
    /// Buffer for FFT processing.
    /// The buffer is used to accumulate filter bank inputs
    /// (in frequency domain) before IFFT, and
    /// to store output signal (in time domain) after IFFT.
    /// buffer_state indicates what the buffer currently contains.
    buffer: Vec<ComplexSample>,
    buffer_state: SynthesisBufferState,
}

#[derive(PartialEq)]
enum SynthesisBufferState {
    /// Buffer is full of zeros.
    /// This is the case if no inputs have been added
    /// since the last call to process.
    CLEAR,
    /// Buffer contains inputs to IFFT in frequency domain,
    /// that is, one or more inputs have been added.
    INPUT,
    /// Buffer contains IFFT output.
    /// A slice of the buffer is used to return output signal.
    OUTPUT,
}

pub struct SynthesisIntermediateResult {
    /// Where in synthesis output IFFT input buffer
    /// to add the input FFT results.
    /// Output IFFT bin indexes are input FFT bin index + offset.
    offset: usize,
    fft_result: Vec<ComplexSample>,
}

impl SynthesisOutputProcessor {
    pub fn new(
        fft_planner: &mut rustfft::FftPlanner<Sample>,
        parameters: SynthesisOutputParameters,
    ) -> Self {
        Self {
            parameters,
            ifft_plan: fft_planner.plan_fft_inverse(parameters.ifft_size),
            buffer: vec![ComplexSample::ZERO; parameters.ifft_size],
            buffer_state: SynthesisBufferState::CLEAR,
        }
    }

    pub fn clear(&mut self) {
        for b in self.buffer.iter_mut() {
            *b = ComplexSample::ZERO;
        }
        self.buffer_state = SynthesisBufferState::CLEAR;
    }

    pub fn add(
        &mut self,
        intermediate_result: &SynthesisIntermediateResult,
    ) {
        // If previous result is still in the buffer, clear it
        // before starting to add inputs.
        // This happens for the first input added to a block.
        if self.buffer_state == SynthesisBufferState::OUTPUT {
            self.clear();
        }

        let ifft_size = self.buffer.len();
        for (index, value) in intermediate_result.fft_result.iter().enumerate() {
            // TODO: handle wrap-around without computing a modulo for each bin
            self.buffer[(intermediate_result.offset + index).rem_euclid(ifft_size)] += value;
        }

        self.buffer_state = SynthesisBufferState::INPUT;
    }

    pub fn process(
        &mut self,
    ) -> &[ComplexSample] {
        match self.buffer_state {
            SynthesisBufferState::CLEAR => {
                // No inputs have been added. Buffer is full of zeros.
                // IFFT of zeros is still zeros, so we can skip processing
                // and just return those zeros as the result.
            },
            SynthesisBufferState::INPUT => {
                // The usual case: buffer contains some inputs and
                // now it is time to process them to get the result.
                self.ifft_plan.process(&mut self.buffer);
                self.buffer_state = SynthesisBufferState::OUTPUT;
            },
            SynthesisBufferState::OUTPUT => {
                // No inputs have been added since the last call to process.
                // The buffer still contains the previous result though,
                // so clear it and return those zeros.
                self.clear();
            }
        }

        let ifft_size = self.buffer.len();
        // Fixed overlap factor of 50% for now
        &self.buffer[ifft_size/4 .. ifft_size/4 * 3]
    }
}


#[derive(Clone)]
pub struct SynthesisInputParameters {
    pub center_bin: isize,
    pub weights: Rc<[Sample]>,
}

impl SynthesisInputParameters {
    /// Design synthesis bank input parameters
    /// for a given input sample rate and frequency.
    pub fn for_frequency(
        output_parameters: SynthesisOutputParameters,
        input_sample_rate: f64,
        input_center_frequency: f64,
        // TODO: add optional passband_width and transition_band_width if needed
    ) -> Self {
        let fft_size = (
            input_sample_rate
            * output_parameters.ifft_size as f64
            / output_parameters.sample_rate
        ).round() as usize;

        let center_bin = ((
            (input_center_frequency - output_parameters.center_frequency)
            * output_parameters.ifft_size as f64
            / output_parameters.sample_rate
        ).round() as isize
        ).rem_euclid(output_parameters.ifft_size as isize);

        Self {
            center_bin,
            weights: raised_cosine_weights(fft_size, None, None),
        }
    }
}


pub struct SynthesisInputProcessor {
    weights: Rc<[Sample]>,
    fft_plan: Arc<dyn rustfft::Fft<Sample>>,
    result: SynthesisIntermediateResult,
}

impl SynthesisInputProcessor {
    pub fn new(
        fft_planner: &mut rustfft::FftPlanner<Sample>,
        output_parameters: SynthesisOutputParameters,
        parameters: SynthesisInputParameters,
    ) -> Self {
        let fft_size = parameters.weights.len();
        Self {
            weights: parameters.weights,
            fft_plan: fft_planner.plan_fft_forward(fft_size),
            result: SynthesisIntermediateResult {
                offset:
                    (parameters.center_bin - (fft_size / 2) as isize)
                    .rem_euclid(output_parameters.ifft_size as isize) as usize,
                fft_result: vec![ComplexSample::ZERO; fft_size],
            }
        }
    }

    pub fn process(
        &mut self,
        input: &[ComplexSample],
    ) -> &SynthesisIntermediateResult {
        self.result.fft_result.copy_from_slice(input);
        self.fft_plan.process(&mut self.result.fft_result[..]);

        // Apply weights
        for (value, weight) in self.result.fft_result.iter_mut().zip(self.weights.iter()) {
            *value *= weight;
        }

        // Swap halves for simpler indexing when results are added
        // to IFFT input. This might not be the most efficient way to do it.
        let fft_size_half = self.result.fft_result.len() / 2;
        for i in 0 .. fft_size_half {
            self.result.fft_result.swap(i, fft_size_half + i);
        }

        &self.result
    }

    pub fn input_block_size(&self) -> InputBlockSize {
        let fft_size = self.result.fft_result.len();
        // Fixed overlap factor of 50% for now
        InputBlockSize {
            new: fft_size / 2,
            overlap: fft_size / 2,
        }
    }

    pub fn make_input_buffer(&self) -> InputBuffer {
        InputBuffer::new(self.input_block_size())
    }

    pub fn new_with_frequency(
        fft_planner: &mut rustfft::FftPlanner<Sample>,
        output_parameters: SynthesisOutputParameters,
        input_sample_rate: f64,
        input_center_frequency: f64,
    ) -> Self {
        Self::new(
            fft_planner,
            output_parameters,
            SynthesisInputParameters::for_frequency(output_parameters, input_sample_rate, input_center_frequency),
        )
    }
}



// ----------------------------------------
//          Filter bank design
// ----------------------------------------


/// Design raised cosine weights for a given IFFT size,
/// passband width and transition band width (given as number of bins).
/// Use None for default values.
pub fn raised_cosine_weights(
    ifft_size: usize,
    passband_bins: Option<usize>,
    transition_bins: Option<usize>,
) -> Rc<[Sample]> {
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

    let mut weights = vec![Sample::zero(); ifft_size];
    for i in 0 .. passband_half {
        weights[i] = 1.0;
        if i != 0 {
            weights[ifft_size - i] = 1.0;
        }
    }
    for i in 0 .. transition_bins_ {
        let v = 0.5 + 0.5 * (sample_consts::PI * (i+1) as Sample / (transition_bins_+1) as Sample).cos();
        let j = passband_half + i;
        weights[j] = v;
        if j != 0 {
            weights[ifft_size - j] = v;
        }
    }

    Rc::<[Sample]>::from(weights)
}


// ----------------------------------------
//                 Tests
// ----------------------------------------

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
            input_center_frequency: 0.0,
            // There is no test for AnalysisOutputProcessor::new_with_frequency yet,
            // so input sample rate does not matter.
            input_sample_rate: 10000.0,
        };
        let output_parameters = AnalysisOutputParameters {
            center_bin: 10,
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
    fn test_synthesis() {
        let mut fft_planner = rustfft::FftPlanner::new();
        let mut sweepgen = sweep::SweepGenerator::new(100000);
        let output_parameters = SynthesisOutputParameters {
            ifft_size: 1000,
            center_frequency: 0.0,
            sample_rate: 100000.0,
        };

        let mut sy = SynthesisOutputProcessor::new(&mut fft_planner, output_parameters);
        let mut sy_input = SynthesisInputProcessor::new_with_frequency(&mut fft_planner, output_parameters, 10000.0, 20000.0);

        let mut input_buffer = sy_input.make_input_buffer();

        let mut output_file = std::fs::File::create("test_results/synthesis_output.cf32").unwrap();

                for _ in 0..10000 {
            for sample in input_buffer.prepare_for_new_samples() {
                *sample = sweepgen.sample();
            }

            sy.add(sy_input.process(input_buffer.buffer()));
            let result = sy.process();

            for sample in result {
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
            assert!(weights[0] == 1.0);
            // Check that it falls to zero at Nyquist frequency
            assert!(weights[ifft_size/2] == 0.0);
        }
        test(32, Some(9), Some(4));
        test(100, None, None);
        test(16, None, None);
    }
}
