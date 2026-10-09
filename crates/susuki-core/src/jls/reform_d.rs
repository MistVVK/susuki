//! Derived from join_logo_scp (https://github.com/tobitti0/join_logo_scp,
//! commit d1a24d758f8b3c04352b82ff6756fdf3d56729b3), Copyright (C) tobitti0,
//! GPL-2.0-or-later. This port is a GPL derivative work, redistributed here
//! under GPL-3.0-or-later (design §10.6).
//! JlsAutoReform Phase D: logo-based post corrections (addLogoComponent /
//! addLogoEdge(+Update) / addLogoMix / addLogoBorder(+Set/Sub/SubUpdate))
//! and the two-point target setter mkReformTarget (JlsAutoReform.cpp 75-168,
//! 426-1017).

use super::cnvstr::Cnv;
use super::dataset::*;
use super::dataset_search::*;
pub use super::reform_base::add_common_component_one;
use super::reform_base::*;
use super::reform_cm::{mk_range_cm, set_interpolar, WideMsecX};
use super::tables::ConfigVarType;

/// mkReformTarget
pub fn mk_reform_target(
    ds: &mut Dataset,
    cnv: &Cnv,
    nsc_from_in: i32,
    nsc_to_in: i32,
    logoon: bool,
    restruct: i32,
) {
    //--- fromを前側、toを後側 ---
    let (mut nsc_from, mut nsc_to) = if nsc_from_in > nsc_to_in {
        (nsc_to_in, nsc_from_in)
    } else {
        (nsc_from_in, nsc_to_in)
    };
    //--- 両方マイナスの時は設定なし ---
    if nsc_to < 0 {
        return;
    }
    let mut msec_to = -1i32;
    let mut msec_from = -1i32;
    //--- 前側位置の確定処理 ---
    if nsc_from >= 0 {
        msec_from = ds.msec_scp(nsc_from);
        let mut nsc_chapfrom = ds.nsc_next_scp_decide(nsc_from - 1, SCP_END_EDGEIN);
        let arstat_from = ds.scp_arstat(nsc_chapfrom);
        let arext_from = ds.scp_arext(nsc_chapfrom);
        ds.set_scp_chap(nsc_from, SCP_CHAP_DFORCE);
        ds.set_scp_arstat(nsc_from, arstat_from);
        ds.set_scp_arext(nsc_from, arext_from);
        while nsc_chapfrom >= 0 && nsc_chapfrom < nsc_to {
            if nsc_chapfrom > nsc_from {
                ds.set_scp_chap(nsc_chapfrom, SCP_CHAP_NONE);
            }
            nsc_chapfrom = ds.nsc_next_scp_decide(nsc_chapfrom, SCP_END_EDGEIN);
        }
    }
    //--- 後側位置の確定処理 ---
    if nsc_to >= 0 {
        msec_to = ds.msec_scp(nsc_to);
        let mut arstat_to = SCP_AR_L_OTHER;
        let mut sec_dif = 0i32;
        if nsc_from >= 0 {
            sec_dif = cnv.get_sec_from_msec(msec_to - msec_from);
        }
        if !logoon {
            if sec_dif > 0 && sec_dif <= 120 && sec_dif % 15 == 0 {
                arstat_to = SCP_AR_N_UNIT;
            } else {
                arstat_to = SCP_AR_N_OTHER;
            }
        }
        ds.set_scp_chap(nsc_to, SCP_CHAP_DFORCE);
        ds.set_scp_arstat(nsc_to, arstat_to);
    }
    //--- ２点間の構成再構築 ---
    if nsc_from >= 0 && nsc_to >= 0 && restruct > 0 {
        if !logoon {
            let mut scope_st = msec_from;
            let mut scope_ed = msec_to;
            let mut fix_st = true;
            let mut fix_ed = true;
            if nsc_from == 0 {
                fix_st = false;
                scope_st = -1;
            }
            if nsc_to == ds.size_scp() - 1 {
                fix_ed = false;
                scope_ed = -1;
            }
            let interfill = 2;
            let (_det, _b) = mk_range_cm(
                ds, cnv, scope_st, scope_ed, fix_st, fix_ed, logoon, interfill,
            );
        } else {
            set_interpolar(ds, cnv, (nsc_from, nsc_to), logoon);
        }
    }
    let _ = (&mut nsc_from, &mut nsc_to);
}

