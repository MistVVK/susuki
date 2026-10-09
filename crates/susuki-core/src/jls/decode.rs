//! Derived from join_logo_scp (https://github.com/tobitti0/join_logo_scp,
//! commit d1a24d758f8b3c04352b82ff6756fdf3d56729b3), Copyright (C) tobitti0,
//! GPL-2.0-or-later. This port is a GPL derivative work, redistributed here
//! under GPL-3.0-or-later (design §10.6).
//! Port of REF-JLS `JlsScriptDecode.cpp` decodeCmd chain (fixed commit
//! d1a24d7). Design 7.3: syntax table driven decode; evaluation of IF
//! conditions stays for the executor stage (numCheckCond marker as reference).

use super::cmdarg::{CmdArg, WideMsec};
use super::cnvstr::Cnv;
use super::tables::{
    CmdCat, CmdType, ConvStrType, OptType, CMD_ALIAS, CMD_CALC_DEFINE, CMD_DEFINE, CONFIG_DEFINE,
    OPT_DEFINE,
};

// CmdErrType (JlsNameSpace.hpp:586)
pub const ERR_NONE: i32 = 0;
pub const ERR_OPT: i32 = 1;
pub const ERR_RANGE: i32 = 2;
pub const ERR_SEB: i32 = 3;
pub const ERR_VAR: i32 = 4;
pub const ERR_TR: i32 = 5;
pub const ERR_CMD: i32 = 6;

const MSEC_DECODE_MARGIN: i32 = 1200;

fn str_ieq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.eq_ignore_ascii_case(y))
}

/// JlscrDecodeRangeRecord
#[derive(Debug, Clone, Default)]
pub struct RangeInfo {
    pub num_read: i32,
    pub needs: i32,
    pub num_from: i32,
    pub flag_m1: bool,
    pub flag_sort: bool,
    pub num_abbr: i32,
    pub wm: WideMsec,
}

/// JlscrDecodeKeepSc (-SC family pending options, cleared per command)
#[derive(Debug, Clone)]
pub struct KeepSc {
    pub opt: OptType,
    pub relative: bool,
    pub wm: WideMsec,
    pub abbr: i32,
}

pub struct Decoder;

impl Decoder {
    /// JlsScriptDecode::decodeCmd
    pub fn decode_cmd(cnv: &Cnv, cmdarg: &mut CmdArg, str_buf: &[u8], only_cmd: bool) -> i32 {
        let mut errval = ERR_NONE;
        cmdarg.clear();

        let mut str_cmd: Vec<u8> = Vec::new();
        let mut pos = cnv.get_str_item(&mut str_cmd, str_buf, 0);
        let csel = if pos >= 0 {
            Self::decode_cmd_id(&str_cmd)
        } else {
            0
        };
        if csel < 0 {
            return ERR_CMD;
        }

        let rec = &CMD_DEFINE[csel as usize];
        cmdarg.cmdsel = rec.cmdsel;
        cmdarg.category = rec.category;
        let (muststr, mustchar, mustrange, needopt) =
            (rec.muststr, rec.mustchar, rec.mustrange, rec.needopt);

        if only_cmd {
            return errval;
        }

        let mut keep_sc: Vec<KeepSc> = Vec::new();
        if muststr > 0 || mustchar > 0 || mustrange > 0 {
            pos = Self::decode_cmd_arg_must(
                cnv,
                cmdarg,
                &mut errval,
                str_buf,
                pos,
                muststr,
                mustchar,
                mustrange,
            );
        }
        if needopt > 0 && pos >= 0 {
            let _ = Self::decode_cmd_arg_opt(cnv, cmdarg, &mut errval, str_buf, pos, &mut keep_sc);
        }
        if muststr > 0 && !Self::calc_cmd_arg(cnv, cmdarg) {
            errval = ERR_OPT;
        }
        errval
    }

    /// decodeCmdId: empty/'#' leading -> Nop; names case-insensitive; alias fallback.
    fn decode_cmd_id(cstr: &[u8]) -> i32 {
        if cstr.is_empty() || cstr[0] == b'#' {
            return 0;
        }
        for (i, rec) in CMD_DEFINE.iter().enumerate() {
            if str_ieq(cstr, rec.cmdname.as_bytes()) {
                return i as i32;
            }
        }
        for (atype, aname) in CMD_ALIAS.iter() {
            if str_ieq(cstr, aname.as_bytes()) {
                for (i, rec) in CMD_DEFINE.iter().enumerate() {
                    if rec.cmdsel == *atype {
                        return i as i32;
                    }
                }
            }
        }
        -1
    }

