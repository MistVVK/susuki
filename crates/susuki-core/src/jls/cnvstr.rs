//! Derived from join_logo_scp (https://github.com/tobitti0/join_logo_scp,
//! commit d1a24d758f8b3c04352b82ff6756fdf3d56729b3), Copyright (C) tobitti0,
//! GPL-2.0-or-later. This port is a GPL derivative work, redistributed here
//! under GPL-3.0-or-later (design §10.6).
//! Port of REF-JLS `CnvStrTime.cpp` string/number/expr functions
//! (fixed commit d1a24d7). Byte-oriented: scripts are CP932 so all parsing
//! runs on &[u8] with the reference's signed-char comparison quirks kept.
//! Frame<->msec formulas are shared with core::timeaxis (design 5.2).

use crate::timeaxis::TimeConverter;

const SIZE_BUF_MAX: usize = 4096;

// D_CALCCAT_* (CnvStrTime.hpp:11-15)
pub const CALCCAT_IMM: i32 = 0;
pub const CALCCAT_OP2: i32 = 1;
pub const CALCCAT_OP1: i32 = 2;
pub const CALCCAT_PAR: i32 = 3;
pub const CALCCAT_OPE: i32 = 4;

// D_CALCOP_* codes
pub const OP_PERD: i32 = 0x0021;
pub const OP_COLON: i32 = 0x0022;
pub const OP_PLUS: i32 = 0x1501;
pub const OP_MINUS: i32 = 0x1502;
pub const OP_MUL: i32 = 0x1401;
pub const OP_DIV: i32 = 0x1402;
pub const OP_MOD: i32 = 0x1403;
pub const OP_CMPLT: i32 = 0x1701;
pub const OP_CMPLE: i32 = 0x1702;
pub const OP_CMPGT: i32 = 0x1703;
pub const OP_CMPGE: i32 = 0x1704;
pub const OP_CMPEQ: i32 = 0x1801;
pub const OP_CMPNE: i32 = 0x1802;
pub const OP_B_AND: i32 = 0x1901;
pub const OP_B_XOR: i32 = 0x1A01;
pub const OP_B_OR: i32 = 0x1B01;
pub const OP_L_AND: i32 = 0x1C01;
pub const OP_L_OR: i32 = 0x1D01;
pub const OP_NOT: i32 = 0x2201;
pub const OP_SIGNP: i32 = 0x2202;
pub const OP_SIGNM: i32 = 0x2203;
pub const OP_SEC: i32 = 0x2204;
pub const OP_FRM: i32 = 0x2205;
pub const OP_PARS: i32 = 0x3101;
pub const OP_PARE: i32 = 0x3102;
pub const OP_ERROR: i32 = 0xFFFF;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delim {
    Quote, // DELIMIT_SPACE_QUOTE
    Only,  // DELIMIT_SPACE_ONLY
    Comma, // DELIMIT_SPACE_COMMA
    Exnum, // DELIMIT_SPACE_EXNUM
}

/// CnvStrTime: frame-rate-aware converter + string parsers.
#[derive(Debug, Clone, Copy)]
pub struct Cnv {
    pub tc: TimeConverter,
    pub unitsec: i32,
    /// m_delimiter: last separator seen by getStrFilePathName ("\" default)
    pub delimiter: u8,
}

/// Reference throws the offending char position (int) out of getStrCalc;
/// callers catch it, set val = that position and return -1. We mirror with
/// this error payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalcErr(pub i32);

fn ch_at(s: &[u8], i: usize) -> i8 {
    // std::string operator[] at len yields '\0'; bytes >=0x80 are signed char
    match s.get(i) {
        Some(&b) => b as i8,
        None => 0,
    }
}

fn is_space(ch: i8) -> bool {
    ch == b' ' as i8 || ch == b'\t' as i8
}

impl Cnv {
    pub fn new(tc: TimeConverter, unitsec: i32) -> Self {
        Cnv {
            tc,
            unitsec,
            delimiter: b'\\',
        }
    }

