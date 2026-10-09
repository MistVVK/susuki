//! FFmpeg-originated error classification per design 3.6 / 8.6.
//!
//! card-1 finding #15 resolution (spec-notes N2): input open/permission/IO
//! failures map to exit code 3 as the §3.4/§3.6 CLI contract requires; §8.6's
//! "2/4/5" enumeration is not used for I/O-origin failures.

use std::io;

#[derive(Debug)]
pub enum DecodeError {
    /// §3.6 code 3: cannot open/read/write input
    Io(String),
    /// §3.6 code 4: no decodable configuration at all (even CPU)
    Unsupported(String),
    /// §3.6 code 5: runtime failure (GPU decode/transfer error, init failure, etc.)
    Runtime(String),
    /// §3.6 code 130: interrupt request observed at iteration boundary
    Interrupted,
}

impl DecodeError {
    /// §3.6 exit-code mapping (input I/O -> 3, see module docs).
    pub fn exit_code(&self) -> u8 {
        match self {
            DecodeError::Io(_) => 3,
            DecodeError::Unsupported(_) => 4,
            DecodeError::Runtime(_) => 5,
            DecodeError::Interrupted => 130,
        }
    }

    pub fn message(&self) -> &str {
        match self {
            DecodeError::Io(m) | DecodeError::Unsupported(m) | DecodeError::Runtime(m) => m,
            DecodeError::Interrupted => "interrupted",
        }
    }

    pub fn io_err(op: &str, e: io::Error) -> Self {
        DecodeError::Io(format!("{op}: {e}"))
    }
}
