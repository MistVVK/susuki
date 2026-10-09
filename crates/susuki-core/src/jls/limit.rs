//! Derived from join_logo_scp (https://github.com/tobitti0/join_logo_scp,
//! commit d1a24d758f8b3c04352b82ff6756fdf3d56729b3), Copyright (C) tobitti0,
//! GPL-2.0-or-later. This port is a GPL derivative work, redistributed here
//! under GPL-3.0-or-later (design §10.6).
//! Port of JlsScriptLimit.cpp: builds the per-command target limitation
//! (cmdset.limit) from decoded options and dataset state.
//! Free functions mirror the reference methods; pdata becomes explicit args.

use super::cmdarg::{CmdArg, WideMsec};
use super::cmdlimit::CmdLimit;
use super::cnvstr::Cnv;
use super::dataset::*;
use super::dataset_search::*;
use super::tables::{CmdType, OptType};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ScrLogoSetBase {
    None,
    BaseLoc,
    ValidList,
}

#[derive(Clone, Copy)]
pub struct ScrLogoInfoCmdRecord {
    pub type_logo: i32,
    pub type_set_base: ScrLogoSetBase,
    pub edge_sel: i32,
    pub msec_sel: i32,
}

fn is_logo_edge_rise(edge: i32) -> bool {
    edge == EDGE_RISE || edge == EDGE_BOTH
}
fn is_logo_edge_fall(edge: i32) -> bool {
    edge == EDGE_FALL || edge == EDGE_BOTH
}

/// limitCommonRange
pub fn limit_common_range(ds: &Dataset, cnv: &Cnv, cmdarg: &mut CmdArg, lim: &mut CmdLimit) {
    limit_head_tail(ds, cmdarg, lim);
    limit_window(ds, cmdarg, lim);
    limit_list_for_target(ds, cnv, cmdarg, lim);
}

/// resizeRangeHeadTail
pub fn resize_range_head_tail(
    ds: &Dataset,
    cnv: &Cnv,
    cmdarg: &mut CmdArg,
    lim: &mut CmdLimit,
    st: i32,
    ed: i32,
) {
    lim.set_head_tail(st, ed);
    limit_window_with_cnv(ds, cnv, cmdarg, lim);
}

/// limitHeadTail
fn limit_head_tail(ds: &Dataset, _cmdarg: &mut CmdArg, lim: &mut CmdLimit) {
    let mut st = ds.rec_hold.rmsec_head;
    if st == -1 {
        st = 0;
    }
    let mut ed = ds.rec_hold.rmsec_tail;
    if ed == -1 {
        ed = ds.msec_total_max;
    }
    lim.set_head_tail(st, ed);
}

/// limitWindow
pub fn limit_window(ds: &Dataset, cmdarg: &mut CmdArg, lim: &mut CmdLimit) {
    // reference constructs no Cnv; keep signature, delegate with own cnv-less path
    limit_window_impl(ds, cmdarg, lim)
}

/// limitWindow with cnv (unused by this function but kept for symmetry)
pub fn limit_window_with_cnv(ds: &Dataset, _cnv: &Cnv, cmdarg: &mut CmdArg, lim: &mut CmdLimit) {
    limit_window_impl(ds, cmdarg, lim)
}

fn limit_window_impl(ds: &Dataset, cmdarg: &mut CmdArg, lim: &mut CmdLimit) {
    let msec_opt_left = cmdarg.get_opt(OptType::MsecFrameL);
    let msec_opt_right = cmdarg.get_opt(OptType::MsecFrameR);
    let mut msec_limit_left = msec_opt_left;
    let mut msec_limit_right = msec_opt_right;
    let type_frame = cmdarg.get_opt(OptType::TypeFrame);
    if type_frame == OptType::FrFR as i32 {
        let nrf_1st_rise = ds.get_nrf_next_logo(-1, EDGE_RISE, LOGO_SELECT_VALID);
        if nrf_1st_rise >= 0 {
            let msec_tmp = ds.msec_logo_nrf(nrf_1st_rise);
            if msec_limit_left != -1 {
                msec_limit_left += msec_tmp;
            }
            if msec_limit_right != -1 {
                msec_limit_right += msec_tmp;
            }
        }
    }
    if !cmdarg.is_set_opt(OptType::TypeFrame) && !cmdarg.tack.full_frame {
        let (h, t) = (ds.rec_hold.rmsec_head, ds.rec_hold.rmsec_tail);
        if h >= 0 {
            msec_limit_left = h;
        }
        if t >= 0 {
            msec_limit_right = t;
        }
    }
    let flag_midext = (cmdarg.get_opt(OptType::TypeFrameSub) & 0x1) != 0;
    if type_frame == OptType::FrFhead as i32
        || type_frame == OptType::FrFtail as i32
        || type_frame == OptType::FrFmid as i32
    {
        let msec_head = lim.get_head();
        let msec_tail = lim.get_tail();
        let nrf_1st_rise = ds.get_nrf_next_logo(-1, EDGE_RISE, LOGO_SELECT_VALID);
        let nrf_end_fall = ds.get_nrf_prev_logo(ds.size_logo() * 2, EDGE_FALL, LOGO_SELECT_VALID);
        let mut msec_window_start = 0i32;
        let mut msec_window_midst = 0i32;
        if nrf_1st_rise >= 0 {
            msec_window_midst = ds.msec_logo_nrf(nrf_1st_rise);
        }
        if msec_window_midst < msec_head {
            msec_window_midst = msec_head;
        }
        if msec_window_start < msec_head {
            msec_window_start = msec_head;
        }
        let mut msec_window_mided = ds.msec_total_max;
        let mut msec_window_end = ds.msec_total_max;
        if nrf_end_fall >= 0 {
            msec_window_mided = ds.msec_logo_nrf(nrf_end_fall);
        }
        if msec_window_mided > msec_tail {
            msec_window_mided = msec_tail;
        }
        if msec_window_end > msec_tail {
            msec_window_end = msec_tail;
        }
        let msec_window_md = (msec_window_midst + msec_window_mided) / 2;
        if type_frame == OptType::FrFhead as i32 {
            msec_limit_left = msec_window_start + msec_opt_left;
            msec_limit_right = msec_window_start + msec_opt_right;
            if msec_opt_left == -1 {
                msec_limit_left = msec_window_start;
            }
            if msec_opt_right == -1 {
                msec_limit_right = msec_window_end;
            }
            if !flag_midext {
                msec_limit_right = msec_limit_right.min(msec_window_md);
            }
        } else if type_frame == OptType::FrFtail as i32 {
            msec_limit_left = msec_window_end - msec_opt_right;
            msec_limit_right = msec_window_end - msec_opt_left;
            if msec_opt_right == -1 {
                msec_limit_left = msec_window_start;
            }
            if msec_opt_left == -1 {
                msec_limit_right = msec_window_end;
            }
            if !flag_midext {
                msec_limit_left = msec_limit_left.max(msec_window_md);
            }
        } else if type_frame == OptType::FrFmid as i32 {
            msec_limit_left = msec_window_start + msec_opt_left;
            msec_limit_right = msec_window_end - msec_opt_right;
            if msec_opt_left == -1 {
                msec_limit_left = msec_window_start;
            }
            if msec_opt_right == -1 {
                msec_limit_right = msec_window_end;
            }
            if !flag_midext {
                msec_limit_left = msec_limit_left.min(msec_window_md);
                msec_limit_right = msec_limit_right.max(msec_window_md);
            }
        }
    }
    lim.set_frame_range(msec_limit_left, msec_limit_right);
}

