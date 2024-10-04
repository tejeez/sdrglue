
pub use clap::Parser;

#[derive(Parser)]
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

    /// Spacing of FFT bins (in Hertz) for fast-convolution
    /// analysis filter bank used for received signals.
    /// All sample rates must be integer multiples of 2 * bin spacing.
    /// This affect severals things and should be documented better,
    /// but for now, just keep it at the default value if unsure.
    #[arg(long, default_value_t = 500.0)]
    pub rx_bin_spacing: f64,

    #[arg(long, default_value_t = 500.0)]
    pub tx_bin_spacing: f64,

    /// Add demodulators with UDP output interface.
    /// Each demodulator takes 3 arguments:
    /// UDP destination address, frequency and modulation.
    /// For example, to add two demodulators:
    /// --demodulate-to-udp 127.0.0.1:7300 432.5e6 FM 127.0.0.1:7301 432.3e6 USB
    #[arg(long, value_delimiter = ' ', num_args = 3..)]
    pub demodulate_to_udp: Vec<String>,
}