    //--- getStrItemRange (CnvStrTime.cpp:570-655) ---
    /// returns (pos_end, st, ed) where pos_end<0 signals failure, exactly as
    /// the reference (pos doubles as return; st/ed valid when pos>=0)
    fn str_item_range(&self, cstr: &[u8], pos: i32, type_: Delim) -> (i32, usize, usize) {
        if pos < 0 {
            return (pos, 0, 0);
        }
        let mut pos = pos as usize;
        let mut flag_quote = 0i32;
        let mut len = 0usize;
        // trim left
        while is_space(ch_at(cstr, pos)) {
            pos += 1;
        }
        // check quote
        if ch_at(cstr, pos) == b'"' as i8 && type_ == Delim::Quote {
            flag_quote += 1;
            pos += 1;
        }
        let st = pos;
        let mut flag_end = 0i32;
        let mut ch;
        loop {
            ch = ch_at(cstr, pos);
            if ch == b'"' as i8 && flag_quote > 0 {
                flag_quote += 1;
                flag_end = 1;
            } else if is_space(ch)
                || (ch == b',' as i8 && type_ == Delim::Comma)
                || ((ch < b'0' as i8 || ch > b'9' as i8) && type_ == Delim::Exnum)
            {
                if type_ != Delim::Quote || flag_quote != 1 {
                    flag_end = 1;
                }
            } else if ch == 0 || len >= SIZE_BUF_MAX - 1 {
                flag_end = 1;
            }
            if flag_end == 0 {
                pos += 1;
                len += 1;
            } else {
                break;
            }
        }
        let ed = pos;
        let mut rpos = pos as i32;
        if flag_quote == 1 {
            rpos = -1; // QUOTE異常
        } else if st == ed && flag_quote <= 1 {
            rpos = -1; // 読み込みデータなし
        } else if ch == b',' as i8 || ch == b'"' as i8 {
            rpos += 1; // 区切り文字は次回スキップ
        }
        (rpos, st, ed)
    }

    //--- getStrItem (290-307) ---
    pub fn get_str_item(&self, dst: &mut Vec<u8>, cstr: &[u8], pos: i32) -> i32 {
        let (rpos, st, ed) = self.str_item_range(cstr, pos, Delim::Quote);
        if rpos >= 0 {
            if ed > st {
                *dst = cstr[st..ed].to_vec();
            } else {
                dst.clear();
            }
        }
        rpos
    }

    //--- getStrWord (309-321) ---
    pub fn get_str_word(&self, dst: &mut Vec<u8>, cstr: &[u8], pos: i32) -> i32 {
        let (rpos, st, ed) = self.str_item_range(cstr, pos, Delim::Comma);
        if rpos >= 0 {
            *dst = cstr[st..ed].to_vec();
        }
        rpos
    }

    //--- getStrWithoutComment (347-359) + getStrPosComment (361-391) ---
    pub fn get_str_without_comment(&self, dst: &mut Vec<u8>, cstr: &[u8]) -> i32 {
        let poscmt = self.get_str_pos_comment(cstr, 0);
        if poscmt > 0 {
            *dst = cstr[0..poscmt as usize].to_vec();
        } else if poscmt == 0 {
            dst.clear();
        } else {
            *dst = cstr.to_vec();
        }
        poscmt
    }

    /// isStrSjisMultiByte (CnvStrTime.cpp:503)
    pub fn is_sjis_multi_byte(s: &[u8], n: usize) -> bool {
        let c0 = *s.get(n).unwrap_or(&0) as i8;
        let mut code = c0 as i32;
        if code < 0 {
            code &= 0xFF;
        } else if code >= 0x80 {
            return false;
        }
        if (0x81..=0x9F).contains(&code) || (0xE0..=0xFC).contains(&code) {
            let c1 = (*s.get(n + 1).unwrap_or(&0) as i8) as i32 & 0xFF;
            if (0x40..=0x7E).contains(&c1) || (0x80..=0xFC).contains(&c1) {
                return true;
            }
        }
        false
    }