    /// decodeCmdArgMust
    // 原典 decodeCmdArgMust の引数構成を維持している
    #[allow(clippy::too_many_arguments)]
    fn decode_cmd_arg_must(
        cnv: &Cnv,
        cmdarg: &mut CmdArg,
        errval: &mut i32,
        str_buf: &[u8],
        mut pos: i32,
        tps: i32,
        tpc: i32,
        tpw: i32,
    ) -> i32 {
        if tps > 0 && pos >= 0 {
            if tps == 9 {
                // reference: while(strBuf[pos]==' ') pos++ then substr(pos);
                // scanning stops at the implicit NUL, substr(len) is ""
                let mut p = pos.max(0) as usize;
                while p < str_buf.len() && str_buf[p] == b' ' {
                    p += 1;
                }
                cmdarg.add_arg_string(str_buf[p.min(str_buf.len())..].to_vec());
            } else {
                let size_arg = tps;
                for _ in 0..size_arg {
                    let mut a: Vec<u8> = Vec::new();
                    pos = cnv.get_str_item(&mut a, str_buf, pos);
                    if pos >= 0 {
                        cmdarg.add_arg_string(a);
                    }
                }
                if pos < 0 {
                    *errval = ERR_OPT;
                }
            }
        }
        if tpc > 0 && pos >= 0 {
            let mut tmp: Vec<u8> = Vec::new();
            let pos_bak = pos;
            pos = cnv.get_str_item(&mut tmp, str_buf, pos);
            if pos >= 0 {
                if tpc == 1 || tpc == 3 {
                    let c0 = *tmp.first().unwrap_or(&0);
                    match c0 {
                        b'S' | b's' => cmdarg.select_edge = 0,
                        b'E' | b'e' => cmdarg.select_edge = 1,
                        b'B' | b'b' => cmdarg.select_edge = 2,
                        _ => {
                            if tpc == 3 {
                                pos = pos_bak;
                            } else {
                                pos = -1;
                                *errval = ERR_SEB;
                            }
                        }
                    }
                } else if tpc == 2
                    && !Self::get_tr_sp_ec_id(&mut cmdarg.select_auto_sub, &tmp, false)
                {
                    pos = -1;
                    *errval = ERR_TR;
                }
            }
        }
        if tpw > 0 && pos >= 0 {
            if tpw == 1 || tpw == 3 {
                let mut info = RangeInfo::default();
                if tpw == 1 {
                    info.num_read = 1;
                } else {
                    info.num_read = 3;
                    info.flag_sort = true;
                }
                pos = Self::decode_range_msec(cnv, &mut info, str_buf, pos);
                cmdarg.wmsec = info.wm;
            }
            if pos < 0 {
                *errval = ERR_RANGE;
            }
        }
        pos
    }

    /// decodeCmdArgOpt
    fn decode_cmd_arg_opt(
        cnv: &Cnv,
        cmdarg: &mut CmdArg,
        errval: &mut i32,
        str_buf: &[u8],
        mut pos: i32,
        keep: &mut Vec<KeepSc>,
    ) -> i32 {
        keep.clear();
        let mut new_buf: Vec<u8> = Vec::new();
        cnv.get_str_without_comment(&mut new_buf, str_buf);
        while pos >= 0 {
            pos = Self::decode_cmd_arg_opt_one(cnv, cmdarg, errval, &new_buf, pos, keep);
        }
        Self::revise_cmd_range(cmdarg, keep);
        Self::set_cmd_tack_opt(cmdarg);
        Self::set_arg_sc_opt(cmdarg, keep);
        pos
    }

    fn decode_cmd_arg_opt_one(
        cnv: &Cnv,
        cmdarg: &mut CmdArg,
        errval: &mut i32,
        str_buf: &[u8],
        mut pos: i32,
        keep: &mut Vec<KeepSc>,
    ) -> i32 {
        let mut word: Vec<u8> = Vec::new();
        pos = cnv.get_str_item(&mut word, str_buf, pos);
        let mut optsel: i32 = -1;
        if pos >= 0 {
            for (i, rec) in OPT_DEFINE.iter().enumerate() {
                if str_ieq(&word, rec.optname.as_bytes()) {
                    optsel = i as i32;
                }
            }
            if optsel < 0 {
                pos = -1;
            }
            if pos >= 0 {
                pos = Self::decode_cmd_arg_opt_one_sub(
                    cnv,
                    cmdarg,
                    str_buf,
                    pos,
                    optsel as usize,
                    keep,
                );
            }
            if pos < 0 {
                *errval = ERR_OPT;
            }
        }
        pos
    }