/// limitListForTarget
pub fn limit_list_for_target(_ds: &Dataset, cnv: &Cnv, cmdarg: &mut CmdArg, lim: &mut CmdLimit) {
    if !cmdarg.is_set_opt(OptType::MsecTgtLimL) {
        return;
    }
    let str_list = cmdarg.get_str_opt(OptType::StrValListR);
    if !str_list.is_empty() {
        lim.clear_target_list();
        let mut pos = 0i32;
        while pos >= 0 {
            let mut dstr = Vec::new();
            pos = cnv.get_str_word(&mut dstr, &str_list, pos);
            if pos > 0 {
                let (p, v) = cnv.get_str_val_msec_m1(&dstr, 0);
                if p > 0 {
                    if let Some(val) = v {
                        if val != -1 {
                            let arg_l = cmdarg.get_opt(OptType::MsecTgtLimL);
                            let arg_r = cmdarg.get_opt(OptType::MsecTgtLimR);
                            let mut st = arg_l;
                            let mut ed = arg_r;
                            if arg_l != -1 {
                                st += val;
                            }
                            if arg_r != -1 {
                                ed += val;
                            }
                            if arg_l != -1 || arg_r != -1 {
                                lim.add_target_list(st, ed);
                            }
                        }
                    }
                }
                let mut up = pos as usize;
                while up < str_list.len() && str_list[up] == b',' {
                    up += 1;
                }
                pos = up as i32;
            }
        }
    }
}

/// selectTargetByLogo
pub fn select_target_by_logo(
    ds: &Dataset,
    cnv: &Cnv,
    cmdarg: &mut CmdArg,
    lim: &mut CmdLimit,
    nlist: i32,
) -> bool {
    let mut exeflag = limit_target_logo(ds, cmdarg, lim, nlist);
    if exeflag {
        exeflag = limit_target_range_by_logo(ds, cmdarg, lim);
    }
    if exeflag {
        get_target_point(ds, cnv, cmdarg, lim);
    }
    exeflag
}

/// selectTargetByRange
pub fn select_target_by_range(
    ds: &Dataset,
    cnv: &Cnv,
    cmdarg: &mut CmdArg,
    lim: &mut CmdLimit,
    wmsec: WideMsec,
    force: bool,
) {
    limit_target_range_by_imm(ds, cmdarg, lim, wmsec, force);
    get_target_point(ds, cnv, cmdarg, lim);
}

/// limitTargetLogo
fn limit_target_logo(ds: &Dataset, cmdarg: &mut CmdArg, lim: &mut CmdLimit, nlist: i32) -> bool {
    let mut exeflag = limit_target_logo_get(ds, cmdarg, lim, nlist);
    if exeflag {
        exeflag = limit_target_logo_check(ds, cmdarg, lim);
    }
    exeflag
}

/// limitTargetLogoGet
fn limit_target_logo_get(
    ds: &Dataset,
    cmdarg: &mut CmdArg,
    lim: &mut CmdLimit,
    nlist: i32,
) -> bool {
    let msec_target = lim.get_logo_list_msec(nlist);
    let edge_sel = lim.get_logo_list_edge(nlist);
    let type_logo_out = if cmdarg.tack.ignore_abort {
        LOGO_SELECT_ALL
    } else {
        LOGO_SELECT_VALID
    };
    if msec_target < 0 {
        return false;
    }
    let mut info_cmd = ScrLogoInfoCmdRecord {
        type_logo: LOGO_SELECT_ALL,
        type_set_base: ScrLogoSetBase::BaseLoc,
        edge_sel,
        msec_sel: msec_target,
    };
    let mut list_tmp: Vec<WideMsec> = Vec::new();
    let det = get_logo_info_list(ds, cmdarg, lim, &mut list_tmp, info_cmd);
    if det {
        info_cmd.type_logo = type_logo_out;
        info_cmd.type_set_base = ScrLogoSetBase::ValidList;
        get_logo_info_list(ds, cmdarg, lim, &mut list_tmp, info_cmd);
    }
    det
}

