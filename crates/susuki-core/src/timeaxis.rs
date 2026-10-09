//! Derived from join_logo_scp (https://github.com/tobitti0/join_logo_scp,
//! commit d1a24d758f8b3c04352b82ff6756fdf3d56729b3), Copyright (C) tobitti0,
//! GPL-2.0-or-later. This port is a GPL derivative work, redistributed here
//! under GPL-3.0-or-later (design §10.6).
//! Design 5.1-5.3 canonical time axis and REF-JLS compatible conversions.
//! Formulas match REF-JLS `src/CnvStrTime.cpp` (fixed commit d1a24d7) exactly:
//! absolute-value half-up rounding with sign restored, integer arithmetic only.

/// Frame<->time converter on the nominal frame rate (frate_n / frate_d).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeConverter {
    pub frate_n: i64,
    pub frate_d: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeError {
    InvalidFrameRate,
}

impl TimeConverter {
    /// §5.2: frate comes from avg_frame_rate (fallback r_frame_rate);
    /// both indeterminate -> caller maps to exit code 4.
    pub fn new(frate_n: i64, frate_d: i64) -> Result<Self, TimeError> {
        if frate_n <= 0 || frate_d <= 0 {
            return Err(TimeError::InvalidFrameRate);
        }
        Ok(TimeConverter { frate_n, frate_d })
    }

    /// REF CnvStrTime::getFrmFromMsec (CnvStrTime.cpp:125-131)
    pub fn get_frm_from_msec(&self, msec: i64) -> i64 {
        let denom = self.frate_d * 1000;
        let r = (msec.abs() * self.frate_n + denom / 2) / denom;
        if msec >= 0 {
            r
        } else {
            -r
        }
    }

    /// REF CnvStrTime::getMsecFromFrm (CnvStrTime.cpp:133-139)
    pub fn get_msec_from_frm(&self, frm: i64) -> i64 {
        let r = (frm.abs() * self.frate_d * 1000 + self.frate_n / 2) / self.frate_n;
        if frm >= 0 {
            r
        } else {
            -r
        }
    }

    /// REF CnvStrTime::getMsecAlignFromMsec (CnvStrTime.cpp:141-144)
    pub fn get_msec_align_from_msec(&self, msec: i64) -> i64 {
        let frm = self.get_frm_from_msec(msec);
        self.get_msec_from_frm(frm)
    }

    /// REF CnvStrTime::getMsecAdjustFrmFromMsec (CnvStrTime.cpp:149-152).
    /// §5.2 / card-1 #7: -lastcut is applied through this function on the
    /// nominal-fps axis (REF-JLS JlsIF.cpp:523-525).
    pub fn get_msec_adjust_frm_from_msec(&self, msec: i64, adj: i64) -> i64 {
        let frm_new = self.get_frm_from_msec(msec) + adj;
        self.get_msec_from_frm(frm_new)
    }

    /// REF CnvStrTime::getSecFromMsec (CnvStrTime.cpp:157-162)
    pub fn get_sec_from_msec(&self, msec: i64) -> i64 {
        if msec < 0 {
            -((-msec + 500) / 1000)
        } else {
            (msec + 500) / 1000
        }
    }
}

/// atoi-equivalent leading signed integer parse (REF `atoi` semantics).
fn atoi_prefix(s: &str) -> i64 {
    let b = s.as_bytes();
    let mut i = 0usize;
    while i < b.len() && (b[i] == b' ' || b[i] == b'\t' || b[i] == b'\n') {
        i += 1;
    }
    let mut neg = false;
    if i < b.len() && (b[i] == b'-' || b[i] == b'+') {
        neg = b[i] == b'-';
        i += 1;
    }
    let mut v: i64 = 0;
    let mut any = false;
    while i < b.len() && b[i].is_ascii_digit() {
        v = v * 10 + (b[i] - b'0') as i64;
        any = true;
        i += 1;
    }
    let _ = any;
    if neg {
        -v
    } else {
        v
    }
}

/// REF-JLS `JlsScript::setOptionsCnvCutMrg` (JlsScript.cpp:465-481, fixed commit):
/// 30fps-based frame count (decimal allowed; digits '.'+1..'+2 are used as
/// hundredths) -> milliseconds via (|x|*1001 + 30/2)/30 integer arithmetic.
///
/// The design book §5.2 parenthetical says "* 1000/30"; the REF fixed source
/// uses 1001/30 (NTSC-compatible ms/frame). Per §2.1 the ported behavior
/// follows the fixed source and the book line is a transcription defect
/// (recorded in evidence/susuki-implementation/impl/spec-notes.md).
pub fn cnv_cut_mrg(s: &str) -> i64 {
    let num = atoi_prefix(s);
    let mut frac: i64 = 0;
    if let Some(dot) = s.find('.') {
        let b = s.as_bytes();
        let d1 = dot + 1;
        if d1 < b.len() && b[d1].is_ascii_digit() {
            frac = (b[d1] - b'0') as i64 * 10;
            let d2 = dot + 2;
            if d2 < b.len() && b[d2].is_ascii_digit() {
                frac += (b[d2] - b'0') as i64;
            }
        }
    }
    let msec_num = (num.abs() * 1001 + 30 / 2) / 30;
    let msec_frac = (frac * 1001 + 30 / 2) / 30 / 100;
    let mut result = msec_num + msec_frac;
    if num < 0 {
        result = -result;
    }
    result
}