    fn decode_cmd_arg_opt_one_sub(
        cnv: &Cnv,
        cmdarg: &mut CmdArg,
        str_buf: &[u8],
        mut pos: i32,
        optsel: usize,
        keep: &mut Vec<KeepSc>,
    ) -> i32 {
        let rec = &OPT_DEFINE[optsel];
        let opt_type = rec.opt_type;
        let opt_type_int = opt_type as i32;
        let category = match CmdArg::get_opt_category(opt_type) {
            Some(c) => c,
            None => {
                pos = -1;
                eprintln!(
                    "error:internal setting (OptDefine-category) {}",
                    rec.optname
                );
                return pos;
            }
        };
        let mut info = RangeInfo {
            num_read: rec.num_arg,
            num_from: rec.num_from,
            ..Default::default()
        };
        if rec.conv_type == ConvStrType::MsecM1 {
            info.flag_m1 = true;
        }
        if rec.sort == 12 && info.num_read == 2 || rec.sort == 23 && info.num_read == 3 {
            info.flag_sort = true;
        } else if rec.sort > 0 {
            eprintln!("error:internal setting (OptDefine-sort) {}", rec.optname);
        }
        match category {
            // PosSC
            1 => {
                pos = Self::decode_range_msec(cnv, &mut info, str_buf, pos);
                if pos >= 0 {
                    keep.push(KeepSc {
                        opt: opt_type,
                        relative: rec.sub_type != 0,
                        wm: info.wm,
                        abbr: info.num_abbr,
                    });
                }
            }
            // NumLG
            2 => {
                if !cmdarg.is_set_opt(OptType::TypeNumLogo) {
                    cmdarg.set_opt(OptType::TypeNumLogo, opt_type_int);
                    let mut sub: Vec<u8> = Vec::new();
                    pos = cnv.get_str_item(&mut sub, str_buf, pos);
                    if pos >= 0 {
                        let mut list = Vec::new();
                        if Self::get_list_str_num_from_str(cnv, &mut list, &sub) {
                            for it in list {
                                cmdarg.add_lg_opt(it);
                            }
                        } else {
                            pos = -1;
                        }
                    }
                }
            }
            // FRAME
            3 => {
                pos = Self::decode_range_msec(cnv, &mut info, str_buf, pos);
                if pos >= 0 {
                    cmdarg.set_opt(OptType::MsecFrameL, info.wm.early);
                    cmdarg.set_opt(OptType::MsecFrameR, info.wm.late);
                    cmdarg.set_opt(OptType::TypeFrame, opt_type_int);
                    cmdarg.set_opt(OptType::TypeFrameSub, rec.sub_type);
                }
            }
            // STR
            4 => {
                let mut sub: Vec<u8> = Vec::new();
                pos = cnv.get_str_item(&mut sub, str_buf, pos);
                if pos >= 0 {
                    if sub.first() != Some(&b'-') {
                        cmdarg.set_str_opt(opt_type, sub);
                    } else {
                        let (np, _v) = cnv.get_str_val_num(&sub, 0);
                        if np >= 0 {
                            cmdarg.set_str_opt(opt_type, sub);
                        } else {
                            pos = -1;
                        }
                    }
                }
            }
            // NUM
            5 => {
                let mut list_opt = [OptType::None; 4];
                let num_used = Self::get_option_type_list(&mut list_opt, opt_type, info.num_read);
                let mut list_val = [0i32; 4];
                match rec.conv_type {
                    ConvStrType::MsecM1 => {
                        pos = Self::decode_range_msec(cnv, &mut info, str_buf, pos);
                        if pos >= 0 {
                            if info.num_read == 3 {
                                list_val[0] = info.wm.just;
                                list_val[1] = info.wm.early;
                                list_val[2] = info.wm.late;
                            } else if info.num_read == 2 {
                                list_val[0] = info.wm.early;
                                list_val[1] = info.wm.late;
                            } else {
                                list_val[0] = info.wm.just;
                            }
                            if num_used > info.num_read {
                                list_val[info.num_read as usize] = info.num_abbr;
                            }
                        }
                    }
                    ConvStrType::Num => {
                        let (np, v) = cnv.get_str_val_num(str_buf, pos);
                        pos = np;
                        list_val[0] = v.unwrap_or(0);
                        if info.num_read > 1 {
                            eprintln!("error:internal setting (OptDefine-numArg) {}", rec.optname);
                        }
                    }
                    ConvStrType::Sec => {
                        let (np, v) = cnv.get_str_val_sec_from_sec(str_buf, pos);
                        pos = np;
                        list_val[0] = v.unwrap_or(0);
                        if info.num_read > 1 {
                            eprintln!("error:internal setting (OptDefine-numArg) {}", rec.optname);
                        }
                    }
                    ConvStrType::TrSpEc => {
                        let mut sub: Vec<u8> = Vec::new();
                        pos = cnv.get_str_item(&mut sub, str_buf, pos);
                        if pos >= 0 {
                            let mut id = 0i32;
                            if Self::get_tr_sp_ec_id(&mut id, &sub, true) {
                                list_val[0] = id;
                            } else {
                                pos = -1;
                            }
                        }
                        if info.num_read > 1 {
                            eprintln!("error:internal setting (OptDefine-numArg) {}", rec.optname);
                        }
                    }
                    _ => {
                        // flag
                        list_val[0] = 1;
                        if info.num_read > 0 {
                            eprintln!("error:internal setting (OptDefine-numArg) {}", rec.optname);
                        }
                    }
                }
                for i in 0..num_used {
                    cmdarg.set_opt(list_opt[i as usize], list_val[i as usize]);
                }
            }
            _ => {}
        }
        pos
    }

