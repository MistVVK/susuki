//! Derived from join_logo_scp (https://github.com/tobitti0/join_logo_scp,
//! commit d1a24d758f8b3c04352b82ff6756fdf3d56729b3), Copyright (C) tobitti0,
//! GPL-2.0-or-later. This port is a GPL derivative work, redistributed here
//! under GPL-3.0-or-later (design §10.6).
//! Port of JlsCmdLimit (JlsCmdSet.hpp/.cpp:337-616): per-command target
//! limitation state. A fresh instance is created per script line, as in the
//! reference (JlsCmdSet is a local in startCmdLoopSub).

use super::cmdarg::WideMsec;

pub const PROC_HEADTAIL: i32 = 0x01;
pub const PROC_FRAMELIMIT: i32 = 0x02;
pub const PROC_VALIDLOGO: i32 = 0x04;
pub const PROC_BASELOGO: i32 = 0x08;
pub const PROC_TARGETRANGE: i32 = 0x10;
pub const PROC_SCPENABLE: i32 = 0x20;
pub const PROC_RESULT: i32 = 0x40;

#[derive(Clone, Copy)]
pub struct ArgValidLogo {
    pub msec: i32,
    pub edge: i32,
}

pub struct CmdLimit {
    pub rmsec_head: i32,
    pub rmsec_tail: i32,
    pub frame_st: i32,
    pub frame_ed: i32,
    pub list_valid_logo: Vec<ArgValidLogo>,
    pub nrf_base: i32,
    pub nsc_base: i32,
    pub edge_base: i32,
    pub list_wmsec_logo_base: Vec<WideMsec>,
    pub loc_msec_logo_base: i32,
    pub wmsec_target: WideMsec,
    pub msec_target_fc: i32,
    pub from_logo: bool,
    pub list_tl_range: Vec<(i32, i32)>,
    pub list_scp_enable: Vec<bool>,
    pub nsc_sel: i32,
    pub nsc_end: i32,
    pub edge_out_pos: i32,
    pub process: i32,
    /// reference writes straight to cerr; buffered here and drained by Exec
    pub err_buf: Vec<u8>,
}

impl CmdLimit {
    pub fn new() -> Self {
        let mut c = CmdLimit {
            rmsec_head: -1,
            rmsec_tail: -1,
            frame_st: -1,
            frame_ed: -1,
            list_valid_logo: Vec::new(),
            nrf_base: -1,
            nsc_base: -1,
            edge_base: 0,
            list_wmsec_logo_base: Vec::new(),
            loc_msec_logo_base: -1,
            wmsec_target: WideMsec {
                just: -1,
                early: -1,
                late: -1,
            },
            msec_target_fc: -1,
            from_logo: false,
            list_tl_range: Vec::new(),
            list_scp_enable: Vec::new(),
            nsc_sel: -1,
            nsc_end: -1,
            edge_out_pos: 0,
            process: 0,
            err_buf: Vec::new(),
        };
        c.clear();
        c
    }

    pub fn clear(&mut self) {
        self.process = 0;
        self.rmsec_head = -1;
        self.rmsec_tail = -1;
        self.frame_st = -1;
        self.frame_ed = -1;
        self.list_valid_logo.clear();
        self.nrf_base = -1;
        self.nsc_base = -1;
        self.edge_base = 0; // LOGO_EDGE_RISE
        self.list_wmsec_logo_base.clear();
        self.loc_msec_logo_base = -1;
        self.wmsec_target = WideMsec {
            just: -1,
            early: -1,
            late: -1,
        };
        self.msec_target_fc = -1;
        self.from_logo = false;
        self.list_tl_range.clear();
        self.list_scp_enable.clear();
        self.nsc_sel = -1;
        self.nsc_end = -1;
        self.edge_out_pos = 0;
    }

    fn signal_internal_error(&mut self, flags: i32) {
        eprintln!(
            "error:internal flow at ArgCmdLimit flag={},process={}",
            flags, self.process
        );
    }

    pub fn get_head(&self) -> i32 {
        self.rmsec_head
    }
    pub fn get_tail(&self) -> i32 {
        self.rmsec_tail
    }
    pub fn set_head_tail(&mut self, st: i32, ed: i32) -> bool {
        self.process |= PROC_HEADTAIL;
        self.rmsec_head = st;
        self.rmsec_tail = ed;
        true
    }
    pub fn get_frame_range(&self) -> (i32, i32) {
        (self.frame_st, self.frame_ed)
    }
    pub fn set_frame_range(&mut self, st: i32, ed: i32) -> bool {
        if self.process & PROC_HEADTAIL == 0 {
            self.signal_internal_error(PROC_FRAMELIMIT);
        }
        self.process |= PROC_FRAMELIMIT;
        self.frame_st = st;
        self.frame_ed = ed;
        true
    }

    pub fn get_logo_list_msec(&self, nlist: i32) -> i32 {
        if nlist < 0 || nlist >= self.list_valid_logo.len() as i32 {
            return -1;
        }
        self.list_valid_logo[nlist as usize].msec
    }
    pub fn get_logo_list_edge(&self, nlist: i32) -> i32 {
        if nlist < 0 || nlist >= self.list_valid_logo.len() as i32 {
            return 0;
        }
        self.list_valid_logo[nlist as usize].edge
    }
    pub fn add_logo_list(&mut self, msec: i32, edge: i32) -> bool {
        if self.process & PROC_HEADTAIL == 0 {
            self.signal_internal_error(PROC_VALIDLOGO);
        }
        self.process |= PROC_VALIDLOGO;
        self.list_valid_logo.push(ArgValidLogo { msec, edge });
        true
    }
    pub fn size_logo_list(&self) -> i32 {
        self.list_valid_logo.len() as i32
    }

