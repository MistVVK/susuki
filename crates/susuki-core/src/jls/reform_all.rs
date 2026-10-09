//! Derived from join_logo_scp (https://github.com/tobitti0/join_logo_scp,
//! commit d1a24d758f8b3c04352b82ff6756fdf3d56729b3), Copyright (C) tobitti0,
//! GPL-2.0-or-later. This port is a GPL derivative work, redistributed here
//! under GPL-3.0-or-later (design §10.6).
//! JlsAutoReform Phases B/C boundary: end-area composition (setFirstArea/
//! setFirstAreaUpdate/setFinalArea), the no-logo CM estimation state
//! machine (detectCM* + setReformParam + mkReformAllNologo).
//! Mirrors JlsAutoReform.cpp 332-414, 1019-1471, 1643-2306.

use super::cmdarg::CmdArg;
use super::cnvstr::Cnv;
use super::dataset::*;
use super::dataset_search::*;
use super::reform_base::*;
use super::reform_cm::mk_range_inter_logo;
use super::reform_cm::{mk_range_cm, set_interpolar};
use super::reform_d::{add_common_component_one, add_logo_component};
use super::tables::OptType;
use super::tables::{CmdType, ConfigVarType};

pub const AUTOCM_ST_FIRST: usize = 0;
pub const AUTOCM_ST_LOGO: usize = 1;
pub const AUTOCM_ST_CM: usize = 2;
pub const AUTOCM_ST_DEFER: usize = 3;
pub const AUTOCM_ST_JUDGE: usize = 4;
pub const AUTOCM_NUM_SMAX: usize = 5;
pub const AUTOCM_T_INIT: i32 = 0;
pub const AUTOCM_T_SFT: i32 = 1;
pub const AUTOCM_T_MRG: i32 = 2;

pub const AUTOFIRST_LAX: i32 = 0;
pub const AUTOFIRST_STRICT: i32 = 1;
pub const AUTOFIRST_NOCUT: i32 = 2;

#[derive(Clone, Copy, Default)]
pub struct FormAutoCMArg {
    pub level_cm_det: i32,
    pub msec_tailarea_keep: i32,
    pub msec_tailarea_unit: i32,
    pub sec_maxcm: i32,
    pub rmsec_headtail: (i32, i32),
}

#[derive(Clone, Copy, Default)]
pub struct AutoCMCount {
    pub rnsc: (i32, i32),
    pub det: i32,
    pub sec: i32,
    pub d15: i32,
    pub d30: i32,
    pub dov15: i32,
}

#[derive(Clone, Copy, Default)]
pub struct FormFirstInfo {
    pub limit_head: i32,
    pub nsc_treat: i32,
}

#[derive(Clone, Copy, Default)]
pub struct FormFirstLoc {
    pub nsc_decide: i32,
    pub nsc_logo1st: i32,
    pub msec_decide: i32,
    pub msec_logo1st: i32,
    pub rnsc_logo1st: (i32, i32),
    pub lv_pos1st_sel: i32,
    pub msec1st_sel: i32,
    pub msec_end1st: i32,
    pub msec1st_zone: i32,
    pub msec_wcomp1st: i32,
    pub msec_lg_cut1st: i32,
}

/// setReformParam
pub fn set_reform_param(ds: &mut Dataset, cmdarg: &CmdArg, reform_all: bool) -> FormAutoCMArg {
    let mut level_cmdet = 6i32;
    let mut msec_tailarea_keep = 0i32;
    let mut msec_tailarea_unit = 0i32;
    let mut sec_maxcm = 60i32;
    let mut rmsec_headtail = if reform_all {
        (-1, -1)
    } else {
        (ds.rec_hold.rmsec_head, ds.rec_hold.rmsec_tail)
    };
    if rmsec_headtail.0 < 0 {
        rmsec_headtail.0 = 0;
    }
    if rmsec_headtail.1 < 0 {
        rmsec_headtail.1 = ds.msec_total_max;
    }
    if cmdarg.cmdsel == CmdType::AutoCM {
        let tmp = cmdarg.get_opt(OptType::AutopCode) % 100;
        if !reform_all || tmp > 0 {
            level_cmdet = tmp;
        }
        if level_cmdet > 0 {
            ds.set_flag_auto_mode(2); // setFlagAutoMode(true)
        }
        let sectmp_keep = cmdarg.get_opt(OptType::AutopScope);
        msec_tailarea_keep = 0;
        if sectmp_keep > 0 {
            msec_tailarea_keep = rmsec_headtail.1 - sectmp_keep * 1000;
        }
        let sectmp_unit = cmdarg.get_opt(OptType::AutopScopeX);
        msec_tailarea_unit = 0;
        if sectmp_unit > 0 {
            msec_tailarea_unit = rmsec_headtail.1 - sectmp_unit * 1000;
        }
        sec_maxcm = cmdarg.get_opt(OptType::AutopMaxPrd);
        if sec_maxcm <= 0 {
            sec_maxcm = 60;
        }
    }
    FormAutoCMArg {
        level_cm_det: level_cmdet,
        msec_tailarea_keep,
        msec_tailarea_unit,
        sec_maxcm,
        rmsec_headtail,
    }
}

/// detectCMCount
pub fn detect_cm_count(
    cntset: &mut [AutoCMCount; AUTOCM_NUM_SMAX],
    command: i32,
    st: usize,
    ed: usize,
) {
    if command == AUTOCM_T_INIT {
        let stnew = if st == AUTOCM_NUM_SMAX {
            AUTOCM_ST_FIRST
        } else {
            st
        };
        let ednew = if ed == AUTOCM_NUM_SMAX {
            AUTOCM_ST_JUDGE
        } else {
            ed
        };
        for cell in cntset[stnew..=ednew].iter_mut() {
            *cell = AutoCMCount::default();
        }
    } else if command == AUTOCM_T_SFT {
        for i in st..AUTOCM_NUM_SMAX {
            if i < AUTOCM_NUM_SMAX + st - ed {
                cntset[i] = cntset[i + ed - st];
            } else {
                cntset[i] = AutoCMCount::default();
            }
        }
    } else if command == AUTOCM_T_MRG {
        for i in st + 1..=ed {
            cntset[st].det += cntset[i].det;
            cntset[st].sec += cntset[i].sec;
            cntset[st].d15 += cntset[i].d15;
            cntset[st].d30 += cntset[i].d30;
            cntset[st].dov15 += cntset[i].dov15;
        }
        cntset[st].rnsc.1 = cntset[ed].rnsc.1;
        for i in st + 1..AUTOCM_NUM_SMAX {
            if i < AUTOCM_NUM_SMAX + st - ed {
                cntset[i] = cntset[i + ed - st];
            } else {
                cntset[i] = AutoCMCount::default();
            }
        }
    }
}

/// detectCMSetLogo
pub fn detect_cm_set_logo(ds: &mut Dataset, cnv: &Cnv, rnsc: (i32, i32)) -> bool {
    if ds.scp_chap(rnsc.0) < SCP_CHAP_DFIX {
        ds.set_scp_chap(rnsc.0, SCP_CHAP_DFIX);
    }
    if ds.scp_chap(rnsc.1) < SCP_CHAP_DFIX {
        ds.set_scp_chap(rnsc.1, SCP_CHAP_DFIX);
    }
    ds.set_scp_arstat(rnsc.1, SCP_AR_L_OTHER);
    mk_range_inter_logo(ds, cnv, rnsc, false)
}