    /// isStrSjisSecond (CnvStrTime.cpp:479)
    pub fn is_sjis_second(s: &[u8], n: usize) -> bool {
        // leading range guard from reference (char always within range; kept for parity)
        for _ in 0..s.len() {
            let code = *s.get(n).unwrap_or(&0) as i8;
            let v = code as i32;
            if !(-128..=255).contains(&v) {
                return false;
            }
        }
        let mut i = 0usize;
        while i < n {
            if Self::is_sjis_multi_byte(s, i) {
                i += 1;
            }
            i += 1;
        }
        if i != n {
            return true;
        }
        false
    }

    /// getStrFileDelimiter
    /// changeUnitSec
    pub fn change_unit_sec(&mut self, n: i32) -> i32 {
        self.unitsec = n;
        1
    }

    pub fn get_file_delimiter(&self) -> u8 {
        self.delimiter
    }

    /// getStrFilePathName (CnvStrTime.cpp:69) — mutates delimiter state
    pub fn get_str_file_path_name(
        &mut self,
        pathname: &mut Vec<u8>,
        fname: &mut Vec<u8>,
        fullname: &[u8],
    ) -> i32 {
        let mut flag_find = false;
        let mut nloc = rfind_byte(fullname, b'\\', fullname.len());
        if nloc >= 0 {
            while nloc >= 0 && !flag_find {
                if nloc == 0 || !Self::is_sjis_second(fullname, nloc as usize) {
                    flag_find = true;
                } else {
                    nloc = rfind_byte(fullname, b'\\', (nloc - 1) as usize);
                }
            }
            if flag_find {
                self.delimiter = b'\\';
            }
        }
        let nloc_sl = rfind_byte(fullname, b'/', fullname.len());
        if nloc_sl >= 0 && (!flag_find || nloc < nloc_sl) {
            flag_find = true;
            nloc = nloc_sl;
            self.delimiter = b'/';
        }
        if flag_find {
            *pathname = fullname[..(nloc + 1) as usize].to_vec();
            *fname = fullname[(nloc + 1) as usize..].to_vec();
        } else {
            pathname.clear();
            *fname = fullname.to_vec();
            nloc = -1;
        }
        nloc
    }

    /// getStrFilePath
    pub fn get_str_file_path(&mut self, pathname: &mut Vec<u8>, fullname: &[u8]) -> i32 {
        let mut tmp = Vec::new();
        self.get_str_file_path_name(pathname, &mut tmp, fullname)
    }

    /// getStrFileAllPath
    pub fn get_str_file_all_path(&mut self, pathname: &mut Vec<u8>) -> bool {
        let mut str_tmp = Vec::new();
        let full = pathname.clone();
        self.get_str_file_path(&mut str_tmp, &full);
        if str_tmp != full {
            pathname.push(self.delimiter);
            return true;
        }
        false
    }

    /// getStrItemWithQuote (CnvStrTime.cpp:~300)
    pub fn get_str_item_with_quote(&self, dst: &mut Vec<u8>, cstr: &[u8], pos: i32) -> i32 {
        let posbk = pos.max(0) as usize;
        let mut pos = self.get_str_item(dst, cstr, pos);
        let loc1 = if posbk < cstr.len() {
            cstr[posbk..]
                .iter()
                .position(|&c| c == b'"')
                .map(|p| (p + posbk) as i32)
        } else {
            None
        }
        .unwrap_or(-1);
        if loc1 < posbk as i32 || loc1 >= pos || pos < 0 {
            return pos;
        }
        let locd = dst
            .iter()
            .position(|&c| c == b'"')
            .map(|p| p as i32)
            .unwrap_or(-1);
        if locd > 0 && locd < pos {
            *dst = dst[..locd as usize].to_vec();
            return loc1;
        }
        let st = (loc1 + 1) as usize;
        let loc2 = if st < cstr.len() {
            cstr[st..]
                .iter()
                .position(|&c| c == b'"')
                .map(|p| (p + st) as i32)
        } else {
            None
        }
        .unwrap_or(-1);
        if loc2 > loc1 && loc2 < pos {
            *dst = cstr[loc1 as usize..(loc2 + 1) as usize].to_vec();
            pos = loc2 + 1;
        }
        pos
    }

