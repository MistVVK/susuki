//! Derived from join_logo_scp (https://github.com/tobitti0/join_logo_scp,
//! commit d1a24d758f8b3c04352b82ff6756fdf3d56729b3), Copyright (C) tobitti0,
//! GPL-2.0-or-later. This port is a GPL derivative work, redistributed here
//! under GPL-3.0-or-later (design §10.6).
//! Port of REF-JLS `JlsCmdSet.cpp` class JlsCmdArg (storage layer only;
//! JlsCmdLimit/executor come with the evaluation stage).
//! Design 7.3: option category routing, defaults and lists match the fixed
//! commit d1a24d7 semantics (strict `>MIN && <MAX` boundaries kept).

use crate::jls::tables::{CmdCat, CmdType, OptType};

pub const WIDE_NONE: i32 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WideMsec {
    pub just: i32,
    pub early: i32,
    pub late: i32,
}

// CmdCat Tack sub-struct + LazyType (JlsNameSpace.hpp:424)
#[derive(Debug, Clone, Copy, Default)]
pub struct Tack {
    pub float_base: bool,
    pub virtual_logo: bool,
    pub ignore_comp: bool,
    pub limit_by_logo: bool,
    pub one_point: bool,
    pub need_auto: bool,
    pub full_frame: bool,
    pub type_lazy: i32,
    pub ignore_abort: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct ScOpt {
    pub opt: OptType,
    pub relative: bool,
    pub min: i32,
    pub max: i32,
}

const SIZE_ARR: usize = (OptType::ArrayMAX as usize) - (OptType::ArrayMIN as usize) - 1;
const SIZE_STR: usize = (OptType::StrMAX as usize) - (OptType::StrMIN as usize) - 1;

#[derive(Debug, Clone)]
pub struct CmdArg {
    pub cmdsel: CmdType,
    pub category: CmdCat,
    pub wmsec: WideMsec,
    pub select_edge: i32,     // LogoEdgeType
    pub select_auto_sub: i32, // CmdTrSpEcID
    pub tack: Tack,
    pub num_check_cond: i32,
    pub flag_cond: bool,
    pub optdata: [i32; SIZE_ARR],
    pub flagset: [bool; SIZE_ARR],
    pub opt_str_data: Vec<Vec<u8>>,
    pub flag_str_set: [bool; SIZE_STR],
    pub flag_str_update: [bool; SIZE_STR],
    pub list_str_arg: Vec<Vec<u8>>,
    pub list_sc_opt: Vec<ScOpt>,
    pub list_lg_val: Vec<Vec<u8>>,
}

impl Default for CmdArg {
    fn default() -> Self {
        Self::new()
    }
}

impl CmdArg {
    /// JlsCmdArg::clear() incl. non-zero defaults and initial strings.
    pub fn new() -> Self {
        let mut c = CmdArg {
            cmdsel: CmdType::Nop,
            category: CmdCat::NONE,
            wmsec: WideMsec::default(),
            select_edge: 0,
            select_auto_sub: 0,
            tack: Tack::default(),
            num_check_cond: 0,
            flag_cond: false,
            optdata: [0; SIZE_ARR],
            flagset: [false; SIZE_ARR],
            opt_str_data: vec![Vec::new(); SIZE_STR],
            flag_str_set: [false; SIZE_STR],
            flag_str_update: [false; SIZE_STR],
            list_str_arg: Vec::new(),
            list_sc_opt: Vec::new(),
            list_lg_val: Vec::new(),
        };
        for t in [
            OptType::MsecFrameL,
            OptType::MsecFrameR,
            OptType::MsecLenPMin,
            OptType::MsecLenPMax,
            OptType::MsecLenNMin,
            OptType::MsecLenNMax,
            OptType::MsecLenPEMin,
            OptType::MsecLenPEMax,
            OptType::MsecLenNEMin,
            OptType::MsecLenNEMax,
            OptType::MsecFromAbs,
            OptType::MsecFromHead,
            OptType::MsecFromTail,
        ] {
            c.set_opt_default(t, -1);
        }
        let defs: [(OptType, &[u8]); 9] = [
            (OptType::StrRegPos, b"POSHOLD"),
            (OptType::StrValPosR, b"-1"),
            (OptType::StrValPosW, b"-1"),
            (OptType::StrRegList, b"LISTHOLD"),
            (OptType::StrValListR, b""),
            (OptType::StrValListW, b""),
            (OptType::StrRegSize, b"SIZEHOLD"),
            (OptType::StrRegEnv, b""),
            (OptType::StrArgVal, b""),
        ];
        for (t, s) in defs {
            c.set_str_opt_default(t, s.to_vec());
        }
        c
    }