/// detectCMCancelShort
pub fn detect_cm_cancel_short(ds: &mut Dataset, cnv: &Cnv, rnsc: (i32, i32)) -> bool {
    if ds.scp_chap(rnsc.0) < SCP_CHAP_DFIX || ds.scp_chap(rnsc.1) < SCP_CHAP_DFIX {
        return false;
    }
    let mut ret = false;
    let msec_st = ds.msec_scp(rnsc.0);
    let msec_ed = ds.msec_scp(rnsc.1);
    let sec_dif = cnv.get_sec_from_msec(msec_ed - msec_st);
    if (0..15).contains(&sec_dif) {
        let mut i = rnsc.0;
        while i > 0 && i < rnsc.1 {
            i = ds.nsc_next_scp_chap(i, SCP_CHAP_DFIX);
            if i > 0 && i <= rnsc.1 {
                let arstat_i = ds.scp_arstat(i);
                if arstat_i == SCP_AR_L_OTHER {
                    ds.set_scp_arstat(i, SCP_AR_N_OTHER);
                    ret = true;
                }
            }
        }
    }
    ret
}

/// detectCMAssistLogo
pub fn detect_cm_assist_logo(ds: &mut Dataset, cnv: &Cnv, nsc_det1st: i32, nsc_detend: i32) {
    let lvlogo = ds.level_use_logo();
    if lvlogo <= 1 {
        return;
    }
    let mut nsc_cur = nsc_det1st;
    while nsc_cur >= 0 && (nsc_detend < 0 || nsc_cur < nsc_detend) {
        let nsc_last = nsc_cur;
        nsc_cur = ds.nsc_next_scp_chap(nsc_cur, SCP_CHAP_DECIDE);
        if nsc_cur > 0 && (nsc_detend < 0 || nsc_cur < nsc_detend) {
            let msec_st = ds.msec_scp(nsc_last);
            let msec_ed = ds.msec_scp(nsc_cur);
            let sec_full = cnv.get_sec_from_msec(msec_ed - msec_st);
            // reference quirk preserved: getMsecScp(nrf_rise) style int usage
            let sec_logo = ds.sec_logo_component_from_logo(cnv, msec_st, msec_ed);
            let arstat_cur = ds.scp_arstat(nsc_cur);
            let flag_logoon_cur = is_scp_ar_type_logo(arstat_cur);
            if lvlogo >= 2
                && !flag_logoon_cur
                && ((sec_full - 1 <= sec_logo) || (sec_full >= 10 && sec_full <= sec_logo * 2))
            {
                if (sec_full + 1) % 15 <= 2 {
                    ds.set_scp_arstat(nsc_cur, SCP_AR_L_UNIT);
                } else {
                    ds.set_scp_arstat(nsc_cur, SCP_AR_L_OTHER);
                }
            }
            if lvlogo >= 3
                && flag_logoon_cur
                && (sec_logo <= 1 || (sec_full >= 15 && sec_full >= sec_logo * 4))
            {
                if (sec_full + 1) % 15 <= 2 {
                    ds.set_scp_arstat(nsc_cur, SCP_AR_N_UNIT);
                } else {
                    ds.set_scp_arstat(nsc_cur, SCP_AR_N_OTHER);
                }
            }
        }
    }
}