    pub fn get_str_pos_comment(&self, cstr: &[u8], pos: i32) -> i32 {
        let mut poscmt = -1i32;
        let mut posdol = -1i32;
        let len = cstr.len() as i32;
        let mut flag_qw = false;
        let mut i = 0i32;
        while i < len {
            if poscmt < 0 {
                let ch = cstr[i as usize] as i8;
                if ch == b'#' as i8 {
                    if i >= pos && !flag_qw && !(posdol >= 0 && posdol + 1 == i) {
                        poscmt = i;
                    }
                } else if ch == b'$' as i8 {
                    posdol = i;
                } else if ch == b'"' as i8 {
                    flag_qw = !flag_qw;
                }
            }
            i += 1;
        }
        poscmt
    }

    //--- getStrValSubDelimit / getStrValSub (200-229, 535-556) ---
    /// (pos, Some(val)) on any value produced (incl. errloc on calc failure);
    /// (pos<0, None) when the item-range failed — the reference leaves `val`
    /// untouched there (uninitialized garbage upstream), so callers must not
    /// consume it.
    fn str_val_sub_delimit(
        &self,
        cstr: &[u8],
        pos: i32,
        unitsec: i32,
        type_: Delim,
    ) -> (i32, Option<i32>) {
        let (rpos, st, ed) = self.str_item_range(cstr, pos, type_);
        if rpos < 0 {
            return (rpos, None);
        }
        match self.get_str_calc(cstr, st as i32, ed as i32 - 1, unitsec) {
            Ok(v) => (rpos, Some(v)),
            Err(CalcErr(p)) => (-1, Some(p)),
        }
    }

    fn str_val_sub(&self, cstr: &[u8], pos: i32, unitsec: i32) -> (i32, Option<i32>) {
        self.str_val_sub_delimit(cstr, pos, unitsec, Delim::Only)
    }

    /// (newpos, val) — the reference family getStrValNum/Msec/MsecM1/Sec...
    pub fn get_str_val_num_head(&self, cstr: &[u8], pos: i32) -> (i32, Option<i32>) {
        self.str_val_sub_delimit(cstr, pos, 2, Delim::Exnum)
    }
    pub fn get_str_val_num(&self, cstr: &[u8], pos: i32) -> (i32, Option<i32>) {
        self.str_val_sub(cstr, pos, 2)
    }
    pub fn get_str_val_msec(&self, cstr: &[u8], pos: i32) -> (i32, Option<i32>) {
        self.str_val_sub(cstr, pos, self.unitsec)
    }
    pub fn get_str_val_msec_from_frm(&self, cstr: &[u8], pos: i32) -> (i32, Option<i32>) {
        self.str_val_sub(cstr, pos, 0)
    }
    /// getStrValMsecM1 (230-244): value is msec, but a bare "-1" input stays -1.
    pub fn get_str_val_msec_m1(&self, cstr: &[u8], pos: i32) -> (i32, Option<i32>) {
        let (posnew, val) = self.str_val_sub(cstr, pos, self.unitsec);
        let Some(mut val) = val else {
            return (posnew, None);
        };
        // reference: cstr.substr(pos, posnew-pos).find(".") < 0
        let cond = (self.unitsec == 0 && self.tc.get_frm_from_msec(val as i64) == -1)
            || (self.unitsec == 1 && val == -1000);
        if cond && posnew > 0 {
            let end = (posnew as usize).min(cstr.len());
            let begin = (pos.max(0)) as usize;
            let has_dot = begin < end && cstr[begin..end].contains(&b'.');
            if !has_dot {
                val = -1;
            }
        }
        (posnew, Some(val))
    }
    pub fn get_str_val_sec(&self, cstr: &[u8], pos: i32) -> (i32, Option<i32>) {
        // reference converts tmpval whenever it was written (errloc included);
        // on item-range failure tmpval is left untouched and val must be None.
        let (pos2, tmp) = self.str_val_sub(cstr, pos, self.unitsec);
        match tmp {
            Some(tmpval) => {
                let mut val = (tmpval.abs() + 500) / 1000;
                if tmpval < 0 {
                    val = -val;
                }
                (pos2, Some(val))
            }
            None => (pos2, None),
        }
    }
    pub fn get_str_val_sec_from_sec(&self, cstr: &[u8], pos: i32) -> (i32, Option<i32>) {
        let (pos2, tmp) = self.str_val_sub(cstr, pos, 1);
        match tmp {
            Some(tmpval) => {
                let mut val = (tmpval.abs() + 500) / 1000;
                if tmpval < 0 {
                    val = -val;
                }
                (pos2, Some(val))
            }
            None => (pos2, None),
        }
    }