/// limitTargetLogoCheck
fn limit_target_logo_check(ds: &Dataset, cmdarg: &mut CmdArg, lim: &mut CmdLimit) -> bool {
    let nrf_base = lim.get_logo_base_nrf();
    let nsc_base = lim.get_logo_base_nsc();
    let edge_base = lim.get_logo_base_edge();
    let mut exeflag = false;
    if nrf_base >= 0 {
        exeflag = true;
        let (_msec_tmp, outtype_rf) = ds.result_logo_at_nrf(nrf_base).unwrap_or((0, 0));
        let igncomp = cmdarg.tack.ignore_comp;
        let ignabort = cmdarg.tack.ignore_abort;
        if outtype_rf == LOGO_RESULT_NONE
            || (outtype_rf == LOGO_RESULT_DECIDE && igncomp)
            || outtype_rf == LOGO_RESULT_ABORT && ignabort
        {
        } else {
            exeflag = false;
        }
        if cmdarg.cmdsel == CmdType::Select
            && cmdarg.get_opt(OptType::FlagReset) == 0
            && ds.prior_logo(nrf_base) >= LOGO_PRIOR_DECIDE
        {
            exeflag = false;
        }
    } else if nsc_base >= 0 {
        exeflag = true;
    }
    if exeflag {
        if cmdarg.is_set_opt(OptType::TypeFrameSub)
            && (cmdarg.get_opt(OptType::TypeFrameSub) & 0x2) == 0
        {
            let wmsec_lg = lim.get_logo_wmsec_base(0, false);
            let (frame_st, frame_ed) = lim.get_frame_range();
            if (frame_st > wmsec_lg.just && frame_st >= 0)
                || (frame_ed < wmsec_lg.just && frame_ed >= 0)
            {
                exeflag = false;
            }
        }
        if exeflag {
            let wmsec_lg = lim.get_logo_wmsec_base(1, false);
            let len_p = (
                cmdarg.get_opt(OptType::MsecLenPMin),
                cmdarg.get_opt(OptType::MsecLenPMax),
            );
            let len_n = (
                cmdarg.get_opt(OptType::MsecLenNMin),
                cmdarg.get_opt(OptType::MsecLenNMax),
            );
            exeflag = limit_target_logo_check_length(ds, wmsec_lg, len_p, len_n);
        }
        if exeflag {
            let wmsec_lg_e = lim.get_logo_wmsec_base(2, false);
            let len_pe = (
                cmdarg.get_opt(OptType::MsecLenPEMin),
                cmdarg.get_opt(OptType::MsecLenPEMax),
            );
            let len_ne = (
                cmdarg.get_opt(OptType::MsecLenNEMin),
                cmdarg.get_opt(OptType::MsecLenNEMax),
            );
            exeflag = limit_target_logo_check_length(ds, wmsec_lg_e, len_pe, len_ne);
        }
    }
    if !cmdarg.tack.float_base && exeflag {
        let wmsec_lg = lim.get_logo_wmsec_base(0, false);
        exeflag = check_opt_scp_from_msec(ds, cmdarg, wmsec_lg.just, edge_base, true, false);
    }
    exeflag
}

/// limitTargetLogoCheckLength
fn limit_target_logo_check_length(
    ds: &Dataset,
    wmsec_lg: WideMsec,
    len_p: (i32, i32),
    len_n: (i32, i32),
) -> bool {
    let mut msec_dif_prev = wmsec_lg.just - wmsec_lg.early;
    let mut msec_dif_next = wmsec_lg.late - wmsec_lg.just;
    if wmsec_lg.early < 0 && wmsec_lg.just >= 0 {
        msec_dif_prev = wmsec_lg.just;
    }
    if wmsec_lg.late < 0 && wmsec_lg.just >= 0 {
        msec_dif_next = ds.msec_total_max - wmsec_lg.just;
    }
    let mut exeflag = true;
    if len_p.0 >= 0 && (msec_dif_prev < len_p.0 || msec_dif_prev < 0) {
        exeflag = false;
    }
    if len_p.1 >= 0 && (msec_dif_prev > len_p.1 || msec_dif_prev < 0) {
        exeflag = false;
    }
    if len_n.0 >= 0 && (msec_dif_next < len_n.0 || msec_dif_next < 0) {
        exeflag = false;
    }
    if len_n.1 >= 0 && (msec_dif_next > len_n.1 || msec_dif_next < 0) {
        exeflag = false;
    }
    exeflag
}

