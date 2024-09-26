
pub use rustfft::num_complex as num_complex;
pub use rustfft::num_traits as num_traits;
/// Floating point type used for signal processing.
pub type Sample = f32;
/// Complex floating point type used for signal processing.
pub type ComplexSample = num_complex::Complex<Sample>;
/// Mathematical consts for the Sample type.
pub use std::f32::consts as sample_consts;

mod fcfb;
mod rxthings;
mod soapyconfig;

struct RxChannel {
    fcfb_output: fcfb::AnalysisOutputProcessor,
    processor: Box<dyn rxthings::RxChannelProcessor>,
}

impl RxChannel {
    fn new(
        fft_planner: &mut rustfft::FftPlanner<Sample>,
        analysis_in_params: fcfb::AnalysisInputParameters,
        sdr_rx_sample_rate: f64,
        sdr_rx_center_frequency: f64,
        processor: Box<dyn rxthings::RxChannelProcessor>,
    ) -> Self {
        let ifft_size = (processor.input_sample_rate() * analysis_in_params.fft_size as f64 / sdr_rx_sample_rate).round() as usize;
        let center_bin = ((processor.input_center_frequency() - sdr_rx_center_frequency) * analysis_in_params.fft_size as f64).round() as isize;
        Self {
            fcfb_output: fcfb::AnalysisOutputProcessor::new(
                fft_planner,
                analysis_in_params,
                fcfb::AnalysisOutputParameters {
                    center_bin,
                    weights: fcfb::raised_cosine_weights(ifft_size, None, None),
                }
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

fn main() {
    // Spacing of FCFB FFT bins in Hz
    let bin_spacing = 500.0;

    let mut sdr = soapyconfig::SoapyIo::init(&soapyconfig::LIMESDR_DEFAULT).unwrap();

    let sdr_rx_sample_rate = sdr.rx_sample_rate().unwrap();
    let sdr_rx_center_frequency = sdr.rx_center_frequency().unwrap();

    let analysis_in_params = fcfb::AnalysisInputParameters {
        fft_size: (sdr_rx_sample_rate / bin_spacing).round() as usize,
    };
    let mut fft_planner = rustfft::FftPlanner::new();
    let mut analysis_bank = fcfb::AnalysisInputProcessor::new(&mut fft_planner, analysis_in_params);
    let mut rx_buffer = analysis_bank.make_input_buffer();

    let mut rx_processors = Vec::<RxChannel>::new();

    rx_processors.push(RxChannel::new(
        &mut fft_planner,
        analysis_in_params,
        sdr_rx_sample_rate,
        sdr_rx_center_frequency,
        Box::new(rxthings::DemodulateToUdp::new(&rxthings::DemodulateToUdpParameters {
            center_frequency: 432.5e6,
            address: "127.0.0.1:7355",
        })),
    ));

    loop {
        match sdr.receive(rx_buffer.prepare_for_new_samples()) {
            Ok(_rx_result) => {
                let ir = analysis_bank.process(rx_buffer.buffer());
                for processor in rx_processors.iter_mut() {
                    processor.process(ir);
                }
            },
            Err(_) => { break },
        }
    }
}