    //--- getStringMsecM1All / getStringZeroRight (420-470) ---
    /// getSecFromMsec
    pub fn get_sec_from_msec(&self, msec: i32) -> i32 {
        if msec < 0 {
            return -((-msec + 500) / 1000);
        }
        (msec + 500) / 1000
    }

    pub fn get_string_msec_m1(&self, msec_val: i32) -> Vec<u8> {
        self.get_string_msec_m1_all(msec_val, self.unitsec == 0)
    }
    pub fn get_string_frame_msec_m1(&self, msec_val: i32) -> Vec<u8> {
        self.get_string_msec_m1_all(msec_val, true)
    }
    pub fn get_string_time_msec_m1(&self, msec_val: i32) -> Vec<u8> {
        self.get_string_msec_m1_all(msec_val, false)
    }
    fn get_string_msec_m1_all(&self, msec_val: i32, type_frm: bool) -> Vec<u8> {
        if msec_val == -1 {
            return b"-1".to_vec();
        }
        if type_frm {
            let n = self.tc.get_frm_from_msec(msec_val as i64);
            return n.to_string().into_bytes();
        }
        let val_abs = if msec_val < 0 {
            -(msec_val as i64)
        } else {
            msec_val as i64
        };
        let val_t = val_abs / 1000;
        let val_h = val_t / 3600;
        let val_m = (val_t / 60) % 60;
        let val_s = val_t % 60;
        let val_x = val_abs % 1000;
        let mut out = Vec::new();
        out.extend(zero_right(val_h, 2));
        out.push(b':');
        out.extend(zero_right(val_m, 2));
        out.push(b':');
        out.extend(zero_right(val_s, 2));
        if val_x > 0 {
            out.push(b'.');
            out.extend(zero_right(val_x, 3));
        }
        if msec_val < 0 {
            let mut v = b"-".to_vec();
            v.extend(out);
            return v;
        }
        out
    }