/// limitTargetRangeByLogo
fn limit_target_range_by_logo(ds: &Dataset, cmdarg: &mut CmdArg, lim: &mut CmdLimit) -> bool {
    let mut exeflag = true;
    // base logo position range
    let wmsec_lg_org = {
        let nrf_base = lim.get_logo_base_nrf();
        if nrf_base >= 0 {
            let mut num_sft = 0;
            if cmdarg.get_opt(OptType::FlagFromLast) > 0 {
                num_sft = -1;
            }
            let flag_wide = cmdarg.get_opt(OptType::FlagWide) > 0;
            let mut w = lim.get_logo_wmsec_base_shift(0, flag_wide, num_sft);
            if w.just < 0 {
                w = WideMsec {
                    just: 0,
                    early: 0,
                    late: 0,
                }; // C++ `= {}`
            }
            w
        } else {
            lim.get_logo_wmsec_base(0, true)
        }
    };
    let wmsec_base = {
        if cmdarg.is_set_opt(OptType::MsecFromAbs) {
            let t = cmdarg.get_opt(OptType::MsecFromAbs);
            if t < 0 {
                exeflag = false;
            }
            WideMsec {
                just: t,
                early: t,
                late: t,
            }
        } else if cmdarg.is_set_opt(OptType::MsecFromHead) {
            let opt = cmdarg.get_opt(OptType::MsecFromHead);
            let t = lim.get_head() + opt;
            if opt < 0 {
                exeflag = false;
            }
            WideMsec {
                just: t,
                early: t,
                late: t,
            }
        } else if cmdarg.is_set_opt(OptType::MsecFromTail) {
            let opt = cmdarg.get_opt(OptType::MsecFromTail);
            let t = lim.get_tail() - opt;
            if opt < 0 {
                exeflag = false;
            }
            WideMsec {
                just: t,
                early: t,
                late: t,
            }
        } else {
            wmsec_lg_org
        }
    };
    let mut wmsec_find = {
        let mut w = WideMsec {
            just: wmsec_base.just + cmdarg.wmsec.just,
            early: wmsec_base.early + cmdarg.wmsec.early,
            late: wmsec_base.late + cmdarg.wmsec.late,
        };
        if cmdarg.is_set_opt(OptType::MsecSftC) {
            w.just += cmdarg.get_opt(OptType::MsecSftC);
            w.early += cmdarg.get_opt(OptType::MsecSftL);
            w.late += cmdarg.get_opt(OptType::MsecSftR);
        }
        if cmdarg.wmsec.early == -1 {
            w.early = 0;
        }
        if cmdarg.wmsec.late == -1 {
            w.late = ds.msec_total_max;
        }
        if cmdarg.cmdsel == CmdType::Select {
            let ext_l = wmsec_lg_org.early + cmdarg.get_opt(OptType::MsecLogoExtL);
            let ext_r = wmsec_lg_org.late + cmdarg.get_opt(OptType::MsecLogoExtR);
            if w.early > ext_r || w.late < ext_l {
                exeflag = false;
            } else {
                if w.early < ext_l {
                    w.early = ext_l;
                }
                if w.just < ext_l {
                    w.just = ext_l;
                }
                if w.late > ext_r {
                    w.late = ext_r;
                }
                if w.just > ext_r {
                    w.just = ext_r;
                }
            }
        }
        if cmdarg.tack.limit_by_logo {
            let wmsec_lgpn = lim.get_logo_wmsec_base(1, true);
            if wmsec_lgpn.early >= 0 {
                if w.early < wmsec_lgpn.early {
                    w.early = wmsec_lgpn.early;
                }
                if w.just < wmsec_lgpn.early {
                    w.just = wmsec_lgpn.early;
                }
            }
            if wmsec_lgpn.late >= 0 {
                if w.late > wmsec_lgpn.late {
                    w.late = wmsec_lgpn.late;
                }
                if w.just > wmsec_lgpn.late {
                    w.just = wmsec_lgpn.late;
                }
            }
        }
        w
    };
    if exeflag {
        let (fs, fe) = lim.get_frame_range();
        exeflag = Dataset::limit_wmsec_from_range(&mut wmsec_find, fs, fe);
    }
    if wmsec_find.early > wmsec_find.late {
        exeflag = false;
    }
    let mut msec_force = -1i32;
    if cmdarg.get_opt(OptType::FlagForce) > 0 || cmdarg.get_opt(OptType::FlagNoForce) > 0 {
        msec_force = wmsec_find.just;
    }
    lim.set_target_range(wmsec_find, msec_force, true);
    exeflag
}

/// limitTargetRangeByImm
fn limit_target_range_by_imm(
    ds: &Dataset,
    _cmdarg: &mut CmdArg,
    lim: &mut CmdLimit,
    wmsec: WideMsec,
    force: bool,
) {
    let mut msec_force = -1i32;
    if force && wmsec.just >= 0 && wmsec.just <= ds.msec_total_max {
        msec_force = wmsec.just;
    }
    lim.set_target_range(wmsec, msec_force, false);
}

