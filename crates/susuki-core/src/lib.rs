//! susuki-core: scene detection (design 6.2), logo detection (6.3), JLS judge (7),
//! time axis (5). FFmpeg types are never visible here (design 8.1).

pub mod error;
pub mod gpu;
pub mod jls;
pub mod logo;
pub mod logo_calc;
pub mod logo_find;
pub mod logo_mul;
pub mod logo_set;
pub mod mvec;
pub mod scene;
pub mod timeaxis;

pub use error::{
    WarnCode, EXIT_ARG, EXIT_INTERRUPTED, EXIT_IO, EXIT_OK, EXIT_RUNTIME, EXIT_SCRIPT,
    EXIT_UNSUPPORTED,
};
