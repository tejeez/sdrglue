use std::collections::HashMap;

pub mod io;
mod soapy_settings;
mod soapy_time;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum RxTxDevError {
    RxEndOfData,
    RxReadError,
}

// TODO: get rid of this.
// Keeping it here now to reduce changes in code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackMode {
    Bs,
    Ms,
    Mon,
}

/// SoapySDR configuration
#[derive(Debug, Clone)]
pub struct CfgSoapySdr {
    /// RX center frequency in Hz. RX disabled if None.
    pub rx_freq: Option<f64>,
    /// TX center frequency in Hz. TX disabled if None.
    pub tx_freq: Option<f64>,
    /// PPM frequency error correction
    pub ppm_err: f64,
    /// Argument string to select a specific SDR device.
    /// If None, devices will be enumerated until the first supported device is found.
    pub device: Option<String>,
    /// RX antenna. Device specific default will be used if None.
    pub rx_ant: Option<String>,
    /// TX antenna. Device specific default will be used if None.
    pub tx_ant: Option<String>,
    /// RX gain values.
    /// Device specific defaults will be used for gains that are not set.
    pub rx_gains: HashMap<String, f64>,
    /// TX gain values.
    /// Device specific defaults will be used for gains that are not set.
    pub tx_gains: HashMap<String, f64>,
    /// RX and TX sample rate. Device specific default will be used if None.
    pub fs: Option<f64>,
    /// RX channel number
    pub rx_ch: Option<usize>,
    /// TX channel number
    pub tx_ch: Option<usize>,
}
