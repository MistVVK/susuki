//! Exit codes (§3.6) and warning codes (§9.2).
//!
//! Error classification follows the §3.6 table as the normative CLI contract
//! (§2.1: CLI contract in §3 is book-authority). Design confirmation card-1 #15:
//! §8.6 lists "2/4/5" for FFmpeg errors, but §3.4 (L349-352) and §3.6 (code 3
//! "cannot open input") explicitly classify input-open failures as 3. We therefore
//! map FFmpeg I/O-originated failures (open/permission) to 3, codec/profile
//! unsupported to 4, decode/analysis runtime failures to 5.

pub const EXIT_OK: u8 = 0;
pub const EXIT_ARG: u8 = 2;
pub const EXIT_IO: u8 = 3;
pub const EXIT_UNSUPPORTED: u8 = 4;
pub const EXIT_RUNTIME: u8 = 5;
pub const EXIT_SCRIPT: u8 = 6;
pub const EXIT_INTERRUPTED: u8 = 130;

/// §9.2 warnings codes (schema_version 1 fixed set)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarnCode {
    NoAudio,
    LogoOutOfFrame,
    LogoResolutionMismatch,
    NoLogoDetected,
    InputChanged,
    AnalysisGpuFallback,
}

impl WarnCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            WarnCode::NoAudio => "no_audio",
            WarnCode::LogoOutOfFrame => "logo_out_of_frame",
            WarnCode::LogoResolutionMismatch => "logo_resolution_mismatch",
            WarnCode::NoLogoDetected => "no_logo_detected",
            WarnCode::InputChanged => "input_changed",
            WarnCode::AnalysisGpuFallback => "analysis_gpu_fallback",
        }
    }
}