/// addLogoComponent (lvlogo unused by the body, as in the reference)
pub fn add_logo_component(ds: &mut Dataset, cnv: &Cnv, _lvlogo: i32) {
    add_logo_edge(ds, cnv);
    add_logo_border(ds, cnv);
    add_logo_mix(ds, cnv);
}

/// addLogoEdge
pub fn add_logo_edge(ds: &mut Dataset, cnv: &Cnv) {
    let msec_recmin = ds.get_config(ConfigVarType::msecWLogoCmMin);
    if ds.get_config_action(2) == 0 {
        // LogoDelWide == 0: no cut
        return;
    }
    let rev_del_edge = ds.get_config_action(0); // LogoDelEdge
    let mut logopt = NrfCurrent::default();
    let mut remain_logo = ds.nrf_pt_next(&mut logopt, LOGO_SELECT_VALID);
    while remain_logo {
        remain_logo = ds.nrf_pt_next(&mut logopt, LOGO_SELECT_VALID);
        let mut valid_last = false;
        if !remain_logo && logopt.msec_last_fall > 0 {
            let mut over = false;
            let mut elg = ElgCurrent::default();
            while ds.elg_pt_next(&mut elg) && !over {
                if elg.msec_fall > logopt.msec_last_fall {
                    over = true;
                    valid_last = true;
                    logopt.msec_rise = elg.msec_fall;
                }
            }
        }
        if (remain_logo || valid_last) && logopt.msec_rise - logopt.msec_last_fall >= msec_recmin {
            let mut area = WideMsecX {
                st: ds.wide_msec_logo_nrf(logopt.nrf_last_fall),
                ed: ds.wide_msec_logo_nrf(logopt.nrf_rise),
                fix_st: false,
                fix_ed: false,
                logomode: false,
            };
            if rev_del_edge == 0 {
                area.st.early = area.st.late - ds.msec_val_spc;
                area.ed.late = area.ed.early + ds.msec_val_spc;
                if area.st.just < area.st.early {
                    area.st.just = area.st.early;
                }
                if area.ed.just > area.ed.late {
                    area.ed.just = area.ed.late;
                }
            } else if rev_del_edge <= 2 {
                area.st.early = area.st.just - ds.msec_val_spc;
                area.ed.late = area.ed.just + ds.msec_val_spc;
            } else {
                area.st.early -= ds.msec_val_spc;
                area.ed.late += ds.msec_val_spc;
            }
            if valid_last {
                area.ed.just = logopt.msec_rise;
                area.ed.early = logopt.msec_rise;
                area.ed.late = logopt.msec_rise;
            }
            add_logo_edge_update(ds, cnv, area, msec_recmin);
        }
    }
}