    pub fn get_logo_base_nrf(&self) -> i32 {
        self.nrf_base
    }
    pub fn get_logo_base_nsc(&self) -> i32 {
        self.nsc_base
    }
    pub fn get_logo_base_edge(&self) -> i32 {
        self.edge_base
    }
    pub fn set_logo_base_nrf(&mut self, nrf: i32, edge: i32) -> bool {
        if self.process & PROC_VALIDLOGO == 0 {
            self.signal_internal_error(PROC_BASELOGO);
        }
        self.process |= PROC_BASELOGO;
        self.nrf_base = nrf;
        self.nsc_base = -1;
        self.edge_base = edge;
        true
    }
    pub fn set_logo_base_nsc(&mut self, nsc: i32, edge: i32) -> bool {
        if self.process & PROC_VALIDLOGO == 0 {
            self.signal_internal_error(PROC_BASELOGO);
        }
        self.process |= PROC_BASELOGO;
        self.nrf_base = -1;
        self.nsc_base = nsc;
        self.edge_base = edge;
        true
    }

    pub fn set_logo_wmsec_list(&mut self, list: &[WideMsec], loc_base: i32) {
        self.list_wmsec_logo_base = list.to_vec();
        self.loc_msec_logo_base = loc_base;
    }
    pub fn get_logo_wmsec_base(&mut self, step: i32, flag_wide: bool) -> WideMsec {
        self.get_logo_wmsec_base_shift(step, flag_wide, 0)
    }
    pub fn get_logo_wmsec_base_shift(&self, step: i32, flag_wide: bool, sft: i32) -> WideMsec {
        let loc_md = self.loc_msec_logo_base + sft;
        let loc_st = self.loc_msec_logo_base + sft - step;
        let loc_ed = self.loc_msec_logo_base + sft + step;
        let loc_size = self.list_wmsec_logo_base.len() as i32;
        let list = &self.list_wmsec_logo_base;
        let mut w = WideMsec {
            just: -1,
            early: -1,
            late: -1,
        };
        if 0 <= loc_md && loc_md < loc_size {
            w.just = list[loc_md as usize].just;
        }
        if 0 <= loc_st && loc_st < loc_size {
            w.early = if flag_wide {
                list[loc_st as usize].early
            } else {
                list[loc_st as usize].just
            };
        }
        if 0 <= loc_ed && loc_ed < loc_size {
            w.late = if flag_wide {
                list[loc_ed as usize].late
            } else {
                list[loc_ed as usize].just
            };
        }
        w
    }

    pub fn get_target_range_wide(&self) -> WideMsec {
        self.wmsec_target
    }
    pub fn get_target_range_force(&self) -> i32 {
        self.msec_target_fc
    }
    pub fn is_target_range_logo(&self) -> bool {
        self.from_logo
    }
    pub fn set_target_range(&mut self, wmsec: WideMsec, msec_force: i32, from_logo: bool) -> bool {
        if self.process & PROC_BASELOGO == 0 && from_logo {
            self.signal_internal_error(PROC_TARGETRANGE);
        }
        self.process |= PROC_TARGETRANGE;
        self.wmsec_target = wmsec;
        self.msec_target_fc = msec_force;
        self.from_logo = from_logo;
        true
    }

    pub fn set_target_out_edge(&mut self, edge: i32) {
        self.edge_out_pos = edge;
    }
    pub fn get_target_out_edge(&self) -> i32 {
        self.edge_out_pos
    }

    pub fn is_target_listed(&self, msec_target: i32) -> bool {
        let nsize = self.list_tl_range.len() as i32;
        if nsize == 0 {
            return true;
        }
        let mut det = false;
        for i in 0..nsize {
            let (st, ed) = self.list_tl_range[i as usize];
            if msec_target >= st && msec_target <= ed {
                det = true;
            }
        }
        det
    }
    pub fn clear_target_list(&mut self) {
        self.list_tl_range.clear();
    }
    pub fn add_target_list(&mut self, st: i32, ed: i32) {
        self.list_tl_range.push((st, ed));
    }

    pub fn get_scp_enable(&self, nsc: i32) -> bool {
        if nsc < 0 || nsc >= self.list_scp_enable.len() as i32 {
            return false;
        }
        self.list_scp_enable[nsc as usize]
    }
    pub fn set_scp_enable(&mut self, list: Vec<bool>) -> bool {
        self.process |= PROC_SCPENABLE;
        self.list_scp_enable = list;
        true
    }
    pub fn size_scp_enable(&self) -> i32 {
        self.list_scp_enable.len() as i32
    }

    pub fn get_result_target_sel(&self) -> i32 {
        self.nsc_sel
    }
    pub fn get_result_target_end(&self) -> i32 {
        self.nsc_end
    }
    pub fn set_result_target(&mut self, nsc_sel_in: i32, nsc_end_in: i32) -> bool {
        if self.process & PROC_TARGETRANGE == 0 || self.process & PROC_SCPENABLE == 0 {
            self.signal_internal_error(PROC_RESULT);
        }
        self.process |= PROC_RESULT;
        self.nsc_sel = nsc_sel_in;
        self.nsc_end = nsc_end_in;
        true
    }
}

impl Default for CmdLimit {
    fn default() -> Self {
        Self::new()
    }
}