/// getTargetPoint
pub fn get_target_point(ds: &Dataset, cnv: &Cnv, cmdarg: &mut CmdArg, lim: &mut CmdLimit) {
    let wmsec_target = lim.get_target_range_wide();
    let mut edge_sel = lim.get_logo_base_edge();
    let mut nsc_scpos_tag = -1i32;
    let mut nsc_scpos_end = -1i32;
    let mut flag_noedge = cmdarg.get_opt(OptType::FlagNoEdge);
    get_target_point_set_scp_enable(ds, cmdarg, lim);
    let mut flag_nexttail = false;
    let mut flag_logorise = false;
    if cmdarg.cmdsel == CmdType::NextTail {
        flag_nexttail = true;
        flag_noedge = 0;
        edge_sel = EDGE_RISE;
        if cmdarg.select_edge == EDGE_RISE {
            flag_logorise = true;
        }
    }
    let size_scp = ds.size_scp();
    let jfrom = flag_noedge;
    let jsize = size_scp - flag_noedge;
    let mut val_difmin = 0i32;
    let mut stat_scpos = PRIOR_DUPE;
    let mut nrf_logo = 0i32;
    let mut msec_logo = 0i32;
    let mut flag_logo = false;
    let mut j = jfrom;
    while j < jsize {
        let msec_now = ds.msec_scp(j);
        let stat_now = ds.prior_scp(j);
        let msec_now_edge = ds.msec_scp_edge(j, edge_sel);
        let val_dif = (msec_now_edge - wmsec_target.just).abs();
        if (lim.get_scp_enable(j) && lim.is_target_listed(msec_now))
            || (j == size_scp - 1 && flag_nexttail)
        {
            let mut flag_near = false;
            if val_difmin > val_dif || nsc_scpos_tag < 0 {
                flag_near = true;
            }
            let mut flag_now_logo = false;
            if flag_logorise {
                while msec_logo + ds.msec_val_spc < msec_now && nrf_logo >= 0 {
                    nrf_logo = ds.get_nrf_next_logo(nrf_logo, EDGE_RISE, LOGO_SELECT_VALID);
                    if nrf_logo >= 0 {
                        msec_logo = ds.msec_logo_nrf(nrf_logo);
                    }
                }
                if (msec_logo - msec_now).abs() <= ds.msec_val_spc
                    && (!ds.is_auto_mode_use() || ds.is_scp_chap_type_decide_from_nsc(j))
                {
                    flag_now_logo = true;
                }
                if j == size_scp - 1 {
                    flag_now_logo = true;
                }
                if !flag_now_logo && flag_logo {
                    flag_near = false;
                }
                if flag_now_logo && !flag_logo {
                    flag_near = true;
                }
            }
            if flag_near && msec_now >= wmsec_target.early && msec_now <= wmsec_target.late {
                let mut chk_stat = false;
                if stat_now >= stat_scpos
                    || cmdarg.get_opt(OptType::FlagFlat) > 0
                    || cmdarg.cmdsel == CmdType::Select
                    || j == size_scp - 1
                {
                    chk_stat = true;
                }
                if chk_stat {
                    let chk_end =
                        get_target_point_end_result(ds, cnv, &mut nsc_scpos_end, cmdarg, msec_now);
                    if chk_end {
                        val_difmin = val_dif;
                        nsc_scpos_tag = j;
                        stat_scpos = stat_now;
                        flag_logo = flag_now_logo;
                    }
                }
            }
        }
        j += 1;
    }
    if nsc_scpos_tag < 0 {
        let msec_force = lim.get_target_range_force();
        if msec_force >= 0 {
            get_target_point_end_result(ds, cnv, &mut nsc_scpos_end, cmdarg, msec_force);
        }
    }
    get_target_point_out_edge(cmdarg, lim);
    lim.set_result_target(nsc_scpos_tag, nsc_scpos_end);
}

/// getTargetPointOutEdge
fn get_target_point_out_edge(cmdarg: &mut CmdArg, lim: &mut CmdLimit) {
    let mut out_edge = lim.get_logo_base_edge();
    if cmdarg.get_opt(OptType::FlagEdgeS) > 0 {
        out_edge = EDGE_RISE;
    }
    if cmdarg.get_opt(OptType::FlagEdgeE) > 0 {
        out_edge = EDGE_FALL;
    }
    lim.set_target_out_edge(out_edge);
}

/// getTargetPointEndResult
fn get_target_point_end_result(
    ds: &Dataset,
    cnv: &Cnv,
    nsc_scpos_end: &mut i32,
    cmdarg: &mut CmdArg,
    msec_base: i32,
) -> bool {
    let nend =
        if cmdarg.is_set_opt(OptType::MsecEndlenC) && cmdarg.get_opt(OptType::MsecEndlenC) != 0 {
            get_target_point_endlen(ds, cmdarg, msec_base)
        } else {
            get_target_point_endarg(ds, cnv, cmdarg, msec_base)
        };
    if nend >= 0 {
        *nsc_scpos_end = nend;
    }
    if nend == -1 {
        return false;
    }
    true
}

/// getTargetPointEndlen
fn get_target_point_endlen(ds: &Dataset, cmdarg: &mut CmdArg, msec_base: i32) -> i32 {
    let msec_endlen_c = msec_base + cmdarg.get_opt(OptType::MsecEndlenC);
    let msec_endlen_l = msec_base + cmdarg.get_opt(OptType::MsecEndlenL);
    let msec_endlen_r = msec_base + cmdarg.get_opt(OptType::MsecEndlenR);
    let size_scp = ds.size_scp();
    let jfrom = cmdarg.get_opt(OptType::FlagNoEdge);
    let jsize = size_scp - cmdarg.get_opt(OptType::FlagNoEdge);
    let mut nsc_scpos_end = -1i32;
    let mut val_difmin = 0i32;
    let mut stat_scpos = PRIOR_NONE;
    let mut j = jfrom;
    while j < jsize {
        let msec_now = ds.msec_scp(j);
        let stat_now = ds.prior_scp(j);
        let val_dif = (msec_now - msec_endlen_c).abs();
        if (val_difmin > val_dif || nsc_scpos_end < 0)
            && msec_now >= msec_endlen_l
            && msec_now <= msec_endlen_r
            && (stat_now >= stat_scpos || cmdarg.get_opt(OptType::FlagFlat) > 0)
        {
            val_difmin = val_dif;
            nsc_scpos_end = j;
            stat_scpos = stat_now;
        }
        j += 1;
    }
    nsc_scpos_end
}