    /// getOptionTypeList (747-838)
    fn get_option_type_list(list: &mut [OptType; 4], org: OptType, num_arg: i32) -> i32 {
        let num_used;
        let pairs: &[(i32, &[OptType])] = &[
            (
                4,
                &[
                    OptType::MsecEndlenC,
                    OptType::MsecEndlenL,
                    OptType::MsecEndlenR,
                    OptType::AbbrEndlen,
                ],
            ),
            (
                4,
                &[
                    OptType::MsecSftC,
                    OptType::MsecSftL,
                    OptType::MsecSftR,
                    OptType::AbbrSft,
                ],
            ),
            (2, &[OptType::MsecTgtLimL, OptType::MsecTgtLimR]),
            (2, &[OptType::MsecLenPMin, OptType::MsecLenPMax]),
            (2, &[OptType::MsecLenNMin, OptType::MsecLenNMax]),
            (2, &[OptType::MsecLenPEMin, OptType::MsecLenPEMax]),
            (2, &[OptType::MsecLenNEMin, OptType::MsecLenNEMax]),
            (2, &[OptType::MsecFromHead, OptType::AbbrFromHead]),
            (2, &[OptType::MsecFromTail, OptType::AbbrFromTail]),
            (2, &[OptType::MsecLogoExtL, OptType::MsecLogoExtR]),
            (2, &[OptType::MsecDrangeL, OptType::MsecDrangeR]),
        ];
        let idx = match org {
            OptType::MsecEndlenC => Some(0),
            OptType::MsecSftC => Some(1),
            OptType::MsecTgtLimL => Some(2),
            OptType::MsecLenPMin => Some(3),
            OptType::MsecLenNMin => Some(4),
            OptType::MsecLenPEMin => Some(5),
            OptType::MsecLenNEMin => Some(6),
            OptType::MsecFromHead => Some(7),
            OptType::MsecFromTail => Some(8),
            OptType::MsecLogoExtL => Some(9),
            OptType::MsecDrangeL => Some(10),
            _ => None,
        };
        match idx {
            Some(i) => {
                let (n, arr) = pairs[i];
                num_used = n;
                for (k, v) in arr.iter().enumerate() {
                    list[k] = *v;
                }
            }
            None => {
                num_used = 1;
                list[0] = org;
            }
        }
        if num_used < num_arg {
            eprintln!("error:internal setting (numArg) type:{}", org as i32);
        }
        num_used
    }

