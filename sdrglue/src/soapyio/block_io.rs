//! SoapySDR I/O in blocks suitable for FCFB

use crate::dsp_types::*;
use crate::fcfb;

use super::io::SoapyIo;

pub struct BlockRx {
    block_size: fcfb::InputBlockSize,
    buffer: Vec<ComplexSample>,
    /// How much of rx_buffer has been filled
    buffer_i: usize,
    block_count: fcfb::BlockCount,
}

impl BlockRx {
    pub fn new(block_size: fcfb::InputBlockSize) -> Self {
        Self {
            block_size,
            buffer: vec![num::zero(); block_size.overlap + block_size.new],
            buffer_i: 0,
            block_count: -1,
        }
    }

    /// Receive a block of samples.
    /// Returned buffer can be passed to fcfb::AnalysisInputProcessor::process.
    pub fn receive(&mut self, sdr: &mut SoapyIo) -> Result<(&[ComplexSample], fcfb::BlockCount), soapysdr::Error> {
        self.block_count = self.block_count.wrapping_add(1);

        // Copy overlapping part from previous block to the beginning
        self.buffer
            .copy_within(self.block_size.new..self.block_size.new + self.block_size.overlap, 0);
        self.buffer_i = self.block_size.overlap;

        loop {
            let result = sdr.receive(&mut self.buffer[self.buffer_i..])?;

            let block_size = self.block_size.new as SampleCount;
            let expected_count = (self.block_count as SampleCount * block_size).wrapping_add(self.buffer_i as SampleCount);
            let samples_lost = result.count.wrapping_sub(expected_count);
            if samples_lost != 0 {
                // Samples have been lost.
                // Mark RX buffer as empty and skip the right number of samples
                // to receive the next full processing block in the next iteration.

                // Expected sample count for the next read,
                // assuming no more samples are lost.
                let next_count = result.count.wrapping_add(result.len as SampleCount);
                // div_euclid always rounds down (towards negative numbers),
                // so use it with negations to round up to the next block.
                // OK, this may break once sample count wraps around,
                // so handling wrapping_add/sub properly in the rest of the code might be useless...
                // It will happen after running for a few million years.
                let next_possible_block = -next_count.div_euclid(-block_size) + 1;
                let next_block_beginning = next_possible_block * block_size;

                let mut samples_to_skip = next_block_beginning.wrapping_sub(next_count);

                tracing::warn!(
                    "Lost {} samples, skipping {} more samples and {} processing blocks",
                    samples_lost,
                    samples_to_skip,
                    next_possible_block.wrapping_sub(self.block_count)
                );

                self.block_count = next_possible_block;
                self.buffer_i = 0;

                // Repeat reads until the correct number of samples has been skipped.
                while samples_to_skip > 0 {
                    let result = sdr.receive(&mut self.buffer[0..samples_to_skip as usize])?;
                    samples_to_skip -= result.len as SampleCount;
                }
            } else {
                self.buffer_i += result.len;
                if self.buffer_i == self.buffer.len() {
                    return Ok((&self.buffer[..], self.block_count));
                }
            }
        }
    }
}

pub struct BlockTx {
    block_size: usize,
    latency_blocks: fcfb::BlockCount,
    block_count: fcfb::BlockCount,
}

impl BlockTx {
    pub fn new(block_size: usize, latency_blocks: fcfb::BlockCount) -> Self {
        Self {
            block_size,
            latency_blocks,
            block_count: 0,
        }
    }

    /// Get the next possible transmit block number.
    /// Return None if no TX blocks can be transmitted yet.
    pub fn next_block(&mut self, sdr: &mut SoapyIo) -> Result<Option<fcfb::BlockCount>, soapysdr::Error> {
        if !sdr.tx_possible() {
            return Ok(None);
        }

        let current_sample = sdr.tx_current_count()?;
        // Current time as block count
        let current_block = current_sample.div_euclid(self.block_size as SampleCount);

        let d = self.block_count.wrapping_sub(current_block);

        // Skip TX blocks in the past or in too near future
        let dmin = 2; // how many blocks in future minimum
        if d < dmin {
            let new_block_count = current_block.wrapping_add(dmin);
            tracing::warn!(
                "Too late to produce TX block {}, skipping {} TX blocks",
                self.block_count,
                new_block_count.wrapping_sub(self.block_count)
            );
            self.block_count = new_block_count;
        }

        // Limit how far into future TX blocks are generated
        if d > self.latency_blocks {
            return Ok(None);
        }

        Ok(Some(self.block_count))
    }

    pub fn transmit(&mut self, sdr: &mut SoapyIo, samples: &[ComplexSample]) -> Result<(), soapysdr::Error> {
        assert!(samples.len() == self.block_size);

        // TODO: compensate for delay of SDR
        let sdr_sample_count = self.block_size as SampleCount * self.block_count;

        self.block_count = self.block_count.wrapping_add(1);

        sdr.transmit(samples, Some(sdr_sample_count))
    }
}