/// getTargetPointEndArg
fn get_target_point_endarg(ds: &Dataset, cnv: &Cnv, cmdarg: &mut CmdArg, _msec_base: i32) -> i32 {
    let mut errnum = -2i32;
    let mut msec_target = 0i32;
    let msec_th = ds.msec_val_exact;
    if cmdarg.is_set_opt(OptType::MsecEndAbs) {
        msec_target = cmdarg.get_opt(OptType::MsecEndAbs);
        errnum = if msec_target >= 0 { 0 } else { -1 };
    } else if cmdarg.is_set_opt(OptType::FlagEndHead) {
        let val = ds.rec_hold.rmsec_head;
        errnum = 0;
        msec_target = if val != -1 { val } else { 0 };
    } else if cmdarg.is_set_opt(OptType::FlagEndTail) {
        let val = ds.rec_hold.rmsec_tail;
        errnum = 0;
        msec_target = if val != -1 { val } else { ds.msec_total_max };
    } else if cmdarg.is_set_opt(OptType::FlagEndHold) {
        let str_val = cmdarg.get_str_opt(OptType::StrValPosR);
        let (pos, v) = cnv.get_str_val_msec_m1(&str_val, 0);
        match v {
            Some(tv) if pos >= 0 && tv >= 0 => {
                msec_target = tv;
                errnum = 0;
            }
            _ => errnum = -1,
        }
    }
    if errnum == 0 {
        ds.nsc_from_msec_full(msec_target, msec_th, SCP_CHAP_NONE, SCP_END_EDGEIN)
    } else {
        errnum
    }
}

/// getTargetPointSetScpEnable
fn get_target_point_set_scp_enable(ds: &Dataset, cmdarg: &mut CmdArg, lim: &mut CmdLimit) {
    let size_scp = ds.size_scp();
    let size_enable = lim.size_scp_enable();
    if size_scp == size_enable {
        return;
    }
    let mut chk_base = false;
    let chk_rel = true;
    if cmdarg.tack.float_base {
        chk_base = true;
    }
    let mut list_enable: Vec<bool> = Vec::new();
    for m in 0..size_scp {
        let msec_base = ds.msec_scp(m);
        let result = check_opt_scp_from_msec(ds, cmdarg, msec_base, EDGE_RISE, chk_base, chk_rel);
        list_enable.push(result);
    }
    lim.set_scp_enable(list_enable);
}

/// getLogoInfoList
fn get_logo_info_list(
    ds: &Dataset,
    cmdarg: &mut CmdArg,
    lim: &mut CmdLimit,
    list_wmsec_logo: &mut Vec<WideMsec>,
    info_cmd: ScrLogoInfoCmdRecord,
) -> bool {
    let mut det = false;
    if !cmdarg.tack.virtual_logo {
        let mut list_nrf_logo: Vec<i32> = Vec::new();
        get_logo_info_list_lg(ds, list_wmsec_logo, &mut list_nrf_logo, info_cmd.type_logo);
        let loc = get_logo_info_list_find(list_wmsec_logo, info_cmd.msec_sel, info_cmd.edge_sel);
        if info_cmd.type_set_base == ScrLogoSetBase::BaseLoc {
            if loc >= 0
                && loc < list_wmsec_logo.len() as i32
                && list_wmsec_logo[loc as usize].just == info_cmd.msec_sel
            {
                det = true;
                lim.set_logo_base_nrf(list_nrf_logo[loc as usize], info_cmd.edge_sel);
            }
        } else {
            det = !list_wmsec_logo.is_empty();
        }
    } else {
        let mut list_nsc_logo: Vec<i32> = Vec::new();
        let flag_out = cmdarg.get_opt(OptType::FlagFinal) != 0;
        get_logo_info_list_elg(ds, list_wmsec_logo, &mut list_nsc_logo, flag_out);
        let loc = get_logo_info_list_find(list_wmsec_logo, info_cmd.msec_sel, info_cmd.edge_sel);
        if info_cmd.type_set_base == ScrLogoSetBase::BaseLoc {
            if loc >= 0
                && loc < list_wmsec_logo.len() as i32
                && list_wmsec_logo[loc as usize].just == info_cmd.msec_sel
            {
                det = true;
                lim.set_logo_base_nsc(list_nsc_logo[loc as usize], info_cmd.edge_sel);
            }
        } else {
            det = !list_wmsec_logo.is_empty();
        }
    }
    if info_cmd.type_set_base == ScrLogoSetBase::ValidList {
        let loc = get_logo_info_list_find(list_wmsec_logo, info_cmd.msec_sel, info_cmd.edge_sel);
        lim.set_logo_wmsec_list(list_wmsec_logo, loc);
    }
    det
}

/// getLogoInfoListFind
fn get_logo_info_list_find(list_wmsec: &mut [WideMsec], msec_logo: i32, edge: i32) -> i32 {
    if list_wmsec.is_empty() {
        return -2;
    }
    let mut loc = if is_logo_edge_rise(edge) { 0 } else { 1 };
    let nmax = list_wmsec.len() as i32;
    let mut flag_cont = true;
    while flag_cont && (loc + 2 < nmax) {
        if list_wmsec[(loc + 2) as usize].just <= msec_logo
            && list_wmsec[loc as usize].just < msec_logo
        {
            loc += 2;
        } else {
            flag_cont = false;
            let flag_in_c = list_wmsec[loc as usize].late >= msec_logo;
            let flag_in_n = list_wmsec[(loc + 2) as usize].early <= msec_logo;
            if flag_in_c && !flag_in_n {
            } else if (!flag_in_c && flag_in_n)
                || list_wmsec[(loc + 1) as usize].just < msec_logo
                || (list_wmsec[(loc + 1) as usize].just == msec_logo
                    && list_wmsec[(loc + 2) as usize].just - msec_logo
                        < msec_logo - list_wmsec[loc as usize].just)
            {
                loc += 2;
            }
        }
    }
    if loc == nmax - 2
        && list_wmsec[(loc + 1) as usize].just <= msec_logo
        && list_wmsec[loc as usize].just < msec_logo
    {
        loc += 2;
    }
    if loc == 1 && list_wmsec[0].just >= msec_logo && list_wmsec[1].just > msec_logo {
        loc = -1;
    }
    loc
}