    //--- expression code recognition (837-1005) ---
    fn calc_code_char(ch: i8, head: i32) -> i32 {
        if ch >= b'0' as i8 && ch <= b'9' as i8 {
            return (ch - b'0' as i8) as i32;
        }
        match ch {
            c if c == b'.' as i8 => OP_PERD,
            c if c == b':' as i8 => OP_COLON,
            c if c == b'+' as i8 => {
                if head > 0 {
                    OP_SIGNP
                } else {
                    OP_PLUS
                }
            }
            c if c == b'-' as i8 => {
                if head > 0 {
                    OP_SIGNM
                } else {
                    OP_MINUS
                }
            }
            c if c == b'*' as i8 => OP_MUL,
            c if c == b'/' as i8 => OP_DIV,
            c if c == b'%' as i8 => OP_MOD,
            c if c == b'!' as i8 => OP_NOT,
            c if c == b'S' as i8 => OP_SEC,
            c if c == b'F' as i8 => OP_FRM,
            c if c == b'(' as i8 => OP_PARS,
            c if c == b')' as i8 => OP_PARE,
            c if c == b'<' as i8 => OP_CMPLT,
            c if c == b'>' as i8 => OP_CMPGT,
            c if c == b'&' as i8 => OP_B_AND,
            c if c == b'^' as i8 => OP_B_XOR,
            c if c == b'|' as i8 => OP_B_OR,
            _ => OP_ERROR,
        }
    }

    fn calc_code_two_char(ch1: i8, ch2: i8, head: i32) -> i32 {
        match ch1 {
            c if c == b'=' as i8 => match ch2 {
                x if x == b'=' as i8 => OP_CMPEQ,
                x if x == b'<' as i8 => OP_CMPLE,
                x if x == b'>' as i8 => OP_CMPGE,
                _ => -1,
            },
            c if c == b'<' as i8 => {
                if ch2 == b'=' as i8 {
                    OP_CMPLE
                } else {
                    -1
                }
            }
            c if c == b'>' as i8 => {
                if ch2 == b'=' as i8 {
                    OP_CMPGE
                } else {
                    -1
                }
            }
            c if c == b'!' as i8 => {
                if ch2 == b'=' as i8 {
                    OP_CMPNE
                } else {
                    -1
                }
            }
            c if c == b'&' as i8 => {
                if ch2 == b'&' as i8 {
                    OP_L_AND
                } else {
                    -1
                }
            }
            c if c == b'|' as i8 => {
                if ch2 == b'|' as i8 {
                    OP_L_OR
                } else {
                    -1
                }
            }
            c if c == b'+' as i8 => {
                if ch2 == b'+' as i8 && head == 1 {
                    OP_NOT // D_CALCOP_P_INC == 0x2201 (same value as NOT; unimplemented)
                } else {
                    -1
                }
            }
            c if c == b'-' as i8 && ch2 == b'-' as i8 && head == 1 => {
                OP_SIGNP // D_CALCOP_P_DEC == 0x2202 (same value as SIGNP; unimplemented)
            }
            _ => -1,
        }
    }

    fn mark_category(code: i32) -> i32 {
        code / 0x1000
    }
    fn mark_prior(code: i32) -> i32 {
        (code % 0x1000) / 0x100
    }

    pub fn get_str_calc(
        &self,
        cstr: &[u8],
        st: i32,
        ed: i32,
        unitsec: i32,
    ) -> Result<i32, CalcErr> {
        self.get_str_calc_decode(cstr, st, ed, unitsec, 0)
    }

