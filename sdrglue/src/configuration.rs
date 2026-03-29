
pub use clap::Parser;

use crate::dsp_types::*;
use super::rx_dsp;
use super::tx_dsp;
use super::rxthings;
use super::txthings;
use super::fcfb;

#[derive(Parser, Default)]
pub struct Cli {
    /// SoapySDR device arguments
    /// as pairs like argument_name argument_value...
    /// For example: --sdr-device driver lime
    #[arg(long, value_delimiter = ' ', num_args = 2..)]
    pub sdr_device: Vec<String>,

    /// Receive center frequency for SDR.
    /// Receiving is disabled if not given.
    #[arg(long)]
    pub sdr_rx_freq: Option<f64>,
    /// Transmit center frequency for SDR.
    /// Transmitting is disabled if not given.
    #[arg(long)]
    pub sdr_tx_freq: Option<f64>,

    /// Receive sample rate for SDR.
    /// Default value depends on the SDR device being used.
    #[arg(long)]
    pub sdr_rx_fs: Option<f64>,
    /// Transmit sample rate for SDR.
    /// Default is equal to receive sample rate.
    #[arg(long)]
    pub sdr_tx_fs: Option<f64>,

    /// Receive channel number for SDR.
    #[arg(long, default_value_t = 0)]
    pub sdr_rx_ch: usize,
    /// Transmit channel number for SDR.
    #[arg(long, default_value_t = 0)]
    pub sdr_tx_ch: usize,

    /// Receive antenna for SDR.
    /// Default value is provided for some SDR devices.
    #[arg(long)]
    pub sdr_rx_ant: Option<String>,
    /// Transmit antenna for SDR.
    /// Default value is provided for some SDR devices.
    #[arg(long)]
    pub sdr_tx_ant: Option<String>,

    /// Receive gain(s) for SDR.
    /// If only one number if given, it will set the overall gain.
    /// If multiple values are given, they will set individual gain elements
    /// given as pairs of element_name gain_value...
    /// Default value is provided for some SDR devices.
    #[arg(long)]
    pub sdr_rx_gain: Vec<String>,
    /// Transmit gain(s) for SDR.
    #[arg(long)]
    pub sdr_tx_gain: Vec<String>,

    /// SoapySDR receive stream arguments.
    #[arg(long, value_delimiter = ' ', num_args = 2..)]
    pub rx_args: Vec<String>,
    /// SoapySDR transmit stream arguments.
    #[arg(long, value_delimiter = ' ', num_args = 2..)]
    pub tx_args: Vec<String>,

    /// If SDR supports timestamps, we can use the latest RX timestamp
    /// to determine the next TX timestamp. This maintains a consistent
    /// delay from RX to TX and lets us adjust transmit latency.
    /// This is the timestamp difference in nanoseconds and roughly
    /// determines how much signal will be kept in transmit buffer.
    #[arg(long, default_value_t = 20000000)]
    pub rx_tx_delay: i64,

    /// Spacing of FFT bins (in Hertz) for fast-convolution
    /// analysis filter bank used for received signals.
    /// All sample rates must be integer multiples of 2 * bin spacing
    /// (for overlap factor 1/2) or 4 * bin spacing (for overlap factor 1/4).
    /// This affect severals things and should be documented better,
    /// but for now, just keep it at the default value if unsure.
    #[arg(long, default_value_t = 500.0)]
    pub rx_bin_spacing: f64,

    #[arg(long, default_value_t = 500.0)]
    pub tx_bin_spacing: f64,

    /// FFT overlap factor for fast-convolution filter bank.
    /// 1/4 results in less CPU use but more spurious products than 1/2.
    #[arg(long, default_value = "1/2")]
    pub rx_overlap: String,
    // TODO: maybe consider using enum here
    //#[arg(long, default_value_t = fcfb::Overlap::O1_2)]
    //pub rx_overlap: fcfb::Overlap, // TODO?

    #[arg(long, default_value = "1/2")]
    pub tx_overlap: String,
    //#[arg(long, default_value_t = fcfb::Overlap::O1_2)]
    //pub tx_overlap: fcfb::Overlap, // TODO?

    /// Add demodulators with UDP output interface.
    /// Each demodulator takes 3 arguments:
    /// UDP destination address, frequency and modulation.
    /// For example, to add two demodulators:
    /// --demodulate-to-udp 127.0.0.1:7300 432.5e6 FM 127.0.0.1:7301 432.3e6 USB
    #[arg(long, value_delimiter = ' ', num_args = 3..)]
    pub demodulate_to_udp: Vec<String>,

    /// Add I/Q file recorders.
    /// Each recorder takes 3 arguments:
    /// File name, sample rate and center frequency.
    #[arg(long, value_delimiter = ' ', num_args = 3..)]
    pub record_iq: Vec<String>,

    /// Add test pulse transmitters.
    /// Each transmitter takes 3 arguments:
    /// Sample rate, center frequency and pulse interval (in samples).
    #[arg(long, value_delimiter = ' ', num_args = 3..)]
    pub tx_test_pulse: Vec<String>,
}

impl Cli {
    pub fn rx_dsp_parameters(
        &self,
        sdr_rx_sample_rate: f64,
        sdr_rx_center_frequency: f64,
    ) -> rx_dsp::RxDspParameters {
        rx_dsp::RxDspParameters {
            sample_rate: sdr_rx_sample_rate,
            center_frequency: sdr_rx_center_frequency,
            bin_spacing: self.rx_bin_spacing,
            overlap: if self.rx_overlap == "1/4" { fcfb::Overlap::O1_4 } else { fcfb::Overlap::O1_2 },
        }
    }

    pub fn tx_dsp_parameters(
        &self,
        sdr_tx_sample_rate: f64,
        sdr_tx_center_frequency: f64,
    ) -> tx_dsp::TxDspParameters {
        tx_dsp::TxDspParameters {
            sample_rate: sdr_tx_sample_rate,
            center_frequency: sdr_tx_center_frequency,
            bin_spacing: self.tx_bin_spacing,
            overlap: if self.tx_overlap == "1/4" { fcfb::Overlap::O1_4 } else { fcfb::Overlap::O1_2 },
        }
    }

    pub fn add_rx_processors(
        &self,
        fft_planner: &mut FftPlanner,
        rx_dsp: &mut rx_dsp::RxDsp
    ) {
        for args in self.demodulate_to_udp.chunks_exact(3) {
            rx_dsp.add_processor(fft_planner, Box::new(
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

        for args in self.record_iq.chunks_exact(3) {
            rx_dsp.add_processor(fft_planner, Box::new(
                rxthings::iqrecorder::RecordIq::new(&rxthings::iqrecorder::RecordIqParameters {
                    sample_rate: args[1].parse().unwrap(),
                    center_frequency: args[2].parse().unwrap(),
                    filename: args[0].as_str(),
                }),
            ));
        }
    }

    pub fn add_tx_processors(
        &self,
        fft_planner: &mut FftPlanner,
        tx_dsp: &mut tx_dsp::TxDsp
    ) {
        for args in self.tx_test_pulse.chunks_exact(3) {
            tx_dsp.add_processor(fft_planner, Box::new(
                txthings::testpulse::TestPulse::new(&txthings::testpulse::TestPulseParameters {
                    sample_rate: args[0].parse().unwrap(),
                    center_frequency: args[1].parse().unwrap(),
                    interval: args[2].parse().unwrap(),
                }),
            ));
        }
    }
}
