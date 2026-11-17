//! Design 8.2 frame/chunk types crossing the decode boundary.

use std::sync::Arc;

use crate::luma::Luma;

/// One decoded video frame on the canonical frame axis (§5.1).
#[derive(Debug, Clone)]
pub struct DecodedFrame {
    /// presentation-order sequential number, 0-based (§5.1)
    pub index: i64,
    /// Raw PTS as reported by the decoder in the source stream time base.
    pub pts: i64,
    /// Presentation timestamp in microseconds using the source stream time base.
    pub pts_us: Option<i64>,
    /// Decoded frame duration in microseconds, when supplied by the demuxer.
    pub duration_us: Option<i64>,
    /// 色記述の連続区間。異なる区間のフレーム同士を比較しない。
    pub color_epoch: u64,
    pub luma: Arc<Luma>,
    /// interlaced source: field lines are consumed with stride 2 by analyzers (§6.0)
    pub interlaced: bool,
}

/// All-channel s16-equivalent peaks, one per native-rate sample position.
#[derive(Debug, Clone)]
pub struct AudioChunk {
    pub peaks: Vec<u16>,
    pub sample_rate: u32,
    /// Presentation timestamp in microseconds using the source stream time base.
    pub pts_us: Option<i64>,
}