/// getLogoInfoListLg
fn get_logo_info_list_lg(
    ds: &Dataset,
    list_wmsec: &mut Vec<WideMsec>,
    list_nrf: &mut Vec<i32>,
    type_sel: i32,
) {
    list_wmsec.clear();
    list_nrf.clear();
    let mut logopt = NrfCurrent::default();
    let mut flag_cont = true;
    while flag_cont {
        flag_cont = ds.nrf_pt_next(&mut logopt, type_sel);
        if flag_cont {
            let w_rise = ds.wide_msec_logo_nrf(logopt.nrf_rise);
            let w_fall = ds.wide_msec_logo_nrf(logopt.nrf_fall);
            list_wmsec.push(w_rise);
            list_wmsec.push(w_fall);
            list_nrf.push(logopt.nrf_rise);
            list_nrf.push(logopt.nrf_fall);
        }
    }
}

/// getLogoInfoListElg
fn get_logo_info_list_elg(
    ds: &Dataset,
    list_wmsec: &mut Vec<WideMsec>,
    list_nsc: &mut Vec<i32>,
    outflag: bool,
) {
    list_wmsec.clear();
    list_nsc.clear();
    let mut elg = ElgCurrent {
        outflag,
        ..Default::default()
    };
    let mut flag_cont = true;
    while flag_cont {
        flag_cont = ds.elg_pt_next(&mut elg);
        if flag_cont {
            let msec_rise_t = ds.msec_scp(elg.nsc_rise);
            let msec_rise_bk = ds.msec_scp_bk(elg.nsc_rise);
            let msec_fall_t = ds.msec_scp(elg.nsc_fall);
            let msec_fall_bk = ds.msec_scp_bk(elg.nsc_fall);
            list_wmsec.push(WideMsec {
                just: msec_rise_t,
                early: msec_rise_bk,
                late: msec_rise_t,
            });
            list_wmsec.push(WideMsec {
                just: msec_fall_t,
                early: msec_fall_bk,
                late: msec_fall_t,
            });
            list_nsc.push(elg.nsc_rise);
            list_nsc.push(elg.nsc_fall);
        }
    }
}

/// checkOptScpFromMsec
pub fn check_opt_scp_from_msec(
    ds: &Dataset,
    cmdarg: &mut CmdArg,
    msec_base: i32,
    edge: i32,
    chk_base: bool,
    chk_rel: bool,
) -> bool {
    let size_scp = ds.size_scp();
    let jfrom = cmdarg.get_opt(OptType::FlagNoEdge);
    let jsize = size_scp - cmdarg.get_opt(OptType::FlagNoEdge);
    let mut result = true;
    let numlist = cmdarg.size_sc_opt();
    for k in 0..numlist {
        let mut sctype = cmdarg.sc_opt_type(k);
        if cmdarg.is_sc_opt_relative(k) {
            if !chk_rel {
                sctype = OptType::ScNone;
            }
        } else if !chk_base {
            sctype = OptType::ScNone;
        }
        if sctype != OptType::ScNone {
            let mut nsc_scpos_sc = -1i32;
            let mut nsc_smute_all = -1i32;
            let mut nsc_smute_part = -1i32;
            let mut nsc_chap_auto = -1i32;
            let lens_min = cmdarg.get_sc_opt_min(k);
            let lens_max = cmdarg.get_sc_opt_max(k);
            let mut j = jfrom;
            while j < jsize {
                let dt = ds.scp[j as usize];
                let msec_now = if is_logo_edge_rise(edge) {
                    dt.msec
                } else {
                    dt.msbk
                };
                if (msec_now - msec_base >= lens_min || lens_min == -1)
                    && (msec_now - msec_base <= lens_max || lens_min == -1)
                {
                    nsc_scpos_sc = j;
                    let chap_now = dt.chap;
                    if chap_now >= SCP_CHAP_DECIDE || chap_now == SCP_CHAP_CDET {
                        nsc_chap_auto = j;
                    }
                }
                let mut msec_smute_s = dt.msmute_s;
                let mut msec_smute_e = dt.msmute_e;
                if msec_smute_s < 0 || msec_smute_e < 0 {
                    msec_smute_s = msec_now;
                    msec_smute_e = msec_now;
                }
                if msec_smute_s - msec_base <= lens_min && msec_smute_e - msec_base >= lens_max {
                    nsc_smute_all = j;
                }
                if (msec_smute_s - msec_base <= lens_max || lens_max == -1)
                    && (msec_smute_e - msec_base >= lens_min || lens_min == -1)
                {
                    nsc_smute_part = j;
                }
                j += 1;
            }
            if nsc_scpos_sc < 0 && sctype == OptType::ScSC
                || nsc_scpos_sc >= 0 && sctype == OptType::ScNoSC
                || nsc_smute_part < 0 && sctype == OptType::ScSM
                || nsc_smute_part >= 0 && sctype == OptType::ScNoSM
                || nsc_smute_all < 0 && sctype == OptType::ScSMA
                || nsc_smute_all >= 0 && sctype == OptType::ScNoSMA
                || nsc_chap_auto < 0 && sctype == OptType::ScAC
                || nsc_chap_auto >= 0 && sctype == OptType::ScNoAC
            {
                result = false;
            }
        }
        if !result {
            break;
        }
    }
    result
}

