//! Derived from logoframe (https://github.com/tobitti0/logoframe,
//! commit c93aec0c89838e9b4dd3a9a3d5496dfaaa41311d), Copyright (C) tobitti0,
//! GPL-2.0-or-later. This port is a GPL derivative work, redistributed here
//! under GPL-3.0-or-later (design §10.6).
//! Design 6.3 shared logo math (REF-LOGO logo.h / logoframe_det.c macros).

pub const LOGO_MAX_DP: i64 = 1000;

/// REF ConvLogo(d, dp_y, y) = (d*(1000-dp) + y*dp + 500)/1000 (design 6.3.1,
/// LOGO_MAX_DP = 1000; C integer division truncation kept).
pub fn conv_logo(d: i64, dp_y: i64, y: i64) -> i64 {
    (d * (LOGO_MAX_DP - dp_y) + y * dp_y + LOGO_MAX_DP / 2) / LOGO_MAX_DP
}

/// REF ConvInvLogo(d, dp_y, y) = (d*1000 - y*dp + (1000-dp)/2)/(1000-dp).
pub fn conv_inv_logo(d: i64, dp_y: i64, y: i64) -> i64 {
    (d * LOGO_MAX_DP - y * dp_y + (LOGO_MAX_DP - dp_y) / 2) / (LOGO_MAX_DP - dp_y)
}
