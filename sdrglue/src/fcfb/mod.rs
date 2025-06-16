
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


// -------------------------------------------
// Common code for both analysis and synthesis
// -------------------------------------------

/// Overlap factor
#[derive(Copy, Clone, PartialEq)]
pub enum Overlap {
    // Overlap factor of 1/2
    O1_2,
    // Overlap factor of 1/4
    O1_4,
}

/// Overlapping amount needs to be an integer number of samples.
/// This means FFT size must be a multiple of the denominator
/// of overlap factor.
fn required_fft_size_factor(overlap: Overlap) -> usize {
    match overlap {
        Overlap::O1_2 => 2,
        Overlap::O1_4 => 4,
    }
}

/// Check that FFT size is a multiple of required_fft_size_factor.
/// For now it panics but the code could be changed to return errors too.
fn check_fft_size(fft_size: usize, overlap: Overlap) {
    let required = required_fft_size_factor(overlap);
    if fft_size % required != 0 {
        panic!("FFT size must be a multiple of {}. {} is not.", required, fft_size);
    }
}

/// Compute input block size for a given FFT/IFFT size and overlap factor.
fn input_block_size(fft_size: usize, overlap: Overlap) -> InputBlockSize {
    match overlap {
        Overlap::O1_2 => InputBlockSize {
            new:     fft_size / 2,
            overlap: fft_size / 2,
        },
        Overlap::O1_4 => InputBlockSize {
            new:     fft_size / 4 * 3,
            overlap: fft_size / 4,
        },
    }
}

fn slice_middle_samples(samples: &[ComplexSample], overlap: Overlap) -> &[ComplexSample] {
    let len = samples.len();
    let (first_sample, n_samples) = match overlap {
        Overlap::O1_2 => ((len + 2) / 4, len / 2),
        Overlap::O1_4 => ((len + 4) / 8, len / 4 * 3),
    };
    &samples[first_sample .. first_sample + n_samples]
}

/// Compute phase rotation for a given center bin number, block counter and overlap factor.
/// All bins in a block will be phase shifted by the amount returned.
/// Return value is a number from 0 to 3, where:
/// 0 means 0° phase shift. Values are not affected.
/// 1 means 90° phase shift. Values are multipled by i.
/// 2 means 180° phase shift. Values are multipled by -1.
/// 3 means 270° phase shift. Values are multipled by -i.
fn get_phase_rotation(center_bin: isize, block_count: usize, overlap: Overlap) -> i8 {
    (
        center_bin.rem_euclid(4) as i8 *
        block_count.rem_euclid(4) as i8 *
        match overlap {
            Overlap::O1_2 => 2,
            Overlap::O1_4 => 1,
        }
    ).rem_euclid(4)
}

// ----------------------------------------
//           Analysis filter bank
// ----------------------------------------

#[derive(Copy, Clone)]
pub struct AnalysisInputParameters {
    pub fft_size: usize,
    /// Input sample rate.
    pub sample_rate: f64,
    /// Input center frequency.
    pub center_frequency: f64,
    /// Overlap factor
    pub overlap: Overlap,
}


pub struct AnalysisIntermediateResult {
    fft_result: Vec<ComplexSample>,
    /// Block counter to implement output phase rotation.
    count: usize,
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
        check_fft_size(parameters.fft_size, parameters.overlap);
        Self {
            parameters,
            fft_plan: fft_planner.plan_fft_forward(parameters.fft_size),
            result: AnalysisIntermediateResult {
                fft_result: vec![ComplexSample::ZERO; parameters.fft_size],
                count: 3,
            }
        }
    }

    pub fn input_block_size(&self) -> InputBlockSize {
        input_block_size(self.parameters.fft_size, self.parameters.overlap)
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

        // With overlap factor of 1/4, counting to 4 is enough.
        // It also works for 1/2 overlap.
        self.result.count = (self.result.count + 1) % 4;

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
            / analysis_in_params.sample_rate
        ).round() as usize;

        let center_bin = ((
            (output_center_frequency - analysis_in_params.center_frequency)
            * analysis_in_params.fft_size as f64
            / analysis_in_params.sample_rate
        ).round() as isize
        ).rem_euclid(analysis_in_params.fft_size as isize);

        Self {
            center_bin,
            weights: raised_cosine_weights(ifft_size, None, None, analysis_in_params.overlap),
        }
    }
}

