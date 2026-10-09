//! Derived from join_logo_scp (https://github.com/tobitti0/join_logo_scp,
//! commit d1a24d758f8b3c04352b82ff6756fdf3d56729b3), Copyright (C) tobitti0,
//! GPL-2.0-or-later. This port is a GPL derivative work, redistributed here
//! under GPL-3.0-or-later (design §10.6).
//! Query/search layer of JlsDataset.cpp (nrf/nsc/elg navigation, term,
//! force-insert, logo span insert). Data layer lives in dataset.rs.
//! cnv-dependent functions take explicit &Cnv since the Rust Dataset does
//! not own the converter (exec wires both from one Rc).

use super::cmdarg::WideMsec;
use super::cnvstr::Cnv;
use super::dataset::*;
use super::tables::ConfigVarType;

pub const SCP_END_EDGEIN: i32 = 0;
pub const SCP_END_NOEDGE: i32 = 1;
pub const DIR_PREV: bool = false;
pub const DIR_NEXT: bool = true;
pub const SCP_CHAP_CPOSIT: i32 = 1;
pub const SCP_CHAP_CPOSQ: i32 = 2;
pub const SCP_CHAP_CDET: i32 = 3;
pub const SCP_CHAP_DINT: i32 = 4; // == SCP_CHAP_DECIDE
pub const SCP_CHAP_DBORDER: i32 = 5;
pub const SCP_CHAP_DFIX: i32 = 6;
pub const SCP_CHAP_DFORCE: i32 = 7;
pub const SCP_CHAP_DUNIT: i32 = 8;
pub const SCP_AR_UNKNOWN: i32 = 0;
pub const SCP_AR_L_UNIT: i32 = 1;
pub const SCP_AR_L_OTHER: i32 = 2;
pub const SCP_AR_L_MIXED: i32 = 3;
pub const SCP_AR_N_UNIT: i32 = 4;
pub const SCP_AR_N_OTHER: i32 = 5;
pub const SCP_AR_N_AUNIT: i32 = 6;
pub const SCP_AR_N_BUNIT: i32 = 7;
pub const SCP_AR_B_UNIT: i32 = 8;
pub const SCP_AR_B_OTHER: i32 = 9;
pub const AREXT_NONE: i32 = 0;
pub const AREXT_L_TRKEEP: i32 = 1;
pub const AREXT_L_TRCUT: i32 = 2;
pub const AREXT_L_TRRAW: i32 = 3;
pub const AREXT_L_ECCUT: i32 = 4;
pub const AREXT_L_EC: i32 = 5;
pub const AREXT_L_SP: i32 = 6;
pub const AREXT_L_LGCUT: i32 = 7;
pub const AREXT_L_LGADD: i32 = 8;
pub const AREXT_N_TRCUT: i32 = 9;
pub const AREXT_N_LGCUT: i32 = 10;
pub const AREXT_N_LGADD: i32 = 11;
pub const PRIOR_LV1: i32 = 1;
pub const PRIOR_DECIDE: i32 = 2;

