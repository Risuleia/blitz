//! WebSocket configuration module

use crate::protocol::compression::WebSocketCompressionConfig;

/// The configuration for WebSocket connection.
///
/// # Example
/// ```
/// # use blitz::protocol::WebSocketConfig;;
/// let conf = WebSocketConfig::default()
///     .read_buffer_size(256 * 1024)
///     .write_buffer_size(256 * 1024);
/// ```
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct WebSocketConfig {
    /// Read buffer capacity. This buffer is eagerly allocated and used for receiving
    /// messages.
    ///
    /// For high read load scenarios a larger buffer, e.g. 128 KiB, improves performance.
    ///
    /// For scenarios where you expect a lot of connections and don't need high read load
    /// performance a smaller buffer, e.g. 4 KiB, would be appropriate to lower total
    /// memory usage.
    ///
    /// The default value is 128 KiB.
    pub read_buffer_size: usize,
    /// The target minimum size of the write buffer to reach before writing the data
    /// to the underlying stream.
    /// The default value is 128 KiB.
    ///
    /// If set to `0` each message will be eagerly written to the underlying stream.
    /// It is often more optimal to allow them to buffer a little, hence the default value.
    ///
    /// Note: [`flush`](WebSocket::flush) will always fully write the buffer regardless.
    pub write_buffer_size: usize,
    /// The max size of the write buffer in bytes. Setting this can provide backpressure
    /// in the case the write buffer is filling up due to write errors.
    /// The default value is unlimited.
    ///
    /// Note: The write buffer only builds up past [`write_buffer_size`](Self::write_buffer_size)
    /// when writes to the underlying stream are failing. So the **write buffer can not
    /// fill up if you are not observing write errors even if not flushing**.
    ///
    /// Note: Should always be at least [`write_buffer_size + 1 message`](Self::write_buffer_size)
    /// and probably a little more depending on error handling strategy.
    pub max_write_buffer_size: usize,
    /// The maximum size of an incoming message. `None` means no size limit. The default value is 64 MiB
    /// which should be reasonably big for all normal use-cases but small enough to prevent
    /// memory eating by a malicious user.
    pub max_message_size: Option<usize>,
    /// The maximum size of a single incoming message frame. `None` means no size limit. The limit is for
    /// frame payload NOT including the frame header. The default value is 16 MiB which should
    /// be reasonably big for all normal use-cases but small enough to prevent memory eating
    /// by a malicious user.
    pub max_frame_size: Option<usize>,
    /// When set to `true`, the server will accept and handle unmasked frames
    /// from the client. According to the RFC 6455, the server must close the
    /// connection to the client in such cases, however it seems like there are
    /// some popular libraries that are sending unmasked frames, ignoring the RFC.
    /// By default this option is set to `false`, i.e. according to RFC 6455.
    pub accept_unmasked_frames: bool,
    /// Configuration for compression module
    pub compression: WebSocketCompressionConfig
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            read_buffer_size: 128 * 1024,
            write_buffer_size: 128 * 1024,
            max_write_buffer_size: usize::MAX,
            max_message_size: Some(64 << 20),
            max_frame_size: Some(64 << 20),
            accept_unmasked_frames: false,
            compression: WebSocketCompressionConfig::default()
        }
    }
}

impl WebSocketConfig {
    /// Set [`Self::read_buffer_size`].
    pub fn read_buffer_size(mut self, size: usize) -> Self {
        assert!(size > 0);
        self.read_buffer_size = size;
        self
    }

    /// Set [`Self::write_buffer_size`].
    pub fn write_buffer_size(mut self, size: usize) -> Self {
        assert!(size > 0);
        self.write_buffer_size = size;
        self
    }

    /// Set [`Self::max_write_buffer_size`].
    pub fn  max_write_buffer_size(mut self, size: usize) -> Self {
        assert!(size > 0);
        self.max_write_buffer_size = size;
        self
    }

    /// Set [`Self::max_message_size`].
    pub fn max_message_size(mut self, size: Option<usize>) -> Self {
        assert!(if size.is_some() { size.unwrap() > 0 } else { true });
        self.max_message_size = size;
        self
    }

    /// Set [`Self::max_frame_size`].
    pub fn max_frame_size(mut self, size: Option<usize>) -> Self {
        assert!(if size.is_some() { size.unwrap() > 0 } else { true });
        self.max_frame_size = size;
        self
    }

    /// Set [`Self::accept_unmasked_frames`].
    pub fn accept_unmasked_frames(mut self, accept_unmasked_frames: bool) -> Self {
        self.accept_unmasked_frames = accept_unmasked_frames;
        self
    }

    /// Panic if values are invalid.
    pub(crate) fn asset_valid(&self) {
        assert!(
            self.max_write_buffer_size > self.write_buffer_size,
            "WebSocketConfig::max_write_buffer_size must be greater than write_buffer_size, \
            see WebSocketConfig docs`"
        );
    }
}