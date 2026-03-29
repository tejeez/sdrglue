/// Feeding TxDsp output into RxDsp input
/// and check that timing between RX and TX sample counters is correct.
#[test]
fn test_dsp_timing() {
    use super::fcfb;
    use super::rx_dsp;
    use super::tx_dsp;
    use super::rxthings::iqrecorder;
    use super::txthings::testpulse;

    let mut fft_planner = rustfft::FftPlanner::new();

    let sample_rate = 512e3;
    let mut rx_dsp = rx_dsp::RxDsp::new(&mut fft_planner, &rx_dsp::RxDspParameters {
        sample_rate,
        center_frequency: 0.0,
        bin_spacing: 500.0,
        overlap: fcfb::Overlap::O1_2,
    });
    let mut tx_dsp = tx_dsp::TxDsp::new(&mut fft_planner, &tx_dsp::TxDspParameters {
        sample_rate,
        center_frequency: 0.0,
        bin_spacing: 500.0,
        overlap: fcfb::Overlap::O1_2,
    });

    rx_dsp.add_processor(&mut fft_planner, Box::new(
        iqrecorder::RecordIq::new(&&iqrecorder::RecordIqParameters {
            sample_rate: 32000.0,
            center_frequency: 0.0,
            filename: "test_results/test_pulse_output.cf32",
        })));

    tx_dsp.add_processor(&mut fft_planner, Box::new(
        testpulse::TestPulse::new(&testpulse::TestPulseParameters {
            sample_rate: 32000.0,
            center_frequency: 0.0,
            interval: 1000,
        })));

    let mut fcfb_buffer = fcfb::InputBuffer::new(rx_dsp.input_block_size());
    for block_count in 0i64..100i64 {
        let block_samples = fcfb_buffer.prepare_for_new_samples();
        block_samples.copy_from_slice(tx_dsp.process(block_count));
        rx_dsp.process(fcfb_buffer.buffer(), block_count);
    }
}
