//! susuki-decode: FFmpeg FFI, device management (design 4.2/4.3), decode engine,
//! luma generation (6.0/6.1 input), audio normalization (5.4).
//! FFmpeg types are confined to this crate (design 8.1).

pub mod audio;
pub mod device;
pub mod engine;
pub mod error;
pub mod frame;
pub mod luma;
pub mod sys;

pub use engine::{DecoderEngine, FrameEvent};
pub use error::DecodeError;
pub use frame::{AudioChunk, DecodedFrame};
