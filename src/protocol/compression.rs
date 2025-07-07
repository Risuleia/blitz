//! Compressing module

#![allow(missing_docs)]
use std::io::{self, Read};

use flate2::{bufread::{DeflateDecoder, DeflateEncoder}, Compression};

const PERMESSAFE_DEFLATE_TRAILER: &[u8] = &[0x00, 0x00, 0xff, 0xff];

#[allow(missing_docs)]
#[derive(Debug, Clone, Copy)]
pub struct WebSocketCompressionConfig {
    pub enabled: bool,
    pub client_no_context_takeover: bool,
    pub server_no_context_takeover: bool,
    pub client_max_window_bits: Option<u8>,
    pub server_max_window_bits: Option<u8>,
}

impl Default for WebSocketCompressionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            client_no_context_takeover: false,
            server_no_context_takeover: false,
            client_max_window_bits: None,
            server_max_window_bits: None
        }
    }
}

#[allow(missing_docs)]
#[derive(Debug, Clone, Copy)]
pub struct Compressor {
    _no_context_takeover: bool
}

#[allow(missing_docs)]
impl Compressor {
    pub fn new(no_context_takeover: bool) -> Self {
        Self { _no_context_takeover: no_context_takeover }
    }

    pub fn compress(&mut self, data: &[u8]) -> io::Result<Vec<u8>> {
        let mut encoder = DeflateEncoder::new(data, Compression::default());
        let mut compressed = Vec::new();

        encoder.read_to_end(&mut compressed)?;
        Ok(compressed)
    }
}

#[allow(missing_docs)]
#[derive(Debug, Copy, Clone)]
pub struct Decompressor {
    _no_context_takeover: bool
}

#[allow(missing_docs)]
impl Decompressor {
    pub fn new(no_context_takeover: bool) -> Self {
        Self { _no_context_takeover: no_context_takeover }
    }

    pub fn decompress(&mut self, data: &[u8]) -> io::Result<Vec<u8>> {
        let mut trailer_data = data.to_vec();
        trailer_data.extend_from_slice(PERMESSAFE_DEFLATE_TRAILER);

        let mut decoder = DeflateDecoder::new(&trailer_data[..]);
        let mut decompressed = Vec::new();

        decoder.read_to_end(&mut decompressed)?;
        Ok(decompressed)
    }
}

#[doc(hidden)]
pub fn compress(data: &[u8]) -> io::Result<Vec<u8>> {
    let mut encoder = DeflateEncoder::new(data, Compression::default());
    let mut compressed = Vec::new();

    encoder.read_to_end(&mut compressed)?;
    compressed.extend_from_slice(PERMESSAFE_DEFLATE_TRAILER);
    Ok(compressed)
}

#[doc(hidden)]
pub fn decompress(data: &[u8]) -> io::Result<Vec<u8>> {
    let stripped = if data.ends_with(PERMESSAFE_DEFLATE_TRAILER) {
        &data[..data.len() - PERMESSAFE_DEFLATE_TRAILER.len()]
    } else {
        data
    };

    let mut decoder = DeflateDecoder::new(stripped);
    let mut decompressed = Vec::new();

    decoder.read_to_end(&mut decompressed)?;
    Ok(decompressed)
}