/// detectCM
pub fn detect_cm(
    ds: &mut Dataset,
    cnv: &Cnv,
    param_autocm: FormAutoCMArg,
) -> (bool, Option<FormFirstInfo>) {
    let mut cntset = [AutoCMCount::default(); AUTOCM_NUM_SMAX];
    let level_cmdet = param_autocm.level_cm_det;
    let msec_tailarea_keep = param_autocm.msec_tailarea_keep;
    let msec_tailarea_unit = param_autocm.msec_tailarea_unit;
    let sec_maxcm = param_autocm.sec_maxcm;
    let rmsec_headtail = param_autocm.rmsec_headtail;
    let msec_wlogo_trmax = ds.get_config(ConfigVarType::msecWLogoTRMax);
    let sub_autocm = ds.get_config(ConfigVarType::AutoCmSub);
    let is_sub_cmhead = (sub_autocm % 10) == 1;
    let is_sub_off45s = (sub_autocm / 10 % 10) == 1;

    if level_cmdet <= 0 {
        return (false, None);
    }
    detect_cm_count(&mut cntset, AUTOCM_T_INIT, AUTOCM_NUM_SMAX, AUTOCM_NUM_SMAX);

    let mut info_first = FormFirstInfo {
        limit_head: AUTOFIRST_LAX,
        nsc_treat: -1,
    };
    if rmsec_headtail.0 <= 0 {
        set_first_area(ds, cnv, &mut info_first);
    } else {
        info_first.limit_head = AUTOFIRST_STRICT;
        info_first.nsc_treat = ds.nsc_from_msec_chap(rmsec_headtail.0, -1, SCP_CHAP_DECIDE);
    }
    let mut nsc_last = info_first.nsc_treat;
    let mut nsc_detend = -1i32;
    if nsc_last < 0 {
        nsc_last = 0;
    }
    let mut flag_final = false;
    let mut state = AUTOCM_ST_FIRST;
    cntset[state].rnsc.0 = nsc_last;
    let mut i = nsc_last;
    while i >= 0 && !flag_final {
        i = ds.nsc_next_scp_check_cm_unit(i, SCP_END_EDGEIN);
        if i >= 0 {
            let msec_i = ds.msec_scp(i);
            if msec_i > rmsec_headtail.1 {
                flag_final = true;
                nsc_detend = nsc_last;
            }
        } else if !flag_final {
            let num_scpos = ds.size_scp();
            flag_final = true;
            i = num_scpos - 1;
        }
        if i >= 0 {
            let msec_last = ds.msec_scp(nsc_last);
            let msec_i = ds.msec_scp(i);
            let secdif_cur = cnv.get_sec_from_msec(msec_i - msec_last);
            let mut change = false;
            let mut cutlevel = level_cmdet;
            let is_state_logo = state == AUTOCM_ST_LOGO || state == AUTOCM_ST_DEFER;
            let mut flag_cm = false;
            if (((secdif_cur % 15 == 0) && secdif_cur <= sec_maxcm)
                || ((secdif_cur % 30 == 0) && secdif_cur <= 120 && cutlevel >= 10))
                && (!is_sub_off45s || (secdif_cur % 30 == 0) || secdif_cur <= 30)
            {
                flag_cm = true;
            }
            let type_mute = check_mute_many(ds, nsc_last, i);
            if type_mute >= 2 {
                flag_cm = false;
            }
            let mut flag_tailcm = false;
            if msec_tailarea_keep > 0
                && msec_tailarea_keep < msec_i
                && (msec_i - msec_last <= msec_wlogo_trmax)
                && (!is_state_logo || cntset[state].dov15 == 0)
            {
                flag_tailcm = true;
            }
            if msec_tailarea_unit > 0
                && msec_tailarea_unit < msec_i
                && ((60..=120).contains(&secdif_cur) && secdif_cur % 30 == 0)
            {
                flag_tailcm = true;
            }
            if !is_state_logo {
                if flag_cm || flag_tailcm {
                    cntset[state].rnsc.1 = i;
                } else {
                    change = true;
                }
            } else if flag_cm || flag_tailcm {
                change = true;
            } else {
                cntset[state].rnsc.1 = i;
            }
            if flag_final {
                change = true;
            }
            if change {
                if !is_state_logo {
                    if state == AUTOCM_ST_FIRST {
                        match info_first.limit_head {
                            AUTOFIRST_STRICT => {
                                if cutlevel > 3 {
                                    cutlevel -= 3;
                                } else if cutlevel > 1 {
                                    cutlevel = 1;
                                }
                            }
                            AUTOFIRST_NOCUT => {
                                cutlevel = 0;
                            }
                            _ => {
                                if cutlevel > 0 {
                                    cutlevel += 3;
                                }
                            }
                        }
                        if is_sub_cmhead && cutlevel < 9 {
                            cutlevel = 9;
                        }
                    }
                    if flag_final {
                        cutlevel = 10;
                    }
                    if state == AUTOCM_ST_JUDGE {
                        if (cntset[AUTOCM_ST_LOGO].det + cntset[AUTOCM_ST_DEFER].det <= 4)
                            && (cntset[AUTOCM_ST_LOGO].sec + cntset[AUTOCM_ST_DEFER].sec >= 120)
                        {
                            if cntset[AUTOCM_ST_LOGO].dov15 > 0 {
                                detect_cm_set_logo(ds, cnv, cntset[AUTOCM_ST_LOGO].rnsc);
                            }
                            detect_cm_count(
                                &mut cntset,
                                AUTOCM_T_SFT,
                                AUTOCM_ST_FIRST,
                                AUTOCM_ST_CM,
                            );
                        } else {
                            detect_cm_count(
                                &mut cntset,
                                AUTOCM_T_MRG,
                                AUTOCM_ST_LOGO,
                                AUTOCM_ST_DEFER,
                            );
                        }
                        state = AUTOCM_ST_CM;
                    }
                    let mut dettype = 0i32;
                    if (cntset[state].det > 3 && cutlevel > 0)
                        || (cntset[state].det > 2 && cutlevel > 3)
                        || (cntset[state].det > 1 && cutlevel > 6)
                        || (cntset[state].det > 0 && cutlevel > 8)
                    {
                        dettype = 1;
                    } else if (cntset[state].det == 2
                        && (cutlevel == 5 || cutlevel == 6)
                        && (cntset[state].d15 >= 2 || cntset[state].d30 >= 2))
                        || (cntset[state].det == 3
                            && (cutlevel == 2 || cutlevel == 3)
                            && (cntset[state].d15 + cntset[state].d30 >= 3))
                        || (cntset[state].det == 1
                            && cutlevel == 8
                            && (cntset[state].d15 + cntset[state].d30 >= 1))
                    {
                        if state == AUTOCM_ST_FIRST || cutlevel == 3 || cutlevel == 6 {
                            dettype = 1;
                        } else {
                            dettype = 2;
                        }
                    }
                    if dettype == 1 || (dettype == 2 && state == AUTOCM_ST_FIRST) {
                        if state != AUTOCM_ST_FIRST {
                            if cntset[AUTOCM_ST_LOGO].dov15 > 0 {
                                detect_cm_set_logo(ds, cnv, cntset[AUTOCM_ST_LOGO].rnsc);
                            } else {
                                detect_cm_cancel_short(ds, cnv, cntset[AUTOCM_ST_LOGO].rnsc);
                            }
                            detect_cm_count(
                                &mut cntset,
                                AUTOCM_T_SFT,
                                AUTOCM_ST_FIRST,
                                AUTOCM_ST_CM,
                            );
                        }
                        state = AUTOCM_ST_LOGO;
                        cntset[state].rnsc.0 = nsc_last;
                        cntset[state].rnsc.1 = i;
                    } else if dettype == 2 {
                        state = AUTOCM_ST_DEFER;
                        cntset[state].rnsc.0 = nsc_last;
                        cntset[state].rnsc.1 = i;
                    } else if state == AUTOCM_ST_FIRST {
                        state = AUTOCM_ST_LOGO;
                        cntset[state].rnsc.0 = cntset[AUTOCM_ST_FIRST].rnsc.0;
                        cntset[state].rnsc.1 = i;
                    } else {
                        if cntset[AUTOCM_ST_LOGO].dov15 > 0 {
                            detect_cm_set_logo(ds, cnv, cntset[AUTOCM_ST_LOGO].rnsc);
                            detect_cm_count(
                                &mut cntset,
                                AUTOCM_T_SFT,
                                AUTOCM_ST_LOGO,
                                AUTOCM_ST_CM,
                            );
                        } else {
                            detect_cm_count(
                                &mut cntset,
                                AUTOCM_T_MRG,
                                AUTOCM_ST_LOGO,
                                AUTOCM_ST_CM,
                            );
                        }
                        state = AUTOCM_ST_LOGO;
                        cntset[state].rnsc.1 = i;
                        cntset[state].dov15 = 0;
                    }
                } else {
                    if flag_final {
                        if cntset[state].dov15 > 0 {
                            detect_cm_set_logo(ds, cnv, cntset[state].rnsc);
                        } else {
                            detect_cm_count(&mut cntset, AUTOCM_T_SFT, AUTOCM_ST_FIRST, state);
                            flag_cm = true;
                        }
                    }
                    if state == AUTOCM_ST_DEFER {
                        state = AUTOCM_ST_JUDGE;
                    } else {
                        state = AUTOCM_ST_CM;
                    }
                    cntset[state].rnsc.0 = nsc_last;
                    cntset[state].rnsc.1 = i;
                }
                if flag_final
                    && (flag_cm || flag_tailcm)
                    && ds.scp_arstat(i) == super::dataset::SCP_AR_UNKNOWN
                {
                    ds.set_scp_arstat(i, SCP_AR_N_OTHER);
                }
            }
            cntset[state].det += 1;
            cntset[state].sec += secdif_cur;
            if secdif_cur == 15 {
                cntset[state].d15 += 1;
            } else if secdif_cur == 30 {
                cntset[state].d30 += 1;
            } else if secdif_cur >= 15 {
                cntset[state].dov15 += 1;
            }
            nsc_last = i;
        }
    }
    detect_cm_assist_logo(ds, cnv, info_first.nsc_treat, nsc_detend);
    if rmsec_headtail.1 >= ds.msec_total_max {
        set_final_area(ds, cnv);
    }
    (true, Some(info_first))
}