    /// getStrCalcDecode (692-817)
    pub fn get_str_calc_decode(
        &self,
        cstr: &[u8],
        st: i32,
        ed: i32,
        dsec: i32,
        draw: i32,
    ) -> Result<i32, CalcErr> {
        if st > ed {
            return Err(CalcErr(st));
        }
        let mut code_mark_op = 0i32;
        let mut prior_mark_op = 0i32;
        let mut n_par_i = 0i32;
        let mut n_par_op = -1i32;
        let mut pos_op_s = -1i32;
        let mut pos_op_e = -1i32;
        let mut flag_head = 1i32;
        let mut flag_two_op = 0i32;
        let mut i = st;
        while i <= ed {
            if flag_two_op > 0 {
                flag_two_op = 0;
                i += 1;
                continue;
            }
            let mut code_mark_i = Self::calc_code_char(ch_at(cstr, i as usize), flag_head);
            if i < ed {
                let code_two = Self::calc_code_two_char(
                    ch_at(cstr, i as usize),
                    ch_at(cstr, (i + 1) as usize),
                    flag_head,
                );
                if code_two > 0 {
                    code_mark_i = code_two;
                    flag_two_op = 1;
                }
            }
            if code_mark_i == OP_ERROR {
                return Err(CalcErr(i));
            }
            let categ = Self::mark_category(code_mark_i);
            let prior = Self::mark_prior(code_mark_i);
            if code_mark_i == OP_PARS {
                n_par_i += 1;
            } else if code_mark_i == OP_PARE {
                n_par_i -= 1;
                if n_par_i < 0 {
                    return Err(CalcErr(i));
                }
            } else if categ == CALCCAT_OP1 {
                let next_i = if flag_two_op > 0 { i + 2 } else { i + 1 };
                if next_i > ed {
                    return Err(CalcErr(i));
                }
                if code_mark_i == OP_SEC || code_mark_i == OP_FRM {
                    let next_code = Self::calc_code_char(ch_at(cstr, next_i as usize), flag_head);
                    if next_code != OP_PARS {
                        return Err(CalcErr(i));
                    }
                }
            } else if categ == CALCCAT_OP2 {
                if (n_par_op == n_par_i && prior_mark_op <= prior)
                    || (n_par_op > n_par_i)
                    || pos_op_s < 0
                {
                    pos_op_s = i;
                    pos_op_e = if flag_two_op > 0 { i + 1 } else { i };
                    prior_mark_op = prior;
                    code_mark_op = code_mark_i;
                    n_par_op = n_par_i;
                }
                flag_head = 1;
            } else {
                flag_head = 0;
                if pos_op_s < 0 && (n_par_op > n_par_i || n_par_op < 0) {
                    n_par_op = n_par_i;
                }
            }
            i += 1;
        }
        if n_par_i != 0 {
            return Err(CalcErr(ed));
        }
        let mut st = st;
        let mut ed = ed;
        let mut flag_loop = 1i32;
        while n_par_op > 0 && flag_loop > 0 {
            let cs = Self::calc_code_char(ch_at(cstr, st as usize), 1);
            let ce = Self::calc_code_char(ch_at(cstr, ed as usize), 0);
            if cs == OP_PARS && ce == OP_PARE {
                st += 1;
                ed -= 1;
                n_par_op -= 1;
            } else {
                flag_loop = 0;
            }
        }
        if pos_op_s > 0 && n_par_op == 0 {
            if pos_op_s == st || pos_op_e == ed {
                return Err(CalcErr(pos_op_s));
            }
            let raw2 = if code_mark_op == OP_MUL || code_mark_op == OP_DIV {
                1
            } else {
                draw
            };
            let d1 = self.get_str_calc_decode(cstr, st, pos_op_s - 1, dsec, draw)?;
            let d2 = self.get_str_calc_decode(cstr, pos_op_e + 1, ed, dsec, raw2)?;
            self.get_str_calc_op2(d1, d2, code_mark_op)
        } else {
            let code_s = Self::calc_code_char(ch_at(cstr, st as usize), 1);
            let categ_s = Self::mark_category(code_s);
            if categ_s == CALCCAT_OP1 {
                let mut dsec2 = dsec;
                if code_s == OP_SEC {
                    dsec2 = 1;
                } else if code_s == OP_FRM {
                    dsec2 = 0;
                }
                let d1 = self.get_str_calc_decode(cstr, st + 1, ed, dsec2, draw)?;
                Ok(Self::get_str_calc_op1(d1, code_s))
            } else {
                self.get_str_calc_time(cstr, st, ed, dsec, draw)
            }
        }
    }

