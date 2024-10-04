
use rustfft;
use crate::{Sample, ComplexSample};
use crate::configuration;
use crate::fcfb;
use crate::rxthings;


struct RxChannel {
    fcfb_output: fcfb::AnalysisOutputProcessor,
    processor: Box<dyn rxthings::RxChannelProcessor>,
}

impl RxChannel {
    fn new(
        fft_planner: &mut rustfft::FftPlanner<Sample>,
        analysis_in_params: fcfb::AnalysisInputParameters,
        processor: Box<dyn rxthings::RxChannelProcessor>,
    ) -> Self {
        Self {
            fcfb_output: fcfb::AnalysisOutputProcessor::new_with_frequency(
                fft_planner,
                analysis_in_params,
                processor.input_sample_rate(),
                processor.input_center_frequency(),
            ),
            processor,
        }
    }

    fn process(
        &mut self,
        intermediate_result: &fcfb::AnalysisIntermediateResult
    ) {
        self.processor.process(self.fcfb_output.process(intermediate_result));
    }
}

/// Everything related to received signal processing.
pub struct RxDsp {
    /// Input parameters for analysis filter bank.
    analysis_params: fcfb::AnalysisInputParameters,
    /// Analysis filter bank for received signal.
    analysis_bank: fcfb::AnalysisInputProcessor,
    /// Input buffer for signal from SDR to filter bank.
    input_buffer: fcfb::InputBuffer,
    /// Receive channel processors.
    processors: Vec<RxChannel>,
}

impl RxDsp {
    pub fn new(
        fft_planner: &mut rustfft::FftPlanner<Sample>,
        cli: &configuration::Cli,
        sdr_rx_sample_rate: f64,
        sdr_rx_center_frequency: f64,
    ) -> Self {
        let bin_spacing = cli.rx_bin_spacing;

        let analysis_params = fcfb::AnalysisInputParameters {
            fft_size: (sdr_rx_sample_rate / bin_spacing).round() as usize,
            sample_rate: sdr_rx_sample_rate,
            center_frequency: sdr_rx_center_frequency,
        };
        let analysis_bank = fcfb::AnalysisInputProcessor::new(fft_planner, analysis_params);
        let input_buffer = analysis_bank.make_input_buffer();
        let mut self_ = Self {
            analysis_params,
            analysis_bank,
            input_buffer,
            processors: Vec::new(),
        };
        self_.add_processors_from_cli(fft_planner, cli);
        self_
    }

    fn add_processors_from_cli(
        &mut self,
        fft_planner: &mut rustfft::FftPlanner<Sample>,
        cli: &configuration::Cli
    ) {
        for args in cli.demodulate_to_udp.chunks_exact(3) {
            self.processors.push(RxChannel::new(
                fft_planner,
                self.analysis_params,
                Box::new(rxthings::DemodulateToUdp::new(&rxthings::DemodulateToUdpParameters {
                    center_frequency: args[1].parse().unwrap(),
                    address: args[0].as_str(),
                    // TODO: different modulations
                })),
            ));
        }
    }

    pub fn prepare_input_buffer(
        &mut self,
    ) -> &mut [ComplexSample] {
        self.input_buffer.prepare_for_new_samples()
    }

    pub fn process(
        &mut self,
    ) {
        let ir = self.analysis_bank.process(self.input_buffer.buffer());
        for processor in self.processors.iter_mut() {
            processor.process(ir);
        }
    }
}