    /// JlsCmdArg::clear(): full reset incl. the same defaults.
    pub fn clear(&mut self) {
        *self = CmdArg::new();
    }

    fn arr_num(tp: OptType) -> Option<usize> {
        let n = tp as i32;
        if n > OptType::ArrayMIN as i32 && n < OptType::ArrayMAX as i32 {
            Some((n - OptType::ArrayMIN as i32 - 1) as usize)
        } else {
            None
        }
    }
    fn str_num(tp: OptType) -> Option<usize> {
        let n = tp as i32;
        if n > OptType::StrMIN as i32 && n < OptType::StrMAX as i32 {
            Some((n - OptType::StrMIN as i32 - 1) as usize)
        } else {
            None
        }
    }

    /// raw-int accessors mirroring the reference (OptType) cast path
    pub fn get_opt_raw(&self, n: i32) -> i32 {
        self.get_opt(unsafe { std::mem::transmute::<u32, OptType>(n as u32) })
    }
    pub fn is_set_opt_raw(&self, n: i32) -> bool {
        self.is_set_opt(unsafe { std::mem::transmute::<u32, OptType>(n as u32) })
    }
    pub fn get_str_opt_raw(&self, n: i32) -> Vec<u8> {
        self.get_str_opt(unsafe { std::mem::transmute::<u32, OptType>(n as u32) })
    }
    pub fn is_set_str_opt_raw(&self, n: i32) -> bool {
        self.is_set_str_opt(unsafe { std::mem::transmute::<u32, OptType>(n as u32) })
    }
    pub fn is_update_str_opt_raw(&self, n: i32) -> bool {
        self.is_update_str_opt(unsafe { std::mem::transmute::<u32, OptType>(n as u32) })
    }

    pub fn set_opt(&mut self, tp: OptType, val: i32) {
        if let Some(n) = Self::arr_num(tp) {
            self.optdata[n] = val;
            self.flagset[n] = true;
        }
    }
    pub fn set_opt_default(&mut self, tp: OptType, val: i32) {
        if let Some(n) = Self::arr_num(tp) {
            self.optdata[n] = val;
            self.flagset[n] = false;
        }
    }
    /// reference returns `false` (0) for out-of-range routing
    pub fn get_opt(&self, tp: OptType) -> i32 {
        match Self::arr_num(tp) {
            Some(n) => self.optdata[n],
            None => 0,
        }
    }
    pub fn is_set_opt(&self, tp: OptType) -> bool {
        match Self::arr_num(tp) {
            Some(n) => self.flagset[n],
            None => false,
        }
    }

    pub fn set_str_opt(&mut self, tp: OptType, s: Vec<u8>) {
        if let Some(n) = Self::str_num(tp) {
            self.opt_str_data[n] = s;
            self.flag_str_set[n] = true;
            self.flag_str_update[n] = true;
        }
    }
    pub fn set_str_opt_default(&mut self, tp: OptType, s: Vec<u8>) {
        if let Some(n) = Self::str_num(tp) {
            self.opt_str_data[n] = s;
            self.flag_str_set[n] = false;
            self.flag_str_update[n] = false;
        }
    }
    pub fn get_str_opt(&self, tp: OptType) -> Vec<u8> {
        match Self::str_num(tp) {
            Some(n) => self.opt_str_data[n].clone(),
            None => Vec::new(),
        }
    }
    pub fn is_set_str_opt(&self, tp: OptType) -> bool {
        match Self::str_num(tp) {
            Some(n) => self.flag_str_set[n],
            None => false,
        }
    }
    pub fn clear_str_opt_update(&mut self, tp: OptType) {
        if let Some(n) = Self::str_num(tp) {
            self.flag_str_update[n] = false;
        }
    }
    pub fn is_update_str_opt(&self, tp: OptType) -> bool {
        match Self::str_num(tp) {
            Some(n) => self.flag_str_update[n],
            None => false,
        }
    }

