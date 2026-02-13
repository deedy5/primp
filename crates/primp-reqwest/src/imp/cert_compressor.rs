//! Certificate compression algorithms for TLS connections.
//!
//! This module provides implementations of certificate compression for various
//! compression algorithms supported by TLS certificate compression ([RFC 8879](https://tools.ietf.org/html/rfc8879)).
//!
//! Note: With rustls, certificate compression is handled internally by the
//! crypto provider. This module provides types for compatibility.

/// Brotli certificate compressor marker type.
///
/// Provides high compression ratio for certificate chains.
/// This is the recommended compression algorithm for Chrome impersonation.
#[derive(Debug, Clone, Default)]
pub struct BrotliCompressor;

impl BrotliCompressor {
    /// Creates a new BrotliCompressor.
    #[inline]
    pub fn new() -> Self {
        Self
    }
}

/// Zlib certificate compressor marker type.
///
/// Provides standard zlib compression for certificate chains.
/// Used by Safari for certificate compression.
#[derive(Debug, Clone, Default)]
pub struct ZlibCompressor;

impl ZlibCompressor {
    /// Creates a new ZlibCompressor.
    #[inline]
    pub fn new() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brotli_compressor_creation() {
        let compressor = BrotliCompressor::new();
        let _ = format!("{:?}", compressor);
    }

    #[test]
    fn test_zlib_compressor_creation() {
        let compressor = ZlibCompressor::new();
        let _ = format!("{:?}", compressor);
    }
}
