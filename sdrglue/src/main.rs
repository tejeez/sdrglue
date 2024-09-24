use rustfft::num_complex as num_complex;
use rustfft::num_traits as num_traits;
use num_complex::Complex32;
use num_traits::Zero;

mod fcfb;
mod soapyconfig;

fn main() {
    // Spacing of FCFB FFT bins in Hz
    let bin_spacing = 500.0;

    let mut sdr = soapyconfig::SoapyIo::init(&soapyconfig::SXCEIVER_DEFAULT).unwrap();

    let ddc_in_params = fcfb::DdcInputParameters {
        fft_size: (sdr.rx_sample_rate().unwrap() / bin_spacing).round() as usize,
    };
    let mut fft_planner = rustfft::FftPlanner::new();
    let mut ddc = fcfb::DdcInputProcessor::new(&mut fft_planner, ddc_in_params);
    let rx_block_size = ddc.input_block_size();

    let mut rx_buffer = vec![Complex32::zero(); rx_block_size.total];

    loop {
        // Move overlapping part from the end of the previous block to the beginning
        rx_buffer.copy_within(rx_block_size.new - rx_block_size.overlap .. rx_block_size.total, 0);
        // Add new samples after the overlapping part
        match sdr.receive(&mut rx_buffer[rx_block_size.overlap .. rx_block_size.total]) {
            Ok(_rx_result) => {
                ddc.process(&rx_buffer);
            },
            Err(_) => { break },
        }
    }
}