    /// getTrSpEcID (561-587)
    pub fn get_tr_sp_ec_id(id: &mut i32, name: &[u8], flag_option: bool) -> bool {
        if str_ieq(name, b"TR") {
            *id = 2;
            true
        } else if str_ieq(name, b"SP") {
            *id = 3;
            true
        } else if str_ieq(name, b"EC") {
            *id = 4;
            true
        } else if flag_option && str_ieq(name, b"LG") {
            *id = 5;
            true
        } else if flag_option && str_ieq(name, b"Off") {
            *id = 1;
            true
        } else {
            *id = 0;
            false
        }
    }

    /// decodeRangeMsec (596-733)
    fn decode_range_msec(cnv: &Cnv, info: &mut RangeInfo, str_buf: &[u8], pos: i32) -> i32 {
        let mut wm = WideMsec::default();
        let mut pos1 = -1i32;
        let mut pos2 = -1i32;
        let mut pos3 = -1i32;
        match info.num_read {
            3 => {
                if info.flag_m1 {
                    let (p1, v1) = cnv.get_str_val_msec_m1(str_buf, pos);
                    let (p2, v2) = cnv.get_str_val_msec_m1(str_buf, p1);
                    let (p3, v3) = cnv.get_str_val_msec_m1(str_buf, p2);
                    if let Some(v) = v1 {
                        wm.just = v;
                    }
                    if let Some(v) = v2 {
                        wm.early = v;
                    }
                    if let Some(v) = v3 {
                        wm.late = v;
                    }
                    pos1 = p1;
                    pos2 = p2;
                    pos3 = p3;
                } else {
                    let (p1, v1) = cnv.get_str_val_msec(str_buf, pos);
                    let (p2, v2) = cnv.get_str_val_msec(str_buf, p1);
                    let (p3, v3) = cnv.get_str_val_msec(str_buf, p2);
                    if let Some(v) = v1 {
                        wm.just = v;
                    }
                    if let Some(v) = v2 {
                        wm.early = v;
                    }
                    if let Some(v) = v3 {
                        wm.late = v;
                    }
                    pos1 = p1;
                    pos2 = p2;
                    pos3 = p3;
                }
            }
            2 => {
                if info.flag_m1 {
                    let (p1, v1) = cnv.get_str_val_msec_m1(str_buf, pos);
                    let (p2, v2) = cnv.get_str_val_msec_m1(str_buf, p1);
                    if let Some(v) = v1 {
                        wm.early = v;
                    }
                    if let Some(v) = v2 {
                        wm.late = v;
                    }
                    pos1 = p1;
                    pos2 = p2;
                } else {
                    let (p1, v1) = cnv.get_str_val_msec(str_buf, pos);
                    let (p2, v2) = cnv.get_str_val_msec(str_buf, p1);
                    if let Some(v) = v1 {
                        wm.early = v;
                    }
                    if let Some(v) = v2 {
                        wm.late = v;
                    }
                    pos1 = p1;
                    pos2 = p2;
                }
            }
            1 => {
                if info.flag_m1 {
                    let (p1, v1) = cnv.get_str_val_msec_m1(str_buf, pos);
                    if let Some(v) = v1 {
                        wm.just = v;
                    }
                    pos1 = p1;
                } else {
                    let (p1, v1) = cnv.get_str_val_msec(str_buf, pos);
                    if let Some(v) = v1 {
                        wm.just = v;
                    }
                    pos1 = p1;
                }
            }
            _ => {}
        }
        if pos1 < 0 {
            wm = WideMsec::default();
        }
        match info.num_read {
            3 => {
                if pos3 < 0 {
                    Self::set_range_margin(&mut wm, -1);
                }
            }
            2 if pos2 < 0 => {
                if info.num_from != 0 {
                    if info.num_from == 2 {
                        wm.late = wm.early;
                        wm.early = 0;
                    }
                } else {
                    if pos1 >= 0 {
                        wm.just = wm.early;
                    }
                    Self::set_range_margin(&mut wm, -1);
                }
            }
            _ => {}
        }
        if info.flag_sort {
            match info.num_read {
                3 | 2 => {
                    if info.flag_m1 {
                        Self::sort_two_val_m1(&mut wm.early, &mut wm.late);
                    } else if wm.early > wm.late {
                        std::mem::swap(&mut wm.early, &mut wm.late);
                    }
                }
                _ => {}
            }
        }
        let mut pos = pos;
        let mut num_abbr = 0i32;
        match info.num_read {
            3 => {
                if pos3 >= 0 || info.needs >= 3 {
                    pos = pos3;
                    num_abbr = 0;
                } else if pos2 >= 0 {
                    pos = -1;
                    num_abbr = 1;
                } else if pos1 >= 0 || info.needs >= 1 {
                    pos = pos1;
                    num_abbr = 2;
                } else {
                    num_abbr = 3;
                }
            }
            2 => {
                if pos2 >= 0 || info.needs >= 2 {
                    pos = pos2;
                    num_abbr = 0;
                } else if pos1 >= 0 || info.needs >= 1 {
                    pos = pos1;
                    num_abbr = 1;
                } else {
                    num_abbr = 2;
                }
            }
            1 => {
                if pos1 >= 0 || info.needs >= 1 {
                    pos = pos1;
                    num_abbr = 0;
                } else {
                    num_abbr = 1;
                }
            }
            _ => {}
        }
        info.num_abbr = num_abbr;
        info.wm = wm;
        pos
    }

