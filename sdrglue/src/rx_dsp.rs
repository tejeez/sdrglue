
use rustfft;
use crate::{RealSample, ComplexSample};
use crate::configuration;
use crate::fcfb;
use crate::rxthings;


struct RxChannel {
    fcfb_output: fcfb::AnalysisOutputProcessor,
    processor: Box<dyn rxthings::RxChannelProcessor>,
}

impl RxChannel {
    fn new(
        fft_planner: &mut rustfft::FftPlanner<RealSample>,
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
    /// Input buffer for signal from SDR to filter bank.
    input_buffer: fcfb::InputBuffer,
    /// Receive channel processors.
    processors: Vec<RxChannel>,
}

impl RxDsp {
    pub fn new(
        fft_planner: &mut rustfft::FftPlanner<RealSample>,
        cli: &configuration::Cli,
        sdr_rx_sample_rate: f64,
        sdr_rx_center_frequency: f64,
    ) -> Self {
        let bin_spacing = cli.rx_bin_spacing;

        let analysis_params = fcfb::AnalysisInputParameters {
            fft_size: (sdr_rx_sample_rate / bin_spacing).round() as usize,
            sample_rate: sdr_rx_sample_rate,
            center_frequency: sdr_rx_center_frequency,
            overlap: if cli.rx_overlap == "1/4" { fcfb::Overlap::O1_4 } else { fcfb::Overlap::O1_2 },
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

    pub fn add_processor(
        &mut self,
        fft_planner: &mut rustfft::FftPlanner<RealSample>,
        processor: Box<dyn rxthings::RxChannelProcessor>,
    ) {
        self.processors.push(RxChannel::new(fft_planner, self.analysis_params, processor));
    }

    fn add_processors_from_cli(
        &mut self,
        fft_planner: &mut rustfft::FftPlanner<RealSample>,
        cli: &configuration::Cli
    ) {
        for args in cli.demodulate_to_udp.chunks_exact(3) {
            self.add_processor(fft_planner, Box::new(
                rxthings::demodulator::DemodulateToUdp::new(&rxthings::demodulator::DemodulateToUdpParameters {
                    center_frequency: args[1].parse().unwrap(),
                    address: args[0].as_str(),
                    modulation: match args[2].to_uppercase().as_str() {
                        "FM"  => rxthings::demodulator::Modulation::FM,
                        "USB" => rxthings::demodulator::Modulation::USB,
                        "LSB" => rxthings::demodulator::Modulation::LSB,
                        // TODO: handle errors more nicely
                        _ => panic!("Unknown modulation {}", args[2]),
                    },
                }),
            ));
        }

        for args in cli.record_iq.chunks_exact(3) {
            self.add_processor(fft_planner, Box::new(
                rxthings::iqrecorder::RecordIq::new(&rxthings::iqrecorder::RecordIqParameters {
                    sample_rate: args[1].parse().unwrap(),
                    center_frequency: args[2].parse().unwrap(),
                    filename: args[0].as_str(),
                }),
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
        block_count: fcfb::BlockCount,
    ) {
        let ir = self.analysis_bank.process(self.input_buffer.buffer(), block_count);
        for processor in self.processors.iter_mut() {
            processor.process(ir);
        }
    }
}
