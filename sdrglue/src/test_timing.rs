use super::fcfb;
use super::rx_dsp;
use super::tx_dsp;
use super::configuration;
use super::rxthings::iqrecorder;
use super::txthings::testpulse;

/// Feeding TxDsp output into RxDsp input
/// and check that timing between RX and TX sample counters is correct.
#[test]
fn test_dsp_timing() {
    let mut fft_planner = rustfft::FftPlanner::new();

    let cli = configuration::Cli {
        rx_bin_spacing: 500.0,
        tx_bin_spacing: 500.0,
        ..Default::default()
    };
    let fs = 512e3;
    let mut rx_dsp = rx_dsp::RxDsp::new(&mut fft_planner, &cli, fs, 0.0);
    let mut tx_dsp = tx_dsp::TxDsp::new(&mut fft_planner, &cli, fs, 0.0);

    rx_dsp.add_processor(&mut fft_planner, Box::new(
        iqrecorder::RecordIq::new(&&iqrecorder::RecordIqParameters {
            sample_rate: 32000.0,
            center_frequency: 0.0,
            filename: "test_pulse_output.cf32",
        })));

    tx_dsp.add_processor(&mut fft_planner, Box::new(
        testpulse::TestPulse::new(&testpulse::TestPulseParameters {
            sample_rate: 32000.0,
            center_frequency: 0.0,
            interval: 1000,
        })));

    //tx_dsp.
    for block_count in 0i64..100i64 {
        let buffer = rx_dsp.prepare_input_buffer();
        buffer.copy_from_slice(tx_dsp.process(block_count));
        rx_dsp.process(block_count);
    }
}