    fn set_range_margin(wm: &mut WideMsec, margin: i32) {
        if margin >= 0 {
            wm.early = wm.just - margin;
            wm.late = wm.just + margin;
        } else {
            wm.early = wm.just - MSEC_DECODE_MARGIN;
            wm.late = wm.just + MSEC_DECODE_MARGIN;
        }
    }

    /// getListStrNumFromStr (769-818): comma separated numbers, "a..b" ranges.
    fn get_list_str_num_from_str(cnv: &Cnv, out: &mut Vec<Vec<u8>>, str_buf: &[u8]) -> bool {
        let mut success = true;
        let mut pos = 0i32;
        while pos >= 0 {
            let mut tmp: Vec<u8> = Vec::new();
            pos = cnv.get_str_word(&mut tmp, str_buf, pos);
            if pos >= 0 {
                if let Some(loc) = find_sub(&tmp, b"..") {
                    let st_part = &tmp[..loc];
                    let ed_part = &tmp[loc + 2..];
                    let (ps, vs) = cnv.get_str_val_num(st_part, 0);
                    let (pe, ve) = cnv.get_str_val_num(ed_part, 0);
                    if ps >= 0 && pe >= 0 {
                        let mut v = vs.unwrap_or(0).to_string().into_bytes();
                        v.extend(b"..");
                        v.extend(ve.unwrap_or(0).to_string().into_bytes());
                        out.push(v);
                    } else {
                        success = false;
                    }
                } else {
                    let (p1, v1) = cnv.get_str_val_num(&tmp, 0);
                    if p1 >= 0 {
                        out.push(v1.unwrap_or(0).to_string().into_bytes());
                    } else {
                        success = false;
                    }
                }
            }
        }
        if success && out.is_empty() {
            success = false;
        }
        success
    }

    fn sort_two_val_m1(a: &mut i32, b: &mut i32) {
        if *a != -1 && *b != -1 && *a > *b {
            std::mem::swap(a, b);
        }
    }

    /// reviseCmdRange (824-884)
    fn revise_cmd_range(cmdarg: &mut CmdArg, keep: &mut [KeepSc]) {
        if cmdarg.is_set_opt(OptType::MsecDcenter) {
            cmdarg.wmsec.just = cmdarg.get_opt(OptType::MsecDcenter);
            if cmdarg.wmsec.just < cmdarg.wmsec.early || cmdarg.wmsec.just > cmdarg.wmsec.late {
                Self::set_range_margin(&mut cmdarg.wmsec, -1);
            }
        }
        if cmdarg.is_set_opt(OptType::MsecDmargin) {
            let m = cmdarg.get_opt(OptType::MsecDmargin).abs();
            Self::set_range_margin(&mut cmdarg.wmsec, m);
        }
        if cmdarg.is_set_opt(OptType::MsecDrangeL) {
            cmdarg.wmsec.early = cmdarg.get_opt(OptType::MsecDrangeL);
        }
        if cmdarg.is_set_opt(OptType::MsecDrangeR) {
            cmdarg.wmsec.late = cmdarg.get_opt(OptType::MsecDrangeR);
        }
        if cmdarg.is_set_opt(OptType::MsecEmargin) {
            let m = cmdarg.get_opt(OptType::MsecEmargin).abs();
            if cmdarg.get_opt(OptType::AbbrEndlen) >= 2 {
                let c = cmdarg.get_opt(OptType::MsecEndlenC);
                cmdarg.set_opt(OptType::MsecEndlenL, c - m);
                cmdarg.set_opt(OptType::MsecEndlenR, c + m);
            }
            if cmdarg.get_opt(OptType::AbbrSft) >= 2 {
                let c = cmdarg.get_opt(OptType::MsecSftC);
                cmdarg.set_opt(OptType::MsecSftL, c - m);
                cmdarg.set_opt(OptType::MsecSftR, c + m);
            }
            for ks in keep.iter_mut() {
                if ks.abbr >= 1 {
                    let mut wm = ks.wm;
                    Self::set_range_margin(&mut wm, m);
                    ks.wm = wm;
                }
            }
        }
    }

