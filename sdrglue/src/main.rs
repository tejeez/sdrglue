use rustfft::num_complex as num_complex;
use rustfft::num_traits as num_traits;

mod fcfb;
mod soapyconfig;

fn main() {
    // Spacing of FCFB FFT bins in Hz
    let bin_spacing = 500.0;

    let mut sdr = soapyconfig::SoapyIo::init(&soapyconfig::SXCEIVER_DEFAULT).unwrap();

    let analysis_in_params = fcfb::AnalysisInputParameters {
        fft_size: (sdr.rx_sample_rate().unwrap() / bin_spacing).round() as usize,
    };
    let mut fft_planner = rustfft::FftPlanner::new();
    let mut analysis_bank = fcfb::AnalysisInputProcessor::new(&mut fft_planner, analysis_in_params);
    let mut rx_buffer = analysis_bank.make_input_buffer();
    loop {
        match sdr.receive(rx_buffer.prepare_for_new_samples()) {
            Ok(_rx_result) => {
                analysis_bank.process(rx_buffer.buffer());
            },
            Err(_) => { break },
        }
    }
}
