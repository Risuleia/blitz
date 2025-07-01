#[derive(Debug)]
pub enum HandshakeError {
    MissingUpgradeHeaders,
    InvalidVersion,
    MissingWebSocketKey
}