    /// setCmdTackOpt (889-1010)
    fn set_cmd_tack_opt(cmdarg: &mut CmdArg) {
        let cmdsel = cmdarg.cmdsel;
        let mut category = cmdarg.category;
        {
            let mut fb = false;
            if cmdsel == CmdType::Select || cmdsel == CmdType::NextTail {
                fb = true;
            }
            if cmdarg.is_set_opt(OptType::MsecSftC) || cmdarg.get_opt(OptType::FlagRelative) > 0 {
                fb = true;
            }
            cmdarg.tack.float_base = fb;
        }
        {
            let mut vt = false;
            if category == CmdCat::AUTO || category == CmdCat::AUTOEACH {
                vt = true;
            }
            let tnl = numlogo(cmdarg);
            if category == CmdCat::AUTOLOGO
                && tnl != OptType::LgNlogo as i32
                && tnl != OptType::LgNFlogo as i32
                && tnl != OptType::LgNFXlogo as i32
            {
                vt = true;
            }
            if (tnl == OptType::LgNauto as i32 || tnl == OptType::LgNFauto as i32)
                && (cmdsel == CmdType::GetPos || cmdsel == CmdType::GetList)
            {
                vt = true;
            }
            if cmdarg.get_opt(OptType::FlagFinal) > 0 {
                vt = true;
            }
            cmdarg.tack.virtual_logo = vt;
        }
        {
            let mut ia = false;
            if numlogo(cmdarg) == OptType::LgNFXlogo as i32
                && (cmdsel == CmdType::GetPos || cmdsel == CmdType::GetList)
            {
                ia = true;
            }
            cmdarg.tack.ignore_abort = ia;
        }
        cmdarg.tack.ignore_comp = matches!(
            cmdsel,
            CmdType::MkLogo
                | CmdType::DivLogo
                | CmdType::DivFile
                | CmdType::GetPos
                | CmdType::GetList
        );
        {
            let mut lim = cmdsel == CmdType::DivLogo;
            if cmdarg.get_opt(OptType::FlagNoLap) > 0 {
                lim = true;
            }
            cmdarg.tack.limit_by_logo = lim;
        }
        {
            let mut one = cmdarg.is_set_opt(OptType::MsecFromAbs)
                || cmdarg.is_set_opt(OptType::MsecFromHead)
                || cmdarg.is_set_opt(OptType::MsecFromTail);
            if cmdsel == CmdType::GetPos {
                one = true;
            }
            cmdarg.tack.one_point = one;
        }
        {
            let mut need = false;
            for i in 0..cmdarg.size_sc_opt() {
                let t = cmdarg.sc_opt_type(i);
                if t == OptType::ScAC || t == OptType::ScNoAC {
                    need = true;
                }
            }
            cmdarg.tack.need_auto = need;
        }
        cmdarg.tack.full_frame = cmdsel == CmdType::GetPos || cmdsel == CmdType::GetList;
        {
            let mut lazy = 0i32; // LazyType::None
            if cmdarg.get_opt(OptType::FlagLazyS) > 0 {
                lazy = 2; // LazyS
            }
            if cmdarg.get_opt(OptType::FlagLazyA) > 0 {
                lazy = 3; // LazyA
            }
            if cmdarg.get_opt(OptType::FlagLazyE) > 0 {
                lazy = 4; // LazyE
            }
            cmdarg.tack.type_lazy = lazy;
        }
        if (cmdsel == CmdType::AutoCut || cmdsel == CmdType::AutoAdd)
            && cmdarg.get_opt(OptType::FlagAutoEach) > 0
        {
            category = CmdCat::AUTOEACH;
            cmdarg.category = category;
        }
    }

