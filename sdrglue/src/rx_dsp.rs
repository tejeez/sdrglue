
use crate::dsp_types::*;
use crate::fcfb;
use crate::rxthings;


struct RxChannel {
    fcfb_output: fcfb::AnalysisOutputProcessor,
    processor: Box<dyn rxthings::RxChannelProcessor>,
}

impl RxChannel {
    fn new(
        fft_planner: &mut FftPlanner,
        analysis_in_params: fcfb::AnalysisInputParameters,
        processor: Box<dyn rxthings::RxChannelProcessor>,
    ) -> Self {
        Self {
            fcfb_output: fcfb::AnalysisOutputProcessor::new_with_frequency(
                fft_planner,
                analysis_in_params,
                processor.input_sample_rate(),
                processor.input_center_frequency(),
                // TODO: maybe add processor.minimum_input_bandwidth()
                None
            ),
            processor,
        }
    }

    fn process(
        &mut self,
        intermediate_result: &fcfb::AnalysisIntermediateResult
    ) {
        let (sample_counter, samples) = self.fcfb_output.process(intermediate_result);
        self.processor.process(sample_counter, samples);
    }
}

/// Everything related to received signal processing.
pub struct RxDsp {
    /// Input parameters for analysis filter bank.
    analysis_params: fcfb::AnalysisInputParameters,
    /// Analysis filter bank for received signal.
    analysis_bank: fcfb::AnalysisInputProcessor,
    /// Receive channel processors.
    processors: Vec<RxChannel>,
}

pub struct RxDspParameters {
    /// Input sample rate (Hz)
    pub sample_rate: f64,
    /// Input sample rate (Hz)
    pub center_frequency: f64,
    /// FCFB bin spacing (Hz)
    pub bin_spacing: f64,
    /// FCFB overlap factor
    pub overlap: fcfb::Overlap,
}

impl RxDsp {
    pub fn new(
        fft_planner: &mut FftPlanner,
        params: &RxDspParameters,
    ) -> Self {
        let analysis_params = fcfb::AnalysisInputParameters {
            fft_size: (params.sample_rate / params.bin_spacing).round() as usize,
            sample_rate: params.sample_rate,
            center_frequency: params.center_frequency,
            overlap: params.overlap,
        };
        Self {
            analysis_params,
            analysis_bank: fcfb::AnalysisInputProcessor::new(fft_planner, analysis_params),
            processors: Vec::new(),
        }
    }

    pub fn add_processor(
        &mut self,
        fft_planner: &mut FftPlanner,
        processor: Box<dyn rxthings::RxChannelProcessor>,
    ) {
        self.processors.push(RxChannel::new(fft_planner, self.analysis_params, processor));
    }

    pub fn input_block_size(&self) -> fcfb::InputBlockSize {
        self.analysis_bank.input_block_size()
    }

    pub fn process(
        &mut self,
        buffer: &[ComplexSample],
        block_count: fcfb::BlockCount,
    ) {
        let ir = self.analysis_bank.process(&buffer, block_count);
        for processor in self.processors.iter_mut() {
            processor.process(ir);
        }
    }
}