    /// OptCat routing (JlsCmdSet.cpp:183-207). Returns 0=.. mapping per
    /// OptCat enum (None=0, PosSC=1, NumLG=2, FRAME=3, STR=4, NUM=5).
    pub fn get_opt_category(tp: OptType) -> Option<i32> {
        let n = tp as i32;
        if n > OptType::ScMIN as i32 && n < OptType::ScMAX as i32 {
            Some(1)
        } else if n > OptType::LgMIN as i32 && n < OptType::LgMAX as i32 {
            Some(2)
        } else if n > OptType::FrMIN as i32 && n < OptType::FrMAX as i32 {
            Some(3)
        } else if n > OptType::StrMIN as i32 && n < OptType::StrMAX as i32 {
            Some(4)
        } else if n > OptType::ArrayMIN as i32 && n < OptType::ArrayMAX as i32 {
            Some(5)
        } else {
            None
        }
    }

    pub fn add_sc_opt(&mut self, tp: OptType, relative: bool, min: i32, max: i32) {
        self.list_sc_opt.push(ScOpt {
            opt: tp,
            relative,
            min,
            max,
        });
    }
    pub fn sc_opt_type(&self, num: i32) -> OptType {
        if num >= 0 && (num as usize) < self.list_sc_opt.len() {
            self.list_sc_opt[num as usize].opt
        } else {
            OptType::ScNone
        }
    }
    pub fn is_sc_opt_relative(&self, num: i32) -> bool {
        if num >= 0 && (num as usize) < self.list_sc_opt.len() {
            self.list_sc_opt[num as usize].relative
        } else {
            false
        }
    }
    pub fn get_sc_opt_min(&self, num: i32) -> i32 {
        if num >= 0 && (num as usize) < self.list_sc_opt.len() {
            self.list_sc_opt[num as usize].min
        } else {
            -1
        }
    }
    pub fn get_sc_opt_max(&self, num: i32) -> i32 {
        if num >= 0 && (num as usize) < self.list_sc_opt.len() {
            self.list_sc_opt[num as usize].max
        } else {
            -1
        }
    }
    pub fn size_sc_opt(&self) -> i32 {
        self.list_sc_opt.len() as i32
    }

    pub fn add_lg_opt(&mut self, s: Vec<u8>) {
        self.list_lg_val.push(s);
    }
    pub fn get_lg_opt(&self, num: i32) -> Vec<u8> {
        if num >= 0 && (num as usize) < self.list_lg_val.len() {
            self.list_lg_val[num as usize].clone()
        } else {
            Vec::new()
        }
    }
    pub fn size_lg_opt(&self) -> i32 {
        self.list_lg_val.len() as i32
    }

    pub fn add_arg_string(&mut self, s: Vec<u8>) {
        self.list_str_arg.push(s);
    }
    /// n is 1-based (reference getStrArg(n) semantics)
    pub fn replace_arg_string(&mut self, n: i32, s: Vec<u8>) -> bool {
        let idx = n - 1;
        if idx >= 0 && (idx as usize) < self.list_str_arg.len() {
            self.list_str_arg[idx as usize] = s;
            true
        } else {
            false
        }
    }
    pub fn get_str_arg(&self, n: i32) -> Vec<u8> {
        let idx = n - 1;
        if idx >= 0 && (idx as usize) < self.list_str_arg.len() {
            self.list_str_arg[idx as usize].clone()
        } else {
            Vec::new()
        }
    }
    /// reference uses std::stoi (throws on non-numeric); unwrap mirrors the
    /// hard-fail, out-of-range returns 0.
    pub fn get_val_str_arg(&self, n: i32) -> i32 {
        let idx = n - 1;
        if idx >= 0 && (idx as usize) < self.list_str_arg.len() {
            let s = String::from_utf8_lossy(&self.list_str_arg[idx as usize]).into_owned();
            match parse_stoi(&s) {
                Some(v) => v,
                None => panic!("stoi threw (invalid argument)"),
            }
        } else {
            0
        }
    }
}

/// std::stoi leading-integer semantics (skips leading whitespace, accepts
/// +/-, parses digits; throws when no digits).
pub fn parse_stoi(s: &str) -> Option<i32> {
    let b = s.as_bytes();
    let mut i = 0usize;
    while i < b.len() && (b[i] == b' ' || b[i] == b'\t' || b[i] == b'\n') {
        i += 1;
    }
    let neg = i < b.len() && b[i] == b'-';
    if i < b.len() && (b[i] == b'-' || b[i] == b'+') {
        i += 1;
    }
    let mut v: i64 = 0;
    let start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        v = v * 10 + (b[i] - b'0') as i64;
        i += 1;
    }
    if i == start {
        return None;
    }
    let v = if neg { -v } else { v };
    Some(v.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
}