pub fn is_scp_chap_type_decide(chap: i32) -> bool {
    chap >= SCP_CHAP_DECIDE
}
pub fn is_scp_ar_type_logo(ar: i32) -> bool {
    (SCP_AR_L_UNIT..=SCP_AR_L_MIXED).contains(&ar)
}
pub fn is_scp_ar_type_logo_border(ar: i32) -> bool {
    (SCP_AR_L_UNIT..=SCP_AR_L_MIXED).contains(&ar) || (SCP_AR_B_UNIT..=SCP_AR_B_OTHER).contains(&ar)
}
fn is_logo_edge_rise(edge: i32) -> bool {
    edge == EDGE_RISE || edge == EDGE_BOTH
}
fn is_logo_edge_fall(edge: i32) -> bool {
    edge == EDGE_FALL || edge == EDGE_BOTH
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Term {
    pub valid: bool,
    pub endfix: bool,
    pub endtype: i32,
    pub ini: i32,
    pub nsc_st: i32,
    pub nsc_ed: i32,
    pub msec_st: i32,
    pub msec_ed: i32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NrfCurrent {
    pub valid: bool,
    pub nrf_rise: i32,
    pub nrf_fall: i32,
    pub nrf_last_rise: i32,
    pub nrf_last_fall: i32,
    pub msec_rise: i32,
    pub msec_fall: i32,
    pub msec_last_rise: i32,
    pub msec_last_fall: i32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ElgCurrent {
    pub valid: bool,
    pub border: bool,
    pub outflag: bool,
    pub nsc_rise: i32,
    pub nsc_fall: i32,
    pub nsc_last_rise: i32,
    pub nsc_last_fall: i32,
    pub msec_rise: i32,
    pub msec_fall: i32,
    pub msec_last_rise: i32,
    pub msec_last_fall: i32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RangeNsc {
    pub st: i32,
    pub ed: i32,
}

impl Dataset {
    // ================= 1-record accessors (originals) =================

    /// getScpStill
    pub fn scp_still(&self, nsc: i32) -> bool {
        if nsc >= 0 && nsc < self.size_scp() {
            self.scp[nsc as usize].still
        } else {
            false
        }
    }
    /// getScpStatpos
    pub fn scp_statpos(&self, nsc: i32) -> i32 {
        if nsc >= 0 && nsc < self.size_scp() {
            self.scp[nsc as usize].statpos
        } else {
            PRIOR_NONE
        }
    }
    /// getScpScore
    pub fn scp_score(&self, nsc: i32) -> i32 {
        if nsc >= 0 && nsc < self.size_scp() {
            self.scp[nsc as usize].score
        } else {
            0
        }
    }
    /// getScpChap
    pub fn scp_chap(&self, nsc: i32) -> i32 {
        if nsc >= 0 && nsc < self.size_scp() {
            self.scp[nsc as usize].chap
        } else {
            SCP_CHAP_NONE
        }
    }
    /// getScpArstat
    pub fn scp_arstat(&self, nsc: i32) -> i32 {
        if nsc >= 0 && nsc < self.size_scp() {
            self.scp[nsc as usize].arstat
        } else {
            SCP_AR_UNKNOWN
        }
    }
    /// getScpArext
    pub fn scp_arext(&self, nsc: i32) -> i32 {
        if nsc >= 0 && nsc < self.size_scp() {
            self.scp[nsc as usize].arext
        } else {
            SCP_AREXT_NONE
        }
    }
    /// setScpStatpos
    pub fn set_scp_statpos(&mut self, nsc: i32, val: i32) {
        if nsc >= 0 && nsc < self.size_scp() {
            self.scp[nsc as usize].statpos = val;
        }
    }
    /// setScpScore
    pub fn set_scp_score(&mut self, nsc: i32, val: i32) {
        if nsc >= 0 && nsc < self.size_scp() {
            self.scp[nsc as usize].score = val;
        }
    }
    /// setScpChap
    pub fn set_scp_chap(&mut self, nsc: i32, val: i32) {
        if nsc >= 0 && nsc < self.size_scp() {
            self.scp[nsc as usize].chap = val;
        }
    }
    /// setScpArstat (resets arext as the reference does)
    pub fn set_scp_arstat(&mut self, nsc: i32, val: i32) {
        if nsc >= 0 && nsc < self.size_scp() {
            self.scp[nsc as usize].arstat = val;
            self.scp[nsc as usize].arext = SCP_AREXT_NONE;
        }
    }
    /// setScpArext
    pub fn set_scp_arext(&mut self, nsc: i32, val: i32) {
        if nsc >= 0 && nsc < self.size_scp() {
            self.scp[nsc as usize].arext = val;
        }
    }
    /// getResultLogoAtNrf: (msec, outtype) on hit
    pub fn result_logo_at_nrf(&self, nrf: i32) -> Option<(i32, i32)> {
        if nrf >= 0 && nrf / 2 < self.size_logo() {
            let lg = &self.logo[(nrf / 2) as usize];
            if nrf % 2 == 0 {
                return Some((lg.result_rise, lg.outtype_rise));
            } else {
                return Some((lg.result_fall, lg.outtype_fall));
            }
        }
        None
    }
    /// setResultLogoAtNrf
    pub fn set_result_logo_at_nrf(&mut self, msec: i32, outtype: i32, nrf: i32) {
        if nrf >= 0 && nrf / 2 < self.size_logo() {
            let lg = &mut self.logo[(nrf / 2) as usize];
            if nrf % 2 == 0 {
                lg.result_rise = msec;
                lg.outtype_rise = outtype;
            } else {
                lg.result_fall = msec;
                lg.outtype_fall = outtype;
            }
        }
    }
    /// getPriorLogo
    pub fn prior_logo(&self, nrf: i32) -> i32 {
        let n = nrf / 2;
        if n >= 0 && n < self.size_logo() {
            let lg = &self.logo[n as usize];
            return if nrf % 2 == 0 {
                lg.stat_rise
            } else {
                lg.stat_fall
            };
        }
        LOGO_PRIOR_NONE
    }
    /// getPriorScp (depends on flag_setup_auto)
    pub fn prior_scp(&self, nsc: i32) -> i32 {
        if nsc >= 0 && nsc < self.size_scp() {
            let sc = &self.scp[nsc as usize];
            if self.get_flag_setup_auto() <= 1 {
                return sc.statpos;
            } else {
                if sc.chap >= SCP_CHAP_DECIDE {
                    return PRIOR_DECIDE;
                } else if sc.chap > SCP_CHAP_NONE {
                    return PRIOR_LV1;
                } else if sc.chap < SCP_CHAP_NONE {
                    return PRIOR_DUPE;
                }
            }
        }
        PRIOR_NONE
    }

    // ================= nrf navigation =================

    /// getNrfPrevLogo
    pub fn get_nrf_prev_logo(&self, nrf: i32, edge: i32, select: i32) -> i32 {
        self.get_nrf_dir_logo(nrf, false, edge, select)
    }
    /// getNrfptNext: advances the cursor, returns (next, cur)
    pub fn nrf_pt_next(&self, cur: &mut NrfCurrent, select: i32) -> bool {
        if !cur.valid {
            cur.nrf_rise = -1;
            cur.nrf_fall = -1;
            cur.valid = true;
        }
        cur.nrf_last_rise = cur.nrf_rise;
        cur.nrf_last_fall = cur.nrf_fall;
        cur.msec_last_rise = cur.msec_rise;
        cur.msec_last_fall = cur.msec_fall;
        cur.nrf_rise = self.get_nrf_next_logo(cur.nrf_fall, EDGE_RISE, select);
        if cur.nrf_rise >= 0 {
            cur.nrf_fall = self.get_nrf_next_logo(cur.nrf_rise, EDGE_FALL, select);
        } else {
            cur.nrf_fall = -1;
        }
        cur.msec_rise = self.msec_logo_nrf(cur.nrf_rise);
        cur.msec_fall = self.msec_logo_nrf(cur.nrf_fall);
        cur.nrf_fall >= 0
    }

    // ================= scp chap / decide navigation =================

    /// getNscDirScpChap
    pub fn nsc_dir_scp_chap(&self, nsc: i32, dir_next: bool, chap_th: i32) -> i32 {
        if !dir_next {
            self.nsc_prev_scp_chap(nsc, chap_th)
        } else {
            self.nsc_next_scp_chap(nsc, chap_th)
        }
    }
    /// getNscPrevScpChap
    pub fn nsc_prev_scp_chap(&self, nsc: i32, chap_th: i32) -> i32 {
        let num = self.size_scp();
        let mut r = -1i32;
        if nsc - 1 < num {
            let mut i = nsc - 1;
            while i > 0 {
                let chap_i = self.scp[i as usize].chap;
                if chap_i >= chap_th {
                    r = i;
                    break;
                }
                i -= 1;
            }
        }
        r
    }
    /// getNscNextScpChap
    pub fn nsc_next_scp_chap(&self, nsc: i32, chap_th: i32) -> i32 {
        let num = self.size_scp();
        let mut r = -1i32;
        if nsc + 1 >= 0 {
            let mut i = nsc + 1;
            while i < num - 1 {
                let chap_i = self.scp[i as usize].chap;
                if chap_i >= chap_th {
                    r = i;
                    break;
                }
                i += 1;
            }
        }
        r
    }
    /// getNscNextScpChapEdge
    pub fn nsc_next_scp_chap_edge(&self, nsc: i32, chap_th: i32, noedge: i32) -> i32 {
        let num = self.size_scp();
        let mut r = -1i32;
        if nsc + 1 >= 0 {
            let mut i = nsc + 1;
            while i < num - noedge {
                let chap_i = self.scp[i as usize].chap;
                if chap_i >= chap_th || i == 0 || i == num - 1 {
                    r = i;
                    break;
                }
                i += 1;
            }
        }
        r
    }
    /// getNscNextScpCheckCmUnit
    pub fn nsc_next_scp_check_cm_unit(&self, nsc: i32, noedge: i32) -> i32 {
        let mut nsc_next = nsc;
        let mut nsc_cur;
        let mut arstat;
        loop {
            nsc_cur = nsc_next;
            nsc_next = self.nsc_next_scp_chap_edge(nsc_cur, SCP_CHAP_DECIDE, noedge);
            arstat = self.scp_arstat(nsc_next);
            if !(nsc_next > 0
                && (arstat == SCP_AR_N_AUNIT || (arstat == SCP_AR_N_BUNIT && nsc_cur == nsc)))
            {
                break;
            }
        }
        if nsc_cur == nsc {
            nsc_cur = nsc_next;
        }
        nsc_cur
    }
    /// getNscDirScpDecide
    pub fn nsc_dir_scp_decide(&self, nsc: i32, dir_next: bool, noedge: i32) -> i32 {
        if !dir_next {
            self.nsc_prev_scp_decide(nsc, noedge)
        } else {
            self.nsc_next_scp_decide(nsc, noedge)
        }
    }
    /// getNscPrevScpDecide
    pub fn nsc_prev_scp_decide(&self, nsc: i32, noedge: i32) -> i32 {
        let num = self.size_scp();
        let mut r = -1i32;
        let mut nstart = nsc - 1;
        if nstart == num - 1 && noedge == SCP_END_NOEDGE {
            nstart = num - 2;
        }
        let mut i = nstart;
        while i >= noedge {
            if self.prior_scp(i) >= PRIOR_DECIDE || i == 0 || i == num - 1 {
                r = i;
                break;
            }
            i -= 1;
        }
        r
    }
    /// getNscNextScpDecide
    pub fn nsc_next_scp_decide(&self, nsc: i32, noedge: i32) -> i32 {
        let num = self.size_scp();
        let mut r = -1i32;
        let mut nstart = nsc + 1;
        if nstart == 0 && noedge == SCP_END_NOEDGE {
            nstart = 1;
        }
        let mut i = nstart;
        while i < num - noedge {
            if self.prior_scp(i) >= PRIOR_DECIDE || i == 0 || i == num - 1 {
                r = i;
                break;
            }
            i += 1;
        }
        r
    }
    /// getNscNextScpOutput
    pub fn nsc_next_scp_output(&self, nsc: i32, noedge: i32) -> i32 {
        if self.is_auto_mode_use() {
            self.nsc_next_scp_check_cm_unit(nsc, noedge)
        } else {
            self.nsc_next_scp_decide(nsc, noedge)
        }
    }

    // ================= elg (estimated-logo) navigation =================

    /// getNscDirElgForAll
    pub fn nsc_dir_elg_for_all(
        &self,
        nsc: i32,
        dir_next: bool,
        edge: i32,
        flag_border: bool,
        flag_out: bool,
    ) -> i32 {
        let size_scp = self.size_scp();
        let mut r = -1i32;
        let mut i = nsc;
        let inext = self.nsc_dir_scp_decide(i, dir_next, SCP_END_EDGEIN);
        if dir_next {
            // arstat は2点間の後側を見るための移動
            i = self.nsc_next_scp_decide(i, SCP_END_EDGEIN);
        }
        let mut inlogo_base = self.is_elg_in_scp_for_all(i, flag_border, flag_out);
        loop {
            let cond = (edge == EDGE_RISE && dir_next && inlogo_base)
                || (edge == EDGE_RISE && !dir_next && !inlogo_base)
                || (edge == EDGE_FALL && dir_next && !inlogo_base)
                || (edge == EDGE_FALL && !dir_next && inlogo_base);
            if !(cond && self.scp_chap(inext) != SCP_CHAP_DUNIT && i >= 0) {
                break;
            }
            i = self.nsc_dir_scp_decide(i, dir_next, SCP_END_EDGEIN);
            inlogo_base = self.is_elg_in_scp_for_all(i, flag_border, flag_out);
        }
        if (i > 0 && i < size_scp) || (i == 0 && dir_next) || (i == size_scp && !dir_next) {
            let mut flag_end = false;
            while !flag_end {
                let ilast = i;
                i = self.nsc_dir_scp_decide(i, dir_next, SCP_END_EDGEIN);
                if i > 0 && i < size_scp {
                    let inlogo_i = self.is_elg_in_scp_for_all(i, flag_border, flag_out);
                    let iset = if dir_next { ilast } else { i };
                    if inlogo_i != inlogo_base || self.scp_chap(iset) == SCP_CHAP_DUNIT {
                        flag_end = true;
                        r = iset;
                    }
                } else {
                    flag_end = true;
                    if inlogo_base {
                        r = if dir_next { size_scp - 1 } else { 0 };
                    }
                }
            }
        }
        r
    }
    /// getNscPrevElg
    pub fn nsc_prev_elg(&self, nsc: i32, edge: i32) -> i32 {
        self.nsc_dir_elg_for_all(nsc, DIR_PREV, edge, false, false)
    }
    /// getNscNextElg
    pub fn nsc_next_elg(&self, nsc: i32, edge: i32) -> i32 {
        self.nsc_dir_elg_for_all(nsc, DIR_NEXT, edge, false, false)
    }
    /// getElgptNext
    pub fn elg_pt_next(&self, elg: &mut ElgCurrent) -> bool {
        let mut flag_1st = false;
        if !elg.valid {
            elg.nsc_fall = 0;
            elg.valid = true;
            flag_1st = true;
        }
        elg.nsc_last_rise = elg.nsc_rise;
        elg.nsc_last_fall = elg.nsc_fall;
        elg.msec_last_rise = elg.msec_rise;
        elg.msec_last_fall = elg.msec_fall;
        if !self.is_same_loc_next_elg(elg.nsc_fall, EDGE_RISE) {
            if flag_1st {
                elg.nsc_fall = -1; // 0位置からチェックするため
            }
            elg.nsc_rise = self.nsc_dir_elg_for_all(
                elg.nsc_fall,
                DIR_NEXT,
                EDGE_RISE,
                elg.border,
                elg.outflag,
            );
        } else {
            elg.nsc_rise = elg.nsc_fall;
        }
        elg.msec_rise = self.msec_scp(elg.nsc_rise);
        if elg.nsc_rise >= 0 {
            elg.nsc_fall = self.nsc_dir_elg_for_all(
                elg.nsc_rise,
                DIR_NEXT,
                EDGE_FALL,
                elg.border,
                elg.outflag,
            );
        } else {
            elg.nsc_fall = -1;
        }
        if elg.outflag {
            elg.msec_fall = self.msec_scp_bk(elg.nsc_fall);
        } else {
            elg.msec_fall = self.msec_scp(elg.nsc_fall);
        }
        elg.nsc_fall >= 0
    }
    /// isSameLocDirElg
    pub fn is_same_loc_dir_elg(&self, nsc: i32, dir_next: bool, edge: i32) -> bool {
        if self.scp_chap(nsc) == SCP_CHAP_DUNIT
            && ((!dir_next && edge == EDGE_FALL) || (dir_next && edge == EDGE_RISE))
        {
            return true;
        }
        false
    }
    pub fn is_same_loc_next_elg(&self, nsc: i32, edge: i32) -> bool {
        self.is_same_loc_dir_elg(nsc, DIR_NEXT, edge)
    }
    pub fn is_same_loc_prev_elg(&self, nsc: i32, edge: i32) -> bool {
        self.is_same_loc_dir_elg(nsc, DIR_PREV, edge)
    }
    /// isElgInScp
    pub fn is_elg_in_scp(&self, nsc: i32) -> bool {
        self.is_elg_in_scp_for_all(nsc, false, false)
    }
    /// isElgInScpForAll
    pub fn is_elg_in_scp_for_all(&self, nsc_in: i32, flag_border: bool, flag_out: bool) -> bool {
        let mut nsc = nsc_in;
        let mut ret = false;
        let num_scpos = self.size_scp();
        if nsc > 0 && nsc < num_scpos {
            if !is_scp_chap_type_decide(self.scp_chap(nsc)) && nsc < num_scpos - 1 {
                nsc = self.nsc_next_scp_decide(nsc, SCP_END_EDGEIN);
            }
            let arstat = self.scp_arstat(nsc);
            if !flag_border {
                ret = is_scp_ar_type_logo(arstat);
            } else {
                ret = is_scp_ar_type_logo_border(arstat);
            }
            if flag_out {
                match arstat {
                    SCP_AR_B_UNIT | SCP_AR_B_OTHER => ret = true,
                    _ => {}
                }
                let arext = self.scp_arext(nsc);
                match arext {
                    AREXT_L_TRKEEP | AREXT_L_TRRAW | AREXT_L_EC | AREXT_L_LGADD => ret = true,
                    AREXT_L_TRCUT => {
                        ret = self.get_config(ConfigVarType::flagCutTR) != 1;
                    }
                    AREXT_L_SP => {
                        ret = self.get_config(ConfigVarType::flagCutSP) != 1;
                    }
                    AREXT_N_TRCUT | AREXT_L_ECCUT | AREXT_L_LGCUT | AREXT_N_LGCUT => ret = false,
                    AREXT_N_LGADD => ret = true,
                    _ => {}
                }
            }
        }
        ret
    }
    /// isScpChapTypeDecideFromNsc
    pub fn is_scp_chap_type_decide_from_nsc(&self, nsc: i32) -> bool {
        is_scp_chap_type_decide(self.scp_chap(nsc))
    }

    // ================= msec based lookups =================

    /// getNrfLogoFromMsec
    pub fn nrf_logo_from_msec(&self, msec_target: i32, edge: i32) -> i32 {
        let size_logo = self.size_logo();
        let mut r = -1i32;
        for i in 0..size_logo {
            let lg = &self.logo[i as usize];
            if is_logo_edge_rise(edge) && lg.rise_l <= msec_target && msec_target <= lg.rise_r {
                r = i * 2;
                break;
            }
            if is_logo_edge_fall(edge) && lg.fall_l <= msec_target && msec_target <= lg.fall_r {
                r = i * 2 + 1;
                break;
            }
        }
        r
    }
    /// getNscFromNrf
    pub fn nsc_from_nrf(&self, nrf_target: i32, msec_th: i32, chap_th: i32, flat: bool) -> i32 {
        if nrf_target < 0 || nrf_target >= self.size_logo() * 2 {
            return -1;
        }
        let w = self.wide_msec_logo_nrf(nrf_target);
        let (msec_lg_c, msec_lg_l, msec_lg_r) = (w.just, w.early, w.late);
        let mut nsc_det = -1i32;
        let mut msec_dif_det = -1i32;
        let mut chap_det = 0i32;
        let mut i = 0i32;
        let mut over = false;
        loop {
            i = self.nsc_next_scp_chap(i, chap_th);
            if i > 0 {
                let msec_i = self.msec_scp(i);
                let mut chap_i = self.scp_chap(i);
                if flat {
                    chap_i = SCP_CHAP_NONE;
                } else if chap_i >= SCP_CHAP_DECIDE {
                    chap_i = SCP_CHAP_DECIDE;
                }
                if msec_i > msec_lg_r + self.msec_val_near3 {
                    over = true;
                } else if msec_i >= msec_lg_l - self.msec_val_near3 {
                    let msec_dif_i = (msec_i - msec_lg_c).abs();
                    if (msec_dif_i <= msec_th || msec_th < 0)
                        && (nsc_det < 0
                            || chap_i > chap_det
                            || (chap_i == chap_det && msec_dif_i < msec_dif_det))
                    {
                        nsc_det = i;
                        msec_dif_det = msec_dif_i;
                        chap_det = chap_i;
                    }
                }
            }
            if !(i > 0 && !over) {
                break;
            }
        }
        nsc_det
    }
    /// getNscFromMsecFull
    pub fn nsc_from_msec_full(
        &self,
        msec_target: i32,
        msec_th: i32,
        chap_th: i32,
        noedge: i32,
    ) -> i32 {
        let num_scpos = self.size_scp();
        let mut r = -1i32;
        let mut msec_min = -1i32;
        let mut i = noedge;
        while i < num_scpos - noedge {
            let msec_i = self.scp[i as usize].msec;
            let chap_i = self.scp[i as usize].chap;
            if chap_i >= chap_th || i == 0 || i == num_scpos - 1 {
                let msec_dif = (msec_target - msec_i).abs();
                if (msec_dif <= msec_th || msec_th < 0) && (msec_dif < msec_min || msec_min < 0) {
                    r = i;
                    msec_min = msec_dif;
                }
                if msec_i >= msec_target {
                    break;
                }
            }
            i += 1;
        }
        r
    }
    /// getNscFromMsecChap
    pub fn nsc_from_msec_chap(&self, msec_target: i32, msec_th: i32, chap_th: i32) -> i32 {
        self.nsc_from_msec_full(msec_target, msec_th, chap_th, SCP_END_NOEDGE)
    }
    /// getNscFromMsecAll
    pub fn nsc_from_msec_all(&self, msec_target: i32) -> i32 {
        self.nsc_from_msec_full(
            msec_target,
            self.msec_val_near2,
            SCP_CHAP_DUPE,
            SCP_END_NOEDGE,
        )
    }
    /// getNscFromMsecAllEdgein
    pub fn nsc_from_msec_all_edgein(&self, msec_target: i32) -> i32 {
        self.nsc_from_msec_full(
            msec_target,
            self.msec_val_near2,
            SCP_CHAP_DUPE,
            SCP_END_EDGEIN,
        )
    }
    /// getNscFromWideMsecFull
    pub fn nsc_from_wide_msec_full(
        &self,
        wmsec_target: &WideMsec,
        chap_th: i32,
        noedge: i32,
    ) -> i32 {
        let num_scpos = self.size_scp();
        let mut r = -1i32;
        let mut msec_min = -1i32;
        let mut i = noedge;
        while i < num_scpos - noedge {
            let msec_i = self.scp[i as usize].msec;
            let chap_i = self.scp[i as usize].chap;
            if chap_i >= chap_th || i == 0 || i == num_scpos - 1 {
                let msec_dif = (wmsec_target.just - msec_i).abs();
                if (wmsec_target.early <= msec_i || wmsec_target.early < 0)
                    && (msec_i <= wmsec_target.late || wmsec_target.late < 0)
                    && (msec_dif < msec_min || msec_min < 0)
                {
                    r = i;
                    msec_min = msec_dif;
                }
                if msec_i >= wmsec_target.late {
                    break;
                }
            }
            i += 1;
        }
        r
    }
    /// getNscFromWideMsecByChap
    pub fn nsc_from_wide_msec_by_chap(&self, wmsec_target: &WideMsec, chap_th: i32) -> i32 {
        self.nsc_from_wide_msec_full(wmsec_target, chap_th, SCP_END_NOEDGE)
    }
    /// getRangeNscFromRangeMsec
    pub fn range_nsc_from_range_msec(&self, rmsec_st: i32, rmsec_ed: i32) -> (RangeNsc, bool) {
        let mut rnsc = RangeNsc { st: -1, ed: -1 };
        let mut det = true;
        if rmsec_st >= 0 {
            rnsc.st = self.nsc_from_msec_chap(rmsec_st, self.msec_val_lap2, SCP_CHAP_DECIDE);
            if rnsc.st < 0 {
                det = false;
            }
        }
        if rmsec_ed >= 0 {
            rnsc.ed = self.nsc_from_msec_chap(rmsec_ed, self.msec_val_lap2, SCP_CHAP_DECIDE);
            if rnsc.ed < 0 {
                det = false;
            }
        }
        (rnsc, det)
    }
    /// getRangeMsecFromRangeNsc
    pub fn range_msec_from_range_nsc(&self, rnsc_st: i32, rnsc_ed: i32) -> (i32, i32) {
        let mut st = self.msec_scp(rnsc_st);
        let mut ed = self.msec_scp(rnsc_ed);
        if rnsc_st < 0 {
            st = -1;
        }
        if rnsc_ed < 0 {
            ed = -1;
        }
        (st, ed)
    }
    /// isStillFromMsec (reference starts det=true; kept as-is)
    pub fn is_still_from_msec(&self, msec_target: i32) -> bool {
        let num_scpos = self.size_scp();
        let mut det = true;
        let mut over = false;
        let mut i = 1i32;
        while i < num_scpos - 1 && !det && !over {
            let sc = &self.scp[i as usize];
            let msec_s = sc.msmute_s;
            let msec_e = sc.msmute_e;
            if msec_s <= msec_target && msec_target <= msec_e && !sc.still {
                det = false;
            }
            if msec_s > msec_target {
                over = true;
            }
            i += 1;
        }
        det
    }
    /// isSmuteFromMsec
    pub fn is_smute_from_msec(&self, msec_target: i32) -> bool {
        let num_scpos = self.size_scp();
        let mut det = false;
        let mut over = false;
        let mut i = 1i32;
        while i < num_scpos - 1 && !det && !over {
            let sc = &self.scp[i as usize];
            let msec_s = sc.msmute_s;
            let msec_e = sc.msmute_e;
            if msec_s <= msec_target && msec_target <= msec_e {
                det = true;
            }
            if msec_s > msec_target {
                over = true;
            }
            i += 1;
        }
        det
    }
    /// isSmuteSameArea
    pub fn is_smute_same_area(&self, nsc1: i32, nsc2: i32) -> bool {
        let num = self.size_scp();
        if nsc1 > 0 && nsc1 < num - 1 && nsc2 > 0 && nsc2 < num - 1 {
            let a = &self.scp[nsc1 as usize];
            let b = &self.scp[nsc2 as usize];
            if a.msmute_s == b.msmute_s && a.msmute_e == b.msmute_e {
                return true;
            }
        }
        false
    }
    /// limitWideMsecFromRange
    pub fn limit_wmsec_from_range(wmsec: &mut WideMsec, rmsec_st: i32, rmsec_ed: i32) -> bool {
        if wmsec.late < rmsec_st && wmsec.late >= 0 && rmsec_st >= 0 {
            return false;
        }
        if wmsec.early > rmsec_ed && wmsec.early >= 0 && rmsec_ed >= 0 {
            return false;
        }
        if rmsec_st >= 0 {
            if wmsec.early < rmsec_st {
                wmsec.early = rmsec_st;
            }
            if wmsec.just < rmsec_st {
                wmsec.just = rmsec_st;
            }
        }
        if rmsec_ed >= 0 {
            if wmsec.late > rmsec_ed || wmsec.late < 0 {
                wmsec.late = rmsec_ed;
            }
            if wmsec.just > rmsec_ed || wmsec.just < 0 {
                wmsec.just = rmsec_ed;
            }
        }
        true
    }

    // ================= term =================

    /// setTermEndtype
    pub fn set_term_endtype(term: &mut Term, endtype: i32) {
        term.endfix = true;
        term.endtype = endtype;
    }
    /// getTermNext
    pub fn term_next(&self, term: &mut Term) -> bool {
        let mut ret = false;
        if !term.valid {
            term.valid = true;
            if !term.endfix {
                term.endfix = true;
                term.endtype = SCP_END_NOEDGE;
            }
            if term.ini > 0 {
                term.nsc_ed = term.ini;
            } else {
                term.nsc_ed = -1;
                term.nsc_ed = self.nsc_next_scp_decide(term.nsc_ed, term.endtype);
            }
            term.msec_ed = self.msec_scp(term.nsc_ed);
        }
        term.nsc_st = term.nsc_ed;
        term.msec_st = term.msec_ed;
        term.nsc_ed = self.nsc_next_scp_decide(term.nsc_ed, term.endtype);
        term.msec_ed = self.msec_scp(term.nsc_ed);
        if term.nsc_ed >= 0 {
            ret = true;
        }
        ret
    }
    /// getTermPrev
    pub fn term_prev(&self, term: &mut Term) -> bool {
        let mut ret = false;
        if !term.valid {
            term.valid = true;
            if !term.endfix {
                term.endfix = true;
                term.endtype = SCP_END_NOEDGE;
            }
            if term.ini > 0 {
                term.nsc_st = term.ini;
            } else {
                term.nsc_st = self.size_logo();
                term.nsc_st = self.nsc_prev_scp_decide(term.nsc_st, term.endtype);
            }
            term.msec_st = self.msec_scp(term.nsc_st);
        }
        term.nsc_ed = term.nsc_st;
        term.msec_ed = term.msec_st;
        term.nsc_st = self.nsc_prev_scp_decide(term.nsc_st, term.endtype);
        term.msec_st = self.msec_scp(term.nsc_st);
        if term.nsc_st >= 0 {
            ret = true;
        }
        ret
    }
    /// getScpArstat(Term)
    pub fn term_arstat(&self, term: &Term) -> i32 {
        self.scp_arstat(term.nsc_ed)
    }
    pub fn term_arext(&self, term: &Term) -> i32 {
        self.scp_arext(term.nsc_ed)
    }
    pub fn set_term_arstat(&mut self, term: &Term, arstat: i32) {
        self.set_scp_arstat(term.nsc_ed, arstat);
    }
    pub fn set_term_arext(&mut self, term: &Term, arext: i32) {
        self.set_scp_arext(term.nsc_ed, arext);
    }
    /// isScpArstatCmUnit(Term)
    pub fn is_term_cm_unit(&self, term: &Term) -> bool {
        let a = self.term_arstat(term);
        a == SCP_AR_N_UNIT || a == SCP_AR_N_AUNIT || a == SCP_AR_N_BUNIT
    }
    /// checkScopeTerm
    pub fn check_scope_term(&self, term: &Term, scope_st: i32, scope_ed: i32) -> bool {
        let spc = self.msec_val_spc;
        term.msec_st >= scope_st - spc && term.msec_ed <= scope_ed + spc
    }
    /// checkScopeRange
    pub fn check_scope_range(
        &self,
        bounds_st: i32,
        bounds_ed: i32,
        scope_st: i32,
        scope_ed: i32,
    ) -> bool {
        let spc = self.msec_val_spc;
        bounds_st >= scope_st - spc && bounds_ed <= scope_ed + spc
    }

    // ================= inserts =================

    /// insertScpos
    pub fn insert_scpos(
        &mut self,
        msec_dst_s: i32,
        msec_dst_bk: i32,
        nsc_mute: i32,
        stat_dst: i32,
    ) -> i32 {
        let num_scpos = self.size_scp();
        if num_scpos <= 1 {
            return -1;
        }
        let mut nsc_ins = 1i32;
        while self.msec_scp(nsc_ins) < msec_dst_s && nsc_ins < num_scpos - 1 {
            nsc_ins += 1;
        }
        let flag_ins = self.msec_scp(nsc_ins) != msec_dst_s || nsc_ins == num_scpos - 1;
        if flag_ins {
            let mut dt = ScpRecord {
                msec: msec_dst_s,
                msbk: msec_dst_bk,
                ..Default::default()
            };
            if nsc_mute >= 0 && nsc_mute < num_scpos {
                let tmp = self.scp[nsc_mute as usize];
                dt.msmute_s = tmp.msmute_s;
                dt.msmute_e = tmp.msmute_e;
            } else {
                dt.msmute_s = msec_dst_bk;
                dt.msmute_e = msec_dst_s;
            }
            dt.statpos = stat_dst;
            self.insert_scp(dt, nsc_ins);
        } else {
            let mut dt = self.scp[nsc_ins as usize];
            dt.statpos = stat_dst;
            self.set_scp(dt, nsc_ins);
        }
        nsc_ins
    }

    /// getNscForceMsec
    pub fn nsc_force_msec(&mut self, cnv: &Cnv, msec_in: i32, edge: i32) -> i32 {
        if (self.ext_opt.nl_g_exact & 0x02) != 0 {
            return self.nsc_force_exact_fix_msec(cnv, msec_in, edge);
        }
        self.nsc_force_msec_org(cnv, msec_in, edge)
    }

    /// getNscForceMsecOrg
    pub fn nsc_force_msec_org(&mut self, cnv: &Cnv, msec_in: i32, edge: i32) -> i32 {
        let msec_clr = self.msec_val_near2;
        let num_scpos = self.size_scp();
        let mut flag_search = true;
        let mut nsc_nearest = 0i32;
        let mut difmsec_nearest = 0i32;
        let mut nsc_mute = -1i32;
        if msec_in == 0 || msec_in >= self.msec_total_max {
            flag_search = false;
        }
        let mut i = 1i32;
        while flag_search {
            let msec_i = self.msec_scp_edge(i, edge);
            let difmsec_i = (msec_in - msec_i).abs();
            if difmsec_nearest > difmsec_i || nsc_nearest == 0 {
                if nsc_nearest > 0 && difmsec_nearest <= msec_clr {
                    self.set_scp_chap(nsc_nearest, SCP_CHAP_DUPE);
                }
                nsc_nearest = i;
                difmsec_nearest = difmsec_i;
            } else if difmsec_i <= msec_clr {
                self.set_scp_chap(i, SCP_CHAP_DUPE);
            }
            {
                let sc = self.scp[i as usize];
                if sc.msmute_s <= msec_i
                    && msec_i <= sc.msmute_e
                    && (msec_i <= msec_in || nsc_nearest == i)
                {
                    nsc_mute = i;
                }
            }
            i += 1;
            if i >= num_scpos - 1 || msec_i >= msec_in + msec_clr {
                flag_search = false;
            }
        }
        let mut msec_in_s = msec_in;
        let mut msec_in_bk = msec_in;
        if edge == EDGE_RISE {
            msec_in_bk = cnv.tc.get_msec_adjust_frm_from_msec(msec_in as i64, -1) as i32;
        } else {
            msec_in_s = cnv.tc.get_msec_adjust_frm_from_msec(msec_in as i64, 1) as i32;
        }
        let mut nsc_ret = nsc_nearest;
        let mut flag_new = true;
        if msec_in == 0 || msec_in >= self.msec_total_max {
            flag_new = false;
            nsc_ret = if msec_in == 0 { 0 } else { num_scpos - 1 };
        } else if nsc_nearest > 0 {
            let msec_near_s = self.msec_scp(nsc_nearest);
            let msec_near_bk = self.msec_scp_bk(nsc_nearest);
            if msec_near_bk <= msec_in_s && msec_in_bk <= msec_near_s {
                flag_new = false;
            }
        }
        if flag_new {
            nsc_ret = self.insert_scpos(msec_in_s, msec_in_bk, nsc_mute, PRIOR_DECIDE);
        }
        nsc_ret
    }

    /// getNscForceExactFixMsec
    pub fn nsc_force_exact_fix_msec(&mut self, cnv: &Cnv, msec_in: i32, edge: i32) -> i32 {
        let msec_clr = self.msec_val_near2;
        let num_scpos = self.size_scp();
        let mut flag_search = true;
        let mut nsc_nearest = 0i32;
        let mut difmsec_nearest = 0i32;
        let mut nsc_mute = -1i32;
        if msec_in == 0 || msec_in >= self.msec_total_max {
            flag_search = false;
        }
        let frm_in = cnv.tc.get_frm_from_msec(msec_in as i64);
        let mut flag_exact_nearest = false;
        let mut flag_fc_nearest = false;
        let mut msec_fc_prev = 0i32;
        let mut msec_fc_over = 0i32;
        let mut i = 1i32;
        while flag_search {
            let dtscp = self.scp[i as usize];
            let msec_i = self.msec_scp_edge(i, edge);
            let difmsec_i = (msec_in - msec_i).abs();
            let chap_i = self.scp_chap(i);
            let flag_chap_fc = chap_i == SCP_CHAP_DFORCE || chap_i == SCP_CHAP_DUNIT;
            let frm_st = cnv.tc.get_frm_from_msec(dtscp.msec as i64);
            let frm_bk = cnv.tc.get_frm_from_msec(dtscp.msbk as i64);
            let mut flag_loc_same = false;
            if frm_st - 2 <= frm_in
                && frm_in <= frm_bk + 2
                && (frm_bk <= frm_in || !is_logo_edge_rise(edge))
                && (frm_st >= frm_in || !is_logo_edge_fall(edge))
            {
                flag_loc_same = true;
            }
            if difmsec_nearest > difmsec_i || nsc_nearest == 0 {
                let mut flag_erase = false;
                let flag_change;
                if flag_loc_same {
                    flag_change = true;
                    flag_exact_nearest = true;
                } else if (msec_fc_prev >= msec_i && msec_fc_prev > 0)
                    || (msec_fc_over <= msec_i && msec_fc_over > 0)
                {
                    flag_change = false;
                } else if flag_chap_fc {
                    flag_change = false;
                    if msec_i < msec_in && (msec_fc_prev < msec_i || msec_fc_prev == 0) {
                        msec_fc_prev = msec_i;
                        if nsc_nearest > 0 && nsc_nearest < i {
                            flag_erase = true;
                        }
                    } else if msec_i > msec_in && (msec_fc_over > msec_i || msec_fc_over == 0) {
                        msec_fc_over = msec_i;
                    }
                } else {
                    flag_change = true;
                }
                if flag_change || flag_erase {
                    if nsc_nearest > 0 && difmsec_nearest <= msec_clr && !flag_fc_nearest {
                        self.set_scp_chap(nsc_nearest, SCP_CHAP_DUPE);
                    }
                    nsc_nearest = 0;
                }
                if flag_change {
                    nsc_nearest = i;
                    difmsec_nearest = difmsec_i;
                    flag_fc_nearest = flag_chap_fc;
                }
            } else if difmsec_i <= msec_clr && !flag_chap_fc {
                self.set_scp_chap(i, SCP_CHAP_DUPE);
            }
            if dtscp.msmute_s <= msec_i
                && msec_i <= dtscp.msmute_e
                && (msec_i <= msec_in || nsc_nearest == i)
            {
                nsc_mute = i;
            }
            i += 1;
            if i >= num_scpos - 1 || msec_i >= msec_in + msec_clr {
                flag_search = false;
            }
        }
        let mut msec_in_s = msec_in;
        let mut msec_in_bk = msec_in;
        if edge == EDGE_RISE {
            msec_in_bk = cnv.tc.get_msec_adjust_frm_from_msec(msec_in as i64, -1) as i32;
        } else {
            msec_in_s = cnv.tc.get_msec_adjust_frm_from_msec(msec_in as i64, 1) as i32;
        }
        let mut nsc_ret = nsc_nearest;
        let mut flag_new = true;
        if msec_in == 0 || msec_in >= self.msec_total_max {
            flag_new = false;
            nsc_ret = if msec_in == 0 { 0 } else { num_scpos - 1 };
        } else if nsc_nearest > 0 && flag_exact_nearest {
            flag_new = false;
            let mut dtscp = self.scp[nsc_nearest as usize];
            let frm_st = cnv.tc.get_frm_from_msec(dtscp.msec as i64);
            let frm_bk = cnv.tc.get_frm_from_msec(dtscp.msbk as i64);
            if is_logo_edge_rise(edge) && frm_st != frm_in {
                dtscp.msec = msec_in;
            }
            if is_logo_edge_fall(edge) && frm_bk != frm_in {
                dtscp.msbk = msec_in;
            }
            self.set_scp(dtscp, nsc_nearest);
        }
        if flag_new {
            nsc_ret = self.insert_scpos(msec_in_s, msec_in_bk, nsc_mute, PRIOR_DECIDE);
        }
        nsc_ret
    }

    // ================= result / component =================

    /// getResultLogoNext
    pub fn result_logo_next(&self, cnv: &Cnv, nlg: i32) -> (i32, i32, i32, bool) {
        let msec_val_cont = 80;
        let size_logo = self.size_logo();
        let mut cont_next = false;
        let mut msec_rise = -1i32;
        let mut msec_fall = -1i32;
        let mut nlg_rise = nlg;
        let mut flag_rise = false;
        while nlg_rise >= 0 && nlg_rise < size_logo && !flag_rise {
            let dt = &self.logo[nlg_rise as usize];
            if dt.outtype_rise == LOGO_RESULT_DECIDE {
                flag_rise = true;
                msec_rise = dt.result_rise;
            }
            if !flag_rise {
                nlg_rise += 1;
            }
        }
        let mut nlg_fall = nlg_rise;
        if !flag_rise {
            nlg_fall = nlg;
        }
        let mut flag_fall = false;
        let mut flag_unit = false;
        while nlg_fall >= 0 && nlg_fall < size_logo && !flag_fall {
            let dt = &self.logo[nlg_fall as usize];
            if dt.outtype_fall == LOGO_RESULT_DECIDE {
                flag_fall = true;
                msec_fall = dt.result_fall;
                if dt.unit != 0 {
                    flag_unit = true;
                }
            }
            if !flag_fall {
                nlg_fall += 1;
            }
        }
        if flag_fall {
            let mut nlg_next = nlg_fall + 1;
            let mut flag_next = false;
            while nlg_next >= 0 && nlg_next < size_logo && !flag_next {
                let dt = &self.logo[nlg_next as usize];
                if dt.outtype_rise == LOGO_RESULT_DECIDE {
                    flag_next = true;
                    let msec_next = dt.result_rise;
                    if msec_fall + msec_val_cont > msec_next {
                        if (self.ext_opt.nl_g_exact & 0x02) != 0 {
                            msec_fall =
                                cnv.tc.get_msec_adjust_frm_from_msec(msec_next as i64, -1) as i32;
                        }
                        if !flag_unit {
                            cont_next = true;
                        }
                    }
                }
                nlg_next += 1;
            }
        }
        let mut nlg_ret = -1i32;
        if flag_rise && flag_fall {
            nlg_ret = nlg_fall + 1;
        } else {
            if (flag_rise || nlg == 0) && !flag_fall {
                nlg_ret = size_logo;
                msec_fall = self.msec_total_max;
            }
            if nlg == 0 {
                msec_rise = 0;
            }
        }
        (nlg_ret, msec_rise, msec_fall, cont_next)
    }

    /// getSecLogoComponent
    pub fn sec_logo_component(&self, cnv: &Cnv, msec_s: i32, msec_e: i32) -> i32 {
        if self.is_auto_logo_only() {
            return self.sec_logo_component_from_elg(cnv, msec_s, msec_e);
        }
        self.sec_logo_component_from_logo(cnv, msec_s, msec_e)
    }
    pub fn sec_logo_component_from_elg(&self, cnv: &Cnv, msec_s: i32, msec_e: i32) -> i32 {
        let mut sum_msec = 0i32;
        let mut elg = ElgCurrent::default();
        while self.elg_pt_next(&mut elg) {
            if elg.msec_rise + self.msec_val_spc < msec_e
                && elg.msec_fall > msec_s + self.msec_val_spc
            {
                let tmp_s = if elg.msec_rise < msec_s {
                    msec_s
                } else {
                    elg.msec_rise
                };
                let tmp_e = if elg.msec_fall > msec_e {
                    msec_e
                } else {
                    elg.msec_fall
                };
                if tmp_s < tmp_e {
                    sum_msec += tmp_e - tmp_s;
                }
            }
        }
        cnv.get_sec_from_msec(sum_msec)
    }
    /// getSecLogoComponentFromLogo (full version with edge processing)
    pub fn sec_logo_component_from_logo(&self, cnv: &Cnv, msec_s: i32, msec_e: i32) -> i32 {
        let mut logopt = NrfCurrent::default();
        let mut sum_msec = 0i32;
        let mut msec_dif_mid = (msec_e - msec_s).abs() / 2;
        let mut msec_dif_min = (msec_e - msec_s).abs() / 4;
        if msec_dif_mid > 11500 {
            msec_dif_mid = 11500;
        }
        if msec_dif_min > 4500 {
            msec_dif_min = 4500;
        }
        while self.nrf_pt_next(&mut logopt, LOGO_SELECT_VALID) {
            if logopt.msec_rise + self.msec_val_spc < msec_e
                && logopt.msec_fall > msec_s + self.msec_val_spc
            {
                let mut tmp_s = if logopt.msec_rise < msec_s {
                    msec_s
                } else {
                    logopt.msec_rise
                };
                let mut tmp_e = if logopt.msec_fall > msec_e {
                    msec_e
                } else {
                    logopt.msec_fall
                };
                // 端部分の処理(rise)
                {
                    let wmsec = self.wide_msec_logo_nrf(logopt.nrf_rise);
                    if logopt.msec_rise < msec_s + msec_dif_mid
                        && logopt.msec_fall >= msec_e
                        && wmsec.early <= msec_s + self.msec_val_spc
                        && tmp_s > msec_s
                    {
                        tmp_s = msec_s;
                    }
                    if logopt.msec_rise > msec_e - msec_dif_min
                        || (logopt.msec_rise > msec_e - msec_dif_mid
                            && logopt.msec_fall >= msec_e
                            && wmsec.late >= msec_e - msec_dif_min)
                    {
                        tmp_s = tmp_e;
                    }
                }
                // 端部分の処理(fall)
                {
                    let wmsec = self.wide_msec_logo_nrf(logopt.nrf_fall);
                    if logopt.msec_fall > msec_e - msec_dif_mid
                        && logopt.msec_rise <= msec_s
                        && wmsec.late >= msec_e - self.msec_val_spc
                        && tmp_e < msec_e
                    {
                        tmp_e = msec_e;
                    }
                    if logopt.msec_fall < msec_s + msec_dif_min {
                        tmp_e = tmp_s;
                    } else if logopt.msec_fall < msec_s + msec_dif_mid
                        && logopt.msec_rise <= msec_s
                        && wmsec.early <= msec_s + msec_dif_min
                    {
                        tmp_s = tmp_e;
                    }
                }
                if tmp_s < tmp_e {
                    sum_msec += tmp_e - tmp_s;
                }
            }
        }
        cnv.get_sec_from_msec(sum_msec)
    }

    /// insertLogo (business span insert)
    pub fn logo_insert(
        &mut self,
        msec_st: i32,
        msec_ed: i32,
        overlap: bool,
        confirm: bool,
        unit: bool,
    ) -> i32 {
        let size_logo = self.size_logo();
        let wid_ovl = self.msec_val_spc;
        let mut num_ins = -1i32;
        for i in 1..size_logo {
            if msec_st >= self.msec_logo_fall(i - 1) && msec_ed <= self.msec_logo_rise(i) {
                num_ins = i;
            }
        }
        if num_ins < 0 {
            if msec_ed <= self.msec_logo_rise(0) {
                num_ins = 0;
            } else if msec_st >= self.msec_logo_fall(size_logo - 1) && size_logo > 0 {
                num_ins = size_logo;
            }
        }
        if size_logo >= MAXSIZE_LOGO {
            num_ins = -1;
        }
        let mut total_ins = 1i32;
        let mut bak_result_rise = 0i32;
        let mut bak_result_fall = 0i32;
        let mut flag_bak_rise = false;
        let mut flag_bak_fall = false;
        let mut bak_outtype_rise = LOGO_RESULT_NONE;
        let mut bak_outtype_fall = LOGO_RESULT_NONE;
        if overlap && num_ins < 0 && size_logo < MAXSIZE_LOGO {
            for i in 0..size_logo {
                let mut dt = self.logo[i as usize];
                if (msec_st - dt.rise).abs() <= wid_ovl {
                    flag_bak_rise = true;
                    bak_outtype_rise = dt.outtype_rise;
                    bak_result_rise = dt.result_rise;
                }
                if (msec_ed - dt.fall).abs() <= wid_ovl {
                    flag_bak_fall = true;
                    bak_outtype_fall = dt.outtype_fall;
                    bak_result_fall = dt.result_fall;
                }
                if msec_st >= dt.rise - wid_ovl
                    && msec_st <= dt.rise + wid_ovl
                    && msec_ed >= dt.fall - wid_ovl
                    && msec_ed <= dt.fall + wid_ovl
                {
                    return 0;
                }
                if msec_st > dt.rise + wid_ovl && msec_st < dt.fall && msec_ed > dt.fall - wid_ovl {
                    dt.fall = msec_st;
                    dt.fall_l = msec_st;
                    dt.fall_r = msec_st;
                    dt.fade_fall = 0;
                    dt.intl_fall = 0;
                    dt.stat_fall = LOGO_PRIOR_DECIDE;
                    dt.outtype_fall = LOGO_RESULT_NONE;
                    self.set_logo(dt, i);
                    if confirm {
                        self.set_result_logo_at_nrf(dt.fall, LOGO_RESULT_DECIDE, i * 2 + 1);
                    }
                }
                if msec_st < dt.rise + wid_ovl && msec_ed > dt.rise && msec_ed < dt.fall - wid_ovl {
                    dt.rise = msec_ed;
                    dt.rise_l = msec_ed;
                    dt.rise_r = msec_ed;
                    dt.fade_rise = 0;
                    dt.intl_rise = 0;
                    dt.stat_rise = LOGO_PRIOR_DECIDE;
                    dt.outtype_rise = LOGO_RESULT_NONE;
                    self.set_logo(dt, i);
                    if confirm {
                        self.set_result_logo_at_nrf(dt.rise, LOGO_RESULT_DECIDE, i * 2);
                    }
                }
                if msec_st - wid_ovl <= dt.rise && msec_ed + wid_ovl >= dt.fall {
                    dt.outtype_rise = LOGO_RESULT_ABORT;
                    dt.outtype_fall = LOGO_RESULT_ABORT;
                    dt.result_rise = -1;
                    dt.result_fall = -1;
                    self.set_logo(dt, i);
                    if num_ins < 0 {
                        num_ins = i;
                        total_ins = 0;
                    }
                }
                if msec_st > dt.rise + wid_ovl && msec_ed < dt.fall - wid_ovl {
                    num_ins = i + 1;
                    total_ins = 2;
                }
                if num_ins < 0 && msec_st <= dt.rise {
                    num_ins = i;
                }
            }
            if num_ins < 0 {
                num_ins = size_logo;
            }
        }
        if size_logo + total_ins > MAXSIZE_LOGO {
            num_ins = -1;
        }
        let mut retval = -1i32;
        if num_ins >= 0 {
            if total_ins == 2 {
                let mut dt = self.logo[num_ins as usize];
                dt.rise = msec_ed;
                dt.rise_l = msec_ed;
                dt.rise_r = msec_ed;
                dt.fade_rise = 0;
                dt.intl_rise = 0;
                dt.stat_rise = LOGO_PRIOR_DECIDE;
                if confirm {
                    dt.outtype_rise = LOGO_RESULT_DECIDE;
                    dt.result_rise = msec_ed;
                } else {
                    dt.outtype_rise = LOGO_RESULT_NONE;
                    dt.result_rise = 0;
                }
                self.insert_logo(dt, num_ins + 1);

                let mut dt = self.logo[num_ins as usize];
                dt.fall = msec_st;
                dt.fall_l = msec_st;
                dt.fall_r = msec_st;
                dt.fade_fall = 0;
                dt.intl_fall = 0;
                dt.stat_fall = LOGO_PRIOR_DECIDE;
                if confirm {
                    dt.outtype_fall = LOGO_RESULT_DECIDE;
                    dt.result_fall = msec_st;
                } else {
                    dt.outtype_fall = LOGO_RESULT_NONE;
                    dt.result_fall = 0;
                }
                self.set_logo(dt, num_ins);
                num_ins += 1;
            }
            retval = num_ins;
            let mut dt = self.logo[num_ins as usize];
            dt.rise = msec_st;
            dt.rise_l = msec_st;
            dt.rise_r = msec_st;
            dt.fall = msec_ed;
            dt.fall_l = msec_ed;
            dt.fall_r = msec_ed;
            dt.fade_rise = 0;
            dt.fade_fall = 0;
            dt.intl_rise = 0;
            dt.intl_fall = 0;
            dt.stat_rise = LOGO_PRIOR_DECIDE;
            dt.stat_fall = LOGO_PRIOR_DECIDE;
            if confirm {
                dt.outtype_rise = LOGO_RESULT_DECIDE;
                dt.outtype_fall = LOGO_RESULT_DECIDE;
                dt.result_rise = msec_st;
                dt.result_fall = msec_ed;
            } else {
                dt.outtype_rise = LOGO_RESULT_NONE;
                dt.outtype_fall = LOGO_RESULT_NONE;
                dt.result_rise = 0;
                dt.result_fall = 0;
            }
            if flag_bak_rise {
                dt.outtype_rise = bak_outtype_rise;
                dt.result_rise = bak_result_rise;
            }
            if flag_bak_fall {
                dt.outtype_fall = bak_outtype_fall;
                dt.result_fall = bak_result_fall;
            }
            dt.unit = LOGO_UNIT_NORMAL;
            if unit {
                dt.unit = LOGO_UNIT_DIVIDE;
            }
            if total_ins == 0 {
                self.set_logo(dt, num_ins);
            } else {
                self.insert_logo(dt, num_ins);
            }
        }
        retval
    }
}

pub const MAXSIZE_LOGO: i32 = 1024;
pub const LOGO_PRIOR_NONE: i32 = 0;
pub const LOGO_SELECT_ALL: i32 = 0;
pub const LOGO_SELECT_VALID: i32 = 1;
pub const LOGO_UNIT_NORMAL: i32 = 0;
pub const LOGO_UNIT_DIVIDE: i32 = 1;
pub const LOGO_PRIOR_DECIDE: i32 = 2;
