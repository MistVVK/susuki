//! Derived from join_logo_scp (https://github.com/tobitti0/join_logo_scp,
//! commit d1a24d758f8b3c04352b82ff6756fdf3d56729b3), Copyright (C) tobitti0,
//! GPL-2.0-or-later. This port is a GPL derivative work, redistributed here
//! under GPL-3.0-or-later (design §10.6).
//! JlsAutoReform Phase E core: CM formation (mkRangeCM/setCMForm*) and
//! two-point rebuild (setInterLogo*/setInterpolar*/setInterMultiCM).
//! Follows JlsAutoReform.cpp:1471-4109 line by line; &Cnv carries the
//! converter the reference accesses through the dataset member.

use super::cmdarg::WideMsec;
use super::cnvstr::Cnv;
use super::dataset::*;
use super::dataset_search::*;
use super::reform_base::*;
use super::tables::ConfigVarType;

#[derive(Clone, Copy)]
pub struct WideMsecX {
    pub st: WideMsec,
    pub ed: WideMsec,
    pub fix_st: bool,
    pub fix_ed: bool,
    pub logomode: bool,
}

#[derive(Clone, Copy, Default)]
pub struct TraceInterpolar {
    pub keep_type: i32,
    pub keep_gap: i32,
    pub nsc_keep: i32,
    pub nsc_base: i32,
    pub keep15s: bool,
}

#[derive(Clone, Copy)]
pub struct FormCMByLogo {
    pub msec_target: i32,
    pub msec_limit: i32,
    pub msec_cm_side: i32,
    pub msec_logo_side: i32,
    pub rev_del_edge: i32,
    pub dir_prev: bool,
}

#[derive(Clone, Copy, Default)]
pub struct FormCMEdgeSide {
    pub end_side: bool,
    pub nsc_detect: i32,
    pub nsc_fixed: i32,
    pub nsc_other: i32,
    pub logo_mode_prev: bool,
    pub logo_mode_next: bool,
}

pub struct RangeNscMsec {
    pub nsc: (i32, i32),
    pub msec: (i32, i32),
}

/// getRangeMsec(RangeNsc)
pub fn range_msec_of(ds: &Dataset, nsc: (i32, i32)) -> (i32, i32) {
    ds.range_msec_from_range_nsc(nsc.0, nsc.1)
}

/// mkRangeCMGetLogoEdge
pub fn mk_range_cm_get_logo_edge(
    ds: &Dataset,
    msec_target: i32,
    flag_fix: bool,
    edge: i32,
) -> (WideMsec, bool) {
    let mut wmsec = WideMsec {
        just: -1,
        early: -1,
        late: -1,
    };
    let mut detect = false;
    let msec_sftmrg = ds.get_config(ConfigVarType::msecWLogoSftMrg);
    if msec_target < 0 {
        // all -1 already
    } else if flag_fix {
        wmsec.just = msec_target;
        wmsec.early = msec_target - ds.msec_val_spc;
        wmsec.late = msec_target + ds.msec_val_spc;
    } else {
        let nrf = ds.nrf_logo_from_msec(msec_target, edge);
        if nrf >= 0 {
            detect = true;
            wmsec = ds.wide_msec_logo_nrf(nrf);
            wmsec.early -= msec_sftmrg;
            wmsec.late += msec_sftmrg;
        } else {
            wmsec.just = msec_target;
            wmsec.early = msec_target;
            wmsec.late = msec_target;
        }
    }
    (wmsec, detect)
}

// 原典の引数構成をそのまま移植しているため 8 個になる（clippy の既定 7 超過は構造維持が優先）
#[allow(clippy::too_many_arguments)]
/// mkRangeCM
pub fn mk_range_cm(
    ds: &mut Dataset,
    cnv: &Cnv,
    fix_st: i32,
    fix_ed: i32,
    fix_scope_st: bool,
    fix_scope_ed: bool,
    logo_1st: bool,
    interfill: i32,
) -> (bool, (i32, i32)) {
    // cmscope edges
    let (st_w, _d1) = mk_range_cm_get_logo_edge(ds, fix_st, fix_scope_st, EDGE_FALL);
    let (ed_w, _d2) = mk_range_cm_get_logo_edge(ds, fix_ed, fix_scope_ed, EDGE_RISE);
    let cmscope = WideMsecX {
        st: st_w,
        ed: ed_w,
        fix_st: fix_scope_st,
        fix_ed: fix_scope_ed,
        logomode: false,
    };
    // save edge chaps when fixed
    let mut chap_bakst = SCP_CHAP_NONE;
    let mut chap_baked = SCP_CHAP_NONE;
    {
        let nsc_st = ds.nsc_from_msec_all(fix_st);
        let nsc_ed = ds.nsc_from_msec_all(fix_ed);
        if nsc_st > 0 {
            chap_bakst = ds.scp_chap(nsc_st);
        }
        if nsc_ed > 0 {
            chap_baked = ds.scp_chap(nsc_ed);
        }
    }
    // score/chap over the scope
    let scope_chap = RangeFixMsec {
        st: if !cmscope.fix_st {
            cmscope.st.early
        } else {
            cmscope.st.just
        },
        ed: if !cmscope.fix_ed {
            cmscope.ed.late
        } else {
            cmscope.ed.just
        },
        fix_st: cmscope.fix_st,
        fix_ed: cmscope.fix_ed,
    };
    set_score(ds, cnv, scope_chap.st, scope_chap.ed);
    set_chap(ds, cnv, scope_chap);

    // CM formation
    let logointer = true;
    let (det, bounds) = set_cm_form(ds, cnv, cmscope, logo_1st, logointer);

    // fill inside CMs
    if det {
        let mut nscbounds = RangeNscPair { st: 0, ed: 0 };
        get_range_nsc_from_range_msec_cnv(ds, cnv, bounds, &mut nscbounds);
        set_inter_multi_cm(ds, cnv, (nscbounds.st, nscbounds.ed), interfill);
    }

    // restore fixed edges
    if fix_scope_st && is_scp_chap_type_decide(chap_bakst) {
        let nsc_st = ds.nsc_from_msec_all(fix_st);
        ds.set_scp_chap(nsc_st, chap_bakst);
    }
    if fix_scope_ed && is_scp_chap_type_decide(chap_baked) {
        let nsc_ed = ds.nsc_from_msec_all(fix_ed);
        ds.set_scp_chap(nsc_ed, chap_baked);
    }
    (det, bounds)
}

pub struct RangeNscPair {
    pub st: i32,
    pub ed: i32,
}

/// getRangeNscFromRangeMsec (returns det)
fn get_range_nsc_from_range_msec_cnv(
    ds: &Dataset,
    _cnv: &Cnv,
    rmsec: (i32, i32),
    out: &mut RangeNscPair,
) -> bool {
    let (rnsc, det) = ds.range_nsc_from_range_msec(rmsec.0, rmsec.1);
    out.st = rnsc.st;
    out.ed = rnsc.ed;
    det
}

