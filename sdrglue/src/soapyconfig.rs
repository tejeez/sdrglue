use soapysdr;

type StreamType = crate::ComplexSample;

pub struct SoapyIoConfig<'a> {
    /// Sample rate
    pub fs:       f64,
    /// Receive center frequency
    pub rx_freq:  f64,
    /// Transmit center frequency
    pub tx_freq:  f64,
    /// Receive channel number
    pub rx_chan:  usize,
    /// Transmit channel number
    pub tx_chan:  usize,
    /// Receive antenna
    pub rx_ant:   &'a str,
    /// Transmit antenna
    pub tx_ant:   &'a str,
    /// Receive gain(s).
    /// Use (None, gain_value) to set the overall gain.
    /// Use ("name", gain_value) to set a specific gain element.
    pub rx_gain:  &'a[(Option<&'a str>, f64)],
    /// Transmit gain(s).
    pub tx_gain:  &'a[(Option<&'a str>, f64)],
    /// Device arguments
    pub dev_args: &'a [(&'a str, &'a str)],
    /// Receive stream arguments
    pub rx_args:  &'a [(&'a str, &'a str)],
    /// Transmit stream arguments
    pub tx_args:  &'a [(&'a str, &'a str)],
}

pub const LIMESDR_DEFAULT: SoapyIoConfig = SoapyIoConfig {
    fs: 8192e3,
    rx_freq: 435e6,
    tx_freq: 435e6,
    rx_chan: 0,
    tx_chan: 0,
    rx_ant:  "LNAL",
    tx_ant:  "BAND1",
    rx_gain: &[
        (Some("LNA"), 20.0),
        (Some("TIA"), 10.0),
        (Some("PGA"), 10.0),
    ],
    tx_gain: &[
        (Some("PAD" ), 52.0),
        (Some("IAMP"), 3.0),
    ],
    dev_args: &[("driver", "lime")],
    rx_args: &[],
    tx_args: &[],
};

pub const SXCEIVER_DEFAULT: SoapyIoConfig = SoapyIoConfig {
    fs: 600e3,
    rx_freq: 432.4e6,
    tx_freq: 432.4e6,
    rx_chan: 0,
    tx_chan: 0,
    rx_ant:  "RX",
    tx_ant:  "TX",
    rx_gain: &[
        (Some("LNA"), 42.0),
        (Some("PGA"), 16.0),
    ],
    tx_gain: &[
        (Some("DAC"  ), 9.0),
        (Some("MIXER"), 30.0),
    ],
    dev_args: &[("driver", "sx")],
    rx_args: &[],
    tx_args: &[],
};

pub struct SoapyIo {
    rx_chan:  usize,
    tx_chan:  usize,
    dev: soapysdr::Device,
    rx:  soapysdr::RxStream<StreamType>,
    tx:  soapysdr::TxStream<StreamType>,
}

/// Convert a slice of ("key", "value") pairs to soapysdr::Args.
/// This might not be really needed, but it makes configuration struct
/// contents easier to write.
fn convert_args(key_value_pairs: &[(&str, &str)]) -> soapysdr::Args {
    let mut args = soapysdr::Args::new();
    for (key, value) in key_value_pairs {
        args.set(*key, *value);
    }
    args
}

/// It is annoying to repeat error handling so do that in a macro.
/// ? could be used but then it could not print which SoapySDR call failed.
macro_rules! soapycheck {
    ($text:literal, $soapysdr_call:expr) => {
        match $soapysdr_call {
            Ok(ret) => { ret },
            Err(err) => {
                eprintln!("SoapySDR: Failed to {}: {}", $text, err);
                return Err(err);
            }
        }
    }
}

impl SoapyIo {
    pub fn init(conf: &SoapyIoConfig) -> Result<Self, soapysdr::Error> {
        let dev = soapycheck!("open SoapySDR device",
            soapysdr::Device::new(convert_args(conf.dev_args)));
        soapycheck!("set RX sample rate",
            dev.set_sample_rate(soapysdr::Direction::Rx, conf.rx_chan, conf.fs));
        soapycheck!("set TX sample rate",
            dev.set_sample_rate(soapysdr::Direction::Tx, conf.tx_chan, conf.fs));
        soapycheck!("set RX center frequency",
            dev.set_frequency(soapysdr::Direction::Rx, conf.rx_chan, conf.rx_freq, soapysdr::Args::new()));
        soapycheck!("set TX center frequency",
            dev.set_frequency(soapysdr::Direction::Tx, conf.tx_chan, conf.tx_freq, soapysdr::Args::new()));
        soapycheck!("set RX antenna",
            dev.set_antenna(soapysdr::Direction::Rx, conf.rx_chan, conf.rx_ant));
        soapycheck!("set TX antenna",
            dev.set_antenna(soapysdr::Direction::Tx, conf.tx_chan, conf.tx_ant));
        for (name, value) in conf.rx_gain {
            if let Some(name) = name {
                soapycheck!("set RX gain element",
                    dev.set_gain_element(soapysdr::Direction::Rx, conf.rx_chan, name.as_bytes().to_vec(), *value));
            } else {
                soapycheck!("set RX overall gain",
                    dev.set_gain(soapysdr::Direction::Rx, conf.rx_chan, *value));
            }
        }
        for (name, value) in conf.tx_gain {
            if let Some(name) = name {
                soapycheck!("set TX gain element",
                    dev.set_gain_element(soapysdr::Direction::Tx, conf.tx_chan, name.as_bytes().to_vec(), *value));
            } else {
                soapycheck!("set TX overall gain",
                    dev.set_gain(soapysdr::Direction::Tx, conf.tx_chan, *value));
            }
        }
        let mut rx = soapycheck!("setup RX stream",
            dev.rx_stream_args(&[conf.rx_chan], convert_args(conf.rx_args)));
        let mut tx = soapycheck!("setup TX stream",
            dev.tx_stream_args(&[conf.tx_chan], convert_args(conf.tx_args)));
        soapycheck!("activate RX stream",
            rx.activate(None));
        soapycheck!("activate TX stream",
            tx.activate(None));
        Ok(Self {
            rx_chan: conf.rx_chan,
            tx_chan: conf.tx_chan,
            dev: dev,
            rx:  rx,
            tx:  tx,
        })
    }

    pub fn receive(&mut self, buffer: &mut [StreamType]) -> Result<soapysdr::StreamResult, soapysdr::Error> {
        // TODO: implement read_exact and use that
        self.rx.read_ext(&mut [buffer], soapysdr::StreamFlags::default(), None, 100000)
    }

    pub fn transmit(&mut self, buffer: &[StreamType], timestamp: Option<i64>) -> Result<(), soapysdr::Error> {
        self.tx.write_all(&[buffer], timestamp, false, 100000)
    }

    pub fn rx_sample_rate(&self) -> Result<f64, soapysdr::Error> {
        self.dev.sample_rate(soapysdr::Direction::Rx, self.rx_chan)
    }

    pub fn tx_sample_rate(&self) -> Result<f64, soapysdr::Error> {
        self.dev.sample_rate(soapysdr::Direction::Tx, self.tx_chan)
    }

    pub fn rx_center_frequency(&self) -> Result<f64, soapysdr::Error> {
        self.dev.frequency(soapysdr::Direction::Rx, self.rx_chan)
    }

    pub fn tx_center_frequency(&self) -> Result<f64, soapysdr::Error> {
        self.dev.frequency(soapysdr::Direction::Tx, self.tx_chan)
    }
}