/// setFirstArea
pub fn set_first_area(ds: &mut Dataset, cnv: &Cnv, info_first: &mut FormFirstInfo) -> bool {
    let mut ret = false;
    info_first.limit_head = AUTOFIRST_LAX;
    info_first.nsc_treat = -1;
    if ds.size_scp() <= 2 {
        return false;
    }
    let mut locinfo = FormFirstLoc::default();
    {
        locinfo.msec1st_sel = ds.get_config(ConfigVarType::msecPosFirst);
        locinfo.lv_pos1st_sel = ds.get_config(ConfigVarType::priorityPosFirst);
        if locinfo.lv_pos1st_sel == 3 && ds.rec_hold.msec_select1st >= 0 {
            locinfo.msec1st_sel = ds.rec_hold.msec_select1st;
        }
        locinfo.msec1st_zone = ds.get_config(ConfigVarType::msecZoneFirst);
        locinfo.msec_wcomp1st = ds.get_config(ConfigVarType::msecWCompFirst);
        if locinfo.msec_wcomp1st <= 0 {
            locinfo.msec_wcomp1st = 30 * 1000;
        }
        locinfo.msec_end1st = 30 * 1000 + ds.msec_val_lap2;
        if locinfo.msec1st_sel > 0 {
            locinfo.msec_end1st += locinfo.msec1st_sel;
        }
        locinfo.msec_lg_cut1st = ds.get_config(ConfigVarType::msecLgCutFirst);
    }
    locinfo.nsc_logo1st = -1;
    locinfo.nsc_decide = -1;
    locinfo.msec_logo1st = -1;
    locinfo.msec_decide = -1;
    locinfo.rnsc_logo1st = (-1, -1);
    let unuse_lvlogo = ds.is_unuse_level_logo();
    if !unuse_lvlogo {
        let mut nrf_1st = ds.get_nrf_next_logo(-1, EDGE_RISE, LOGO_SELECT_VALID);
        let nrf_fall = ds.get_nrf_next_logo(nrf_1st, EDGE_FALL, LOGO_SELECT_VALID);
        let msec_tmp = ds.msec_logo_nrf(nrf_fall);
        if msec_tmp < locinfo.msec1st_zone && locinfo.msec1st_zone > 0 {
            nrf_1st = ds.get_nrf_next_logo(nrf_fall, EDGE_RISE, LOGO_SELECT_VALID);
        }
        if nrf_1st >= 0 {
            let mut wmsec_logo1st = ds.wide_msec_logo_nrf(nrf_1st);
            if wmsec_logo1st.early >= ds.msec_val_near1 {
                wmsec_logo1st.early -= ds.msec_val_spc;
                wmsec_logo1st.late += ds.msec_val_spc;
                locinfo.nsc_logo1st = ds.nsc_from_wide_msec_by_chap(&wmsec_logo1st, SCP_CHAP_NONE);
                if locinfo.nsc_logo1st > 0 {
                    for i in 1..ds.size_scp() - 1 {
                        let msec_tmp = ds.msec_scp(i);
                        if (msec_tmp >= wmsec_logo1st.early && msec_tmp <= wmsec_logo1st.late)
                            || i == locinfo.nsc_logo1st
                        {
                            locinfo.rnsc_logo1st.1 = i;
                            if locinfo.rnsc_logo1st.0 < 0 {
                                locinfo.rnsc_logo1st.0 = i;
                            }
                        }
                    }
                }
            }
        }
        let msec_logo1st_org = ds.msec_logo_nrf(nrf_1st);
        if locinfo.nsc_logo1st >= 0 {
            locinfo.msec_logo1st = ds.msec_scp(locinfo.nsc_logo1st);
            if msec_logo1st_org < locinfo.msec_logo1st - msec_logo1st_org
                && locinfo.msec_lg_cut1st >= 0
                && locinfo.msec_lg_cut1st < locinfo.msec_logo1st
            {
                locinfo.nsc_logo1st = -1;
                locinfo.msec_logo1st = 0;
            }
        } else {
            locinfo.msec_logo1st = msec_logo1st_org;
        }
        if locinfo.msec_logo1st < ds.msec_val_near2
            && locinfo.msec_lg_cut1st < 0
            && locinfo.lv_pos1st_sel > 0
        {
            locinfo.nsc_logo1st = -1;
            locinfo.msec_logo1st = -1;
        }
        locinfo.nsc_decide = ds.nsc_next_scp_chap(0, SCP_CHAP_DECIDE);
        if locinfo.nsc_decide > 0 {
            locinfo.msec_decide = ds.msec_scp(locinfo.nsc_decide);
            if locinfo.msec_decide > locinfo.msec_end1st {
                locinfo.nsc_decide = -1;
                locinfo.msec_decide = -1;
            }
        }
    }
    {
        if locinfo.msec1st_sel == 0 {
            let mut logopt = NrfCurrent::default();
            if ds.nrf_pt_next(&mut logopt, LOGO_SELECT_VALID) {
                let wmsec_1stlogo = ds.wide_msec_logo_nrf(logopt.nrf_rise);
                if wmsec_1stlogo.early <= ds.msec_val_near2 {
                    info_first.nsc_treat = -1;
                    info_first.limit_head = AUTOFIRST_NOCUT;
                }
            }
        }
        if info_first.limit_head != AUTOFIRST_NOCUT {
            let update = set_first_area_update(ds, cnv, info_first, locinfo);
            if update {
                ret = true;
            }
            if update && locinfo.msec_logo1st > ds.msec_val_lap2 {
                let msec_head = ds.msec_scp(info_first.nsc_treat);
                if msec_head + ds.msec_val_lap2 < locinfo.msec_decide || locinfo.nsc_decide < 0 {
                    set_first_area_update(ds, cnv, info_first, locinfo);
                }
            }
        }
    }
    if info_first.limit_head != AUTOFIRST_NOCUT {
        let mut nsc_head = ds.nsc_next_scp_chap(0, SCP_CHAP_DECIDE);
        let mut msec_head = 0i32;
        if nsc_head > 0 {
            msec_head = ds.msec_scp(nsc_head);
            if msec_head <= locinfo.msec_wcomp1st {
                let mut before_logo = false;
                if unuse_lvlogo || msec_head <= locinfo.msec_logo1st && locinfo.msec_logo1st >= 0 {
                    before_logo = true;
                }
                if before_logo || msec_head <= locinfo.msec_lg_cut1st || locinfo.msec_lg_cut1st < 0
                {
                    ret = true;
                    info_first.nsc_treat = nsc_head;
                    ds.set_scp_arstat(nsc_head, SCP_AR_N_OTHER);
                }
            }
        }
        let mut msec_nologo = locinfo.msec_logo1st;
        if msec_nologo < locinfo.msec1st_zone && locinfo.msec1st_zone > 0 {
            msec_nologo = locinfo.msec1st_zone;
        }
        let mut msec_last = 0i32;
        while nsc_head > 0 && msec_head <= msec_nologo {
            let arstat_head = ds.scp_arstat(nsc_head);
            if arstat_head != SCP_AR_N_UNIT && arstat_head != SCP_AR_N_OTHER {
                ret = true;
                info_first.nsc_treat = nsc_head;
                let sec_dif = cnv.get_sec_from_msec(msec_head - msec_last);
                if sec_dif > 0 && sec_dif % 15 == 0 {
                    ds.set_scp_arstat(nsc_head, SCP_AR_N_UNIT);
                } else {
                    ds.set_scp_arstat(nsc_head, SCP_AR_N_OTHER);
                }
            }
            msec_last = msec_head;
            nsc_head = ds.nsc_next_scp_chap(nsc_head, SCP_CHAP_DECIDE);
            msec_head = ds.msec_scp(nsc_head);
        }
    }
    if ret && !ds.is_unuse_level_logo() {
        let nsc_head = info_first.nsc_treat;
        let nsc_next = ds.nsc_next_scp_chap(nsc_head, SCP_CHAP_DECIDE);
        if nsc_head > 0 && nsc_next > 0 {
            add_common_component_one(ds, cnv, nsc_head, nsc_next, true);
        }
    }
    ret
}