/// setCMForm
pub fn set_cm_form(
    ds: &mut Dataset,
    cnv: &Cnv,
    cmscope: WideMsecX,
    logo_1st: bool,
    logointer: bool,
) -> (bool, (i32, i32)) {
    let num_scpos = ds.size_scp();
    let mut logoon_st = logo_1st;
    if num_scpos - 1 <= 1 {
        return (false, (-1, -1));
    }
    let rev_del_edge = ds.get_config_action(0); // ConfigActType::LogoDelEdge

    // 15s search window
    let mut findscope = cmscope;
    findscope.st.early = cmscope.st.just - ds.msec_val_lap2;
    findscope.st.late = cmscope.st.just + ds.msec_val_lap2;
    findscope.ed.early = cmscope.ed.just - ds.msec_val_lap2;
    findscope.ed.late = cmscope.ed.just + ds.msec_val_lap2;
    if rev_del_edge == 0 {
        if findscope.st.late < cmscope.st.late {
            findscope.st.late = cmscope.st.late;
        }
        if findscope.ed.early > cmscope.ed.early {
            findscope.ed.early = cmscope.ed.early;
        }
    }
    if rev_del_edge == 4 {
        if findscope.st.early > cmscope.st.early {
            findscope.st.early = cmscope.st.early;
        }
        if findscope.ed.late < cmscope.ed.late {
            findscope.ed.late = cmscope.ed.late;
        }
    }
    if findscope.st.just < 0 {
        findscope.st.early = -1;
        findscope.st.late = -1;
    }
    if findscope.ed.just < 0 {
        findscope.ed.early = -1;
        findscope.ed.late = -1;
    }
    // unclear logo edge correction
    if cmscope.st.just > 0 {
        let mut wmsec_logo = cmscope.st;
        wmsec_logo.early -= ds.msec_val_spc;
        wmsec_logo.late += ds.msec_val_spc;
        let mut nsc_st = ds.nsc_from_wide_msec_by_chap(&wmsec_logo, SCP_CHAP_CDET);
        if nsc_st < 0 {
            nsc_st = ds.nsc_from_wide_msec_by_chap(&wmsec_logo, SCP_CHAP_NONE);
        }
        if nsc_st > 0 {
            let msec_st = ds.msec_scp(nsc_st);
            if findscope.st.early > msec_st - ds.msec_val_spc {
                findscope.st.early = msec_st - ds.msec_val_spc;
            }
        }
    }
    if cmscope.ed.just > 0 {
        let mut wmsec_logo = cmscope.ed;
        wmsec_logo.early -= ds.msec_val_spc;
        wmsec_logo.late += ds.msec_val_spc;
        let mut nsc_ed = ds.nsc_from_wide_msec_by_chap(&wmsec_logo, SCP_CHAP_CDET);
        if nsc_ed < 0 {
            nsc_ed = ds.nsc_from_wide_msec_by_chap(&wmsec_logo, SCP_CHAP_NONE);
        }
        if nsc_ed > 0 {
            let msec_ed = ds.msec_scp(nsc_ed);
            if findscope.ed.late < msec_ed + ds.msec_val_spc {
                findscope.ed.late = msec_ed + ds.msec_val_spc;
            }
        }
    }
    // scan candidates
    let mut bounds = (-1i32, -1i32);
    let mut det = false;
    {
        let mut nsc_cur = 1i32;
        let mut over = false;
        while !over {
            let msec_cur = ds.msec_scp(nsc_cur);
            let chap_cur = ds.scp_chap(nsc_cur);
            if (msec_cur >= findscope.st.early || findscope.st.early < 0)
                && (msec_cur <= findscope.ed.late || findscope.ed.late < 0)
                && chap_cur >= SCP_CHAP_CDET
            {
                let (det_tmp, msec_stpoint) =
                    set_cm_form_detect(ds, cnv, nsc_cur, findscope, logoon_st);
                if msec_stpoint > 0 && (bounds.0 > msec_stpoint || bounds.0 < 0) {
                    bounds.0 = msec_stpoint;
                }
                if det_tmp || chap_cur >= SCP_CHAP_DFIX {
                    bounds.1 = msec_cur;
                    if bounds.0 < 0 {
                        bounds.0 = bounds.1;
                    }
                }
                if det_tmp {
                    det = true;
                    logoon_st = logointer;
                }
            }
            // nsc shift on insertion
            let nsc_tmp = ds.nsc_from_msec_all(msec_cur);
            if nsc_tmp > nsc_cur && nsc_tmp > 0 && nsc_cur > 0 {
                nsc_cur = nsc_tmp;
            }
            nsc_cur += 1;
            if nsc_cur >= num_scpos - 1 || (msec_cur > findscope.ed.late && findscope.ed.late >= 0)
            {
                over = true;
            }
        }
    }
    // revise large-error CM ranges
    let fix_mode = cmscope.fix_st || cmscope.fix_ed;
    if !cmscope.logomode && det {
        set_cm_form_det_revise(ds, cnv, &mut bounds, fix_mode);
    }
    // edge corrections
    if !cmscope.logomode && !fix_mode {
        let lvlogo = ds.level_use_logo();
        if det && lvlogo >= 4 {
            let det_tmp = set_cm_form_by_logo(ds, cnv, &mut bounds, cmscope);
            if det_tmp {
                det = true;
            }
        }
        if det || lvlogo > 4 {
            let det_tmp = set_cm_form_edge(ds, cnv, &mut bounds, cmscope, logo_1st);
            if det_tmp {
                det = true;
            }
        }
    }
    (det, bounds)
}

/// setCMFormDetect
pub fn set_cm_form_detect(
    ds: &mut Dataset,
    cnv: &Cnv,
    nsc_base: i32,
    findscope: WideMsecX,
    logoon_st: bool,
) -> (bool, i32) {
    let mut msec_stpoint = -1i32;
    let type_nosc = ds.get_config_action(5); // ConfigActType::MuteNoSc
    let mut det = false;
    let mut flag_force = false;
    let mut nsc_save = nsc_base;
    {
        let msec_base = ds.msec_scp(nsc_base);
        let mut sec_difend = 0i32;
        let mut i = nsc_base;
        let mut fin = false;
        while !fin {
            i = ds.nsc_prev_scp_chap(i, SCP_CHAP_CDET);
            if i <= 0 {
                break;
            }
            let msec_i = ds.msec_scp(i);
            if (msec_base <= findscope.st.late && findscope.st.late >= 0)
                || (msec_i < findscope.st.early && findscope.st.early >= 0)
            {
                break;
            }
            let mut chap_i = ds.scp_chap(i);
            if chap_i >= SCP_CHAP_DFORCE {
                flag_force = true;
                if msec_stpoint > msec_i || msec_stpoint < 0 {
                    msec_stpoint = msec_i;
                }
            }
            if chap_i == SCP_CHAP_DINT {
                ds.set_scp_chap(i, SCP_CHAP_NONE);
                chap_i = SCP_CHAP_NONE;
            }
            let sec_dif = cnv.get_sec_from_msec(msec_base - msec_i);
            let sec_dif15 = ((sec_dif + 7) / 15) * 15;
            let msec_align = msec_base - sec_dif15 * 1000;
            let nsc_near = ds.nsc_from_msec_chap(msec_align, ds.msec_val_lap2, SCP_CHAP_CDET);
            let nsc_chk = ds.nsc_from_msec_chap(msec_align, ds.msec_val_lap2, SCP_CHAP_DFORCE);
            if sec_dif15 > 120 || (sec_dif15 > sec_difend && sec_difend > 0) {
                fin = true;
            } else {
                let flag_dif_near = (msec_i - msec_align).abs() < ds.msec_val_lap2;
                if chap_i >= SCP_CHAP_CDET && sec_difend <= 0 {
                    let score_tmp = ds.scp_score(i);
                    let score_base = ds.scp_score(nsc_base);
                    if flag_dif_near || score_tmp + 30 >= score_base {
                        sec_difend = sec_dif15;
                    }
                }
                if chap_i >= SCP_CHAP_DFIX && nsc_near != i && !flag_dif_near {
                    fin = true;
                }
            }
            if !fin && nsc_near == i && nsc_chk < 0 && sec_dif15 > 0 && sec_dif15 <= 120 {
                det = true;
                fin = true;
                ds.set_scp_chap(nsc_save, SCP_CHAP_DFIX);
                if !findscope.logomode {
                    ds.set_scp_arstat(nsc_save, SCP_AR_N_UNIT);
                } else {
                    ds.set_scp_arstat(nsc_save, SCP_AR_L_UNIT);
                }
                let msec_update = ds.msec_scp(nsc_save);
                if msec_stpoint > msec_update || msec_stpoint < 0 {
                    msec_stpoint = msec_update;
                }
                if !flag_force {
                    nsc_save = i;
                    let nsc_tmp2 =
                        ds.nsc_from_msec_chap(msec_align, ds.msec_val_near2, SCP_CHAP_CDET);
                    if ds.is_smute_from_msec(msec_align) && nsc_tmp2 < 0 && type_nosc != 1 {
                        // C++ getScore(msec_align, findscope) uses the
                        // RangeWideMsec conversion {st.early, ed.late}
                        let score_align0 =
                            get_score(ds, cnv, msec_align, findscope.st.early, findscope.ed.late);
                        let score_align = if sec_dif15 > 60 && sec_dif15 % 30 > 0 {
                            0
                        } else {
                            score_align0
                        };
                        if score_align > ds.scp_score(i) {
                            let msec_newrev =
                                cnv.tc.get_msec_align_from_msec(msec_align as i64) as i32;
                            let msec_newbk =
                                cnv.tc.get_msec_adjust_frm_from_msec(msec_align as i64, -1) as i32;
                            ds.set_scp_chap(i, SCP_CHAP_NONE);
                            ds.set_scp_statpos(i, PRIOR_NONE);
                            let nsc_tmp = ds.insert_scpos(msec_newrev, msec_newbk, i, PRIOR_DECIDE);
                            ds.set_scp_chap(nsc_tmp, SCP_CHAP_CDET);
                            ds.set_scp_score(nsc_tmp, score_align);
                            nsc_save = nsc_tmp;
                            let _ = msec_newrev;
                            fin = false;
                            sec_difend = 0;
                            if i > nsc_tmp {
                                i = nsc_tmp;
                            }
                        }
                    }
                }
            }
            let _ = nsc_near;
        }
    }
    if det && nsc_save > 0 && !flag_force {
        if ds.scp_chap(nsc_save) < SCP_CHAP_DECIDE {
            ds.set_scp_chap(nsc_save, SCP_CHAP_DFIX);
            let arstat_save = if logoon_st {
                SCP_AR_L_OTHER
            } else {
                SCP_AR_N_OTHER
            };
            ds.set_scp_arstat(nsc_save, arstat_save);
        }
        let msec_update = ds.msec_scp(nsc_save);
        if msec_stpoint > msec_update || msec_stpoint < 0 {
            msec_stpoint = msec_update;
        }
    }
    (det, msec_stpoint)
}

