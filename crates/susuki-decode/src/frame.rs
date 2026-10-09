//! Design 8.2 frame/chunk types crossing the decode boundary.

use std::sync::Arc;

use crate::luma::Luma;

/// One decoded video frame on the canonical frame axis (§5.1).
#[derive(Debug, Clone)]
pub struct DecodedFrame {
    /// presentation-order sequential number, 0-based (§5.1)
    pub index: i64,
    /// raw PTS as reported by the decoder (rational stream tb); analysis never
    /// depends on it (§5.3); kept for diagnostics only.
    pub pts: i64,
    pub luma: Arc<Luma>,
    /// interlaced source: field lines are consumed with stride 2 by analyzers (§6.0)
    pub interlaced: bool,
}

/// All-channel s16-equivalent peaks, one per native-rate sample position.
#[derive(Debug, Clone)]
pub struct AudioChunk {
    pub peaks: Vec<u16>,
    pub sample_rate: u32,
}
