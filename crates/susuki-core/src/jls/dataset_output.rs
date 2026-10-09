//! Derived from join_logo_scp (https://github.com/tobitti0/join_logo_scp,
//! commit d1a24d758f8b3c04352b82ff6756fdf3d56729b3), Copyright (C) tobitti0,
//! GPL-2.0-or-later. This port is a GPL derivative work, redistributed here
//! under GPL-3.0-or-later (design §10.6).
//! Port of JlsDataset.cpp output layer: outputResultTrimGen*, detail
//! get-line strings, debug display (result of design 7 §9 / 8.4).

use super::cnvstr::Cnv;
use super::dataset::*;
use super::dataset_search::*;
use super::tables::ConfigVarType;

#[derive(Debug, Clone, Copy, Default)]
pub struct DetailRec {
    pub frm_from: i32,
    pub frm_to: i32,
    pub sec_dif: i32,
    pub frm_dif: i32,
    pub sec_logo: i32,
    pub arstat: i32,
    pub arext: i32,
    pub with_label: bool,
}

fn fmt_w(v: i32, width: usize) -> String {
    let s = v.to_string();
    if s.len() >= width {
        s
    } else {
        let mut out = " ".repeat(width - s.len());
        out.push_str(&s);
        out
    }
}

impl Dataset {
    /// outputResultTrimGen
    pub fn output_result_trim_gen(&mut self, cnv: &Cnv) {
        self.result_trim.clear();
        if self.is_auto_mode_use() {
            self.output_result_trim_gen_auto();
        } else {
            self.output_result_trim_gen_manual(cnv);
        }
    }

    /// outputResultTrimGenManual
    pub fn output_result_trim_gen_manual(&mut self, cnv: &Cnv) {
        let mut nlg_fall = 0i32;
        loop {
            let (msec_rise, mut msec_fall, mut cont_next);
            let ret = self.result_logo_next(cnv, nlg_fall);
            nlg_fall = ret.0;
            msec_rise = ret.1;
            msec_fall = ret.2;
            cont_next = ret.3;
            if nlg_fall >= 0 {
                while cont_next && nlg_fall >= 0 {
                    let ret = self.result_logo_next(cnv, nlg_fall);
                    nlg_fall = ret.0;
                    msec_fall = ret.2;
                    cont_next = ret.3;
                }
                self.result_trim.push(msec_rise);
                self.result_trim.push(msec_fall);
            }
            if nlg_fall == 0 {
                nlg_fall = -1; // logo が全くない場合は終了
            }
            if !(nlg_fall >= 0) {
                break;
            }
        }
    }

    /// outputResultTrimGenAuto
    pub fn output_result_trim_gen_auto(&mut self) {
        let mut elg = ElgCurrent {
            outflag: true,
            ..Default::default()
        };
        while self.elg_pt_next(&mut elg) {
            self.result_trim.push(elg.msec_rise);
            self.result_trim.push(elg.msec_fall);
        }
    }

    /// outputResultDetailReset
    pub fn output_result_detail_reset(&mut self) {
        self.nsc_out_detail = 0;
    }

    /// outputResultDetailGetLine: (eof, record). The display string is
    /// rebuilt by the caller from these fields (same inputs as the
    /// reference's snprintf).
    pub fn output_result_detail_get_line(&mut self, cnv: &Cnv) -> (bool, DetailRec) {
        let num_scpos = self.size_scp();
        let mut i = self.nsc_out_detail;
        if i < 0 || i >= num_scpos - 1 {
            return (true, DetailRec::default());
        }
        let msec_from = self.msec_scp(i);
        let mut msec_next;
        loop {
            i = self.nsc_next_scp_output(i, SCP_END_EDGEIN);
            msec_next = self.msec_scp(i);
            if !(msec_from == msec_next && i >= 0) {
                break;
            }
        }
        self.nsc_out_detail = i;
        if i < 0 {
            return (true, DetailRec::default());
        }
        let msec_dif = msec_next - msec_from;
        let sec_dif = cnv.get_sec_from_msec(msec_dif);
        let frm_from = cnv.tc.get_frm_from_msec(msec_from as i64) as i32;
        let frm_next = cnv.tc.get_frm_from_msec(msec_next as i64) as i32;
        let frm_dif = frm_next
            - cnv
                .tc
                .get_frm_from_msec((sec_dif * 1000 + msec_from) as i64) as i32;
        let sec_logo = self.sec_logo_component(cnv, msec_from, msec_next);
        let msec_to = self.msec_scp_bk(i);
        let frm_to = cnv.tc.get_frm_from_msec(msec_to as i64) as i32;
        let arstat = self.scp_arstat(i);
        let arext = self.scp_arext(i);
        (
            false,
            DetailRec {
                frm_from,
                frm_to,
                sec_dif,
                frm_dif,
                sec_logo,
                arstat,
                arext,
                with_label: self.get_flag_setup_auto() > 1,
            },
        )
    }