/// limitLogoList
pub fn limit_logo_list(ds: &Dataset, cmdarg: &mut CmdArg, lim: &mut CmdLimit) -> i32 {
    let mut type_logo_sel = LOGO_SELECT_ALL;
    let mut flag_limit_range = false;
    match cmdarg.get_opt(OptType::TypeNumLogo) {
        n if n == OptType::LgN as i32 || n == OptType::LgNFXlogo as i32 => {
            type_logo_sel = LOGO_SELECT_ALL;
            flag_limit_range = false;
        }
        n if n == OptType::LgNR as i32 => {
            type_logo_sel = LOGO_SELECT_VALID;
            flag_limit_range = false;
        }
        n if n == OptType::LgNlogo as i32 || n == OptType::LgNauto as i32 => {
            type_logo_sel = LOGO_SELECT_VALID;
            flag_limit_range = true;
        }
        n if n == OptType::LgNFlogo as i32 || n == OptType::LgNFauto as i32 => {
            type_logo_sel = LOGO_SELECT_VALID;
            flag_limit_range = false;
        }
        _ => {}
    }
    let mut list_wmsec_logo: Vec<WideMsec> = Vec::new();
    let mut loc_start = -1i32;
    let mut loc_end = -1i32;
    let valid_list;
    {
        let info_cmd = ScrLogoInfoCmdRecord {
            type_logo: type_logo_sel,
            type_set_base: ScrLogoSetBase::None,
            edge_sel: 0,
            msec_sel: 0,
        };
        get_logo_info_list(ds, cmdarg, lim, &mut list_wmsec_logo, info_cmd);
        let (hs, he) = if flag_limit_range {
            (lim.get_head(), lim.get_tail())
        } else {
            (-1, -1)
        };
        valid_list =
            limit_logo_list_range(&mut loc_start, &mut loc_end, &list_wmsec_logo, (hs, he), ds);
        if cmdarg.get_opt(OptType::FlagPair) > 0 {
            if loc_start > 0 && loc_start % 2 != 0 {
                loc_start -= 1;
            }
            if loc_end > 0 && loc_end % 2 == 0 {
                loc_end += 1;
            }
        }
    }
    if valid_list {
        let edge_sel = cmdarg.select_edge;
        let max_rise = (loc_end / 2) - ((loc_start + 1) / 2) + 1;
        let max_fall = ((loc_end + 1) / 2) - (loc_start / 2);
        let mut cur_rise = 0i32;
        let mut cur_fall = 0i32;
        let mut i = loc_start;
        while i <= loc_end {
            let flag_loc_rise = i % 2 == 0;
            if flag_loc_rise && is_logo_edge_rise(edge_sel) {
                cur_rise += 1;
                if limit_logo_list_sub(cmdarg, cur_rise, max_rise) {
                    let msec = list_wmsec_logo[i as usize].just;
                    lim.add_logo_list(msec, EDGE_RISE);
                }
            }
            let flag_loc_fall = i % 2 == 1;
            if flag_loc_fall && is_logo_edge_fall(edge_sel) {
                cur_fall += 1;
                if limit_logo_list_sub(cmdarg, cur_fall, max_fall) {
                    let msec = list_wmsec_logo[i as usize].just;
                    lim.add_logo_list(msec, EDGE_FALL);
                }
            }
            i += 1;
        }
    }
    lim.size_logo_list()
}

/// limitLogoListSub
fn limit_logo_list_sub(cmdarg: &mut CmdArg, cur_num: i32, max_num: i32) -> bool {
    let mut result = false;
    let numlist = cmdarg.size_lg_opt();
    if numlist == 0 {
        result = true;
    }
    for m in 0..numlist {
        let str_val = cmdarg.get_lg_opt(m);
        let rloc = str_val.iter().position(|&c| c == b'.').map(|p| {
            // find ".." position
            let mut i = p;
            while i + 1 < str_val.len() {
                if str_val[i] == b'.' && str_val[i + 1] == b'.' {
                    return i as i32;
                }
                i += 1;
            }
            p as i32
        });
        match rloc {
            None => {
                let val =
                    super::cmdarg::parse_stoi(&String::from_utf8_lossy(&str_val)).unwrap_or(0);
                if val == 0 || val == cur_num || max_num + val + 1 == cur_num {
                    result = true;
                }
            }
            Some(r) => {
                let str_st = &str_val[..r as usize];
                let str_ed = if (r + 2) as usize <= str_val.len() {
                    &str_val[(r + 2) as usize..]
                } else {
                    &[]
                };
                let mut val_st =
                    super::cmdarg::parse_stoi(&String::from_utf8_lossy(str_st)).unwrap_or(0);
                let mut val_ed =
                    super::cmdarg::parse_stoi(&String::from_utf8_lossy(str_ed)).unwrap_or(0);
                if val_st < 0 {
                    val_st = max_num + val_st + 1;
                }
                if val_ed < 0 {
                    val_ed = max_num + val_ed + 1;
                }
                if val_st <= cur_num && cur_num <= val_ed {
                    result = true;
                }
            }
        }
    }
    result
}

/// limitLogoListRange
fn limit_logo_list_range(
    st: &mut i32,
    ed: &mut i32,
    list_wmsec: &[WideMsec],
    rmsec: (i32, i32),
    ds: &Dataset,
) -> bool {
    *st = -1;
    *ed = -1;
    if list_wmsec.is_empty() {
        return false;
    }
    let msec_spc = ds.msec_val_spc;
    let mut st_1st = true;
    for i in 0..list_wmsec.len() as i32 {
        let mut msec_sel = list_wmsec[i as usize].just;
        if i % 2 == 0 {
            msec_sel += msec_spc;
        } else {
            msec_sel -= msec_spc;
        }
        if (msec_sel >= rmsec.0 || rmsec.0 < 0) && st_1st {
            *st = i;
            st_1st = false;
        }
        if msec_sel <= rmsec.1 || rmsec.1 < 0 {
            *ed = i;
        }
    }
    if *st > *ed || *st < 0 || *ed < 0 {
        *st = -1;
        *ed = -1;
        return false;
    }
    true
}