/// addLogoEdgeUpdate
pub fn add_logo_edge_update(ds: &mut Dataset, cnv: &Cnv, area_nologo: WideMsecX, msec_recmin: i32) {
    let mut rnsc_target = (-1i32, -1i32);
    let mut rnsc_cmp = (-1i32, -1i32);
    {
        let mut area_search = area_nologo;
        area_search.st.late = area_nologo.ed.early;
        area_search.ed.early = area_nologo.st.late;
        rnsc_target.0 =
            ds.nsc_from_wide_msec_full(&area_search.st, SCP_CHAP_DECIDE, SCP_END_EDGEIN);
        rnsc_target.1 =
            ds.nsc_from_wide_msec_full(&area_search.ed, SCP_CHAP_DECIDE, SCP_END_EDGEIN);
        rnsc_cmp.0 = ds.nsc_from_wide_msec_full(&area_search.st, SCP_CHAP_NONE, SCP_END_EDGEIN);
        rnsc_cmp.1 = ds.nsc_from_wide_msec_full(&area_search.ed, SCP_CHAP_NONE, SCP_END_EDGEIN);
    }
    if rnsc_cmp.0 <= 0 || rnsc_cmp.1 <= 0 {
        return;
    }
    let mut rnsc_select = rnsc_target;
    if rnsc_target.0 != rnsc_cmp.0 {
        if rnsc_target.0 < 0 {
            rnsc_select.0 = rnsc_cmp.0;
        } else {
            let mut cont = true;
            let mut nsc_cur = rnsc_cmp.0;
            while cont && nsc_cur > 0 && nsc_cur < rnsc_target.0 {
                let msec_dif = ds.msec_scp(rnsc_target.0) - ds.msec_scp(nsc_cur);
                if is_length_logo_level(ds, cnv, msec_dif, true, true, false) {
                    rnsc_select.0 = nsc_cur;
                    cont = false;
                }
                nsc_cur = ds.nsc_next_scp_chap(nsc_cur, SCP_CHAP_NONE);
            }
            if rnsc_select.0 != rnsc_cmp.0 {
                let msec_dif = ds.msec_scp(rnsc_select.0) - ds.msec_scp(rnsc_cmp.0);
                if msec_dif >= msec_recmin {
                    rnsc_select.0 = rnsc_cmp.0;
                }
            }
        }
    }
    if rnsc_target.1 != rnsc_cmp.1 {
        if rnsc_target.1 < 0 {
            rnsc_select.1 = rnsc_cmp.1;
        } else {
            let mut cont = true;
            let mut nsc_cur = rnsc_cmp.1;
            while cont && nsc_cur > 0 && nsc_cur > rnsc_target.1 {
                let msec_dif = ds.msec_scp(nsc_cur) - ds.msec_scp(rnsc_target.1);
                if is_length_logo_level(ds, cnv, msec_dif, true, true, false) {
                    rnsc_select.1 = nsc_cur;
                    cont = false;
                }
                nsc_cur = ds.nsc_prev_scp_chap(nsc_cur, SCP_CHAP_NONE);
            }
            if rnsc_select.1 != rnsc_cmp.1 {
                let msec_dif = ds.msec_scp(rnsc_cmp.1) - ds.msec_scp(rnsc_select.1);
                if msec_dif >= msec_recmin {
                    rnsc_select.1 = rnsc_cmp.1;
                }
            }
        }
    }
    let rmsec_select = (ds.msec_scp(rnsc_select.0), ds.msec_scp(rnsc_select.1));
    if rmsec_select.1 - rmsec_select.0 >= msec_recmin {
        let mut elg = ElgCurrent::default();
        while ds.elg_pt_next(&mut elg) {
            let mut rnsc_del = rnsc_select;
            if rnsc_del.0 < elg.nsc_rise {
                rnsc_del.0 = elg.nsc_rise;
            }
            if rnsc_del.1 > elg.nsc_fall {
                rnsc_del.1 = elg.nsc_fall;
            }
            if rnsc_del.0 < rnsc_del.1 {
                let rmsec_del = (ds.msec_scp(rnsc_del.0), ds.msec_scp(rnsc_del.1));
                if (rmsec_del.1 - rmsec_del.0 >= msec_recmin)
                    || (rnsc_del.0 <= elg.nsc_rise && rnsc_del.1 >= elg.nsc_fall)
                {
                    let chap_st = ds.scp_chap(rnsc_del.0);
                    let chap_ed = ds.scp_chap(rnsc_del.1);
                    if chap_st < SCP_CHAP_DFIX {
                        ds.set_scp_chap(rnsc_del.0, SCP_CHAP_DFIX);
                        ds.set_scp_arstat(rnsc_del.0, SCP_AR_L_OTHER);
                    }
                    if chap_ed < SCP_CHAP_DFIX {
                        ds.set_scp_chap(rnsc_del.1, SCP_CHAP_DFIX);
                        ds.set_scp_arstat(rnsc_del.1, SCP_AR_N_OTHER);
                    }
                    let mut nsc_tmp = rnsc_del.0;
                    while nsc_tmp > 0 && nsc_tmp < rnsc_del.1 {
                        nsc_tmp = ds.nsc_next_scp_chap(nsc_tmp, SCP_CHAP_DECIDE);
                        if nsc_tmp > 0 && nsc_tmp <= rnsc_del.1 {
                            let arstat_tmp = ds.scp_arstat(nsc_tmp);
                            let mut arstat_new = arstat_tmp;
                            if arstat_tmp == SCP_AR_L_UNIT {
                                arstat_new = SCP_AR_N_UNIT;
                            } else if arstat_tmp == SCP_AR_L_OTHER {
                                arstat_new = SCP_AR_N_OTHER;
                            }
                            if arstat_tmp != arstat_new {
                                ds.set_scp_arstat(nsc_tmp, arstat_new);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// addLogoMix
pub fn add_logo_mix(ds: &mut Dataset, _cnv: &Cnv) {
    let mut logopt = NrfCurrent::default();
    let mut remain_logo = ds.nrf_pt_next(&mut logopt, LOGO_SELECT_VALID);
    if !remain_logo {
        return;
    }
    let mut nsc_last = ds.nsc_next_scp_chap(0, SCP_CHAP_DECIDE);
    while nsc_last > 0 {
        let nsc_cur = ds.nsc_next_scp_chap(nsc_last, SCP_CHAP_DECIDE);
        let msec_cur = ds.msec_scp(nsc_cur);
        let msec_last = ds.msec_scp(nsc_last);
        let mut sumlogo = 0i32;
        let mut cmpnext = true;
        while nsc_cur > 0
            && remain_logo
            && cmpnext
            && logopt.msec_rise < msec_cur - ds.msec_val_lap2
        {
            if remain_logo {
                if logopt.msec_fall > msec_last + ds.msec_val_lap2 {
                    sumlogo += 1;
                }
                if logopt.msec_fall > msec_cur - ds.msec_val_lap2 {
                    cmpnext = false;
                } else {
                    remain_logo = ds.nrf_pt_next(&mut logopt, LOGO_SELECT_VALID);
                }
            }
        }
        if sumlogo > 1 {
            let arstat_cur = ds.scp_arstat(nsc_cur);
            if arstat_cur == SCP_AR_L_OTHER {
                ds.set_scp_arstat(nsc_cur, SCP_AR_L_MIXED);
            }
        }
        nsc_last = nsc_cur;
    }
}

/// addLogoBorder
pub fn add_logo_border(ds: &mut Dataset, cnv: &Cnv) {
    let mut logopt = NrfCurrent::default();
    let mut remain_logo = ds.nrf_pt_next(&mut logopt, LOGO_SELECT_VALID);
    if !remain_logo {
        return;
    }
    let mut nsc_last = ds.nsc_next_scp_chap(0, SCP_CHAP_DECIDE);
    while nsc_last > 0 {
        let mut nsc_cur = ds.nsc_next_scp_chap(nsc_last, SCP_CHAP_DECIDE);
        let mut msec_cur = ds.msec_scp(nsc_cur);
        let mut msec_last = ds.msec_scp(nsc_last);
        let mut msec_res;
        let mut enable_st = true;
        let mut cmpnext = true;
        while nsc_cur > 0
            && remain_logo
            && cmpnext
            && logopt.msec_rise < msec_cur - ds.msec_val_lap2
        {
            if remain_logo {
                msec_res = add_logo_border_sub(
                    ds,
                    cnv,
                    (msec_last, msec_cur),
                    (logopt.nrf_rise, logopt.nrf_fall),
                    enable_st,
                );
                enable_st = msec_res >= 0;
                if msec_res > msec_last {
                    msec_last = msec_res;
                }
                if logopt.msec_fall > msec_cur - ds.msec_val_lap2 {
                    cmpnext = false;
                } else {
                    remain_logo = ds.nrf_pt_next(&mut logopt, LOGO_SELECT_VALID);
                }
            }
            if logopt.nrf_last_fall > 0 && cmpnext {
                msec_res = add_logo_border_sub(
                    ds,
                    cnv,
                    (msec_last, msec_cur),
                    (logopt.nrf_last_fall, logopt.nrf_rise),
                    enable_st,
                );
                enable_st = msec_res >= 0;
                if msec_res > msec_last {
                    msec_last = msec_res;
                }
            }
        }
        if nsc_cur > 0 {
            let mut back = ds.nsc_from_msec_all(msec_cur);
            if back > 0 && back < nsc_cur {
                back = nsc_cur;
            }
            nsc_last = back;
            msec_cur = ds.msec_scp(nsc_last);
            nsc_cur = if nsc_last >= ds.size_scp() - 1 {
                -1
            } else {
                ds.nsc_next_scp_chap(nsc_last, SCP_CHAP_DECIDE)
            };
        } else {
            nsc_last = nsc_cur;
        }
    }
    add_logo_border_set(ds, cnv);
}

/// addLogoBorderSet
pub fn add_logo_border_set(ds: &mut Dataset, cnv: &Cnv) {
    let mut nsc_last = ds.nsc_next_scp_chap(0, SCP_CHAP_DECIDE);
    let mut nsc_cur = ds.nsc_next_scp_chap(nsc_last, SCP_CHAP_DECIDE);
    while nsc_cur > 0 {
        let nsc_next = ds.nsc_next_scp_chap(nsc_cur, SCP_CHAP_DECIDE);
        if ds.scp_chap(nsc_cur) == SCP_CHAP_DBORDER {
            let arstat_cur = ds.scp_arstat(nsc_cur);
            if arstat_cur == SCP_AR_N_UNIT {
                ds.set_scp_arstat(nsc_cur, SCP_AR_B_UNIT);
            } else if arstat_cur == SCP_AR_N_OTHER {
                ds.set_scp_arstat(nsc_cur, SCP_AR_B_OTHER);
            } else if is_scp_ar_type_logo(arstat_cur) {
                add_common_component_one(ds, cnv, nsc_cur, nsc_last, false);
            }
            if nsc_next > 0 {
                let arstat_next = ds.scp_arstat(nsc_next);
                if arstat_next == SCP_AR_N_UNIT {
                    ds.set_scp_arstat(nsc_next, SCP_AR_B_UNIT);
                } else if arstat_next == SCP_AR_N_OTHER {
                    ds.set_scp_arstat(nsc_next, SCP_AR_B_OTHER);
                } else if is_scp_ar_type_logo(arstat_next) {
                    add_common_component_one(ds, cnv, nsc_cur, nsc_next, false);
                }
            }
        }
        nsc_last = nsc_cur;
        nsc_cur = nsc_next;
    }
}

/// addLogoBorderSub
pub fn add_logo_border_sub(
    ds: &mut Dataset,
    cnv: &Cnv,
    component: (i32, i32),
    rnrf_logo: (i32, i32),
    enable_st: bool,
) -> i32 {
    if rnrf_logo.0 < 0 {
        return -1;
    }
    let mut logo_targets = component;
    {
        let wmsec_logost = ds.wide_msec_logo_nrf(rnrf_logo.0);
        if wmsec_logost.early < component.0 + ds.msec_val_lap2 {
            if enable_st {
                return component.0;
            } else {
                return -1;
            }
        } else if wmsec_logost.late > component.1 - ds.msec_val_lap2 {
            return component.1;
        }
        logo_targets.0 = wmsec_logost.just;
        let nsc_inspos = ds.nsc_from_msec_all(wmsec_logost.just);
        if nsc_inspos >= 0 {
            logo_targets.0 = ds.msec_scp(nsc_inspos);
        }
    }
    if rnrf_logo.1 >= 0 {
        let wmsec_logoed = ds.wide_msec_logo_nrf(rnrf_logo.1);
        if wmsec_logoed.early < logo_targets.0 + ds.msec_val_lap2 {
            return component.0;
        } else if wmsec_logoed.late <= component.1 - ds.msec_val_lap2 {
            logo_targets.1 = wmsec_logoed.just;
        }
    }
    let rise_logost = rnrf_logo.0 % 2 == 0;
    add_logo_border_sub_update(ds, cnv, component, logo_targets, rise_logost, enable_st)
}

/// addLogoBorderSubUpdate
pub fn add_logo_border_sub_update(
    ds: &mut Dataset,
    cnv: &Cnv,
    component: (i32, i32),
    logo_targets: (i32, i32),
    rise_logost: bool,
    enable_st: bool,
) -> i32 {
    let mut msec_ret = -1i32;
    if logo_targets.0 <= 0 {
        return -1;
    }
    let mut unit15s_logost = false;
    let mut unit15s_logoed = false;
    let mut update_logost = 0i32;
    let mut update_logoed = 0i32;
    {
        let mut calc1 = CalcDifInfo::default();
        let mut calc2 = CalcDifInfo::default();
        let type1 = calc_dif_select(ds, cnv, &mut calc1, component.0, logo_targets.0);
        let type2 = calc_dif_select(ds, cnv, &mut calc2, logo_targets.0, logo_targets.1);
        let mut chknum1 = 0i32;
        let mut chknum2 = 0i32;
        for i in 0..2i32 {
            let (types, calcs) = if i == 0 {
                (type1, calc1)
            } else {
                (type2, calc2)
            };
            let lvlogo = ds.level_use_logo();
            if (types > 0 && calcs.sec % 15 == 0 && calcs.sec >= 15)
                || (types > 0 && calcs.sec % 5 == 0 && lvlogo >= 6)
                || (types > 0 && calcs.sec >= 3 && lvlogo >= 7)
            {
                if i == 0 {
                    chknum1 = 5;
                } else {
                    chknum2 = 5;
                }
            }
        }
        if chknum1 < 5 {
            if calc1.sec > 30 {
                chknum1 = 4;
            } else if type1 > 0 && calc1.sec > calc2.sec + 5 {
                chknum1 = 3;
            }
        }
        if chknum2 < 5 {
            if calc2.sec > 30 {
                chknum2 = 4;
            } else if type2 > 0 && calc2.sec > calc1.sec + 5 {
                chknum2 = 3;
            }
        }
        let nsc_ed = ds.nsc_from_msec_all(component.1);
        let arstat_ed = ds.scp_arstat(nsc_ed);
        let flag_arstat_logo = is_scp_ar_type_logo(arstat_ed);
        if chknum1 + chknum2 >= 7 {
            if flag_arstat_logo && rise_logost {
                if type1 > 0 && calc1.sec <= 60 && enable_st {
                    update_logost = 1;
                }
            } else if !flag_arstat_logo && !rise_logost {
                if type1 > 0 && calc1.sec <= 60 && enable_st {
                    update_logost = 2;
                }
            } else if flag_arstat_logo && !rise_logost {
                if type2 > 0 && calc2.sec <= 60 {
                    update_logoed = 1;
                }
            } else if !flag_arstat_logo && rise_logost && type2 > 0 && calc2.sec <= 60 {
                update_logoed = 2;
            }
        }
        if calc1.sec % 15 == 0 {
            unit15s_logost = true;
        }
        if calc2.sec % 15 == 0 {
            unit15s_logoed = true;
        }
    }
    if update_logost > 0 || update_logoed > 0 {
        let mut border_logost = false;
        let mut border_logoed = false;
        let mut nsc_logost = ds.nsc_from_msec_all(logo_targets.0);
        if nsc_logost < 0 {
            nsc_logost = ds.nsc_force_msec(cnv, logo_targets.0, EDGE_RISE);
            border_logost = true;
        }
        if update_logost > 0 || update_logoed > 0 {
            msec_ret = logo_targets.0;
            if border_logost {
                ds.set_scp_chap(nsc_logost, SCP_CHAP_DBORDER);
            } else if ds.scp_chap(nsc_logost) < SCP_CHAP_DINT {
                ds.set_scp_chap(nsc_logost, SCP_CHAP_DINT);
            }
            if update_logost == 1 {
                let arstat_st = if unit15s_logost {
                    SCP_AR_N_UNIT
                } else {
                    SCP_AR_N_OTHER
                };
                ds.set_scp_arstat(nsc_logost, arstat_st);
            } else if update_logost == 2 {
                let arstat_st = if unit15s_logost {
                    SCP_AR_L_UNIT
                } else {
                    SCP_AR_L_OTHER
                };
                ds.set_scp_arstat(nsc_logost, arstat_st);
            }
        }
        if update_logoed > 0 {
            msec_ret = logo_targets.1;
            let mut nsc_logoed = ds.nsc_from_msec_all(logo_targets.1);
            if nsc_logoed < 0 {
                nsc_logoed = ds.nsc_force_msec(cnv, logo_targets.1, EDGE_RISE);
                border_logoed = true;
            }
            if border_logoed {
                ds.set_scp_chap(nsc_logoed, SCP_CHAP_DBORDER);
            } else if ds.scp_chap(nsc_logoed) < SCP_CHAP_DINT {
                ds.set_scp_chap(nsc_logoed, SCP_CHAP_DINT);
            }
            if update_logoed == 1 {
                let arstat_st = if unit15s_logost {
                    SCP_AR_L_UNIT
                } else {
                    SCP_AR_L_OTHER
                };
                let arstat_ed = if unit15s_logoed {
                    SCP_AR_N_UNIT
                } else {
                    SCP_AR_N_OTHER
                };
                ds.set_scp_arstat(nsc_logost, arstat_st);
                ds.set_scp_arstat(nsc_logoed, arstat_ed);
            } else if update_logoed == 2 {
                let arstat_st = if unit15s_logost {
                    SCP_AR_N_UNIT
                } else {
                    SCP_AR_N_OTHER
                };
                let arstat_ed = if unit15s_logoed {
                    SCP_AR_L_UNIT
                } else {
                    SCP_AR_L_OTHER
                };
                ds.set_scp_arstat(nsc_logost, arstat_st);
                ds.set_scp_arstat(nsc_logoed, arstat_ed);
            }
        }
    }
    msec_ret
}