/// setFirstAreaUpdate
pub fn set_first_area_update(
    ds: &mut Dataset,
    cnv: &Cnv,
    info_first: &mut FormFirstInfo,
    locinfo: FormFirstLoc,
) -> bool {
    let num_scpos = ds.size_scp();
    let mut limit_cand = AUTOFIRST_LAX;
    let mut nsc_cand = -1i32;
    let mut npoint_cand = 0i32;
    let mut msec_hold1st = -1i32;
    let mut lvchap_max = 1i32;
    for i in 1..num_scpos - 1 {
        let msec_i = ds.msec_scp(i);
        let chap_i = ds.scp_chap(i);
        if msec_i > locinfo.msec_end1st {
            break;
        }
        if (i > locinfo.rnsc_logo1st.1 || locinfo.rnsc_logo1st.1 < 0)
            && msec_i > locinfo.msec_logo1st
            && locinfo.msec_logo1st >= 0
            && msec_i > locinfo.msec_lg_cut1st
            && locinfo.msec_lg_cut1st >= 0
        {
            break;
        }
        let mut lvchap = 0i32;
        if locinfo.nsc_decide > 0 || locinfo.nsc_logo1st > 0 {
            let mut calc1 = CalcDifInfo::default();
            if i == locinfo.nsc_decide
                || (i >= locinfo.rnsc_logo1st.0 && i <= locinfo.rnsc_logo1st.1)
            {
                lvchap = lvchap_max;
            } else if locinfo.nsc_decide > 0 {
                lvchap = calc_dif_select(ds, cnv, &mut calc1, msec_i, locinfo.msec_decide);
                if calc1.sec < 15 && calc1.gap > ds.msec_val_exact {
                    lvchap = -1;
                }
                if calc1.sec < 3
                    && (locinfo.msec1st_sel < 0
                        || (msec_i - locinfo.msec1st_sel).abs()
                            > (locinfo.msec_decide - locinfo.msec1st_sel).abs())
                {
                    lvchap = -1;
                }
            } else if locinfo.nsc_logo1st > 0
                && (i <= locinfo.nsc_logo1st
                    || (locinfo.msec1st_sel >= 0 && locinfo.lv_pos1st_sel >= 1))
            {
                lvchap = 0;
                for _k in locinfo.rnsc_logo1st.0..=locinfo.rnsc_logo1st.1 {
                    let lvchap_tmp =
                        calc_dif_select(ds, cnv, &mut calc1, msec_i, locinfo.msec_logo1st);
                    if calc1.sec < 15
                        && calc1.gap > ds.msec_val_exact
                        && ((msec_i - locinfo.msec1st_sel).abs() > 3000 || locinfo.msec1st_sel < 0)
                    {
                        // lvchap_tmp = 0 (loop body below reads it)
                    }
                    let lvchap_tmp = if calc1.sec < 15
                        && calc1.gap > ds.msec_val_exact
                        && ((msec_i - locinfo.msec1st_sel).abs() > 3000 || locinfo.msec1st_sel < 0)
                    {
                        0
                    } else {
                        lvchap_tmp
                    };
                    if lvchap_tmp > 0 {
                        lvchap = 1;
                    }
                }
            }
            if lvchap > lvchap_max {
                if i < locinfo.nsc_logo1st || i < locinfo.nsc_decide {
                    lvchap_max = lvchap;
                } else {
                    lvchap = lvchap_max;
                }
            }
        }
        if lvchap >= 0 {
            if msec_hold1st < 0 {
                msec_hold1st = msec_i;
            }
            let mut npoint_i = 0i32;
            if locinfo.msec1st_sel >= 0 {
                let msec_dif = (msec_i - locinfo.msec1st_sel).abs();
                if msec_dif < 3000 {
                    npoint_i = 3000 - msec_dif;
                }
                if npoint_i == 0 && msec_hold1st == msec_i {
                    npoint_i = 1;
                }
            } else if locinfo.msec_logo1st >= 0
                && (locinfo.msec_logo1st + 13000 < locinfo.msec_decide || locinfo.nsc_decide < 0)
            {
                let msec_dif = (msec_i - locinfo.msec_logo1st).abs();
                npoint_i = 3000 - msec_dif / 100;
                if npoint_i <= 0 && msec_hold1st == msec_i {
                    npoint_i = 1;
                }
            } else {
                let msec_dif = (msec_i - msec_hold1st).abs();
                if msec_dif < 3000 {
                    npoint_i = 3000 - msec_dif;
                }
            }
            let mut limit_i = AUTOFIRST_LAX;
            if npoint_i > 0 {
                limit_i = AUTOFIRST_STRICT;
            }
            if locinfo.lv_pos1st_sel >= 2 {
                npoint_i *= 10;
            }
            if i >= locinfo.rnsc_logo1st.0
                && i <= locinfo.rnsc_logo1st.1
                && (locinfo.nsc_logo1st < locinfo.nsc_decide || locinfo.nsc_decide < 0)
                && (locinfo.msec_logo1st - locinfo.msec_decide).abs() > ds.msec_val_lap2
            {
                if locinfo.msec1st_sel < 0 {
                    npoint_i = 3001;
                }
                if i == locinfo.nsc_logo1st {
                    npoint_i += 2;
                }
            }
            if i >= locinfo.nsc_decide && locinfo.nsc_decide >= 0 {
            } else if chap_i >= SCP_CHAP_DECIDE || lvchap == 2 {
                npoint_i += 15000;
            } else if chap_i >= SCP_CHAP_CPOSIT || lvchap == 1 {
                npoint_i += 10000;
            }
            if (npoint_cand < npoint_i || nsc_cand < 0) && npoint_i > 0 {
                npoint_cand = npoint_i;
                limit_cand = limit_i;
                nsc_cand = i;
            }
        }
    }
    let mut update = false;
    if nsc_cand > 0 && (nsc_cand < info_first.nsc_treat || info_first.nsc_treat < 0) {
        if locinfo.msec1st_sel >= 0 && locinfo.msec_logo1st >= 0 {
            let msec_cand = ds.msec_scp(nsc_cand);
            if (locinfo.msec1st_sel - msec_cand).abs() > locinfo.msec1st_sel
                && msec_cand - locinfo.msec_logo1st > locinfo.msec_logo1st
            {
                info_first.nsc_treat = -1;
                info_first.limit_head = AUTOFIRST_NOCUT;
                return false;
            }
        }
        update = true;
        ds.set_scp_chap(nsc_cand, SCP_CHAP_DFIX);
        ds.set_scp_arstat(nsc_cand, SCP_AR_L_OTHER);
        info_first.limit_head = limit_cand;
        info_first.nsc_treat = nsc_cand;
    }
    update
}