    /// reference line string rebuilt from a record (for trace compat)
    pub fn detail_line_string(&self, cnv: &Cnv, r: &DetailRec) -> Vec<u8> {
        let _ = cnv;
        let mut s = format!(
            "{} {} {} {} {} ",
            fmt_w(r.frm_from, 6),
            fmt_w(r.frm_to, 6),
            fmt_w(r.sec_dif, 4),
            fmt_w(r.frm_dif, 3),
            fmt_w(r.sec_logo, 4)
        )
        .into_bytes();
        if r.with_label {
            s.extend_from_slice(&self.output_result_detail_get_line_label(r.arstat, r.arext));
        }
        s
    }

    /// outputResultDetailGetLineLabel (label bytes only)
    pub fn output_result_detail_get_line_label(&self, arstat: i32, arext: i32) -> Vec<u8> {
        let mut pstr: &[u8];
        match arstat {
            SCP_AR_N_UNIT | SCP_AR_N_AUNIT | SCP_AR_N_BUNIT => pstr = b":CM",
            SCP_AR_N_OTHER => pstr = b":Nologo",
            SCP_AR_B_UNIT => pstr = b":Border15s",
            SCP_AR_B_OTHER => pstr = b":Border",
            SCP_AR_L_UNIT | SCP_AR_L_OTHER => pstr = b":L",
            SCP_AR_L_MIXED => pstr = b":Mix",
            _ => pstr = b":",
        }
        match arext {
            AREXT_L_TRKEEP => pstr = b":Trailer(add)",
            AREXT_L_ECCUT => pstr = b":Trailer(cut)",
            AREXT_L_TRRAW => pstr = b":Trailer",
            AREXT_L_TRCUT => {
                pstr = if self.get_config(ConfigVarType::flagCutTR) == 1 {
                    b":Trailer(cut)"
                } else {
                    b":Trailer(cut-cancel)"
                }
            }
            AREXT_L_SP => {
                pstr = if self.get_config(ConfigVarType::flagCutSP) == 1 {
                    b":Sponsor(cut)"
                } else {
                    b":Sponsor(add)"
                }
            }
            AREXT_L_EC => pstr = b":Endcard(add)",
            AREXT_L_LGCUT => pstr = b":L-Edge(cut)",
            AREXT_L_LGADD => pstr = b":L-Edge(add)",
            AREXT_N_TRCUT => pstr = b":Nologo(cut)",
            AREXT_N_LGCUT => pstr = b":N-Edge(cut)",
            AREXT_N_LGADD => pstr = b":N-Edge(add)",
            _ => {}
        }
        pstr.to_vec()
    }

    /// displayLogo (debug printf)
    pub fn display_logo(&self, cnv: &Cnv, out: &mut Vec<u8>) {
        for i in 0..self.size_logo() {
            let l = &self.logo[i as usize];
            out.extend_from_slice(
                format!(
                    "{} {} {} {} {} {} {} {}{}{}{} {} {} {} {} {} {} {}\n",
                    fmt_w(i, 3),
                    fmt_w(cnv.tc.get_frm_from_msec(l.rise as i64) as i32, 6),
                    fmt_w(cnv.tc.get_frm_from_msec(l.fall as i64) as i32, 6),
                    fmt_w(cnv.tc.get_frm_from_msec(l.rise_l as i64) as i32, 6),
                    fmt_w(cnv.tc.get_frm_from_msec(l.rise_r as i64) as i32, 6),
                    fmt_w(cnv.tc.get_frm_from_msec(l.fall_l as i64) as i32, 6),
                    fmt_w(cnv.tc.get_frm_from_msec(l.fall_r as i64) as i32, 6),
                    l.fade_rise,
                    l.fade_fall,
                    l.intl_rise,
                    l.intl_fall,
                    l.stat_rise,
                    l.stat_fall,
                    l.unit,
                    l.outtype_rise,
                    l.outtype_fall,
                    fmt_w(cnv.tc.get_frm_from_msec(l.result_rise as i64) as i32, 6),
                    fmt_w(cnv.tc.get_frm_from_msec(l.result_fall as i64) as i32, 6),
                )
                .as_bytes(),
            );
        }
    }

    /// displayScp (debug printf)
    pub fn display_scp(&self, cnv: &Cnv, out: &mut Vec<u8>) {
        for i in 0..self.size_scp() {
            let r = &self.scp[i as usize];
            out.extend_from_slice(
                format!(
                    "{} {} {} {} {} {} {} ({} {} {} {}){} {} {} {})\n",
                    fmt_w(i, 3),
                    fmt_w(cnv.tc.get_frm_from_msec(r.msec as i64) as i32, 6),
                    fmt_w(cnv.tc.get_frm_from_msec(r.msbk as i64) as i32, 6),
                    fmt_w(cnv.tc.get_frm_from_msec(r.msmute_s as i64) as i32, 6),
                    fmt_w(cnv.tc.get_frm_from_msec(r.msmute_e as i64) as i32, 6),
                    r.still as i32,
                    r.statpos,
                    fmt_w(r.msec, 8),
                    fmt_w(r.msbk, 8),
                    fmt_w(r.msmute_s, 8),
                    fmt_w(r.msmute_e, 8),
                    fmt_w(r.score, 4),
                    fmt_w(r.chap, 2),
                    fmt_w(r.arstat, 2),
                    fmt_w(r.arext, 2),
                )
                .as_bytes(),
            );
        }
    }
}