    /// getStrCalcTime (969-1052). Division by zero in Op2 throws with the
    /// right operand value (reference `throw din2`), kept as CalcErr payload.
    fn get_str_calc_time(
        &self,
        cstr: &[u8],
        st: i32,
        ed: i32,
        dsec: i32,
        draw: i32,
    ) -> Result<i32, CalcErr> {
        let mut vin = 0i32;
        let mut val = 0i32;
        let mut vms = 0i32;
        let mut flag_sec = 0i32;
        let mut flag_prd = 0i32;
        let mut mult_prd = 0i32;
        let mut i = st;
        while i <= ed {
            let code = Self::calc_code_char(ch_at(cstr, i as usize), 0);
            let categ = Self::mark_category(code);
            if categ == CALCCAT_IMM {
                if code == OP_COLON {
                    flag_sec = 1;
                    val = val.wrapping_add(vin).wrapping_mul(60);
                    vin = 0;
                } else if code == OP_PERD {
                    flag_sec = 1;
                    flag_prd += 1;
                    mult_prd = 100;
                    val += vin;
                    vin = 0;
                } else if flag_prd == 0 {
                    // reference relies on int wrap here (UB on overflow);
                    // wrapping keeps deterministic identical behaviour
                    vin = vin.wrapping_mul(10).wrapping_add(code);
                } else if flag_prd == 1 {
                    vms = code.wrapping_mul(mult_prd).wrapping_add(vms);
                    mult_prd /= 10;
                }
            } else {
                return Err(CalcErr(i));
            }
            i += 1;
        }
        val += vin;
        let data = if draw > 0 || dsec == 2 {
            val
        } else if flag_sec == 0 {
            if dsec == 0 {
                self.tc.get_msec_from_frm(val as i64) as i32
            } else {
                val.wrapping_mul(1000)
            }
        } else {
            val.wrapping_mul(1000).wrapping_add(vms)
        };
        Ok(data)
    }

    fn get_str_calc_op1(din: i32, code: i32) -> i32 {
        match code {
            OP_NOT => {
                if din == 0 {
                    1
                } else {
                    0
                }
            }
            OP_SIGNP => din,
            OP_SIGNM => -din,
            _ => din,
        }
    }

    fn get_str_calc_op2(&self, din1: i32, din2: i32, code: i32) -> Result<i32, CalcErr> {
        let ret = match code {
            OP_PLUS => din1 + din2,
            OP_MINUS => din1 - din2,
            OP_MUL => din1 * din2,
            OP_DIV => {
                if din2 == 0 {
                    return Err(CalcErr(din2));
                }
                din1 / din2
            }
            OP_MOD => {
                if din2 == 0 {
                    return Err(CalcErr(din2));
                }
                din1 % din2
            }
            OP_CMPLT => (din1 < din2) as i32,
            OP_CMPLE => (din1 <= din2) as i32,
            OP_CMPGT => (din1 > din2) as i32,
            OP_CMPGE => (din1 >= din2) as i32,
            OP_CMPEQ => (din1 == din2) as i32,
            OP_CMPNE => (din1 != din2) as i32,
            OP_B_AND => din1 & din2,
            OP_B_XOR => din1 ^ din2,
            OP_B_OR => din1 | din2,
            OP_L_AND => (din1 != 0 && din2 != 0) as i32,
            OP_L_OR => (din1 != 0 || din2 != 0) as i32,
            _ => din1,
        };
        Ok(ret)
    }
}

/// getStringZeroRight: zero-pad to len, take the right `len` chars.
fn zero_right(val: i64, len: i32) -> Vec<u8> {
    let mut sv = vec![b'0'; (len - 1).max(0) as usize];
    sv.extend(val.to_string().into_bytes());
    let take = sv.len().min(len as usize);
    sv[sv.len() - take..].to_vec()
}

/// string::rfind(c, pos) for char: last occurrence at or before pos
fn rfind_byte(hay: &[u8], needle: u8, pos: usize) -> i32 {
    let limit = pos.min(hay.len().saturating_sub(1));
    if hay.is_empty() {
        return -1;
    }
    let mut i = limit;
    loop {
        if hay[i] == needle {
            return i as i32;
        }
        if i == 0 {
            return -1;
        }
        i -= 1;
    }
}