pub struct AnalysisOutputProcessor {
    input_parameters: AnalysisInputParameters,
    parameters: AnalysisOutputParameters,
    ifft_plan: Arc<dyn rustfft::Fft<Sample>>,
    buffer: Vec<ComplexSample>,
    /// Scaling factor to get unity gain in passband.
    scaling: Sample,
}

impl AnalysisOutputProcessor {
    pub fn new(
        fft_planner: &mut rustfft::FftPlanner<Sample>,
        input_parameters: AnalysisInputParameters,
        parameters: AnalysisOutputParameters,
    ) -> Self {
        let ifft_size = parameters.weights.len();
        check_fft_size(ifft_size, input_parameters.overlap);
        Self {
            input_parameters,
            parameters: parameters.clone(),
            ifft_plan: fft_planner.plan_fft_inverse(ifft_size),
            buffer: vec![ComplexSample::ZERO; ifft_size],
            scaling: 1.0 / input_parameters.fft_size as Sample,
        }
    }

    pub fn process(
        &mut self,
        intermediate_result: &AnalysisIntermediateResult,
    ) -> &[ComplexSample] {
        assert!(intermediate_result.fft_result.len() == self.input_parameters.fft_size);

        let phasenum = get_phase_rotation(
            self.parameters.center_bin, intermediate_result.count, self.input_parameters.overlap);

        // Convert to scaling factor and multiply_by_i value.
        let scaling = if phasenum >= 2 { -self.scaling } else { self.scaling };
        let multiply_by_i = phasenum % 2 == 1;

        // Or should we just make scaling factor a complex number?
        /*let scaling = self.scaling * match phasenum {
            0 => ComplexSample { re:  1.0, im:  0.0 },
            1 => ComplexSample { re:  0.0, im:  1.0 },
            2 => ComplexSample { re: -1.0, im:  0.0 },
            3 => ComplexSample { re:  0.0, im: -1.0 },
            _ => panic!("Bug"),
        };*/


        let fft_size = self.input_parameters.fft_size;
        let ifft_size = self.buffer.len();
        let half_size = (ifft_size / 2) as isize;

        // This could probably be optimized a lot.
        // Now it computes each index using modulos which might be slow.
        for bin_number in -half_size .. half_size {
            let bin_index_in = (self.parameters.center_bin + bin_number).rem_euclid(fft_size as isize) as usize;
            let bin_index_out = bin_number.rem_euclid(ifft_size as isize) as usize;
            // Apply weight
            let weighted = self.parameters.weights[bin_index_out] * intermediate_result.fft_result[bin_index_in] * scaling;
            // Apply 90° phase rotation if needed.
            self.buffer[bin_index_out] = if multiply_by_i {
                ComplexSample { re: -weighted.im, im: weighted.re }
            } else {
                weighted
            }

            // Or should we just make scaling factor a complex number?
            //self.buffer[bin_index_out] = self.parameters.weights[bin_index_out] * intermediate_result.fft_result[bin_index_in] * scaling;
        }

        self.ifft_plan.process(&mut self.buffer);

        slice_middle_samples(&self.buffer, self.input_parameters.overlap)
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
    /// Overlap factor
    pub overlap: Overlap,
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
    /// Block counter to implement input phase rotation.
    count: usize,
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
    /// This is a bit redundant since offset contains the information
    /// already, but it simplifies phase code rotation for now, maybe...
    center_bin: isize,
    fft_result: Vec<ComplexSample>,
}

impl SynthesisOutputProcessor {
    pub fn new(
        fft_planner: &mut rustfft::FftPlanner<Sample>,
        parameters: SynthesisOutputParameters,
    ) -> Self {
        check_fft_size(parameters.ifft_size, parameters.overlap);
        Self {
            parameters,
            ifft_plan: fft_planner.plan_fft_inverse(parameters.ifft_size),
            buffer: vec![ComplexSample::ZERO; parameters.ifft_size],
            buffer_state: SynthesisBufferState::CLEAR,
            count: 0,
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

        // Compute phase rotation.
        //
        // It might be more efficient to combine this with
        // scaling factor in input processors,
        // but then we would need a more complex API to let them
        // know the counter value of the output processor,
        // or keep separate counters in each inputs processor (which
        // would then get out of sync if input blocks are skipped).
        //
        // It might actually be more better to pass block counter
        // value to input processors and move the counting outside
        // of the FCFB implementation, since then a caller could
        // also skip synthesizing blocks if needed.
        let phasenum = get_phase_rotation(intermediate_result.center_bin, self.count, self.parameters.overlap);

        let ifft_size = self.buffer.len();
        for (index, value) in intermediate_result.fft_result.iter().enumerate() {
            // TODO: handle wrap-around without computing a modulo for each bin
            let out_index = (intermediate_result.offset + index).rem_euclid(ifft_size);

            // Rotate phase if needed.
            // Or would be faster to just multiply by a complex number
            // to do the phase rotation here?
            match phasenum {
                0 => self.buffer[out_index] += value,
                1 => {
                    self.buffer[out_index].re += value.im;
                    self.buffer[out_index].im -= value.re;
                },
                2 => self.buffer[out_index] -= value,
                3 => {
                    self.buffer[out_index].re -= value.im;
                    self.buffer[out_index].im += value.re;
                },
                _ => panic!("Bug"),
            }
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

        self.count = (self.count + 1) % 4;

        slice_middle_samples(&self.buffer, self.parameters.overlap)
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
            weights: raised_cosine_weights(fft_size, None, None, output_parameters.overlap),
        }
    }
}


pub struct SynthesisInputProcessor {
    weights: Rc<[Sample]>,
    fft_plan: Arc<dyn rustfft::Fft<Sample>>,
    result: SynthesisIntermediateResult,
    /// Scaling factor for unity gain in passband.
    /// This could be included in weights to avoid some
    /// multiplications but that might complicate other things.
    /// Have to think about it a bit more.
    scaling: Sample,
    overlap: Overlap,
}

impl SynthesisInputProcessor {
    pub fn new(
        fft_planner: &mut rustfft::FftPlanner<Sample>,
        output_parameters: SynthesisOutputParameters,
        parameters: SynthesisInputParameters,
    ) -> Self {
        let fft_size = parameters.weights.len();
        check_fft_size(fft_size, output_parameters.overlap);
        Self {
            weights: parameters.weights,
            fft_plan: fft_planner.plan_fft_forward(fft_size),
            result: SynthesisIntermediateResult {
                offset:
                    (parameters.center_bin - (fft_size / 2) as isize)
                    .rem_euclid(output_parameters.ifft_size as isize) as usize,
                center_bin: parameters.center_bin,
                fft_result: vec![ComplexSample::ZERO; fft_size],
            },
            scaling: 1.0 / (fft_size as Sample),
            overlap: output_parameters.overlap,
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
            *value = *value * weight * self.scaling;
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
        input_block_size(self.result.fft_result.len(), self.overlap)
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
/// Default transition band width depends on overlap factor.
/// Maybe a separate function with defaults would be better
/// since now the overlap parameter is useless if defaults are not used.
pub fn raised_cosine_weights(
    ifft_size: usize,
    passband_bins: Option<usize>,
    transition_bins: Option<usize>,
    overlap: Overlap,
) -> Rc<[Sample]> {
    // I am not sure if it this would work correctly for an odd size,
    // but currently supported overlap factors needs an even IFFT size anyway.
    // Maybe returning an error instead of panicing with invalid values
    // would be better though.
    assert!(ifft_size % 2 == 0);

    let default_max_transition = match overlap {
        Overlap::O1_2 => 15,
        // Smaller overlap factor needs a wider transition band
        // for similar level of spurious products.
        Overlap::O1_4 => 31,
    };
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
            center_frequency: 0.0,
            // There is no test for AnalysisOutputProcessor::new_with_frequency yet,
            // so input sample rate does not matter.
            sample_rate: 10000.0,
            overlap: Overlap::O1_4,
        };
        let output_parameters = AnalysisOutputParameters {
            center_bin: 11,
            weights: raised_cosine_weights(100, None, None, input_parameters.overlap),
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
            overlap: Overlap::O1_4,
        };

        let mut sy = SynthesisOutputProcessor::new(&mut fft_planner, output_parameters);
        let mut sy_input = SynthesisInputProcessor::new_with_frequency(&mut fft_planner, output_parameters, 9600.0, 100.0);

        let mut input_buffer = sy_input.make_input_buffer();

        let mut output_file = std::fs::File::create("test_results/synthesis_output.cf32").unwrap();

        for _ in 0..2000 {
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
            let weights = raised_cosine_weights(ifft_size, passband_bins, transition_bins, Overlap::O1_2);
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