/// setFinalArea
pub fn set_final_area(ds: &mut Dataset, cnv: &Cnv) -> bool {
    let num_scpos = ds.size_scp();
    let mut ret = false;
    let nsc_total_last = num_scpos - 1;
    let msec_total_last = ds.msec_scp(nsc_total_last);
    let msec_var_lastzone = ds.get_config(ConfigVarType::msecZoneLast);
    let mut msec_lastzone = -1i32;
    if msec_var_lastzone >= 0 {
        msec_lastzone = msec_total_last - msec_var_lastzone;
    }
    let mut msec_wcomp_lastloc = msec_total_last - ds.get_config(ConfigVarType::msecWCompLast);
    if msec_wcomp_lastloc >= msec_total_last {
        msec_wcomp_lastloc = msec_total_last - 30 * 1000;
    }
    if ds.scp_arstat(nsc_total_last) == super::dataset::SCP_AR_UNKNOWN {
        ds.set_scp_arstat(nsc_total_last, SCP_AR_L_OTHER);
    }
    let nsc_logolast;
    let mut msec_logolast = -1i32;
    let lvlogo = ds.level_use_logo();
    if lvlogo >= 4 {
        let nlg_last = ds.size_logo() - 1;
        let mut nrf_last = nlg_last * 2 + 1;
        nrf_last = ds.get_nrf_prev_logo(nrf_last + 1, EDGE_FALL, LOGO_SELECT_VALID);
        let nrf_rise = ds.get_nrf_prev_logo(nrf_last, EDGE_RISE, LOGO_SELECT_VALID);
        // reference quirk: getMsecScp(nrf_rise) — preserved as-is
        let msec_tmp = ds.msec_scp(nrf_rise);
        if msec_tmp > msec_lastzone && msec_lastzone > 0 {
            nrf_last = ds.get_nrf_prev_logo(nrf_rise, EDGE_FALL, LOGO_SELECT_VALID);
        }
        nsc_logolast = ds.nsc_from_nrf(nrf_last, 3000, SCP_CHAP_NONE, false);
        if nsc_logolast > 0 {
            msec_logolast = ds.msec_scp(nsc_logolast);
        } else {
            msec_logolast = ds.msec_logo_nrf(nrf_last);
        }
    }
    {
        let mut msec_bound = msec_wcomp_lastloc;
        if msec_lastzone < msec_wcomp_lastloc {
            msec_bound = msec_lastzone;
        }
        let arstat_last = ds.scp_arstat(nsc_total_last);
        let flag_last_logo = is_scp_ar_type_logo(arstat_last);
        let nsc_last_decide = ds.nsc_prev_scp_chap(nsc_total_last, SCP_CHAP_DECIDE);
        let msec_last_decide = ds.msec_scp(nsc_last_decide);
        let msec_thr_decide = msec_last_decide + 60 * 1000;
        if nsc_last_decide > 0 && msec_thr_decide < msec_bound && flag_last_logo {
            let mut list_nsc_chk: Vec<i32> = Vec::new();
            let mut num_chk_total = 0i32;
            let mut num_chk_bnd = 0i32;
            let mut num_chk_lim = 0i32;
            {
                let msec_thr_lim = msec_bound - 60 * 1000;
                let mut nsc_tmp = nsc_total_last;
                loop {
                    nsc_tmp = ds.nsc_prev_scp_chap(nsc_tmp, SCP_CHAP_NONE);
                    if nsc_tmp > 0 {
                        let msec_tmp = ds.msec_scp(nsc_tmp);
                        if msec_thr_decide >= msec_tmp {
                            nsc_tmp = -1;
                        } else {
                            list_nsc_chk.push(nsc_tmp);
                            num_chk_total += 1;
                            if msec_bound < msec_tmp {
                                num_chk_bnd = num_chk_total;
                            }
                            if msec_thr_lim < msec_tmp {
                                num_chk_lim = num_chk_total;
                            }
                        }
                    }
                    if !(nsc_tmp > 0) {
                        break;
                    }
                }
            }
            if num_chk_total >= 2 && num_chk_lim >= 1 {
                let mut nsc_chk_det = -1i32;
                let mut flag_posit_det = false;
                let mut flag_cont = true;
                for i1 in 0..num_chk_lim {
                    if flag_cont {
                        for i2 in i1 + 1..num_chk_total {
                            let nsc_i1 = list_nsc_chk[i1 as usize];
                            let nsc_i2 = list_nsc_chk[i2 as usize];
                            let mut flag_posit_tmp = false;
                            if ds.scp_chap(nsc_i1) != SCP_CHAP_NONE
                                && ds.scp_chap(nsc_i2) != SCP_CHAP_NONE
                            {
                                flag_posit_tmp = true;
                            }
                            if !flag_posit_det || flag_posit_tmp {
                                let msec_chk_from = ds.msec_scp(nsc_i1);
                                let msec_chk_to = ds.msec_scp(nsc_i2);
                                let mut calcdif = CalcDifInfo::default();
                                let diftype = calc_dif_select(
                                    ds,
                                    cnv,
                                    &mut calcdif,
                                    msec_chk_from,
                                    msec_chk_to,
                                );
                                if diftype > 0 {
                                    nsc_chk_det = nsc_i2;
                                    flag_posit_det = flag_posit_tmp;
                                    if i1 >= num_chk_bnd {
                                        flag_cont = false;
                                    }
                                }
                            }
                        }
                    }
                }
                if nsc_chk_det > 0 {
                    ds.set_scp_chap(nsc_chk_det, SCP_CHAP_DFIX);
                    ds.set_scp_arstat(nsc_chk_det, SCP_AR_L_OTHER);
                }
            }
        }
    }
    if msec_var_lastzone >= 0 {
        let arstat_last = ds.scp_arstat(nsc_total_last);
        let flag_last_logo = is_scp_ar_type_logo(arstat_last);
        let nsc_last_decide = ds.nsc_prev_scp_chap(nsc_total_last, SCP_CHAP_DECIDE);
        let msec_last_decide = ds.msec_scp(nsc_last_decide);
        if nsc_last_decide > 0 && msec_last_decide < msec_lastzone {
            set_interpolar(ds, cnv, (nsc_last_decide, -1), flag_last_logo);
        }
    }
    {
        let arstat_last = ds.scp_arstat(nsc_total_last);
        let flag_last_logo = is_scp_ar_type_logo(arstat_last);
        let nsc_last_decide = ds.nsc_prev_scp_chap(nsc_total_last, SCP_CHAP_DECIDE);
        let nsc_last_cdet = ds.nsc_prev_scp_chap(nsc_total_last, SCP_CHAP_CDET);
        if nsc_last_cdet > 0 {
            let msec_last_decide = ds.msec_scp(nsc_last_decide);
            let msec_last_cdet = ds.msec_scp(nsc_last_cdet);
            if nsc_last_decide < nsc_last_cdet && nsc_last_decide > 0 {
                let msec_dif = msec_last_cdet - msec_last_decide;
                let mut calcmod = CalcModInfo::default();
                let modtype = calc_dif_mod0515(ds, &mut calcmod, msec_dif);
                if modtype >= 2 {
                    ret = true;
                    ds.set_scp_chap(nsc_last_cdet, SCP_CHAP_DFIX);
                    if flag_last_logo {
                        ds.set_scp_arstat(nsc_last_cdet, SCP_AR_L_UNIT);
                    } else {
                        ds.set_scp_arstat(nsc_last_cdet, SCP_AR_N_UNIT);
                    }
                    set_interpolar(ds, cnv, (nsc_last_decide, nsc_last_cdet), flag_last_logo);
                } else {
                    let nsc_last2_cdet = ds.nsc_prev_scp_chap(nsc_last_cdet, SCP_CHAP_CDET);
                    if nsc_last_decide < nsc_last2_cdet {
                        let msec_last2_cdet = ds.msec_scp(nsc_last2_cdet);
                        if msec_last2_cdet - msec_last_decide > ds.msec_val_lap2 {
                            let msec_dif = msec_last_cdet - msec_last2_cdet;
                            let mut calcmod2 = CalcModInfo::default();
                            let modtype2 = calc_dif_mod0515(ds, &mut calcmod2, msec_dif);
                            if modtype2 >= 2 {
                                ret = true;
                                ds.set_scp_chap(nsc_last2_cdet, SCP_CHAP_DFIX);
                                ds.set_scp_chap(nsc_last_cdet, SCP_CHAP_DFIX);
                                if flag_last_logo {
                                    ds.set_scp_arstat(nsc_last2_cdet, SCP_AR_L_OTHER);
                                    ds.set_scp_arstat(nsc_last_cdet, SCP_AR_L_UNIT);
                                } else {
                                    ds.set_scp_arstat(nsc_last2_cdet, SCP_AR_N_OTHER);
                                    ds.set_scp_arstat(nsc_last_cdet, SCP_AR_N_UNIT);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    if msec_var_lastzone >= 0 {
        let mut nsc_last_decide = ds.nsc_prev_scp_chap(nsc_total_last, SCP_CHAP_DECIDE);
        let mut nsc_last_cand = ds.nsc_prev_scp_chap(nsc_total_last, SCP_CHAP_NONE);
        if nsc_last_decide < nsc_last_cand && nsc_last_decide > 0 {
            let mut msec_last_decide = ds.msec_scp(nsc_last_decide);
            let mut msec_last_cand = ds.msec_scp(nsc_last_cand);
            let mut calcdif = CalcDifInfo::default();
            let mut diftype =
                calc_dif_select(ds, cnv, &mut calcdif, msec_last_decide, msec_last_cand);
            if !(diftype > 0 && calcdif.gap <= ds.msec_val_exact) {
                let nsc_tmp = ds.nsc_prev_scp_chap(nsc_last_cand, SCP_CHAP_NONE);
                if nsc_last_decide < nsc_tmp {
                    nsc_last_cand = nsc_tmp;
                    msec_last_cand = ds.msec_scp(nsc_last_cand);
                    diftype =
                        calc_dif_select(ds, cnv, &mut calcdif, msec_last_decide, msec_last_cand);
                }
            }
            if diftype > 0
                && calcdif.gap <= ds.msec_val_exact
                && msec_last_cand >= msec_wcomp_lastloc
                && msec_wcomp_lastloc > 0
                && msec_last_decide < msec_lastzone
            {
                let arstat_last = ds.scp_arstat(nsc_total_last);
                let flag_last_logo = is_scp_ar_type_logo(arstat_last);
                ret = true;
                ds.set_scp_chap(nsc_last_cand, SCP_CHAP_DFIX);
                if flag_last_logo {
                    ds.set_scp_arstat(nsc_last_cand, SCP_AR_L_OTHER);
                } else {
                    ds.set_scp_arstat(nsc_last_cand, SCP_AR_N_OTHER);
                }
                set_interpolar(ds, cnv, (nsc_last_decide, nsc_last_cand), flag_last_logo);
            }
            let _ = &mut nsc_last_decide;
            let _ = &mut msec_last_decide;
            let _ = msec_last_cand;
        }
    }
    if msec_lastzone > 0 || msec_logolast > 0 {
        let mut nsc_last_start = nsc_total_last;
        let mut msec_endtarget = msec_lastzone;
        if (msec_lastzone > msec_logolast && msec_logolast > 0) || msec_lastzone <= 0 {
            msec_endtarget = msec_logolast;
        }
        let mut checkend = false;
        while !checkend && nsc_last_start > 0 {
            let nsc_tmp = ds.nsc_prev_scp_chap(nsc_last_start, SCP_CHAP_DECIDE);
            let msec_tmp = ds.msec_scp(nsc_tmp);
            if msec_tmp < msec_endtarget || nsc_tmp < 0 {
                checkend = true;
            } else {
                nsc_last_start = nsc_tmp;
            }
        }
        if nsc_last_start > 0 && nsc_last_start < nsc_total_last {
            let mut nsc_cur;
            loop {
                nsc_cur = ds.nsc_next_scp_chap(nsc_last_start, SCP_CHAP_DECIDE);
                if nsc_cur < 0 {
                    nsc_cur = nsc_total_last;
                }
                let arstat_cur = ds.scp_arstat(nsc_cur);
                let flag_logo_cur = is_scp_ar_type_logo(arstat_cur);
                if flag_logo_cur {
                    ret = true;
                    if arstat_cur == SCP_AR_L_OTHER {
                        ds.set_scp_arstat(nsc_cur, SCP_AR_N_OTHER);
                    } else if arstat_cur == SCP_AR_L_UNIT {
                        ds.set_scp_arstat(nsc_cur, SCP_AR_N_UNIT);
                    }
                }
                nsc_last_start = nsc_cur;
                if !(nsc_cur > 0 && nsc_cur < nsc_total_last) {
                    break;
                }
            }
        }
    }
    if msec_wcomp_lastloc > 0 {
        let nsc_prev = ds.nsc_prev_scp_chap(nsc_total_last, SCP_CHAP_DECIDE);
        let msec_prev = ds.msec_scp(nsc_prev);
        if msec_prev > msec_wcomp_lastloc {
            ret = true;
            ds.set_scp_arstat(nsc_total_last, SCP_AR_N_OTHER);
        }
    }
    ret
}

/// mkReformAllNologo
pub fn mk_reform_all_nologo(ds: &mut Dataset, cnv: &Cnv, cmdarg: &CmdArg) -> bool {
    let (_det, _b) = mk_range_cm(ds, cnv, -1, -1, false, false, true, 0);
    let param = set_reform_param(ds, cmdarg, true);
    let (det, _info) = detect_cm(ds, cnv, param);
    let _ = det;
    true
}

/// mkReformAll dispatcher
pub fn mk_reform_all(ds: &mut Dataset, cnv: &Cnv, cmdarg: &CmdArg) {
    if ds.is_unuse_level_logo() {
        mk_reform_all_nologo(ds, cnv, cmdarg);
    } else {
        mk_reform_all_logo(ds, cnv);
    }
}

/// startAutoCM
pub fn start_auto_cm(ds: &mut Dataset, cnv: &Cnv, cmdarg: &CmdArg) -> bool {
    let mut exeflag = false;
    if ds.is_unuse_level_logo() {
        let param = set_reform_param(ds, cmdarg, false);
        exeflag = detect_cm(ds, cnv, param).0;
    }
    exeflag
}

/// mkReformAllLogo
pub fn mk_reform_all_logo(ds: &mut Dataset, cnv: &Cnv) {
    let mut lastlogo = true; // ロゴのない期間をCM化する処理
    {
        let mut logopt = NrfCurrent::default();
        let mut scope = RangeFixMsec {
            st: -1,
            ..Default::default()
        };
        let mut msec_lg_remain_st: i32 = -1; // 次の残す箇所の先頭位置
        let mut logoon_st = false; // 先頭部分のロゴ状態
        let mut flagend = false;
        while !flagend {
            // --- ロゴ位置取得 ---
            if ds.nrf_pt_next(&mut logopt, LOGO_SELECT_VALID) {
                scope.ed = logopt.msec_rise;
            } else {
                scope.ed = -1;
                flagend = true; // 最後の端処理をして終了
            }
            // --- CM構成を作成 ---
            let interfill = 1; // 複数のCM構成では間もCM
            let (mut det, bounds) = mk_range_cm(
                ds,
                cnv,
                scope.st,
                scope.ed,
                scope.fix_st,
                scope.fix_ed,
                logoon_st,
                interfill,
            );
            // --- 最終位置のロゴ有無判断 ---
            if flagend && det && logopt.msec_fall < bounds.1 + ds.msec_val_lap2 {
                lastlogo = false;
            }
            // --- 残す箇所（CM手前）の構成内部を構成分割 ---
            if flagend || det {
                // 最後の位置またはCM検出した場合
                if logoon_st {
                    // 手前にロゴがあった場合
                    let msec_lg_remain_ed = if det { bounds.0 } else { -1 };
                    mk_reform_all_logo_update(ds, cnv, (msec_lg_remain_st, msec_lg_remain_ed));
                }
                msec_lg_remain_st = bounds.1; // 次の残す箇所の先頭位置
            }
            // --- CMが見つからない時はロゴ期間自体が１構成にならないか確認 ---
            else if !det {
                det = mk_reform_all_logo_unit(ds, cnv, &mut msec_lg_remain_st, &logopt);
            }
            // --- CM認識またはロゴ期間が長い場合は次の開始をロゴ終了位置に設定 ---
            let msec_recmin = ds.get_config(ConfigVarType::msecWLogoLgMin);
            if !flagend && (det || (logopt.msec_fall - logopt.msec_rise) >= msec_recmin) {
                scope.st = logopt.msec_fall;
            }
            // --- 次の位置設定 ---
            logoon_st = true;
        }
    }
    // --- 先頭位置の追加処理 ---
    {
        let mut tmpinfo = FormFirstInfo::default();
        set_first_area(ds, cnv, &mut tmpinfo);
    }
    // --- 最後位置の追加処理 ---
    {
        let nsc_final = ds.size_scp() - 1;
        if !lastlogo {
            ds.set_scp_arstat(nsc_final, SCP_AR_N_OTHER);
        } else {
            ds.set_scp_arstat(nsc_final, SCP_AR_L_OTHER);
        }
        set_final_area(ds, cnv);
    }
    // --- 構成要素のロゴ部分分離（MIX/Border追加） ---
    let lvlogo = ds.level_use_logo();
    add_logo_component(ds, cnv, lvlogo);
}

/// mkReformAllLogoUnit
pub fn mk_reform_all_logo_unit(
    ds: &mut Dataset,
    cnv: &Cnv,
    msec_lg_remain_st: &mut i32,
    logopt: &NrfCurrent,
) -> bool {
    // --- 対応するシーンチェンジ番号を取得 ---
    let mut rnsc_lg_cur = (-1, -1);
    {
        let nsc_decide_st =
            ds.nsc_from_msec_chap(logopt.msec_rise, ds.msec_val_lap2, SCP_CHAP_DECIDE);
        let nsc_decide_ed =
            ds.nsc_from_msec_chap(logopt.msec_fall, ds.msec_val_lap2, SCP_CHAP_DECIDE);
        let nsc_lg_any_st =
            ds.nsc_from_msec_chap(logopt.msec_rise, ds.msec_val_lap2, SCP_CHAP_NONE);
        let nsc_lg_any_ed =
            ds.nsc_from_msec_chap(logopt.msec_fall, ds.msec_val_lap2, SCP_CHAP_NONE);
        if nsc_decide_st >= 0 {
            rnsc_lg_cur.0 = nsc_decide_st;
        } else if nsc_lg_any_st >= 0 {
            rnsc_lg_cur.0 = nsc_lg_any_st;
        }
        if nsc_decide_ed >= 0 {
            rnsc_lg_cur.1 = nsc_decide_ed;
        } else if nsc_lg_any_ed >= 0 {
            rnsc_lg_cur.1 = nsc_lg_any_ed;
        }
    }
    // --- 対応する無音シーンチェンジがあれば１構成期間の判別 ---
    let mut det = false;
    if rnsc_lg_cur.0 >= 0 && rnsc_lg_cur.1 >= 0 && rnsc_lg_cur.0 < rnsc_lg_cur.1 {
        let rmsec_lg_cur = ds.range_msec_from_range_nsc(rnsc_lg_cur.0, rnsc_lg_cur.1);
        // --- 現在のロゴ長が認識期間であるか確認 ---
        let match_len =
            is_length_logo_level(ds, cnv, rmsec_lg_cur.1 - rmsec_lg_cur.0, true, false, false);
        if match_len {
            det = true;
            if rnsc_lg_cur.0 > 0 && ds.scp_chap(rnsc_lg_cur.0) < SCP_CHAP_DFIX {
                ds.set_scp_chap(rnsc_lg_cur.0, SCP_CHAP_DFIX);
            }
            if rnsc_lg_cur.1 > 0 && ds.scp_chap(rnsc_lg_cur.1) < SCP_CHAP_DFIX {
                ds.set_scp_chap(rnsc_lg_cur.1, SCP_CHAP_DFIX);
            }
            if *msec_lg_remain_st < rmsec_lg_cur.0 {
                // 直前までのロゴ設定
                mk_reform_all_logo_update(ds, cnv, (*msec_lg_remain_st, rmsec_lg_cur.0));
            }
            mk_reform_all_logo_update(ds, cnv, (rmsec_lg_cur.0, rmsec_lg_cur.1));
            *msec_lg_remain_st = rmsec_lg_cur.1; // ロゴ先頭位置を更新
        }
    }
    det
}

/// mkReformAllLogoUpdate
pub fn mk_reform_all_logo_update(ds: &mut Dataset, cnv: &Cnv, rmsec_new: (i32, i32)) {
    let (rnsc_new, _det) = ds.range_nsc_from_range_msec(rmsec_new.0, rmsec_new.1);
    if rnsc_new.ed > 0 {
        ds.set_scp_arstat(rnsc_new.ed, SCP_AR_L_UNIT); // 念のため設定
    }
    let cm_inter = true;
    mk_range_inter_logo(ds, cnv, (rnsc_new.st, rnsc_new.ed), cm_inter);
}