/// setCMFormDetRevise
pub fn set_cm_form_det_revise(
    ds: &mut Dataset,
    cnv: &Cnv,
    bounds: &mut (i32, i32),
    fix_mode: bool,
) {
    let mgn_cm_detect = ds.get_config(ConfigVarType::msecMgnCmDetect);
    let mut nscbounds;
    {
        let (rnsc, ok) = ds.range_nsc_from_range_msec(bounds.0, bounds.1);
        if !ok {
            // reference returns on false? it checks == false of getRangeNsc (det)
            // keep scanning with the returned values as in C++ early return
            let _ = rnsc;
            return;
        }
        nscbounds = (rnsc.st, rnsc.ed);
    }
    {
        let nsc_head = ds.nsc_next_scp_chap(0, SCP_CHAP_DECIDE);
        let mut nsc_loc0 = -1i32;
        let mut nsc_loc1 = nscbounds.0;
        let mut msec_loc0 = 0i32;
        let mut msec_loc1 = ds.msec_scp(nsc_loc1);
        let mut flag_cm0 = false;
        let mut flag_cm1 = false;
        let mut calc1 = CalcModInfo {
            mod15: -1,
            mod05: -1,
        };
        let mut cont1 = true;
        while cont1 {
            let nsc_loc2 = ds.nsc_next_scp_chap(nsc_loc1, SCP_CHAP_DECIDE);
            let msec_loc2 = ds.msec_scp(nsc_loc2);
            let cont2 = nsc_loc2 <= nscbounds.1 && nsc_loc2 > 0;
            let arstat_loc2 = ds.scp_arstat(nsc_loc2);
            let mut flag_cm2 = arstat_loc2 == SCP_AR_N_UNIT;
            let mut calc2 = CalcModInfo::default();
            calc_dif_mod0515(ds, &mut calc2, msec_loc2 - msec_loc1);
            if calc2.mod15 == -1 {
                flag_cm2 = false;
            }
            if flag_cm1 && calc1.mod15 > mgn_cm_detect && nsc_loc0 >= 0 {
                let mut ignore1 = false;
                if cont2 && flag_cm2 {
                    let msec_dif20 = msec_loc2 - msec_loc0;
                    if msec_dif20 <= 120 * 1000 + ds.msec_val_lap2 {
                        let mut calcsum = CalcModInfo::default();
                        calc_dif_mod0515(ds, &mut calcsum, msec_loc2 - msec_loc0);
                        if calcsum.mod15 < mgn_cm_detect
                            && (calcsum.mod15 <= mgn_cm_detect / 2
                                || calcsum.mod15 < (calc1.mod15 + calc2.mod15) / 6)
                        {
                            ds.set_scp_chap(nsc_loc1, SCP_CHAP_NONE);
                            ignore1 = true;
                            nsc_loc1 = nsc_loc0;
                            msec_loc1 = msec_loc0;
                            calc2 = calcsum;
                        }
                    }
                }
                if !ignore1 && !fix_mode {
                    let mut nsc_subs = -1i32;
                    let mut nsc_sube = -1i32;
                    let mut dr_next = true;
                    if !flag_cm0 && flag_cm2 {
                        nsc_subs = nsc_loc0;
                        nsc_sube = nsc_loc1;
                        dr_next = true;
                    }
                    if flag_cm0 && !flag_cm2 {
                        nsc_subs = nsc_loc1;
                        nsc_sube = nsc_loc0;
                        dr_next = false;
                    }
                    if nsc_sube > 0 {
                        let mut cont_tmp = true;
                        let mut nsc_tmp = nsc_subs;
                        let msec_subs = ds.msec_scp(nsc_subs);
                        let msec_sube = ds.msec_scp(nsc_sube);
                        while cont_tmp {
                            nsc_tmp = ds.nsc_dir_scp_chap(nsc_tmp, dr_next, SCP_CHAP_NONE);
                            if nsc_tmp <= 0 || nsc_tmp <= nsc_loc0 || nsc_tmp >= nsc_loc1 {
                                cont_tmp = false;
                            } else {
                                let extype = (false, false, false);
                                let msec_tmp = ds.msec_scp(nsc_tmp);
                                if (msec_tmp - msec_subs).abs() >= ds.msec_val_lap2 {
                                    let mut calc_mod = CalcModInfo::default();
                                    let msec_diftmp = (msec_sube - msec_tmp).abs();
                                    let flag_len = is_length_logo_level(
                                        ds,
                                        cnv,
                                        msec_diftmp,
                                        extype.0,
                                        extype.1,
                                        extype.2,
                                    );
                                    let flag_s05 = calc_dif_mod0515(ds, &mut calc_mod, msec_diftmp);
                                    if flag_len && flag_s05 > 0 {
                                        cont_tmp = false;
                                        ds.set_scp_chap(nsc_tmp, SCP_CHAP_DINT);
                                        ds.set_scp_arstat(nsc_tmp, SCP_AR_N_OTHER);
                                        ds.set_scp_arstat(nsc_loc1, SCP_AR_N_OTHER);
                                        let sec_logos = ds
                                            .sec_logo_component_from_logo(cnv, msec_loc0, msec_tmp);
                                        if sec_logos > ((msec_tmp - msec_loc0).abs() + 500) / 2000 {
                                            ds.set_scp_arstat(nsc_tmp, SCP_AR_L_OTHER);
                                            if bounds.0 == msec_loc0 {
                                                bounds.0 = msec_tmp;
                                            }
                                        }
                                        let sec_logoe = ds
                                            .sec_logo_component_from_logo(cnv, msec_tmp, msec_loc1);
                                        if sec_logoe * 1000 + 500
                                            >= (msec_loc1 - msec_tmp).abs() / 2
                                        {
                                            ds.set_scp_arstat(nsc_loc1, SCP_AR_L_OTHER);
                                            if bounds.1 == msec_loc1 {
                                                bounds.1 = msec_tmp;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if !ignore1 && !fix_mode {
                    let mut nsc_delete = -1i32;
                    if ds.get_config_action(3) > 0 {
                        // LogoUCGapCm
                        if nsc_loc0 == nsc_head {
                            let arstat_loc0 = ds.scp_arstat(nsc_loc0);
                            if is_scp_ar_type_logo(arstat_loc0) {
                                ds.set_scp_arstat(nsc_loc1, SCP_AR_L_OTHER);
                            } else {
                                ds.set_scp_arstat(nsc_loc1, SCP_AR_N_OTHER);
                            }
                            ds.set_scp_chap(nsc_loc0, SCP_CHAP_NONE);
                            nsc_delete = nsc_loc0;
                        } else if nsc_loc2 < 0 {
                            ds.set_scp_chap(nsc_loc1, SCP_CHAP_NONE);
                            nsc_delete = nsc_loc1;
                        } else {
                            ds.set_scp_arstat(nsc_loc1, SCP_AR_L_UNIT);
                        }
                    } else {
                        if nsc_loc0 == nsc_head && msec_loc0 <= ds.msec_val_lap2 {
                            ds.set_scp_chap(nsc_loc0, SCP_CHAP_NONE);
                            nsc_delete = nsc_loc0;
                        } else if nsc_loc2 < 0 && msec_loc1 >= ds.msec_total_max - ds.msec_val_lap2
                        {
                            ds.set_scp_chap(nsc_loc1, SCP_CHAP_NONE);
                            nsc_delete = nsc_loc1;
                        }
                    }
                    if nsc_delete == nscbounds.0 {
                        nscbounds.0 = ds.nsc_next_scp_chap(nsc_delete, SCP_CHAP_DECIDE);
                        bounds.0 = ds.msec_scp(nscbounds.0);
                    } else if nsc_delete == nscbounds.1 {
                        nscbounds.1 = ds.nsc_prev_scp_chap(nsc_delete, SCP_CHAP_DECIDE);
                        bounds.1 = ds.msec_scp(nscbounds.1);
                    }
                }
            }
            nsc_loc0 = nsc_loc1;
            nsc_loc1 = nsc_loc2;
            msec_loc0 = msec_loc1;
            msec_loc1 = msec_loc2;
            flag_cm0 = flag_cm1;
            flag_cm1 = flag_cm2;
            calc1 = calc2;
            cont1 = cont2;
        }
    }
}

/// setCMFormByLogoLimit
pub fn set_cm_form_by_logo_limit(ds: &Dataset, msec_point: i32, dir_prev: bool) -> i32 {
    if msec_point < 0 {
        return -1;
    }
    let edge_logo = if dir_prev { EDGE_RISE } else { EDGE_FALL };
    let mut nrf_target = if dir_prev { -1 } else { ds.size_logo() };
    let mut msec_result = -1i32;
    let mut msec_last_r = -1i32;
    let mut ddet = false;
    loop {
        let msec_lg_c;
        let msec_lg_l;
        let msec_lg_r;
        nrf_target = ds.get_nrf_dir_logo(nrf_target, !dir_prev, edge_logo, LOGO_SELECT_VALID);
        if nrf_target >= 0 {
            let w = ds.wide_msec_logo_nrf(nrf_target);
            msec_lg_c = w.just;
            msec_lg_l = w.early;
            msec_lg_r = w.late;
            if dir_prev {
                if msec_lg_r > msec_point {
                    ddet = true;
                    msec_result = msec_last_r + ds.msec_val_lap2;
                }
            } else if msec_lg_l > msec_point {
                ddet = true;
                msec_result = msec_lg_l - ds.msec_val_lap2;
            }
            msec_last_r = msec_lg_r;
            let _ = msec_lg_c;
        }
        if !(nrf_target >= 0 && !ddet) {
            break;
        }
    }
    let mut nsc_point = ds.nsc_from_msec_chap(msec_point, ds.msec_val_near2, SCP_CHAP_DFIX);
    if nsc_point <= 0 {
        msec_result = msec_point;
    } else {
        nsc_point = ds.nsc_dir_scp_chap(nsc_point, !dir_prev, SCP_CHAP_DECIDE);
        if nsc_point >= 0 {
            let msec_point2 = ds.msec_scp(nsc_point);
            if dir_prev {
                if msec_result < msec_point2 || msec_result < 0 {
                    msec_result = msec_point2;
                }
            } else if msec_result > msec_point2 || msec_result < 0 {
                msec_result = msec_point2;
            }
        }
    }
    msec_result
}

/// setCMFormByLogo
pub fn set_cm_form_by_logo(
    ds: &mut Dataset,
    cnv: &Cnv,
    bounds: &mut (i32, i32),
    cmscope: WideMsecX,
) -> bool {
    let mut det = false;
    let rev_del_edge = ds.get_config_action(0);
    if !cmscope.fix_st && bounds.0 > 0 && cmscope.st.just > 0 {
        let msec_limit = set_cm_form_by_logo_limit(ds, bounds.0, true);
        if (bounds.0 > msec_limit || msec_limit < 0) && bounds.0 > cmscope.st.just {
            let form = FormCMByLogo {
                msec_target: cmscope.st.just,
                msec_limit,
                msec_cm_side: cmscope.st.late,
                msec_logo_side: cmscope.st.early,
                rev_del_edge,
                dir_prev: true,
            };
            let mut m = bounds.0;
            if set_cm_form_by_logo_add(ds, cnv, &mut m, form) {
                det = true;
            }
        }
    }
    if !cmscope.fix_ed && bounds.1 > 0 && cmscope.ed.just > 0 {
        let msec_limit = set_cm_form_by_logo_limit(ds, bounds.1, false);
        if (bounds.1 < msec_limit || msec_limit < 0) && bounds.1 < cmscope.ed.just {
            let form = FormCMByLogo {
                msec_target: cmscope.ed.just,
                msec_limit,
                msec_cm_side: cmscope.ed.early,
                msec_logo_side: cmscope.ed.late,
                rev_del_edge,
                dir_prev: false,
            };
            let mut m = bounds.1;
            if set_cm_form_by_logo_add(ds, cnv, &mut m, form) {
                det = true;
            }
        }
    }
    det
}

/// setCMFormByLogoAdd
pub fn set_cm_form_by_logo_add(
    ds: &mut Dataset,
    cnv: &Cnv,
    msec_result: &mut i32,
    form: FormCMByLogo,
) -> bool {
    let nsc_point = ds.nsc_from_msec_chap(*msec_result, ds.msec_val_near2, SCP_CHAP_DFIX);
    if nsc_point < 0 || form.rev_del_edge <= 1 {
        return false;
    }
    let mut arstat_new = SCP_AR_N_OTHER;
    let step: i32;
    let msec_cmdetect: i32;
    let msec_limit_rev: i32;
    if form.dir_prev {
        step = -1;
        msec_cmdetect = *msec_result - form.msec_target;
        msec_limit_rev = form.msec_limit + ds.msec_val_lap2;
        let arstat_point = ds.scp_arstat(nsc_point);
        if is_scp_ar_type_logo(arstat_point) {
            arstat_new = SCP_AR_L_OTHER;
        }
    } else {
        step = 1;
        msec_cmdetect = form.msec_target - *msec_result;
        msec_limit_rev = form.msec_limit - ds.msec_val_lap2;
    }
    if msec_cmdetect < 0 {
        return false;
    }
    let type_nosc = ds.get_config_action(5);
    let mut det = false;
    let mut msec_set_point = *msec_result;
    for i in 0..4i32 {
        let msec_difpt = 15 * 1000 * (i + 1);
        let msec_revpt = *msec_result + step * msec_difpt;
        let _ = msec_difpt;
        if ds.is_smute_from_msec(msec_revpt) {
            let mut flag_match = true;
            if form.msec_limit >= 0
                && ((msec_revpt <= msec_limit_rev && step < 0)
                    || (msec_revpt >= msec_limit_rev && step > 0))
            {
                flag_match = false;
            }
            if flag_match {
                let mut match_none = false;
                let mut match_full = false;
                let mut match_half = false;
                let mut match_minlogo = false;
                let mut match_later = false;
                {
                    let msec_difbase = (*msec_result - msec_set_point).abs();
                    let msec_diftarget = msec_cmdetect - msec_difbase;
                    let msec_difrev = (msec_revpt - msec_set_point).abs();
                    if msec_diftarget <= ds.msec_val_lap2 {
                        match_none = true;
                    } else if msec_diftarget >= msec_difrev - ds.msec_val_lap2 {
                        match_full = true;
                    } else if msec_diftarget >= msec_difrev / 2 {
                        match_half = true;
                    }
                    let msec_recmin = ds.get_config(ConfigVarType::msecWLogoLgMin);
                    if msec_difrev - msec_diftarget >= msec_recmin {
                        let msec_difcm_max = (*msec_result - form.msec_logo_side).abs();
                        if msec_difrev - msec_difcm_max >= msec_recmin {
                            match_minlogo = true;
                        }
                    }
                    if msec_diftarget < 15 * 1000 - ds.msec_val_lap2
                        && msec_diftarget > 0
                        && is_length_logo_level(ds, cnv, msec_diftarget, false, false, true)
                    {
                        match_later = true;
                    }
                }
                let mut match_logo = false;
                if (step > 0 && form.msec_logo_side < msec_revpt)
                    || (step < 0 && form.msec_logo_side > msec_revpt)
                {
                    match_logo = true;
                }
                let mut match_cm = false;
                if (step > 0 && form.msec_cm_side > msec_set_point)
                    || (step < 0 && form.msec_cm_side < msec_set_point)
                {
                    match_cm = true;
                }
                if form.rev_del_edge == 4 {
                    flag_match = true;
                } else if match_none || match_minlogo || match_later {
                    flag_match = false;
                } else if match_full {
                    flag_match = true;
                } else if form.rev_del_edge <= 1 {
                    if !match_half || match_logo {
                        flag_match = false;
                    }
                } else if form.rev_del_edge == 2 {
                    if !match_half {
                        flag_match = false;
                    }
                } else if form.rev_del_edge == 3 && !match_half && !match_cm {
                    flag_match = false;
                }
            }
            if flag_match {
                let mut nsc_new =
                    ds.nsc_from_msec_chap(msec_revpt, ds.msec_val_near2, SCP_CHAP_DFIX);
                if nsc_new < 0 {
                    nsc_new = ds.nsc_from_msec_chap(msec_revpt, ds.msec_val_near2, SCP_CHAP_DUPE);
                    if nsc_new < 0 && type_nosc != 1 {
                        nsc_new = ds.nsc_force_msec(cnv, msec_revpt, EDGE_RISE);
                    }
                }
                if nsc_new > 0 {
                    det = true;
                    let nsc_set_point =
                        ds.nsc_from_msec_chap(msec_set_point, ds.msec_val_near2, SCP_CHAP_DFIX);
                    ds.set_scp_chap(nsc_new, SCP_CHAP_DFIX);
                    if step < 0 {
                        ds.set_scp_arstat(nsc_new, arstat_new);
                        ds.set_scp_arstat(nsc_set_point, SCP_AR_N_UNIT);
                    } else {
                        ds.set_scp_arstat(nsc_new, SCP_AR_N_UNIT);
                    }
                    msec_set_point = msec_revpt;
                }
            }
        }
    }
    *msec_result = msec_set_point;
    det
}

/// setCMFormEdgeSideInfo
pub fn set_cm_form_edge_side_info(
    ds: &Dataset,
    msec_bounds: i32,
    wmsec_scope: WideMsec,
    chap_cmp: i32,
    endside: bool,
) -> (bool, FormCMEdgeSide) {
    let mut sidesel = FormCMEdgeSide::default();
    let rev_del_edge = ds.get_config_action(0);
    let mut valid_detect = true;
    let mut usescope = wmsec_scope;
    if wmsec_scope.just < 0 {
        valid_detect = false;
    } else {
        if rev_del_edge <= 3 {
            if !endside {
                if wmsec_scope.early < wmsec_scope.just - ds.msec_val_spc {
                    usescope.early = wmsec_scope.just - ds.msec_val_spc;
                }
            } else if wmsec_scope.late > wmsec_scope.just + ds.msec_val_spc {
                usescope.late = wmsec_scope.just + ds.msec_val_spc;
            }
        }
        if rev_del_edge == 0 {
            if !endside {
                if msec_bounds <= wmsec_scope.late && msec_bounds >= 0 {
                    valid_detect = false;
                }
            } else if msec_bounds >= wmsec_scope.early && msec_bounds >= 0 {
                valid_detect = false;
            }
        }
    }
    if !valid_detect {
        sidesel.nsc_detect = -1;
        return (false, sidesel);
    }
    sidesel.end_side = endside;
    sidesel.nsc_detect = ds.nsc_from_wide_msec_by_chap(&usescope, chap_cmp);
    sidesel.nsc_fixed = ds.nsc_from_msec_chap(msec_bounds, ds.msec_val_spc, SCP_CHAP_DECIDE);
    if ds.nsc_from_wide_msec_by_chap(&usescope, SCP_CHAP_DFORCE) >= 0
        || ds.nsc_from_msec_chap(msec_bounds, ds.msec_val_spc, SCP_CHAP_DFORCE) >= 0
    {
        sidesel.nsc_detect = -1;
    }
    sidesel.nsc_other = -1;
    if sidesel.nsc_detect >= 0 && sidesel.nsc_fixed >= 0 {
        if sidesel.nsc_detect < sidesel.nsc_fixed {
            sidesel.nsc_other = ds.nsc_prev_scp_chap(sidesel.nsc_fixed, SCP_CHAP_DECIDE);
            if sidesel.nsc_other >= sidesel.nsc_detect && sidesel.nsc_other >= 0 {
                sidesel.nsc_detect = -1;
            }
        } else if sidesel.nsc_detect > sidesel.nsc_fixed {
            sidesel.nsc_other = ds.nsc_next_scp_chap(sidesel.nsc_fixed, SCP_CHAP_DECIDE);
            if sidesel.nsc_other <= sidesel.nsc_detect && sidesel.nsc_other >= 0 {
                sidesel.nsc_detect = -1;
            }
        } else {
            sidesel.nsc_detect = -1;
        }
    }
    if !endside {
        sidesel.logo_mode_prev = true;
        sidesel.logo_mode_next = false;
    } else {
        sidesel.logo_mode_prev = false;
        sidesel.logo_mode_next = true;
    }
    if sidesel.nsc_detect < 0 {
        valid_detect = false;
    }
    (valid_detect, sidesel)
}

/// setCMFormEdgeSetSide
pub fn set_cm_form_edge_set_side(
    ds: &mut Dataset,
    cnv: &Cnv,
    sidesel: &mut FormCMEdgeSide,
    level: i32,
) -> bool {
    let msec_detect = ds.msec_scp(sidesel.nsc_detect);
    let msec_fixed = ds.msec_scp(sidesel.nsc_fixed);
    let msec_other = ds.msec_scp(sidesel.nsc_other.max(0));
    let msec_dif_fixed = (msec_detect - msec_fixed).abs();
    let msec_dif_other = (msec_detect - msec_other).abs();
    let dir_prev = sidesel.nsc_detect < sidesel.nsc_fixed;
    if sidesel.nsc_detect < 0
        || sidesel.nsc_fixed < 0
        || msec_dif_fixed <= ds.msec_val_lap2
        || (msec_dif_other <= ds.msec_val_lap2 && sidesel.nsc_other >= 0)
    {
        return false;
    }
    let mut det = false;
    if level <= 1 {
        let mut calcmod = CalcModInfo::default();
        let tmpdet = calc_dif_mod0515(ds, &mut calcmod, msec_dif_fixed);
        if tmpdet >= 2 || (level == 1 && tmpdet >= 1) {
            det = true;
        }
    } else {
        let mut calc1 = CalcDifInfo::default();
        calc_dif_select(ds, cnv, &mut calc1, 0, msec_dif_fixed);
        if calc1.gap <= ds.msec_val_exact || level == 3 {
            det = true;
        }
    }
    if det {
        let flag_other_s15 = is_cm_length_msec(cnv, msec_dif_other) && sidesel.nsc_other >= 0;
        let flag_fixed_s15 = is_cm_length_msec(cnv, msec_dif_fixed);
        ds.set_scp_chap(sidesel.nsc_detect, SCP_CHAP_DFIX);
        if dir_prev {
            let arstat_fixed = if sidesel.logo_mode_next {
                if flag_fixed_s15 {
                    SCP_AR_L_UNIT
                } else {
                    SCP_AR_L_OTHER
                }
            } else {
                if flag_fixed_s15 {
                    SCP_AR_N_UNIT
                } else {
                    SCP_AR_N_OTHER
                }
            };
            let arstat_detect = if sidesel.logo_mode_prev {
                if flag_other_s15 {
                    SCP_AR_L_UNIT
                } else {
                    SCP_AR_L_OTHER
                }
            } else {
                if flag_other_s15 {
                    SCP_AR_N_UNIT
                } else {
                    SCP_AR_N_OTHER
                }
            };
            ds.set_scp_arstat(sidesel.nsc_fixed, arstat_fixed);
            ds.set_scp_arstat(sidesel.nsc_detect, arstat_detect);
        } else {
            let arstat_detect = if sidesel.logo_mode_prev {
                if flag_fixed_s15 {
                    SCP_AR_L_UNIT
                } else {
                    SCP_AR_L_OTHER
                }
            } else {
                if flag_fixed_s15 {
                    SCP_AR_N_UNIT
                } else {
                    SCP_AR_N_OTHER
                }
            };
            let arstat_other = if sidesel.logo_mode_next {
                if flag_other_s15 {
                    SCP_AR_L_UNIT
                } else {
                    SCP_AR_L_OTHER
                }
            } else {
                if flag_other_s15 {
                    SCP_AR_N_UNIT
                } else {
                    SCP_AR_N_OTHER
                }
            };
            ds.set_scp_arstat(sidesel.nsc_detect, arstat_detect);
            ds.set_scp_arstat(sidesel.nsc_other, arstat_other);
        }
    }
    det
}

/// setCMFormEdgeSetBoth
pub fn set_cm_form_edge_set_both(
    ds: &mut Dataset,
    cnv: &Cnv,
    rnsc_detect: (i32, i32),
    rnsc_scope: (i32, i32),
    level: i32,
    logo_1st: bool,
) -> bool {
    let rmsec_detect = range_msec_of(ds, rnsc_detect);
    let rmsec_scope = range_msec_of(ds, rnsc_scope);
    let msec_dif_st = (rmsec_detect.0 - rmsec_scope.0).abs();
    let msec_dif_ed = (rmsec_detect.1 - rmsec_scope.1).abs();
    let msec_dif_detect = (rmsec_detect.0 - rmsec_detect.1).abs();
    if rnsc_detect.0 < 0
        || rnsc_detect.1 < 0
        || rnsc_detect.0 >= rnsc_detect.1
        || (rnsc_detect.0 <= rnsc_scope.0 && rnsc_scope.0 >= 0)
        || (rnsc_detect.1 >= rnsc_scope.1 && rnsc_scope.1 >= 0)
        || (msec_dif_st <= ds.msec_val_lap2 && rnsc_scope.0 >= 0)
        || (msec_dif_ed <= ds.msec_val_lap2 && rnsc_scope.1 >= 0)
    {
        return false;
    }
    let mut det = false;
    if level <= 2 {
        let mut calcmod = CalcModInfo::default();
        let tmpdet = calc_dif_mod0515(ds, &mut calcmod, msec_dif_detect);
        if tmpdet >= 2 || (level == 1 && tmpdet >= 1) {
            det = true;
        }
    } else {
        let mut calc1 = CalcDifInfo::default();
        calc_dif_select(ds, cnv, &mut calc1, 0, msec_dif_detect);
        if calc1.gap <= ds.msec_val_exact || level == 3 {
            det = true;
        }
    }
    if det {
        let nsc_end = rnsc_scope.1;
        let flag_st_s15 = is_cm_length_msec(cnv, msec_dif_st);
        let flag_ed_s15 = is_cm_length_msec(cnv, msec_dif_ed);
        let flag_detect_s15 = is_cm_length_msec(cnv, msec_dif_detect);
        let mut logo_next = true;
        if nsc_end >= 0 {
            logo_next = is_scp_ar_type_logo(ds.scp_arstat(nsc_end));
        }
        let arstat_c1 = if logo_1st {
            if flag_st_s15 {
                SCP_AR_L_UNIT
            } else {
                SCP_AR_L_OTHER
            }
        } else {
            if flag_st_s15 {
                SCP_AR_N_UNIT
            } else {
                SCP_AR_N_OTHER
            }
        };
        let arstat_c2 = if flag_detect_s15 {
            SCP_AR_N_UNIT
        } else {
            SCP_AR_N_OTHER
        };
        let arstat_c3 = if logo_next {
            if flag_ed_s15 {
                SCP_AR_L_UNIT
            } else {
                SCP_AR_L_OTHER
            }
        } else {
            if flag_ed_s15 {
                SCP_AR_N_UNIT
            } else {
                SCP_AR_N_OTHER
            }
        };
        ds.set_scp_chap(rnsc_detect.0, SCP_CHAP_DFIX);
        ds.set_scp_chap(rnsc_detect.1, SCP_CHAP_DFIX);
        ds.set_scp_arstat(rnsc_detect.0, arstat_c1);
        ds.set_scp_arstat(rnsc_detect.1, arstat_c2);
        if nsc_end >= 0 {
            ds.set_scp_arstat(nsc_end, arstat_c3);
        }
    }
    det
}

/// setCMFormEdge
pub fn set_cm_form_edge(
    ds: &mut Dataset,
    cnv: &Cnv,
    bounds: &mut (i32, i32),
    cmscope: WideMsecX,
    logo_1st: bool,
) -> bool {
    let lvlogo = ds.level_use_logo();
    let chap_cmp = if lvlogo < 7 {
        SCP_CHAP_CPOSIT
    } else {
        SCP_CHAP_NONE
    };
    let (valid_st, mut sidest) =
        set_cm_form_edge_side_info(ds, bounds.0, cmscope.st, chap_cmp, false);
    let (valid_ed, mut sideed) =
        set_cm_form_edge_side_info(ds, bounds.1, cmscope.ed, chap_cmp, true);
    let mut level = if lvlogo <= 4 {
        0
    } else if lvlogo < 7 {
        1
    } else if lvlogo == 7 {
        2
    } else {
        3
    };
    let mut change_st = false;
    let mut change_ed = false;
    if valid_st && sidest.nsc_fixed >= 0 {
        sidest.logo_mode_prev = logo_1st;
        change_st = set_cm_form_edge_set_side(ds, cnv, &mut sidest, level);
    }
    if valid_ed && sideed.nsc_fixed >= 0 {
        change_ed = set_cm_form_edge_set_side(ds, cnv, &mut sideed, level);
    }
    if valid_st && valid_ed && sidest.nsc_fixed < 0 && sideed.nsc_fixed < 0 {
        if lvlogo == 5 {
            level = 0;
        }
        let rnsc_detect = (sidest.nsc_detect, sideed.nsc_detect);
        let rnsc_scope = (
            ds.nsc_prev_scp_chap(sideed.nsc_detect, SCP_CHAP_DECIDE),
            ds.nsc_next_scp_chap(sidest.nsc_detect, SCP_CHAP_DECIDE),
        );
        change_st = set_cm_form_edge_set_both(ds, cnv, rnsc_detect, rnsc_scope, level, logo_1st);
        change_ed = change_st;
    }
    if change_st {
        bounds.0 = ds.msec_scp(sidest.nsc_detect);
    }
    if change_ed {
        bounds.1 = ds.msec_scp(sideed.nsc_detect);
    }
    change_st || change_ed
}

/// mkRangeInterLogo
pub fn mk_range_inter_logo(
    ds: &mut Dataset,
    cnv: &Cnv,
    nscbounds: (i32, i32),
    cm_inter: bool,
) -> bool {
    let bounds = range_msec_of(ds, nscbounds);
    let scope_logo = RangeFixMsec {
        st: bounds.0,
        ed: bounds.1,
        fix_st: bounds.0 >= 0,
        fix_ed: bounds.1 >= 0,
    };
    let mut rnsc_fix = (-1i32, -1i32);
    let mut chap_bakst = SCP_CHAP_NONE;
    let mut chap_baked = SCP_CHAP_NONE;
    if scope_logo.fix_st {
        rnsc_fix.0 = ds.nsc_from_msec_all(scope_logo.st);
        if rnsc_fix.0 > 0 {
            chap_bakst = ds.scp_chap(rnsc_fix.0);
        }
    }
    if scope_logo.fix_ed {
        rnsc_fix.1 = ds.nsc_from_msec_all(scope_logo.ed);
        if rnsc_fix.1 > 0 {
            chap_baked = ds.scp_chap(rnsc_fix.1);
        }
    }
    set_score(ds, cnv, scope_logo.st, scope_logo.ed);
    set_chap(ds, cnv, scope_logo);
    if scope_logo.fix_st && is_scp_chap_type_decide(chap_bakst) {
        ds.set_scp_chap(rnsc_fix.0, chap_bakst);
    }
    if scope_logo.fix_ed && is_scp_chap_type_decide(chap_baked) {
        ds.set_scp_chap(rnsc_fix.1, chap_baked);
    }
    let mut nsc_to_ar = nscbounds.1;
    if nsc_to_ar < 0 {
        nsc_to_ar = ds.size_scp() - 1;
    }
    ds.set_scp_arstat(nsc_to_ar, SCP_AR_L_OTHER);
    set_inter_logo(ds, cnv, nscbounds, cm_inter)
}

/// setInterLogo
pub fn set_inter_logo(ds: &mut Dataset, cnv: &Cnv, nscbounds: (i32, i32), cm_inter: bool) -> bool {
    let rev_del_mid = ds.get_config_action(1) != 0; // LogoDelMid
    let mut det = false;
    if rev_del_mid && cm_inter {
        let bounds = range_msec_of(ds, nscbounds);
        let cmscope = WideMsecX {
            st: WideMsec {
                just: bounds.0,
                early: bounds.0,
                late: bounds.0,
            },
            ed: WideMsec {
                just: bounds.1,
                early: bounds.1,
                late: bounds.1,
            },
            fix_st: true,
            fix_ed: true,
            logomode: true,
        };
        let (det_tmp, _b) = set_cm_form(ds, cnv, cmscope, true, true);
        det = det_tmp;
    }
    if !det {
        det = set_interpolar(ds, cnv, nscbounds, true);
    } else {
        let num_scpos = ds.size_scp();
        let mut nscope = nscbounds;
        if nscope.0 < 0 {
            nscope.0 = ds.nsc_next_scp_chap(0, SCP_CHAP_DECIDE);
        }
        if nscope.1 < 0 {
            nscope.1 = ds.nsc_prev_scp_chap(num_scpos - 1, SCP_CHAP_DECIDE);
        }
        let mut cmterm = (-1i32, -1i32);
        let mut count = 0i32;
        let mut nsc_fin = nscope.0;
        let mut nsc_cur = nscope.0;
        while nsc_cur >= nscope.0 && nsc_cur < nscope.1 && nsc_cur >= 0 {
            let nsc_last = nsc_cur;
            nsc_cur = ds.nsc_next_scp_chap(nsc_cur, SCP_CHAP_DECIDE);
            if nsc_cur >= nscope.0 && nsc_cur <= nscope.1 {
                let mut det15s = ds.scp_arstat(nsc_cur) == SCP_AR_L_UNIT;
                let msec_last = ds.msec_scp(nsc_last);
                let msec_cur = ds.msec_scp(nsc_cur);
                let mut calc1 = CalcDifInfo::default();
                let type1 = calc_dif_select(ds, cnv, &mut calc1, msec_last, msec_cur);
                if calc1.sec > 31 {
                    det15s = false;
                }
                let shortcm = nsc_fin != nscope.0;
                if det15s && type1 == 2 {
                    if count == 0 {
                        cmterm.0 = nsc_last;
                    }
                    cmterm.1 = nsc_cur;
                    count += 1;
                } else {
                    if count > 0 {
                        if cmterm.0 > nscope.0 && rev_del_mid && count >= 2 {
                            let msec_st = ds.msec_scp(cmterm.0);
                            let msec_ed = ds.msec_scp(cmterm.1);
                            let mut calc2 = CalcDifInfo::default();
                            calc_dif_select(ds, cnv, &mut calc2, msec_st, msec_ed);
                            if calc2.sec % 30 == 0 {
                                let (u, _d) =
                                    set_inter_logo_update(ds, cnv, nsc_fin, cmterm, shortcm);
                                if u {
                                    det = true;
                                }
                            }
                        }
                        nsc_fin = cmterm.1;
                        count = 0;
                    }
                }
                if nsc_cur == nscope.1 {
                    cmterm = (nsc_cur, nsc_cur);
                    let (det_tmp, _d) = set_inter_logo_update(ds, cnv, nsc_fin, cmterm, shortcm);
                    if det_tmp {
                        det = true;
                    }
                }
            }
        }
    }
    det
}

/// setInterLogoUpdate: (det, msec_stpoint ignored output)
pub fn set_inter_logo_update(
    ds: &mut Dataset,
    cnv: &Cnv,
    nsc_fin: i32,
    cmterm: (i32, i32),
    shortcm: bool,
) -> (bool, i32) {
    let mut det = false;
    if nsc_fin >= 0 && nsc_fin < cmterm.0 {
        let mut nsc_cur = ds.nsc_next_scp_chap(nsc_fin, SCP_CHAP_DECIDE);
        while nsc_cur > 0 && nsc_cur < cmterm.0 {
            ds.set_scp_chap(nsc_cur, SCP_CHAP_CDET);
            nsc_cur = ds.nsc_next_scp_chap(nsc_cur, SCP_CHAP_DECIDE);
        }
        if shortcm {
            let msec_fin = ds.msec_scp(nsc_fin);
            let msec_st = ds.msec_scp(cmterm.0);
            if (msec_st - msec_fin).abs() < 15 * 1000 - ds.msec_val_lap2 {
                ds.set_scp_arstat(cmterm.0, SCP_AR_N_OTHER);
            }
        }
        det = set_interpolar(ds, cnv, (nsc_fin, cmterm.0), true);
    }
    if cmterm.0 >= 0 && cmterm.0 < cmterm.1 {
        det = true;
        let mut nsc_cur = ds.nsc_next_scp_chap(cmterm.0, SCP_CHAP_DECIDE);
        while nsc_cur > 0 && nsc_cur <= cmterm.1 {
            ds.set_scp_arstat(nsc_cur, SCP_AR_N_UNIT);
            nsc_cur = ds.nsc_next_scp_chap(nsc_cur, SCP_CHAP_DECIDE);
        }
    }
    (det, -1)
}

/// setInterMultiCM
pub fn set_inter_multi_cm(
    ds: &mut Dataset,
    cnv: &Cnv,
    nscbounds: (i32, i32),
    interfill: i32,
) -> bool {
    let msec_rev_logo = ds.get_config(ConfigVarType::msecWLogoRevMin);
    let rev_logo = ds.get_config_action(4); // LogoUCRemain
    let rev_add = rev_logo > 0;
    let mut upcnt = 0i32;
    let mut nsc_cur = nscbounds.0;
    while nsc_cur >= nscbounds.0 && nsc_cur <= nscbounds.1 && nsc_cur >= 0 {
        let nsc_last = nsc_cur;
        nsc_cur = ds.nsc_next_scp_chap(nsc_cur, SCP_CHAP_DECIDE);
        if nsc_cur >= nscbounds.0 && nsc_cur <= nscbounds.1 {
            let mut logomode = false;
            let mut check_inter = interfill > 0;
            let arstat_cur = ds.scp_arstat(nsc_cur);
            if is_scp_ar_type_logo(arstat_cur) {
                logomode = true;
                if interfill > 0 {
                    let msec_last = ds.msec_scp(nsc_last);
                    let msec_cur = ds.msec_scp(nsc_cur);
                    let msec_dif = msec_cur - msec_last;
                    if !rev_add || msec_dif <= msec_rev_logo || interfill == 2 {
                        ds.set_scp_arstat(nsc_cur, SCP_AR_N_OTHER);
                        logomode = false;
                    }
                }
            } else {
                check_inter = true;
            }
            if check_inter {
                let update = set_interpolar(ds, cnv, (nsc_last, nsc_cur), logomode);
                if update {
                    upcnt += 1;
                }
            }
        } else {
            break;
        }
    }
    upcnt > 0
}

/// setInterpolar
pub fn set_interpolar(
    ds: &mut Dataset,
    cnv: &Cnv,
    rnsc_target: (i32, i32),
    logomode: bool,
) -> bool {
    let mut target = RangeNscMsec {
        nsc: rnsc_target,
        msec: (0, 0),
    };
    let mut nside = 3i32;
    if target.nsc.0 < 0 {
        target.nsc.0 = 0;
        nside &= 0x2;
    }
    if target.nsc.1 < 0 {
        target.nsc.1 = ds.size_scp() - 1;
        nside &= 0x1;
    }
    if nside == 0 {
        return false;
    }
    for i in target.nsc.0 + 1..target.nsc.1 {
        set_interpolar_clear_chap(ds, i);
    }
    target.msec = range_msec_of(ds, target.nsc);
    let mut upcnt = 0i32;
    let nsc_fix;
    {
        let mut trace = TraceInterpolar {
            keep_type: 0,
            keep_gap: 0,
            nsc_keep: -1,
            nsc_base: target.nsc.0,
            keep15s: false,
        };
        for i in target.nsc.0 + 1..=target.nsc.1 {
            if (!ds.scp_still(i) || i == target.nsc.1)
                && set_interpolar_detect(ds, cnv, &mut trace, i, &target, logomode, &mut nside)
            {
                upcnt += 1;
            }
        }
        nsc_fix = trace.nsc_base;
    }
    {
        let mut trace = TraceInterpolar {
            keep_type: 0,
            keep_gap: 0,
            nsc_keep: -1,
            nsc_base: target.nsc.1,
            keep15s: false,
        };
        let mut i = target.nsc.1 - 1;
        while i >= nsc_fix {
            if (!ds.scp_still(i) || i == nsc_fix)
                && set_interpolar_detect(ds, cnv, &mut trace, i, &target, logomode, &mut nside)
            {
                upcnt += 1;
            }
            i -= 1;
        }
    }
    if set_interpolar_extra(ds, cnv, &target, logomode) {
        upcnt += 1;
    }
    upcnt > 0
}

/// setInterpolarClearChap
pub fn set_interpolar_clear_chap(ds: &mut Dataset, nsc_n: i32) {
    let num_scpos = ds.size_scp();
    if nsc_n <= 0 || nsc_n >= num_scpos - 1 {
        return;
    }
    let chap_n = ds.scp_chap(nsc_n);
    if chap_n >= SCP_CHAP_DECIDE {
        let chap_set = if chap_n >= SCP_CHAP_DFIX {
            SCP_CHAP_CDET
        } else {
            SCP_CHAP_NONE
        };
        ds.set_scp_chap(nsc_n, chap_set);
    }
}

/// setInterpolarDetect
pub fn set_interpolar_detect(
    ds: &mut Dataset,
    cnv: &Cnv,
    trace: &mut TraceInterpolar,
    nsc_cur: i32,
    target: &RangeNscMsec,
    logomode: bool,
    nside_in: &mut i32,
) -> bool {
    let num_scpos = ds.size_scp();
    let mut update = false;
    let msec_val_lap2 = ds.msec_val_lap2;
    let msec_val_near2 = ds.msec_val_near2;
    let msec_cur = ds.msec_scp(nsc_cur);
    let mut msec_base = ds.msec_scp(trace.nsc_base);
    let msec_mid = (target.msec.0 + target.msec.1) / 2;
    let flag_last = nsc_cur == target.nsc.0 || nsc_cur == target.nsc.1;

    if nsc_cur != trace.nsc_base
        && nsc_cur != target.nsc.0
        && nsc_cur != target.nsc.1
        && ((msec_cur - msec_base).abs() < msec_val_lap2
            || (msec_cur - target.msec.0).abs() < msec_val_lap2
            || (msec_cur - target.msec.1).abs() < msec_val_lap2)
    {
        return false;
    }
    let mgn_cm_detect = ds.get_config(ConfigVarType::msecMgnCmDetect);
    let mgn_cm_divide = ds.get_config(ConfigVarType::msecMgnCmDivide);
    let mut msec_mgn = mgn_cm_detect;
    if mgn_cm_divide < mgn_cm_detect
        && (target.msec.1 - target.msec.0).abs() <= 15 * 1000 + msec_val_near2
    {
        msec_mgn = mgn_cm_divide;
    }
    let mut flag_curmgn = false;
    {
        let mut calc_st = CalcDifInfo::default();
        let mut calc_ed = CalcDifInfo::default();
        calc_dif_select(ds, cnv, &mut calc_st, target.msec.0, msec_cur);
        calc_dif_select(ds, cnv, &mut calc_ed, msec_cur, target.msec.1);
        if calc_st.sec % 5 == 0
            && calc_st.gap <= msec_val_near2
            && calc_ed.sec % 5 == 0
            && calc_ed.gap <= msec_val_near2
        {
            flag_curmgn = true;
        }
    }
    let mut msec_curmgn = msec_mgn;
    if flag_curmgn && msec_curmgn < msec_val_near2 {
        msec_curmgn = msec_val_near2;
    }

    let mut calc1 = CalcDifInfo::default();
    let mut type1 = calc_dif_select(ds, cnv, &mut calc1, msec_base, msec_cur);
    if calc1.gap > msec_curmgn {
        type1 = 0;
    }
    // previous kept candidate finalize
    if trace.nsc_keep > 0 {
        let msec_keep = ds.msec_scp(trace.nsc_keep);
        let msec_dif_keepcur = (msec_cur - msec_keep).abs();
        let msec_dif_keepbase = (msec_base - msec_keep).abs();
        if msec_dif_keepcur > msec_val_lap2 || flag_last {
            let mut calc2 = CalcDifInfo::default();
            let mut type2 = 0i32;
            if trace.keep_type == 2 || flag_last {
                type2 = calc_dif_select(ds, cnv, &mut calc2, msec_keep, msec_cur);
            }
            let mut flag_keepsteady = false;
            {
                let mut cs = CalcDifInfo::default();
                let mut ce = CalcDifInfo::default();
                calc_dif_select(ds, cnv, &mut cs, target.msec.0, msec_keep);
                calc_dif_select(ds, cnv, &mut ce, msec_keep, target.msec.1);
                if cs.sec % 5 == 0
                    && cs.gap <= msec_val_near2
                    && ce.sec % 5 == 0
                    && ce.gap <= msec_val_near2
                {
                    flag_keepsteady = true;
                }
            }
            let mut flag_mute = false;
            if (trace.nsc_base - trace.nsc_keep).abs() > 1 {
                let rnsc = if trace.nsc_base > trace.nsc_keep {
                    (trace.nsc_keep, trace.nsc_base)
                } else {
                    (trace.nsc_base, trace.nsc_keep)
                };
                let type_mute = check_mute_many(ds, rnsc.0, rnsc.1);
                if type_mute >= 2
                    || (type_mute == 1
                        && (msec_dif_keepbase >= 29500 || !trace.keep15s && !flag_keepsteady))
                {
                    flag_mute = true;
                }
                if ds.get_config(ConfigVarType::cancelCntSc) != 0 {
                    flag_mute = false;
                }
            }
            if flag_mute
                || trace.keep_type == 2 && (type1 == 0 || type2 == 0 || calc1.sec <= 30)
                || flag_last
                    && type2 == 0
                    && (msec_dif_keepcur + msec_val_near2 < msec_dif_keepbase
                        || msec_dif_keepcur < 14500)
            {
            } else {
                ds.set_scp_chap(trace.nsc_keep, SCP_CHAP_DINT);
                let arstat_new = if logomode {
                    SCP_AR_L_OTHER
                } else {
                    SCP_AR_N_OTHER
                };
                ds.set_scp_arstat(trace.nsc_keep, arstat_new);
                update = true;
                trace.nsc_base = trace.nsc_keep;
                msec_base = ds.msec_scp(trace.nsc_base);
                trace.nsc_keep = -1;
                type1 = calc_dif_select(ds, cnv, &mut calc1, msec_base, msec_cur);
                if calc1.gap > msec_curmgn {
                    type1 = 0;
                }
            }
        }
    }
    // candidate placement
    let flag_decr = nsc_cur < trace.nsc_base;
    let (msec_st, msec_ed, enable_nside, enable_mid);
    let mut nside = *nside_in;
    if !flag_decr {
        msec_st = target.msec.0;
        msec_ed = target.msec.1;
        enable_nside = (nside & 0x01) != 0;
        enable_mid = msec_cur < msec_mid + msec_val_near2;
    } else {
        msec_st = target.msec.1;
        msec_ed = target.msec.0;
        enable_nside = (nside & 0x02) != 0;
        enable_mid = msec_cur > msec_mid - msec_val_near2;
    }
    let sec30_cur_st = (msec_cur - msec_st).abs() <= 30 * 1000 + msec_val_near2;
    let sec30_cur_ed = (msec_cur - msec_ed).abs() <= 30 * 1000 + msec_val_near2;
    let mut enable_from = (msec_cur - target.msec.0).abs() > msec_val_lap2;
    let mut enable_to = (msec_cur - target.msec.1).abs() > msec_val_lap2;
    let mut enable_base = (msec_cur - msec_base).abs() > msec_val_lap2;
    if target.nsc.0 == 0 {
        nside &= 0x2;
        enable_from = true;
        enable_base = true;
    } else if target.nsc.1 == num_scpos - 1 {
        nside &= 0x1;
        enable_to = true;
        enable_base = true;
    }
    if enable_from && enable_to && enable_base {
        {
            let mut calcsub = CalcDifInfo::default();
            let typesub = calc_dif_select(ds, cnv, &mut calcsub, msec_cur, msec_st);
            if typesub > 0 && (type1 == 0 || (type1 <= typesub && calc1.gap > calcsub.gap)) {
                type1 = typesub;
                calc1 = calcsub;
            }
        }
        if (enable_mid || trace.nsc_keep >= 0) && (sec30_cur_st || type1 > 1) {
            let mut type_upkeep = 0i32;
            let mut calc2 = CalcDifInfo::default();
            let mut type2 = calc_dif_select(ds, cnv, &mut calc2, msec_cur, msec_ed);
            if calc2.gap > msec_curmgn {
                type2 = 0;
            }
            if sec30_cur_ed && nside == 3 && !flag_decr {
                if (trace.keep_gap > calc1.gap || trace.nsc_keep < 0) && type2 > 0 && type1 > 0 {
                    type_upkeep = 1;
                }
            } else if enable_nside && type1 > 0 {
                if trace.keep_gap > calc1.gap || trace.nsc_keep < 0 {
                    type_upkeep = 1;
                }
            } else if enable_nside
                && trace.nsc_keep < 0
                && calc1.sec <= 15
                && calc1.gap <= msec_val_near2
            {
                type_upkeep = 2;
            }
            let mut flag_det15s = false;
            if type_upkeep > 0 {
                let step = if flag_decr { -1 } else { 1 };
                let mut cnt_15s = 0i32;
                for i in -4..=4i32 {
                    let msec_tmp = msec_cur + step * i * 15000;
                    if i != 0
                        && msec_tmp >= target.msec.0 - msec_val_near2
                        && msec_tmp <= target.msec.1 + msec_val_near2
                    {
                        let chap_cmp = if i < 0 {
                            SCP_CHAP_DECIDE
                        } else {
                            SCP_CHAP_NONE
                        };
                        let nsc_tmp = ds.nsc_from_msec_chap(msec_tmp, msec_val_near2, chap_cmp);
                        if nsc_tmp > 0 {
                            cnt_15s += 1;
                        }
                    }
                }
                if cnt_15s >= 2 {
                    flag_det15s = true;
                }
            }
            if type_upkeep > 0 {
                trace.nsc_keep = nsc_cur;
                trace.keep_gap = calc1.gap;
                trace.keep_type = type_upkeep;
                trace.keep15s = flag_det15s;
            }
        }
    }
    let _ = (msec_st, msec_ed);
    update
}

/// setInterpolarExtra
pub fn set_interpolar_extra(
    ds: &mut Dataset,
    cnv: &Cnv,
    target: &RangeNscMsec,
    logomode: bool,
) -> bool {
    let num_scpos = ds.size_scp();
    let mut mode = if logomode { 1 } else { 0 };
    if !logomode
        && target.nsc.0 > 0
        && target.nsc.1 < num_scpos - 1
        && ds.scp_arstat(target.nsc.1) == SCP_AR_N_UNIT
    {
        let msec_dif = target.msec.1 - target.msec.0;
        let secdif = cnv.get_sec_from_msec(msec_dif);
        if secdif == 15 || secdif == 30 {
            mode = 2;
        } else {
            mode = 3;
        }
    }
    let mut update = false;
    if mode == 1 {
        for i in target.nsc.0 + 1..=target.nsc.1 {
            let mut flag_search = true;
            let msec_i = ds.msec_scp(i);
            if (msec_i - target.msec.0).abs() <= 89 * 1000
                || ((msec_i - target.msec.1).abs() <= ds.msec_val_lap2 && i != target.nsc.1)
            {
                flag_search = false;
            }
            if flag_search {
                let nsc_tmp = ds.nsc_from_msec_chap(msec_i, ds.msec_val_lap2, SCP_CHAP_DECIDE);
                if nsc_tmp >= 0 && nsc_tmp != i {
                    flag_search = false;
                }
            }
            if flag_search {
                let mut nsc_pts =
                    ds.nsc_from_msec_chap(msec_i - 90000, ds.msec_val_lap2, SCP_CHAP_DECIDE);
                if nsc_pts < 0 {
                    nsc_pts =
                        ds.nsc_from_msec_chap(msec_i - 90000, ds.msec_val_lap2, SCP_CHAP_NONE);
                }
                if nsc_pts > 0 && (i - nsc_pts) > 2 {
                    nsc_pts = -1;
                } else if nsc_pts > 0 && (i - nsc_pts) == 2 {
                    let nsc_tmp = i - 1;
                    let mut flag_dist = false;
                    {
                        const MSEC_CM90S_SAME: i32 = 4500;
                        let msec_pts = ds.msec_scp(nsc_pts);
                        let msec_tmp = ds.msec_scp(nsc_tmp);
                        if (msec_tmp - msec_pts).abs() < MSEC_CM90S_SAME
                            || (msec_tmp - msec_i).abs() < MSEC_CM90S_SAME
                        {
                            flag_dist = true;
                        }
                    }
                    if is_scp_chap_type_decide(ds.scp_chap(nsc_tmp))
                        || (!ds.is_smute_same_area(nsc_tmp, nsc_pts)
                            && !ds.is_smute_same_area(nsc_tmp, i)
                            && !flag_dist)
                    {
                        nsc_pts = -1;
                    }
                }
                if nsc_pts > 0 {
                    let nsc_pte = i;
                    let msec_pts = ds.msec_scp(nsc_pts);
                    let msec_pte = msec_i;
                    let sec_dif = cnv.get_sec_from_msec((msec_pte - msec_pts).abs());
                    if sec_dif == 90 {
                        let chap_pts = ds.scp_chap(nsc_pts);
                        let chap_pte = ds.scp_chap(nsc_pte);
                        if chap_pts < SCP_CHAP_DECIDE {
                            ds.set_scp_chap(nsc_pts, SCP_CHAP_DINT);
                            ds.set_scp_arstat(nsc_pts, SCP_AR_L_OTHER);
                            update = true;
                        }
                        if chap_pte < SCP_CHAP_DECIDE {
                            ds.set_scp_chap(nsc_pte, SCP_CHAP_DINT);
                            ds.set_scp_arstat(nsc_pte, SCP_AR_L_OTHER);
                            update = true;
                        }
                        let nsc_a05 = ds.nsc_from_msec_chap(
                            msec_pte + 5000,
                            ds.msec_val_near1,
                            SCP_CHAP_NONE,
                        );
                        let nsc_a10 = ds.nsc_from_msec_chap(
                            msec_pte + 10000,
                            ds.msec_val_near1,
                            SCP_CHAP_NONE,
                        );
                        let nsc_d05 = ds.nsc_from_msec_chap(msec_pte + 5000, 4500, SCP_CHAP_DECIDE);
                        let nsc_d10 =
                            ds.nsc_from_msec_chap(msec_pte + 10000, 4500, SCP_CHAP_DECIDE);
                        if nsc_d05 <= 0 && nsc_d10 <= 0 {
                            if nsc_a05 > 0 && msec_pte + 5000 < target.msec.1 - ds.msec_val_lap2 {
                                ds.set_scp_chap(nsc_a05, SCP_CHAP_DINT);
                                ds.set_scp_arstat(nsc_a05, SCP_AR_L_OTHER);
                                update = true;
                            } else if nsc_a10 > 0
                                && msec_pte + 10000 < target.msec.1 - ds.msec_val_lap2
                            {
                                ds.set_scp_chap(nsc_a10, SCP_CHAP_DINT);
                                ds.set_scp_arstat(nsc_a10, SCP_AR_L_OTHER);
                                update = true;
                            }
                        }
                    }
                }
            }
        }
    }
    if mode == 2 || mode == 3 {
        let mut flag_1st = true;
        for i in target.nsc.0 + 1..=target.nsc.1 {
            let msec_i = ds.msec_scp(i);
            let chap_i = ds.scp_chap(i);
            if chap_i >= SCP_CHAP_DECIDE {
                let msec_dif_target = (target.msec.1 - target.msec.0).abs();
                let mut msec_minst = if mode == 3 {
                    60 * 1000
                } else {
                    msec_dif_target / 4
                };
                let mut msec_mined = if mode == 3 {
                    60 * 1000
                } else {
                    msec_dif_target / 8
                };
                if msec_minst < ds.msec_val_lap2 {
                    msec_minst = ds.msec_val_lap2;
                }
                if msec_mined < ds.msec_val_lap2 {
                    msec_mined = ds.msec_val_lap2;
                }
                let mut flag_div = false;
                if (msec_i - target.msec.0).abs() > msec_minst
                    && (msec_i - target.msec.1).abs() > msec_mined
                {
                    flag_div = true;
                }
                if flag_div {
                    let mut calc_st = CalcDifInfo::default();
                    let mut calc_ed = CalcDifInfo::default();
                    calc_dif_select(ds, cnv, &mut calc_st, msec_i, target.msec.0);
                    calc_dif_select(ds, cnv, &mut calc_ed, msec_i, target.msec.1);
                    let msec_prm_gap = ds.get_config(ConfigVarType::msecMgnCmDivide);
                    if (calc_st.gap > msec_prm_gap || calc_ed.gap > msec_prm_gap)
                        && (calc_st.sec % 5 != 0
                            || calc_ed.sec % 5 != 0
                            || calc_st.gap >= ds.msec_val_near1
                            || calc_ed.gap >= ds.msec_val_near1)
                    {
                        flag_div = false;
                    }
                }
                if flag_div || i == target.nsc.1 {
                    if flag_1st && i < target.nsc.1 {
                        ds.set_scp_arstat(i, SCP_AR_N_BUNIT);
                        flag_1st = false;
                    } else if !flag_1st {
                        ds.set_scp_arstat(i, SCP_AR_N_AUNIT);
                    }
                } else {
                    ds.set_scp_chap(i, SCP_CHAP_CPOSQ);
                }
            }
        }
    }
    update
}
