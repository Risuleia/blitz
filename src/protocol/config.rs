use crate::{protocol::compression::WebSocketCompressionConfig, MAX_ALLOWED_LEN, MAX_CONTINUATION_FRAMES, MAX_CONTROL_FRAME_PAYLOAD};

#[derive(Debug, Clone)]
pub struct WebSocketConfig {
    pub max_message_size: usize,
    pub max_control_frame_payload: usize,
    pub max_continuation_frames: usize,
    pub compression: WebSocketCompressionConfig
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            max_message_size: MAX_ALLOWED_LEN,
            max_control_frame_payload: MAX_CONTROL_FRAME_PAYLOAD,
            max_continuation_frames: MAX_CONTINUATION_FRAMES,
            compression: WebSocketCompressionConfig::default()           
        }
    }
}