    /// setArgScOpt (1014-1024)
    fn set_arg_sc_opt(cmdarg: &mut CmdArg, keep: &[KeepSc]) {
        for ks in keep {
            cmdarg.add_sc_opt(ks.opt, ks.relative, ks.wm.early, ks.wm.late);
        }
    }

    /// calcCmdArg (1027-1062)
    fn calc_cmd_arg(cnv: &Cnv, cmdarg: &mut CmdArg) -> bool {
        let mut success = true;
        for (cmdsel, nlist, type_val) in CMD_CALC_DEFINE.iter() {
            if cmdarg.cmdsel == *cmdsel {
                let n_list = *nlist;
                match type_val {
                    ConvStrType::CondIF => {
                        cmdarg.num_check_cond = n_list;
                    }
                    ConvStrType::Param => {
                        if n_list >= 2 {
                            let mut name = cmdarg.get_str_arg(n_list - 1);
                            let mut val = cmdarg.get_str_arg(n_list);
                            success = Self::convert_string_reg_param(cnv, &mut name, &mut val);
                            if success {
                                success = cmdarg.replace_arg_string(n_list - 1, name.clone());
                            }
                            if success {
                                success = cmdarg.replace_arg_string(n_list, val.clone());
                            }
                        }
                    }
                    _ => {
                        let mut val = cmdarg.get_str_arg(n_list);
                        success = Self::convert_string_value(cnv, &mut val, *type_val);
                        if success {
                            success = cmdarg.replace_arg_string(n_list, val.clone());
                        }
                    }
                }
            }
            if !success {
                break;
            }
        }
        success
    }

    /// convertStringRegParam (1066-1090)
    fn convert_string_reg_param(cnv: &Cnv, name: &mut Vec<u8>, val: &mut Vec<u8>) -> bool {
        let mut csel = -1i32;
        for (i, (prm, _, nm)) in CONFIG_DEFINE.iter().enumerate() {
            if str_ieq(name, nm.as_bytes()) {
                csel = i as i32;
                let _ = prm;
                break;
            }
        }
        if csel >= 0 {
            let (type_param, type_val, _) = CONFIG_DEFINE[csel as usize];
            *name = (type_param as i32).to_string().into_bytes();
            return Self::convert_string_value(cnv, val, type_val);
        }
        false
    }

    /// convertStringValue (1095-1136)
    pub fn convert_string_value(cnv: &Cnv, sval: &mut Vec<u8>, type_val: ConvStrType) -> bool {
        let mut pos = 0i32;
        match type_val {
            ConvStrType::Msec => {
                let (p, v) = cnv.get_str_val_msec(sval, 0);
                pos = p;
                if p >= 0 {
                    *sval = v.unwrap_or(0).to_string().into_bytes();
                }
            }
            ConvStrType::MsecM1 => {
                let (p, v) = cnv.get_str_val_msec_m1(sval, 0);
                pos = p;
                if p >= 0 {
                    *sval = v.unwrap_or(0).to_string().into_bytes();
                }
            }
            ConvStrType::Sec => {
                let (p, v) = cnv.get_str_val_sec(sval, 0);
                pos = p;
                if p >= 0 {
                    *sval = v.unwrap_or(0).to_string().into_bytes();
                }
            }
            ConvStrType::Num => {
                let (p, v) = cnv.get_str_val_num(sval, 0);
                pos = p;
                if p >= 0 {
                    *sval = v.unwrap_or(0).to_string().into_bytes();
                }
            }
            ConvStrType::Frame => {
                let (p, v) = cnv.get_str_val_msec_m1(sval, 0);
                pos = p;
                if p >= 0 {
                    *sval = cnv.get_string_frame_msec_m1(v.unwrap_or(0));
                }
            }
            ConvStrType::Time => {
                let (p, v) = cnv.get_str_val_msec_m1(sval, 0);
                pos = p;
                if p >= 0 {
                    *sval = cnv.get_string_time_msec_m1(v.unwrap_or(0));
                }
            }
            _ => {}
        }
        pos >= 0
    }
}

fn numlogo(c: &CmdArg) -> i32 {
    c.get_opt(OptType::TypeNumLogo)
}

pub fn find_sub(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.len() > hay.len() {
        return None;
    }
    (0..=hay.len() - needle.len()).find(|&i| &hay[i..i + needle.len()] == needle)
}
