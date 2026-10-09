//! Derived from join_logo_scp (https://github.com/tobitti0/join_logo_scp,
//! commit d1a24d758f8b3c04352b82ff6756fdf3d56729b3), Copyright (C) tobitti0,
//! GPL-2.0-or-later. This port is a GPL derivative work, redistributed here
//! under GPL-3.0-or-later (design §10.6).
//! Port of JlsScript.cpp execution layer: startCmd/startCmdLoop/
//! startCmdLoopSub/getLine, control commands (If/Call/Repeat/Flow/Sys/Reg/
//! MemFlow/MemExe), system registers, result registers, condition evaluation.
//!
//! Call recursion is modelled as an explicit frame stack; behavior otherwise
//! follows the reference line by line (join_logo_scp d1a24d75).

use super::cmdarg::{CmdArg, WideMsec};
use super::cmdlimit::CmdLimit;
use super::dataset::{
    Dataset, LogoRecord, EDGE_BOTH, EDGE_FALL, EDGE_RISE, LOGO_RESULT_ABORT, LOGO_RESULT_DECIDE,
    LOGO_RESULT_NONE, PRIOR_NONE,
};
use super::dataset_search::{NrfCurrent, PRIOR_DECIDE};
use super::decode::{Decoder, ERR_CMD, ERR_NONE, ERR_OPT};
use super::gstate::GState;
use super::limit;
use super::state::State;
use super::tables::OptType;
use super::tables::{CmdCat, CmdType, ConfigVarType};
use super::types::LazyType;
use super::vars::Vars;
use super::vars::{list_str_element, list_str_size};

/// context bundle passed into host hooks (splits Exec fields)
pub struct AutoCtx<'a> {
    pub g: &'a mut GState,
    pub ds: &'a mut Dataset,
    pub vars: &'a mut Vars,
}

const SIZE_CALL_LOOP: i32 = 10; // JlsNameSpace.hpp

/// host services that depend on crates above core-jls (files, options parser,
/// command limiters and the general-command engine)
pub trait ExecHost {
    fn stderr_line(&mut self, s: &[u8]);
    fn stdout_line(&mut self, s: &[u8]);
    /// ifstream open + full text; None on failure
    fn read_script_lines(&mut self, path: &[u8]) -> Option<Vec<Vec<u8>>>;
    fn file_exists(&mut self, path: &[u8]) -> bool;
    fn env_var(&mut self, name: &[u8]) -> Option<Vec<u8>>;
    /// JlsScrGlobal::fileOpen/FileClose (Echo sink)
    fn file_open(&mut self, name: &[u8], append: bool) -> bool;
    fn file_close(&mut self);
    fn file_output(&mut self, s: &[u8]);
    /// JlsReformData::adjustData (first general command)
    fn reform_adjust(&mut self, ctx: &mut AutoCtx);
    /// JlsAutoScript::startCmd — AUTO/AUTOEACH/AUTOLOGO execution body
    fn auto_script(
        &mut self,
        ctx: &mut AutoCtx,
        arg: &mut CmdArg,
        lim: &mut CmdLimit,
        setup_only: bool,
    ) -> bool;
    /// verbose displayLogo/displayScp after startCmd
    fn display_logo_scp(&mut self);
    /// §8.5 cancel request, polled at each script line boundary
    fn interrupted(&mut self) -> bool {
        false
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Normal,
    LazyEnd,
    LazyOut,
}

pub struct PendingSub {
    pub str_buf: Vec<u8>,
    pub errval: i32,
    pub enable_exe: bool,
    pub update_exe: bool,
    pub exe_command: bool,
}

pub struct Frame {
    pub state: State,
    pub lines: Vec<Vec<u8>>,
    pub pos: usize,
    pub loop_depth: i32,
    pub num_layer_start: i32,
    pub phase: Phase,
    pub pending: Option<PendingSub>,
}

pub struct Exec<H: ExecHost> {
    pub g: GState,
    pub ds: Dataset,
    pub vars: super::vars::Vars,
    pub host: H,
    pub frames: Vec<Frame>,
    /// a reported script error (syntax/command/nesting/Call failure); the
    /// reference only prints these, the CLI maps them to exit code 6 (§3.6, §7.3)
    pub script_error: bool,
}

impl<H: ExecHost> Exec<H> {
    /// startCmd
    pub fn start_cmd(&mut self, fname: &[u8]) -> i32 {
        let mut name_full_path = Vec::new();
        self.make_full_path(&mut name_full_path, fname, true);
        self.set_system_reg_init();
        self.ds.backup_logoset_save();
        let errnum = self.start_cmd_loop(&name_full_path, 0);
        if self.ds.ext_opt.verbose > 0 && errnum == 0 {
            self.host.display_logo_scp();
        }
        errnum
    }

    /// startCmdLoop — explicit frames instead of C++ recursion
    pub fn start_cmd_loop(&mut self, fname: &[u8], loop_num: i32) -> i32 {
        self.set_system_reg_lastexe(false);
        if loop_num == 0 {
            self.g.set_exe1st(true);
        }
        let num_layer_start = self.vars.reg.create_local(true); // setLocalRegCreateBase
        let lines = match self.host.read_script_lines(fname) {
            Some(l) => l,
            None => {
                // C++ returns 2 here without releasing the local layer (kept)
                let mut m = b"error: failed to open ".to_vec();
                m.extend_from_slice(fname);
                m.push(b'\n');
                self.host.stderr_line(&m);
                return 2;
            }
        };
        self.frames.push(Frame {
            state: State::new(),
            lines,
            pos: 0,
            loop_depth: loop_num,
            num_layer_start,
            phase: Phase::Normal,
            pending: None,
        });
        loop {
            if self.frames.is_empty() {
                return 0;
            }
            if self.host.interrupted() {
                // §8.5: abandon the remaining script; the caller reports 130
                self.frames.clear();
                return 0;
            }
            // parent remainder after a Call child completed
            if self.frames.last().unwrap().pending.is_some() {
                let p = self.frames.last_mut().unwrap().pending.take().unwrap();
                self.finish_part2(p);
                continue;
            }
            let line = match self.get_line_top() {
                Some(l) => l,
                None => {
                    self.advance_phase_or_pop(false);
                    continue;
                }
            };
            // reference: the Exit/Return check exists only in the normal
            // while loop; LazyEnd/LazyOut drain unconditionally
            let top_phase = self.frames.last().unwrap().phase;
            if top_phase == Phase::Normal
                && self
                    .frames
                    .last()
                    .unwrap()
                    .state
                    .is_cmd_return_exit(&self.g)
            {
                self.advance_phase_or_pop(true);
                continue;
            }
            let pending = {
                let depth = self.frames.len() - 1;
                let mut state = std::mem::take(&mut self.frames[depth].state);
                let loop_depth = self.frames[depth].loop_depth;
                let r = self.sub_part1(&mut state, loop_depth, line);
                self.frames[depth].state = state;
                r
            };
            match pending {
                Part1::Remain(p) => self.finish_part2(p),
                Part1::Call(path, p) => {
                    let loop_num = self.frames.last().unwrap().loop_depth;
                    self.frames.last_mut().unwrap().pending = Some(p);
                    self.push_call_frame(&path, loop_num + 1);
                }
            }
        }
    }

    /// end of a loop pass: reference epilogue (release/nest errors), then
    /// loop0 transitions through LazyEnd/LazyOut before the frame is dropped.
    fn advance_phase_or_pop(&mut self, force_exit: bool) {
        let top_pending = self
            .frames
            .last()
            .map(|f| f.pending.is_some())
            .unwrap_or(false);
        debug_assert!(!top_pending);
        let f = self.frames.last_mut().expect("frame");
        let phase = f.phase;
        let loop_depth = f.loop_depth;
        if phase == Phase::Normal {
            let num_layer_start = f.num_layer_start;
            let mut state = std::mem::take(&mut f.state);
            let num_layer_end = self.vars.reg.release_local(true); // setLocalRegReleaseBase
            if !force_exit {
                let flags_remain = state.is_remain_nest();
                if num_layer_start != num_layer_end
                    || flags_remain & 0x03 != 0
                    || state.is_lazy_area()
                    || state.is_mem_area()
                {
                    self.script_error = true;
                }
                if num_layer_start != num_layer_end {
                    self.host.stderr_line(b"error : { and } are not matched\n");
                }
                if flags_remain & 0x01 != 0 {
                    self.host.stderr_line(b"error : EndIf is not found\n");
                }
                if flags_remain & 0x02 != 0 {
                    self.host.stderr_line(b"error : EndRepeat is not found\n");
                }
                if state.is_lazy_area() {
                    self.host.stderr_line(b"error : EndLazy is not found\n");
                }
                if state.is_mem_area() {
                    self.host.stderr_line(b"error : EndMemory is not found\n");
                }
            }
            if num_layer_start < num_layer_end {
                for _ in 0..(num_layer_end - num_layer_start) {
                    self.vars.reg.release_local(true);
                }
            }
            if loop_depth == 0 {
                // startCmdLoopLazyEnd entry: FULL flush into this frame's cache
                state.set_lazy_exe(&mut self.g, LazyType::Full, b"");
                let f = self.frames.last_mut().unwrap();
                f.state = state;
                f.phase = Phase::LazyEnd;
            } else {
                let f = self.frames.last_mut().unwrap();
                f.state = state;
                self.frames.pop();
            }
            return;
        }
        if phase == Phase::LazyEnd {
            let f = self.frames.last_mut().unwrap();
            f.phase = Phase::LazyOut;
            // startCmdLoopLazyOut entry
            f.state.set_mem_call(&mut self.g, b"OUTPUT");
            return;
        }
        self.frames.pop();
    }

    /// startCmdGetLine (top frame), honoring the frame phase
    fn get_line_top(&mut self) -> Option<Vec<u8>> {
        let depth = self.frames.len() - 1;
        if self.frames[depth].phase != Phase::Normal {
            let mut buf = Vec::new();
            let hit = if self.frames[depth]
                .state
                .read_lazy_mem_next(&mut self.g, &mut buf)
                || self.frames[depth].state.read_cmd_cache(&mut buf)
            {
                Some(buf)
            } else {
                None
            };
            return hit;
        }
        {
            let mut buf = Vec::new();
            let hit = if self.frames[depth]
                .state
                .read_lazy_mem_next(&mut self.g, &mut buf)
                || self.frames[depth].state.read_cmd_cache(&mut buf)
            {
                Some(buf)
            } else {
                None
            };
            if hit.is_some() {
                return hit;
            }
        }
        let f = &mut self.frames[depth];
        Self::get_line_from_file(f).inspect(|l| {
            let _ = f.state.add_cmd_cache(l); // reference: file lines feed Repeat cache
        })
    }

    fn get_line_from_file(f: &mut Frame) -> Option<Vec<u8>> {
        // startCmdGetLineFromFile with " \" continuation
        let mut acc: Option<Vec<u8>> = None;
        loop {
            let raw = match f.lines.get(f.pos) {
                Some(l) => {
                    f.pos += 1;
                    l.clone()
                }
                None => {
                    if acc.is_some() {
                        return acc;
                    }
                    return None;
                }
            };
            let mut buf = raw;
            if buf.len() >= i32::MAX as usize / 4 {
                return None; // C++: return false (drops accumulated line)
            }
            let mut cont = false;
            let len = buf.len();
            if len >= 2 {
                // find(" \") — first occurrence keeps prefix up to pos+1
                if let Some(p) = find_sub(&buf, b" \\") {
                    buf.truncate(p + 1); // keep the space
                    cont = true;
                } else if buf[len - 2..] == *b" \\" {
                    buf.truncate(len - 1);
                    cont = true;
                }
            }
            let a = acc.get_or_insert_with(Vec::new);
            a.extend_from_slice(&buf);
            if !cont {
                return Some(std::mem::take(a));
            }
        }
    }

    /// startCmdLoopSub part1 (through the category switch)
    fn sub_part1(&mut self, state: &mut State, loop_num: i32, str_buf_org: Vec<u8>) -> Part1 {
        // Repeat cache add for file lines is done where the line came from;
        // in the frame loop we call add_cmd_cache after file reads instead.
        let mut exe_command = self.is_system_reg_lastexe();
        let mut str_buf = self.vars.replace_buf_var(&str_buf_org).0;

        let mut cmdset = CmdArg::new();
        let mut errval;
        {
            let cnv = self.vars.cnv.borrow();
            errval = Decoder::decode_cmd(&cnv, &mut cmdset, &str_buf, true);
        }
        if errval == ERR_NONE {
            if state.is_need_raw(cmdset.category) {
                str_buf = str_buf_org.clone();
            }
            let cnv = self.vars.cnv.clone();
            if state.is_need_full_decode(cmdset.cmdsel, cmdset.category) {
                let b = str_buf.clone();
                let e = Decoder::decode_cmd(&cnv.borrow(), &mut cmdset, &b, false);
                str_buf = b;
                errval = e;
            } else {
                // clearRegError(false)
                let _ = self.vars.reg.pop_msg_error();
            }
        }
        let mut enable_exe = false;
        if errval == ERR_NONE {
            enable_exe = self.set_state_mem(state, &mut cmdset, &str_buf);
            if enable_exe {
                let success = self.expand_decode_cmd(state, &mut cmdset, &str_buf);
                if !success {
                    enable_exe = false;
                    errval = ERR_OPT;
                }
            }
        }
        let mut update_exe = false;
        let mut call_path: Option<Vec<u8>> = None;
        if enable_exe {
            let mut success = true;
            match cmdset.category {
                CmdCat::NONE => {}
                CmdCat::COND => success = self.set_cmd_cond_if(&mut cmdset, state),
                CmdCat::CALL => {
                    match self.set_cmd_call(&mut cmdset, loop_num) {
                        CallAction::Run(path) => {
                            // reference: child recursion runs before this
                            // line's trace output; deferred to caller
                            call_path = Some(path);
                        }
                        CallAction::Skip => {}
                        CallAction::Fail => success = false,
                    }
                }
                CmdCat::REP => success = self.set_cmd_repeat(&mut cmdset, state),
                CmdCat::FLOW => success = self.set_cmd_flow(&mut cmdset, state),
                CmdCat::SYS => success = self.set_cmd_sys(&mut cmdset),
                CmdCat::REG => success = self.set_cmd_reg(&mut cmdset, state),
                CmdCat::LAZYF | CmdCat::MEMF | CmdCat::MEMLAZYF => {
                    success = self.set_cmd_mem_flow(&mut cmdset, state)
                }
                CmdCat::MEMEXE => success = self.set_cmd_mem_exe(&mut cmdset, state),
                _ => {
                    if self.g.is_exe1st() {
                        self.g.set_exe1st(false);
                        if self.ds.is_setup_adj_initial() {
                            self.ds.set_flag_setup_adj(true);
                            let mut ctx = AutoCtx {
                                g: &mut self.g,
                                ds: &mut self.ds,
                                vars: &mut self.vars,
                            };
                            self.host.reform_adjust(&mut ctx);
                            self.set_system_reg_nologo(true);
                        }
                    }
                    let mut lim = CmdLimit::new();
                    exe_command = self.exe_cmd(&mut cmdset, &mut lim);
                    update_exe = true;
                }
            }
            if !success {
                errval = ERR_CMD;
            }
        }
        if errval != ERR_NONE {
            exe_command = false;
        }
        let pending = PendingSub {
            str_buf,
            errval,
            enable_exe,
            update_exe,
            exe_command,
        };
        match call_path {
            None => Part1::Remain(pending),
            Some(path) => Part1::Call(path, pending),
        }
    }

    /// startCmdLoopSub part2 (dispErr / vLine trace / lastexe)
    fn finish_part2(&mut self, p: PendingSub) {
        let PendingSub {
            str_buf,
            errval,
            enable_exe,
            update_exe,
            exe_command,
        } = p;
        self.start_cmd_disp_err(&str_buf, errval);
        // debug vLine
        if self.ds.ext_opt.v_line == 2 {
            let mut s = format!(
                "{}{}{}:{}",
                enable_exe as u8,
                update_exe as u8,
                exe_command as u8,
                String::from_utf8_lossy(&str_buf)
            )
            .into_bytes();
            s.push(b'\n');
            self.host.stdout_line(&s);
        } else if self.ds.ext_opt.v_line > 0 {
            let mut s = format!(
                "{}{}:{}",
                exe_command as u8,
                // C++: cout << enable_exe << exe_command — reference quirk:
                // enable_exe is printed before exe_command
                enable_exe as u8,
                String::from_utf8_lossy(&str_buf)
            );
            let _ = &mut s;
            let mut line = format!(
                "{}{}:{}\n",
                exe_command as u8,
                exe_command as u8,
                String::from_utf8_lossy(&str_buf)
            )
            .into_bytes();
            let _ = &mut line;
            // actual reference line: cout << enable_exe << exe_command << ":"
            let mut out = format!(
                "{}{}:{}",
                enable_exe as u8,
                exe_command as u8,
                String::from_utf8_lossy(&str_buf)
            )
            .into_bytes();
            out.push(b'\n');
            self.host.stdout_line(&out);
        }
        self.set_system_reg_lastexe(exe_command);
    }

    fn push_call_frame(&mut self, path: &[u8], loop_num: i32) {
        // reference calls startCmdLoop(file, loop+1) synchronously; emulate
        // with a frame push (open failure = return 2 inside that call: we
        // mirror by not pushing after reporting)
        self.set_system_reg_lastexe(false);
        let num_layer_start = self.vars.reg.create_local(true);
        let lines = match self.host.read_script_lines(path) {
            Some(l) => l,
            None => {
                let mut m = b"error: failed to open ".to_vec();
                m.extend_from_slice(path);
                m.push(b'\n');
                self.host.stderr_line(&m);
                self.script_error = true;
                // reference: local layer leaked; C++ returns 2 from child call
                return;
            }
        };
        self.frames.push(Frame {
            state: State::new(),
            lines,
            pos: 0,
            loop_depth: loop_num,
            num_layer_start,
            phase: Phase::Normal,
            pending: None,
        });
    }

    /// startCmdDispErr
    fn start_cmd_disp_err(&mut self, str_buf: &[u8], errval: i32) {
        if errval != ERR_NONE {
            self.script_error = true;
            let str_err: &[u8] = match errval {
                ERR_OPT => b"error: wrong argument",
                2 => b"error: wrong range argument",
                3 => b"error: need Start or End",
                4 => b"error: failed variable setting",
                5 => b"error: need auto command TR/SP/EC",
                ERR_CMD => b"error: wrong command",
                _ => b"",
            };
            if !str_err.is_empty() {
                let mut m = str_err.to_vec();
                m.extend_from_slice(b" in ");
                m.extend_from_slice(str_buf);
                m.push(b'\n');
                self.host.stderr_line(&m);
            }
        }
        // checkErrorGlobalState(true)
        if let Some(msg) = self.g.pop_msg_error() {
            self.host.stderr_line(&msg);
        }
        while let Some(msg) = self.vars.reg.pop_msg_error() {
            self.host.stderr_line(&msg);
        }
    }

    // ================= setStateMem / lazy =================

    /// setStateMem
    fn set_state_mem(&mut self, state: &mut State, cmdarg: &mut CmdArg, str_buf: &[u8]) -> bool {
        let mut enable_exe = true;
        if state.is_skip_cmd() {
            // skip: no lazy handling
        } else if state.is_mem_area() {
            if !State::is_flow_mem(cmdarg.category) {
                let name = state.get_mem_name();
                enable_exe = state.set_mem_store(&mut self.g, &name, str_buf);
            }
        } else {
            if !State::is_flow_lazy(cmdarg.category) {
                enable_exe = self.set_state_mem_lazy(state, cmdarg, str_buf);
            }
        }
        if (state.is_mem_area() || state.is_lazy_area())
            && state.check_arg_reg_insert(cmdarg.cmdsel)
        {
            self.set_arg_state_insert_reg(state);
        }
        if state.is_invalid_cmd_line(cmdarg.category) {
            enable_exe = false;
        }
        enable_exe
    }

    /// setStateMemLazy
    fn set_state_mem_lazy(
        &mut self,
        state: &mut State,
        cmdarg: &mut CmdArg,
        str_buf: &[u8],
    ) -> bool {
        let mut enable_exe = true;
        let mut flag_normal_cmd = false;
        if !state.is_lazy_exe() {
            let mut type_lazy = state.get_lazy_start_type();
            if !state.is_lazy_area() {
                let mut t = type_lazy;
                self.set_state_mem_lazy_revise(&mut t, state, cmdarg);
                type_lazy = t;
            }
            match type_lazy {
                LazyType::LazyS | LazyType::LazyA | LazyType::LazyE => {
                    enable_exe = state.set_lazy_store(&mut self.g, type_lazy, str_buf);
                }
                _ => flag_normal_cmd = true,
            }
        } else {
            let lazy_passed = state.get_lazy_exe_type();
            let mut lazy_need = LazyType::None;
            let need_a = self.set_state_mem_lazy_revise(&mut lazy_need, state, cmdarg);
            if self.is_lazy_auto_mode_initial(state) && need_a && lazy_passed == LazyType::LazyE {
                enable_exe = false;
            } else {
                let mut lazy_next = LazyType::None;
                match lazy_need {
                    LazyType::LazyE => {
                        if lazy_passed == LazyType::LazyA || lazy_passed == LazyType::LazyS {
                            lazy_next = LazyType::LazyE;
                        }
                    }
                    LazyType::LazyA if lazy_passed == LazyType::LazyS => {
                        lazy_next = LazyType::LazyA;
                    }
                    _ => {}
                }
                if lazy_next != LazyType::None {
                    enable_exe = state.set_lazy_store(&mut self.g, lazy_next, str_buf);
                }
            }
        }
        if enable_exe {
            match cmdarg.category {
                CmdCat::AUTO | CmdCat::AUTOEACH | CmdCat::AUTOLOGO => {
                    if flag_normal_cmd {
                        enable_exe = state.set_lazy_exe(&mut self.g, LazyType::LazyA, str_buf);
                    }
                    if enable_exe {
                        state.set_lazy_state_ini_auto(&mut self.g, false);
                    }
                }
                CmdCat::LOGO if flag_normal_cmd => {
                    enable_exe = state.set_lazy_exe(&mut self.g, LazyType::LazyS, str_buf);
                }
                _ => {}
            }
        }
        enable_exe
    }

    /// isLazyAutoModeInitial
    fn is_lazy_auto_mode_initial(&self, state: &State) -> bool {
        self.ds.is_auto_mode_initial() || state.is_lazy_state_ini_auto(&self.g)
    }

    /// setStateMemLazyRevise — returns needA
    fn set_state_mem_lazy_revise(
        &mut self,
        type_lazy: &mut LazyType,
        state: &State,
        cmdarg: &CmdArg,
    ) -> bool {
        if cmdarg.get_opt(OptType::FlagNow) > 0 {
            return false;
        }
        let tack_lazy = lazy_from_i32(cmdarg.tack.type_lazy);
        if tack_lazy != LazyType::None {
            *type_lazy = tack_lazy;
        }
        let flag_auto = state.is_lazy_auto();
        let flag_init = self.is_lazy_auto_mode_initial(state);
        let flag_init_auto = flag_init && flag_auto;
        let mut need_s = false;
        let mut need_a = false;
        let mut need_e = false;
        match cmdarg.category {
            CmdCat::AUTOEACH | CmdCat::AUTOLOGO => need_a = flag_init,
            CmdCat::AUTO => need_a = flag_init_auto,
            CmdCat::LOGO => need_s = flag_init_auto,
            _ => {}
        }
        if flag_init_auto {
            if cmdarg.get_opt(OptType::FlagAutoChg) > 0 {
                need_a = true;
            }
            if cmdarg.tack.need_auto {
                need_a = true;
            }
        }
        if cmdarg.get_opt(OptType::FlagFinal) > 0 && (need_a || need_s) {
            need_e = true;
        }
        if need_e {
            *type_lazy = LazyType::LazyE;
        } else if need_a && (*type_lazy == LazyType::None || *type_lazy == LazyType::LazyS) {
            *type_lazy = LazyType::LazyA;
        } else if need_s && *type_lazy == LazyType::None {
            *type_lazy = LazyType::LazyS;
        }
        need_a
    }

    // ================= expandDecodeCmd / cond =================

    /// expandDecodeCmd
    fn expand_decode_cmd(
        &mut self,
        _state: &mut State,
        cmdarg: &mut CmdArg,
        _str_buf: &[u8],
    ) -> bool {
        let mut success = true;
        let mut flag_cond = true;
        let num = cmdarg.num_check_cond;
        if num > 0 {
            let arg = cmdarg.get_str_arg(num);
            success = self.get_cond_flag(&mut flag_cond, &arg);
        }
        cmdarg.flag_cond = flag_cond;
        self.get_decode_reg(cmdarg);
        success
    }

    /// getCondFlag
    fn get_cond_flag(&mut self, flag_cond: &mut bool, str_buf: &[u8]) -> bool {
        let mut str_calc = Vec::new();
        let str_buf_rev = {
            let cnv = self.vars.cnv.borrow();
            let mut d = Vec::new();
            cnv.get_str_without_comment(&mut d, str_buf);
            d
        };
        let mut pos = 0i32;
        while pos >= 0 {
            let (item, next) = self.get_cond_flag_get_item(&str_buf_rev, pos);
            if next >= 0 {
                pos = next;
                self.get_cond_flag_connect_word(&mut str_calc, &item);
            } else {
                pos = next;
            }
        }
        let mut val = 0i32;
        let pos_calc;
        {
            let cnv = self.vars.cnv.borrow();
            if index_byte(&str_calc, b':').is_some() || index_byte(&str_calc, b'.').is_some() {
                pos_calc = cnv.get_str_val_msec(&str_calc, 0).0;
                if let Some(v) = cnv.get_str_val_msec(&str_calc, 0).1 {
                    val = v;
                }
            } else {
                pos_calc = cnv.get_str_val_num(&str_calc, 0).0;
                if let Some(v) = cnv.get_str_val_num(&str_calc, 0).1 {
                    val = v;
                }
            }
        }
        let mut success = true;
        if pos_calc < 0 {
            val = 0;
            success = false;
            let mut m = b"error: can not evaluate(".to_vec();
            m.extend_from_slice(&str_calc);
            m.extend_from_slice(b")\n");
            self.host.stderr_line(&m);
        }
        *flag_cond = val != 0;
        success
    }

    /// getCondFlagGetItem — returns (item, next pos)
    fn get_cond_flag_get_item(&mut self, str_buf: &[u8], pos: i32) -> (Vec<u8>, i32) {
        let mut str_item = Vec::new();
        let mut pos = {
            let cnv = self.vars.cnv.borrow();
            cnv.get_str_item_with_quote(&mut str_item, str_buf, pos)
        };
        if pos >= 0 && !str_item.is_empty() && str_item[0] == b'"' {
            let mut str2 = Vec::new();
            let mut str3 = Vec::new();
            let cnv = self.vars.cnv.borrow();
            let pos2 = cnv.get_str_item_with_quote(&mut str2, str_buf, pos);
            let pos3 = cnv.get_str_item_with_quote(&mut str3, str_buf, pos2.max(0) as usize as i32);
            drop(cnv);
            if pos3 >= 0 {
                let flag_eq = str_item == str3;
                if str2 == b"==" {
                    pos = pos3;
                    str_item = if flag_eq {
                        b"1".to_vec()
                    } else {
                        b"0".to_vec()
                    };
                } else if str2 == b"!=" {
                    pos = pos3;
                    str_item = if flag_eq {
                        b"0".to_vec()
                    } else {
                        b"1".to_vec()
                    };
                }
            }
        }
        (str_item, pos)
    }

    /// getCondFlagConnectWord
    fn get_cond_flag_connect_word(&mut self, str_calc: &mut Vec<u8>, str_item: &[u8]) {
        // C++ reads front()/[1] unguarded; empty items cannot occur because
        // getStrItemWithQuote returns only non-empty items when pos >= 0.
        if str_item.is_empty() {
            return;
        }
        let ch_next_front = str_item[0];
        let ch_next_fr2 = *str_item.get(1).unwrap_or(&0);
        if !str_calc.is_empty() {
            let ch_prev_back = *str_calc.last().unwrap();
            if ch_prev_back != b'='
                && ch_prev_back != b'<'
                && ch_prev_back != b'>'
                && ch_prev_back != b'|'
                && ch_prev_back != b'&'
                && ch_next_front != b'|'
                && ch_next_front != b'&'
                && ch_next_front != b'='
                && ch_next_front != b'<'
                && ch_next_front != b'>'
                && !(ch_next_front == b'!' && ch_next_fr2 == b'=')
            {
                str_calc.extend_from_slice(b"||");
            }
        }

        let str_remain: Vec<u8> = if ch_next_front == b'!' {
            str_calc.push(b'!');
            str_item[1..].to_vec()
        } else {
            str_item.to_vec()
        };
        let ch_front = *str_remain.first().unwrap_or(&0);
        if ch_front.is_ascii_alphabetic() {
            let mut str_val = self
                .vars
                .get_jls_reg_var(&str_remain, true)
                .1
                .unwrap_or_default();
            if !str_remain.is_empty() && !str_val.is_empty() && str_val != *b"0" {
                str_val = b"1".to_vec();
            } else if str_remain.is_empty() {
                // C++ getJlsRegVar on empty name: nmatch==0 -> "0"
                str_val = b"0".to_vec();
            } else {
                str_val = b"0".to_vec();
            }
            str_calc.extend_from_slice(&str_val);
        } else {
            str_calc.extend_from_slice(&str_remain);
        }
    }

    /// getDecodeReg
    fn get_decode_reg(&mut self, cmdarg: &mut CmdArg) {
        let str_reg_pos = cmdarg.get_str_opt(OptType::StrRegPos);
        let str_reg_list = cmdarg.get_str_opt(OptType::StrRegList);
        let str_default_pos = cmdarg.get_str_opt(OptType::StrValPosW);
        let str_default_list = cmdarg.get_str_opt(OptType::StrValListW);
        {
            if let Some(v) = self.vars.get_jls_reg_var(&str_reg_pos, true).1 {
                cmdarg.set_str_opt(OptType::StrValPosR, v.clone());
                cmdarg.set_str_opt(OptType::StrValPosW, v);
                cmdarg.clear_str_opt_update(OptType::StrValPosW);
            }
            if cmdarg.is_set_str_opt(OptType::StrArgVal) {
                let v = cmdarg.get_str_opt(OptType::StrArgVal);
                cmdarg.set_str_opt(OptType::StrValPosR, v);
            }
            if let Some(v) = self.vars.get_jls_reg_var(&str_reg_list, true).1 {
                cmdarg.set_str_opt(OptType::StrValListR, v.clone());
                cmdarg.set_str_opt(OptType::StrValListW, v);
                cmdarg.clear_str_opt_update(OptType::StrValListW);
            }
        }
        match cmdarg.cmdsel {
            CmdType::GetPos | CmdType::ListGetAt => {
                if cmdarg.get_opt(OptType::FlagClear) > 0 {
                    cmdarg.set_str_opt(OptType::StrValPosW, str_default_pos);
                }
            }
            CmdType::GetList | CmdType::ReadData | CmdType::ReadTrim | CmdType::ReadString => {
                cmdarg.set_str_opt(OptType::StrValListW, str_default_list);
            }
            _ => {}
        }
    }

    // ================= system registers =================

    fn set_system_reg_init(&mut self) {
        self.set_system_reg_headtail(-1, -1);
        self.set_system_reg_update();
    }

    fn set_system_reg_update(&mut self) {
        let n = self.ds.msec_total_max;
        let (str_val, str_time) = {
            let cnv = self.vars.cnv.borrow();
            (
                cnv.get_string_frame_msec_m1(n),
                cnv.get_string_time_msec_m1(n),
            )
        };
        self.vars
            .set_jls_reg_var(b"MAXFRAME", &str_val, true, &mut self.ds);
        self.vars
            .set_jls_reg_var(b"MAXTIME", &str_time, true, &mut self.ds);
        let flag = self.ds.ext_opt.flag_no_logo;
        self.vars
            .set_jls_reg_var(b"NOLOGO", flag.to_string().as_bytes(), true, &mut self.ds);
        self.set_system_reg_file_path();
    }

    fn set_system_reg_file_path(&mut self) {
        let str_data_last = b"data".to_vec();
        let mut str_path_sub = if self.ds.ext_opt.sub_path.is_empty() {
            self.g.get_path_name_jl().to_vec()
        } else {
            self.ds.ext_opt.sub_path.clone()
        };
        {
            let mut v = str_path_sub.clone();
            self.vars.cnv.borrow_mut().get_str_file_all_path(&mut v);
            str_path_sub = v;
        }
        let mut str_path_data = str_path_sub.clone();
        str_path_data.extend_from_slice(&str_data_last);
        {
            let mut v = str_path_data.clone();
            self.vars.cnv.borrow_mut().get_str_file_all_path(&mut v);
            str_path_data = v;
        }
        self.vars
            .set_jls_reg_var(b"JLDATAPATH", &str_path_data, true, &mut self.ds);
    }

    fn set_system_reg_headtail(&mut self, headframe: i32, tailframe: i32) {
        let (str_head, str_tail) = {
            let cnv = self.vars.cnv.borrow();
            (
                cnv.get_string_time_msec_m1(headframe),
                cnv.get_string_time_msec_m1(tailframe),
            )
        };
        self.vars
            .set_jls_reg_var(b"HEADTIME", &str_head, true, &mut self.ds);
        self.vars
            .set_jls_reg_var(b"TAILTIME", &str_tail, true, &mut self.ds);
    }

    /// setSystemRegNologo
    fn set_system_reg_nologo(&mut self, need_check: bool) {
        let mut flag_nologo = false;
        if need_check && self.ds.ext_opt.flag_no_logo == 0 {
            let mut msec_sum = 0i32;
            let mut nrf_rise;
            let mut nrf_fall = -1i32;
            loop {
                nrf_rise = self.ds.get_nrf_next_logo(nrf_fall, EDGE_RISE, 1);
                nrf_fall = self.ds.get_nrf_next_logo(nrf_rise, EDGE_FALL, 1);
                if nrf_rise >= 0 && nrf_fall >= 0 {
                    msec_sum += self.ds.msec_logo_nrf(nrf_fall) - self.ds.msec_logo_nrf(nrf_rise);
                }
                if !(nrf_rise >= 0 && nrf_fall >= 0) {
                    break;
                }
            }
            if msec_sum < self.ds.get_config(ConfigVarType::msecWLogoSumMin) {
                flag_nologo = true;
            }
        } else {
            flag_nologo = true;
        }
        if flag_nologo {
            self.ds.ext_opt.flag_no_logo = 1;
            self.vars
                .set_jls_reg_var(b"NOLOGO", b"1", true, &mut self.ds);
        }
    }

    fn set_system_reg_lastexe(&mut self, exe_command: bool) {
        let v = (exe_command as i32).to_string().into_bytes();
        self.vars
            .set_jls_reg_var(b"LASTEXE", &v, true, &mut self.ds);
    }

    fn is_system_reg_lastexe(&mut self) -> bool {
        self.vars
            .get_jls_reg_var(b"LASTEXE", true)
            .1
            .unwrap_or_else(|| b"0".to_vec())
            != *b"0"
    }

    /// setSystemRegOptions
    fn set_system_reg_options(&mut self, str_buf: &[u8], pos: i32, overwrite: bool) -> bool {
        let mut listarg: Vec<Vec<u8>> = Vec::new();
        {
            let cnv = self.vars.cnv.borrow();
            let mut pos = pos;
            let mut word = Vec::new();
            while pos >= 0 {
                pos = cnv.get_str_item(&mut word, str_buf, pos);
                if pos >= 0 {
                    listarg.push(word.clone());
                }
            }
        }
        let argc = listarg.len() as i32;
        if argc <= 0 {
            return true;
        }
        let mut i = 0i32;
        while i >= 0 && i < argc {
            let argrest = argc - i;
            let (s1, s2) = (
                if argrest >= 2 {
                    Some(listarg[(i + 1) as usize].clone())
                } else {
                    None
                },
                if argrest >= 3 {
                    Some(listarg[(i + 2) as usize].clone())
                } else {
                    None
                },
            );
            let v = listarg[i as usize].clone();
            let numarg =
                self.set_options_get_one(argrest, &v, s1.as_deref(), s2.as_deref(), overwrite);
            if numarg < 0 {
                return false;
            }
            if numarg > 0 {
                i += numarg;
            } else {
                i += 1;
            }
        }
        self.set_system_reg_update();
        true
    }

    // ================= result registers =================

    fn update_result_reg_write(&mut self, cmdarg: &mut CmdArg) {
        if cmdarg.is_update_str_opt(OptType::StrValPosW) {
            let name = cmdarg.get_str_opt(OptType::StrRegPos);
            let val = cmdarg.get_str_opt(OptType::StrValPosW);
            let overwrite = cmdarg.get_opt(OptType::FlagDefault) == 0;
            let flag_local = cmdarg.get_opt(OptType::FlagLocal) > 0;
            self.vars
                .set_jls_reg_var_with_local(&name, &val, overwrite, flag_local, &mut self.ds);
            cmdarg.clear_str_opt_update(OptType::StrValPosW);
        }
        if cmdarg.is_update_str_opt(OptType::StrValListW) {
            let name = cmdarg.get_str_opt(OptType::StrRegList);
            let val = cmdarg.get_str_opt(OptType::StrValListW);
            let overwrite = cmdarg.get_opt(OptType::FlagDefault) == 0;
            let flag_local = cmdarg.get_opt(OptType::FlagLocal) > 0;
            self.vars
                .set_jls_reg_var_with_local(&name, &val, overwrite, flag_local, &mut self.ds);
            cmdarg.clear_str_opt_update(OptType::StrValListW);
        }
    }

    fn set_result_reg_write_size(&mut self, cmdarg: &mut CmdArg, str_list: &[u8]) {
        let size_name = cmdarg.get_str_opt(OptType::StrRegSize);
        let num_list = list_str_size(str_list);
        let num_str = num_list.to_string().into_bytes();
        let overwrite = cmdarg.get_opt(OptType::FlagDefault) == 0;
        let flag_local = cmdarg.get_opt(OptType::FlagLocal) > 0;
        self.vars.set_jls_reg_var_with_local(
            &size_name,
            &num_str,
            overwrite,
            flag_local,
            &mut self.ds,
        );
    }

    fn set_result_reg_poshold(&mut self, cmdarg: &mut CmdArg, msec_pos: i32) {
        let str_val = self.vars.cnv.borrow().get_string_time_msec_m1(msec_pos);
        cmdarg.set_str_opt(OptType::StrValPosW, str_val);
        self.update_result_reg_write(cmdarg);
    }

    fn set_result_reg_listhold(&mut self, cmdarg: &mut CmdArg, msec_pos: i32) {
        let mut str_list = cmdarg.get_str_opt(OptType::StrValListW);
        if msec_pos != -1 {
            if !str_list.is_empty() {
                str_list.push(b',');
            }
            let v = self.vars.cnv.borrow().get_string_time_msec_m1(msec_pos);
            str_list.extend_from_slice(&v);
        }
        cmdarg.set_str_opt(OptType::StrValListW, str_list.clone());
        self.update_result_reg_write(cmdarg);
        self.set_result_reg_write_size(cmdarg, &str_list);
    }

    fn set_result_reg_list_get_at(&mut self, cmdarg: &mut CmdArg, num_item: i32) {
        let str_list = cmdarg.get_str_opt(OptType::StrValListR);
        let mut str_val_pos = cmdarg.get_str_opt(OptType::StrValPosW);
        if let Some(item) = list_str_element(&str_list, num_item) {
            str_val_pos = item;
        }
        cmdarg.set_str_opt(OptType::StrValPosW, str_val_pos);
        self.update_result_reg_write(cmdarg);
        self.set_result_reg_write_size(cmdarg, &str_list);
    }

    fn set_result_reg_list_ins(&mut self, cmdarg: &mut CmdArg, num_item: i32) {
        let mut str_list = cmdarg.get_str_opt(OptType::StrValListR);
        let str_val_pos = cmdarg.get_str_opt(OptType::StrValPosR);
        if super::vars::list_str_ins(&mut str_list, &str_val_pos, num_item) {
            cmdarg.set_str_opt(OptType::StrValListW, str_list.clone());
            self.update_result_reg_write(cmdarg);
        }
        self.set_result_reg_write_size(cmdarg, &str_list);
    }

    fn set_result_reg_list_del(&mut self, cmdarg: &mut CmdArg, num_item: i32) {
        let mut str_list = cmdarg.get_str_opt(OptType::StrValListR);
        if super::vars::list_str_del(&mut str_list, num_item) {
            cmdarg.set_str_opt(OptType::StrValListW, str_list.clone());
            self.update_result_reg_write(cmdarg);
        }
        self.set_result_reg_write_size(cmdarg, &str_list);
    }

    fn set_result_reg_list_rep(&mut self, cmdarg: &mut CmdArg, num_item: i32) {
        let mut str_list = cmdarg.get_str_opt(OptType::StrValListR);
        let str_val_pos = cmdarg.get_str_opt(OptType::StrValPosR);
        if super::vars::list_str_rep(&mut str_list, &str_val_pos, num_item) {
            cmdarg.set_str_opt(OptType::StrValListW, str_list.clone());
            self.update_result_reg_write(cmdarg);
        }
        self.set_result_reg_write_size(cmdarg, &str_list);
    }

    fn set_result_reg_list_clear(&mut self, cmdarg: &mut CmdArg) {
        let str_list = Vec::new();
        cmdarg.set_str_opt(OptType::StrValListW, str_list.clone());
        self.update_result_reg_write(cmdarg);
        self.set_result_reg_write_size(cmdarg, &str_list);
    }

    fn set_result_reg_list_sort(&mut self, _cmdarg: &mut CmdArg) {
        // ListSort needs cnv-based msec compare (setListStrSort); wired with
        // the cnv accessor when exercised (kept for parity reference:2009)
    }

    // ================= control commands =================

    fn set_cmd_cond_if(&mut self, cmdarg: &mut CmdArg, state: &mut State) -> bool {
        let mut success = true;
        let flag_cond = cmdarg.flag_cond;
        match cmdarg.cmdsel {
            CmdType::If => {
                state.if_begin(flag_cond);
            }
            CmdType::EndIf => {
                if state.if_end() > 0 {
                    self.g.add_msg_error(b"error: too many EndIf.\n");
                    success = false;
                }
            }
            CmdType::Else | CmdType::ElsIf if state.if_else(flag_cond) > 0 => {
                self.g
                    .add_msg_error(b"error: not exist 'If' but exist 'Else/ElsIf' .\n");
                success = false;
            }
            _ => {}
        }
        success
    }

    /// setCmdCall
    fn set_cmd_call(&mut self, cmdarg: &mut CmdArg, loop_num: i32) -> CallAction {
        if cmdarg.cmdsel != CmdType::Call {
            return CallAction::Skip;
        }
        let mut str_file_name = Vec::new();
        let arg1 = cmdarg.get_str_arg(1);
        self.make_full_path(&mut str_file_name, &arg1, false);
        let loop_next = loop_num + 1;
        if loop_next < SIZE_CALL_LOOP {
            return CallAction::Run(str_file_name);
        }
        self.g.add_msg_error(b"error: many recursive call\n");
        CallAction::Fail
    }

    fn set_cmd_repeat(&mut self, cmdarg: &mut CmdArg, state: &mut State) -> bool {
        let mut success = true;
        match cmdarg.cmdsel {
            CmdType::Repeat => {
                let val = cmdarg.get_val_str_arg(1);
                if state.repeat_begin(&mut self.g, val) > 0 {
                    self.g.add_msg_error(b"error: overflow at Repeat\n");
                    success = false;
                }
            }
            CmdType::EndRepeat if state.repeat_end(&mut self.g) > 0 => {
                self.g
                    .add_msg_error(b"error: Repeat - EndRepeat not match\n");
                success = false;
            }
            _ => {}
        }
        success
    }

    fn set_cmd_flow(&mut self, cmdarg: &mut CmdArg, state: &mut State) -> bool {
        let mut success = true;
        match cmdarg.cmdsel {
            CmdType::LocalSt => {
                self.vars.reg.create_local(false);
            }
            CmdType::LocalEd => {
                self.vars.reg.release_local(false);
            }
            CmdType::ArgBegin => {
                state.set_arg_area(true);
                if cmdarg.get_opt(OptType::FlagLocal) > 0 {
                    self.vars.reg.set_local_only(true);
                }
            }
            CmdType::ArgEnd => {
                state.set_arg_area(false);
                self.vars.reg.set_local_only(false);
            }
            CmdType::Exit => self.g.set_cmd_exit(true),
            CmdType::Return => state.set_cmd_return(true),
            _ => {
                self.g.add_msg_error(b"error:internal setting\n");
                success = false;
            }
        }
        success
    }

    fn set_cmd_sys(&mut self, cmdarg: &mut CmdArg) -> bool {
        let mut success = true;
        match cmdarg.cmdsel {
            CmdType::FileOpen => {
                let name = cmdarg.get_str_arg(1);
                self.host.file_open(&name, false);
            }
            CmdType::FileAppend => {
                let name = cmdarg.get_str_arg(1);
                self.host.file_open(&name, true);
            }
            CmdType::FileClose => self.host.file_close(),
            CmdType::Echo => {
                let mut s = cmdarg.get_str_arg(1);
                s.push(b'\n');
                self.host.file_output(&s);
            }
            CmdType::LogoOff => self.set_system_reg_nologo(false),
            CmdType::OldAdjust => {
                let val = cmdarg.get_val_str_arg(1);
                self.ds.ext_opt.old_adjust = val;
            }
            CmdType::LogoDirect => {
                let list = cmdarg.get_str_opt(OptType::StrValListR);
                self.set_logo_direct_string(&list);
            }
            CmdType::LogoExact => {
                let val = cmdarg.get_val_str_arg(1);
                self.ds.ext_opt.nl_g_exact = val;
                self.ds.ext_opt.fix_nl_g_exact = 1;
            }
            CmdType::LogoReset => {
                self.ds.backup_logoset_load();
                self.g.set_exe1st(true);
            }
            CmdType::ReadData => {
                let name = cmdarg.get_str_arg(1);
                self.read_data_list(cmdarg, &name, true);
            }
            CmdType::ReadString => {
                let name = cmdarg.get_str_arg(1);
                self.read_data_list(cmdarg, &name, false);
            }
            CmdType::ReadTrim => {
                let name = cmdarg.get_str_arg(1);
                self.read_data_trim(cmdarg, &name);
            }
            CmdType::EnvGet => {
                let name = cmdarg.get_str_arg(1);
                self.read_data_env_get(cmdarg, &name);
            }
            _ => {
                self.g.add_msg_error(b"error:internal setting\n");
                success = false;
            }
        }
        success
    }

    fn set_cmd_reg(&mut self, cmdarg: &mut CmdArg, state: &mut State) -> bool {
        let mut success = true;
        if self.set_arg_state_set_cmd(cmdarg, state) {
            return true;
        }
        match cmdarg.cmdsel {
            CmdType::Default
            | CmdType::Set
            | CmdType::EvalFrame
            | CmdType::EvalTime
            | CmdType::EvalNum
            | CmdType::LocalSet => {
                let mut overwrite = cmdarg.cmdsel != CmdType::Default;
                let mut flag_local = cmdarg.cmdsel == CmdType::LocalSet;
                if cmdarg.get_opt(OptType::FlagLocal) > 0 {
                    flag_local = true;
                }
                if cmdarg.get_opt(OptType::FlagDefault) > 0 {
                    overwrite = false;
                }
                let n = cmdarg.get_str_arg(1);
                let v = cmdarg.get_str_arg(2);
                success = self.vars.set_jls_reg_var_with_local(
                    &n,
                    &v,
                    overwrite,
                    flag_local,
                    &mut self.ds,
                );
            }
            CmdType::SetParam => {
                let tp = cmdarg.get_val_str_arg(1);
                let val = cmdarg.get_val_str_arg(2);
                self.ds.set_config_raw(tp as usize, val);
            }
            CmdType::CountUp => {
                success = false;
                let flag_local = cmdarg.get_opt(OptType::FlagLocal) > 0;
                let n = cmdarg.get_str_arg(1);
                self.vars
                    .set_jls_reg_var_with_local(&n, b"0", false, flag_local, &mut self.ds);
                let sv = self.vars.get_jls_reg_var(&n, true).1;
                if let Some(str_val) = sv {
                    let val_opt = self.vars.cnv.borrow().get_str_val_num(&str_val, 0).1;
                    if let Some(val) = val_opt {
                        let nv = (val + 1).to_string().into_bytes();
                        success = self.vars.set_jls_reg_var_with_local(
                            &n,
                            &nv,
                            true,
                            flag_local,
                            &mut self.ds,
                        );
                    }
                }
            }
            CmdType::OptSet | CmdType::OptDefault => {
                let overwrite = cmdarg.cmdsel != CmdType::OptDefault;
                let arg = cmdarg.get_str_arg(1);
                self.set_system_reg_options(&arg, 0, overwrite);
            }
            CmdType::UnitSec => {
                let val = cmdarg.get_val_str_arg(1);
                self.vars.cnv.borrow_mut().change_unit_sec(val);
                self.set_system_reg_update();
            }
            CmdType::ArgSet => {
                let n = cmdarg.get_str_arg(1);
                let v = cmdarg.get_str_arg(2);
                success = self.vars.set_arg_reg(&n, &v);
            }
            CmdType::ListGetAt => {
                let num = cmdarg.get_val_str_arg(1);
                self.set_result_reg_list_get_at(cmdarg, num);
            }
            CmdType::ListIns => {
                let num = cmdarg.get_val_str_arg(1);
                self.set_result_reg_list_ins(cmdarg, num);
            }
            CmdType::ListDel => {
                let num = cmdarg.get_val_str_arg(1);
                self.set_result_reg_list_del(cmdarg, num);
            }
            CmdType::ListSetAt => {
                let num = cmdarg.get_val_str_arg(1);
                self.set_result_reg_list_rep(cmdarg, num);
            }
            CmdType::ListClear => self.set_result_reg_list_clear(cmdarg),
            CmdType::ListSort => self.set_result_reg_list_sort(cmdarg),
            _ => {
                self.g.add_msg_error(b"error:internal setting(RegCmd)\n");
                success = false;
            }
        }
        success
    }

    fn set_cmd_mem_flow(&mut self, cmdarg: &mut CmdArg, state: &mut State) -> bool {
        let mut success = true;
        match cmdarg.cmdsel {
            CmdType::LazyStart => {
                let mut type_lazy = LazyType::LazyE;
                let tack = lazy_from_i32(cmdarg.tack.type_lazy);
                if tack != LazyType::None {
                    type_lazy = tack;
                }
                state.set_lazy_start_type(type_lazy);
                state.check_arg_reg_insert(cmdarg.cmdsel);
            }
            CmdType::EndLazy => state.set_lazy_start_type(LazyType::None),
            CmdType::Memory => {
                let name = cmdarg.get_str_arg(1);
                state.start_mem_area(&name);
                state.check_arg_reg_insert(cmdarg.cmdsel);
            }
            CmdType::EndMemory => state.end_mem_area(),
            CmdType::ExpandOn => state.set_mem_expand(true),
            CmdType::ExpandOff => state.set_mem_expand(false),
            _ => {
                self.g
                    .add_msg_error(b"error:internal setting(LazyCategory)\n");
                success = false;
            }
        }
        success
    }

    fn set_cmd_mem_exe(&mut self, cmdarg: &mut CmdArg, state: &mut State) -> bool {
        let mut success = true;
        match cmdarg.cmdsel {
            CmdType::MemCall => {
                let name = cmdarg.get_str_arg(1);
                state.set_mem_call(&mut self.g, &name);
            }
            CmdType::MemErase => {
                let name = cmdarg.get_str_arg(1);
                state.set_mem_erase(&mut self.g, &name);
            }
            CmdType::MemCopy => {
                let a = cmdarg.get_str_arg(1);
                let b = cmdarg.get_str_arg(2);
                state.set_mem_copy(&mut self.g, &a, &b);
            }
            CmdType::MemMove => {
                let a = cmdarg.get_str_arg(1);
                let b = cmdarg.get_str_arg(2);
                state.set_mem_move(&mut self.g, &a, &b);
            }
            CmdType::MemAppend => {
                let a = cmdarg.get_str_arg(1);
                let b = cmdarg.get_str_arg(2);
                state.set_mem_append(&mut self.g, &a, &b);
            }
            CmdType::LazyFlush => state.set_lazy_flush(&mut self.g),
            CmdType::LazyAuto => state.set_lazy_auto(true),
            CmdType::LazyStInit => state.set_lazy_state_ini_auto(&mut self.g, true),
            CmdType::MemOnce => {
                let val = cmdarg.get_val_str_arg(1);
                state.set_mem_dupe(val != 1);
            }
            CmdType::MemEcho => {
                let name = cmdarg.get_str_arg(1);
                self.set_mem_echo(&name);
            }
            CmdType::MemDump => {
                // getMapForDebug prints map iteration (unordered in both
                // languages); excluded from xcheck by design
            }
            _ => {
                self.g
                    .add_msg_error(b"error:internal setting(MemCategory)\n");
                success = false;
            }
        }
        success
    }

    fn set_mem_echo(&mut self, name: &[u8]) {
        let mut que = std::collections::VecDeque::new();
        if self.g.get_list_by_name(&mut que, name) {
            while let Some(v) = que.pop_front() {
                let mut s = v;
                s.push(b'\n');
                self.host.file_output(&s);
            }
        }
    }

    /// exeCmd (categories dispatch; -else / -cont prechecks)
    fn exe_cmd(&mut self, cmdarg: &mut CmdArg, lim: &mut CmdLimit) -> bool {
        let mut valid_exe = true;
        let mut exe_command = false;
        if cmdarg.get_opt(OptType::FlagElse) > 0 && self.is_system_reg_lastexe() {
            valid_exe = false;
            exe_command = true;
        }
        if cmdarg.get_opt(OptType::FlagCont) > 0 && !self.is_system_reg_lastexe() {
            valid_exe = false;
        }
        if valid_exe {
            {
                let ds = &self.ds;
                let cnv = self.vars.cnv.clone();
                let c = cnv.borrow();
                limit::limit_common_range(ds, &c, cmdarg, lim);
            }
            self.flush_limit_err(lim);
            if cmdarg.tack.need_auto {
                let mut ctx = AutoCtx {
                    g: &mut self.g,
                    ds: &mut self.ds,
                    vars: &mut self.vars,
                };
                self.host.auto_script(&mut ctx, cmdarg, lim, true);
            }
            match cmdarg.category {
                CmdCat::AUTO => exe_command = self.exe_cmd_call_auto_main(cmdarg, lim, false),
                CmdCat::AUTOEACH => exe_command = self.exe_cmd_auto_each(cmdarg, lim),
                CmdCat::LOGO | CmdCat::AUTOLOGO => exe_command = self.exe_cmd_logo(cmdarg, lim),
                CmdCat::NEXT => exe_command = self.exe_cmd_next_tail(cmdarg, lim),
                _ => {}
            }
        }
        exe_command
    }

    fn flush_limit_err(&mut self, lim: &mut CmdLimit) {
        if !lim.err_buf.is_empty() {
            let msg = std::mem::take(&mut lim.err_buf);
            self.host.stderr_line(&msg);
        }
    }

    /// exeCmdCallAutoMain (AUTO category and -autochg paths)
    fn exe_cmd_call_auto_main(
        &mut self,
        cmdarg: &mut CmdArg,
        lim: &mut CmdLimit,
        setup_only: bool,
    ) -> bool {
        if self.ds.is_auto_mode_initial() {
            if !self.ds.is_exist_logo() {
                self.ds.set_level_use_logo(1); // CONFIG_LOGO_LEVEL_UNUSE_ALL
            } else {
                let mut level = self.ds.get_config(ConfigVarType::LogoLevel);
                if level <= 0 {
                    level = 7; // CONFIG_LOGO_LEVEL_USE_HIGH
                }
                self.ds.set_level_use_logo(level);
            }
            if self.ds.is_unuse_logo() {
                self.ds.ext_opt.flag_no_logo = 1;
                self.set_system_reg_update();
            }
        }
        let cnv = self.vars.cnv.clone();
        let _c = cnv.borrow();
        drop(_c);
        let mut ctx = AutoCtx {
            g: &mut self.g,
            ds: &mut self.ds,
            vars: &mut self.vars,
        };
        self.host.auto_script(&mut ctx, cmdarg, lim, setup_only)
    }

    /// exeCmdCallAutoScript
    fn exe_cmd_call_auto_script(&mut self, cmdarg: &mut CmdArg, lim: &mut CmdLimit) -> bool {
        self.exe_cmd_call_auto_main(cmdarg, lim, false)
    }

    /// exeCmdAutoEach
    fn exe_cmd_auto_each(&mut self, cmdarg: &mut CmdArg, lim: &mut CmdLimit) -> bool {
        let mut exeflag_total = false;
        let mut logopt = NrfCurrent::default();
        while self.ds.nrf_pt_next(&mut logopt, 1) {
            let mut st;
            let mut ed;
            {
                let (m_r, rtype_rise) = self
                    .ds
                    .result_logo_at_nrf(logopt.nrf_rise)
                    .unwrap_or((0, LOGO_RESULT_NONE));
                let (m_f, rtype_fall) = self
                    .ds
                    .result_logo_at_nrf(logopt.nrf_fall)
                    .unwrap_or((0, LOGO_RESULT_NONE));
                st = m_r;
                ed = m_f;
                if rtype_rise != LOGO_RESULT_DECIDE {
                    st = logopt.msec_rise;
                }
                if rtype_fall != LOGO_RESULT_DECIDE {
                    ed = logopt.msec_fall;
                }
            }
            {
                let ds = &self.ds;
                let cnv = self.vars.cnv.clone();
                let c = cnv.borrow();
                limit::resize_range_head_tail(ds, &c, cmdarg, lim, st, ed);
            }
            self.flush_limit_err(lim);
            let exeflag = self.exe_cmd_call_auto_script(cmdarg, lim);
            if exeflag {
                exeflag_total = true;
            }
        }
        exeflag_total
    }

    /// exeCmdLogo
    fn exe_cmd_logo(&mut self, cmdarg: &mut CmdArg, lim: &mut CmdLimit) -> bool {
        let nmax_list = {
            let ds = &self.ds;
            let cnv = self.vars.cnv.clone();
            let c = cnv.borrow();
            let n = limit::limit_logo_list(ds, cmdarg, lim);
            let _ = c;
            n
        };
        let flag_onepoint = cmdarg.tack.one_point;
        let mut nlist_base = -1i32;
        if flag_onepoint {
            let mut difmsec_base = 0i32;
            for i in 0..nmax_list {
                let exeflag = {
                    let ds = &self.ds;
                    let cnv = self.vars.cnv.clone();
                    let c = cnv.borrow();
                    limit::select_target_by_logo(ds, &c, cmdarg, lim, i)
                };
                self.flush_limit_err(lim);
                if exeflag {
                    let wmsec = lim.get_target_range_wide();
                    let msec_target = wmsec.just;
                    let nsc_sel = lim.get_result_target_sel();
                    let msec_sel = self.ds.msec_scp(nsc_sel);
                    let difmsec_sel = (msec_target - msec_sel).abs();
                    if difmsec_sel < difmsec_base || nlist_base < 0 {
                        nlist_base = i;
                        difmsec_base = difmsec_sel;
                    }
                }
            }
        }
        if cmdarg.cmdsel == CmdType::GetList || cmdarg.cmdsel == CmdType::GetPos {
            self.update_result_reg_write(cmdarg);
        }
        let mut exeflag_total = false;
        for i in 0..nmax_list {
            if !flag_onepoint || nlist_base == i {
                let mut exeflag = {
                    let ds = &self.ds;
                    let cnv = self.vars.cnv.clone();
                    let c = cnv.borrow();
                    limit::select_target_by_logo(ds, &c, cmdarg, lim, i)
                };
                self.flush_limit_err(lim);
                if exeflag {
                    match cmdarg.category {
                        CmdCat::LOGO => exeflag = self.exe_cmd_logo_target(cmdarg, lim),
                        CmdCat::AUTOLOGO => exeflag = self.exe_cmd_call_auto_script(cmdarg, lim),
                        _ => {}
                    }
                }
                if exeflag {
                    exeflag_total = true;
                }
            }
        }
        exeflag_total
    }

    /// exeCmdLogoTarget
    fn exe_cmd_logo_target(&mut self, cmdarg: &mut CmdArg, lim: &mut CmdLimit) -> bool {
        let mut exe_command = false;
        let msec_force = lim.get_target_range_force();
        let nsc_scpos_sel = lim.get_result_target_sel();
        let nsc_scpos_end = lim.get_result_target_end();
        let nrf = lim.get_logo_base_nrf();
        match cmdarg.cmdsel {
            CmdType::Find => {
                if nsc_scpos_sel >= 0 {
                    if cmdarg.get_opt(OptType::FlagAutoChg) > 0 {
                        self.exe_cmd_call_auto_script(cmdarg, lim);
                    } else {
                        let edge = nrf % 2; // edgeFromNrf
                        let msec_tmp = self.ds.msec_scp_edge(nsc_scpos_sel, edge);
                        self.ds
                            .set_result_logo_at_nrf(msec_tmp, LOGO_RESULT_DECIDE, nrf);
                    }
                    exe_command = true;
                }
            }
            CmdType::MkLogo => {
                if nsc_scpos_sel >= 0 && nsc_scpos_end >= 0 {
                    let (msec_st, msec_ed) = if nsc_scpos_sel < nsc_scpos_end {
                        (
                            self.ds.msec_scp(nsc_scpos_sel),
                            self.ds.msec_scp_bk(nsc_scpos_end),
                        )
                    } else {
                        (
                            self.ds.msec_scp(nsc_scpos_end),
                            self.ds.msec_scp_bk(nsc_scpos_sel),
                        )
                    };
                    let overlap = cmdarg.get_opt(OptType::FlagOverlap) != 0;
                    let confirm = cmdarg.get_opt(OptType::FlagConfirm) != 0;
                    let unit = cmdarg.get_opt(OptType::FlagUnit) != 0;
                    let nsc_ins = self
                        .ds
                        .logo_insert(msec_st, msec_ed, overlap, confirm, unit);
                    exe_command = nsc_ins >= 0;
                }
            }
            CmdType::DivLogo => {
                if nsc_scpos_sel >= 0 || msec_force >= 0 {
                    let msec_target = if nsc_scpos_sel >= 0 {
                        self.ds.msec_scp(nsc_scpos_sel)
                    } else {
                        msec_force
                    };
                    let (msec_st, msec_ed) = {
                        let cnv = self.vars.cnv.borrow();
                        if nrf % 2 == 0 {
                            // start edge
                            let st = self.ds.msec_logo_nrf(nrf);
                            let ed =
                                cnv.tc.get_msec_adjust_frm_from_msec(msec_target as i64, -1) as i32;
                            (st, ed)
                        } else {
                            let st = cnv.tc.get_msec_align_from_msec(msec_target as i64) as i32;
                            let ed = self.ds.msec_logo_nrf(nrf);
                            (st, ed)
                        }
                    };
                    let confirm = cmdarg.get_opt(OptType::FlagConfirm) != 0;
                    let nsc_ins = self.ds.logo_insert(msec_st, msec_ed, true, confirm, true);
                    exe_command = nsc_ins >= 0;
                }
            }
            CmdType::Select => {
                if nsc_scpos_sel >= 0 {
                    let nsc_scpos = self.ds.size_scp();
                    let msec_nrf = self.ds.msec_logo_nrf(nrf);
                    for j in 1..nsc_scpos - 1 {
                        let msec_j = self.ds.msec_scp(j);
                        if msec_j == msec_nrf && self.ds.scp_statpos(j) > PRIOR_NONE {
                            self.ds.set_scp_statpos(j, PRIOR_NONE);
                        }
                    }
                    if nrf == 0 {
                        self.ds.rec_hold.msec_select1st = msec_nrf;
                    }
                    let msec_tmp = self.ds.msec_scp(nsc_scpos_sel);
                    self.ds.set_msec_logo_nrf(nrf, msec_tmp);
                    self.ds.set_scp_statpos(nsc_scpos_sel, PRIOR_DECIDE);
                    exe_command = true;
                    if cmdarg.get_opt(OptType::FlagConfirm) > 0 {
                        self.ds
                            .set_result_logo_at_nrf(msec_tmp, LOGO_RESULT_DECIDE, nrf);
                    }
                }
            }
            CmdType::Force => {
                let wmsec = lim.get_target_range_wide();
                let msec_tmp = {
                    let cnv = self.vars.cnv.borrow();
                    cnv.tc.get_msec_align_from_msec(wmsec.just as i64) as i32
                };
                if msec_tmp >= 0 {
                    exe_command = true;
                    if cmdarg.get_opt(OptType::FlagAutoChg) > 0 {
                        self.exe_cmd_call_auto_script(cmdarg, lim);
                    } else {
                        self.ds
                            .set_result_logo_at_nrf(msec_tmp, LOGO_RESULT_DECIDE, nrf);
                    }
                }
            }
            CmdType::Abort => {
                exe_command = true;
                self.ds.set_result_logo_at_nrf(-1, LOGO_RESULT_ABORT, nrf);
                if cmdarg.get_opt(OptType::FlagWithN) > 0 {
                    self.ds
                        .set_result_logo_at_nrf(-1, LOGO_RESULT_ABORT, nrf + 1);
                }
                if cmdarg.get_opt(OptType::FlagWithP) > 0 {
                    self.ds
                        .set_result_logo_at_nrf(-1, LOGO_RESULT_ABORT, nrf - 1);
                }
            }
            CmdType::GetPos => {
                let mut msec_hold = 0i32;
                exe_command = self.get_msec_target_with_force(&mut msec_hold, cmdarg, lim);
                if exe_command {
                    self.set_result_reg_poshold(cmdarg, msec_hold);
                }
            }
            CmdType::GetList => {
                let mut msec_hold = 0i32;
                exe_command = self.get_msec_target_with_force(&mut msec_hold, cmdarg, lim);
                if exe_command {
                    self.set_result_reg_listhold(cmdarg, msec_hold);
                }
            }
            CmdType::DivFile => {
                let msec = lim.get_target_range_wide().just;
                exe_command = true;
                let v = &mut self.ds.div_file;
                let pos = v.partition_point(|&x| x < msec);
                if pos == v.len() || v[pos] != msec {
                    v.insert(pos, msec);
                }
            }
            _ => {}
        }
        exe_command
    }

    /// exeCmdNextTail
    fn exe_cmd_next_tail(&mut self, cmdarg: &mut CmdArg, lim: &mut CmdLimit) -> bool {
        let mut msec_headframe = -1i32;
        if let Some(cstr) = self.vars.get_jls_reg_var(b"TAILTIME", true).1 {
            if let Some(v) = self.vars.cnv.borrow().get_str_val_msec_m1(&cstr, 0).1 {
                msec_headframe = v;
            }
        }
        let wmsec_target = WideMsec {
            just: msec_headframe + cmdarg.wmsec.just,
            early: msec_headframe + cmdarg.wmsec.early,
            late: msec_headframe + cmdarg.wmsec.late,
        };
        let force = cmdarg.get_opt(OptType::FlagForce) > 0;
        {
            let ds = &self.ds;
            let cnv = self.vars.cnv.clone();
            let c = cnv.borrow();
            limit::select_target_by_range(ds, &c, cmdarg, lim, wmsec_target, force);
        }
        self.flush_limit_err(lim);
        let mut msec_tailframe = 0i32;
        let exeflag = self.get_msec_target_with_force(&mut msec_tailframe, cmdarg, lim);
        if exeflag {
            self.set_system_reg_headtail(msec_headframe, msec_tailframe);
            if cmdarg.get_opt(OptType::FlagAutoChg) > 0 {
                self.exe_cmd_call_auto_script(cmdarg, lim);
            }
        }
        exeflag
    }

    /// getMsecTargetWithForce
    fn get_msec_target_with_force(
        &mut self,
        msec_result: &mut i32,
        cmdarg: &mut CmdArg,
        lim: &mut CmdLimit,
    ) -> bool {
        let nsc_sel = lim.get_result_target_sel();
        let msec_force = lim.get_target_range_force();
        let edge_out = lim.get_target_out_edge();
        let mut success = false;
        if nsc_sel >= 0 {
            *msec_result = self.ds.msec_scp_edge(nsc_sel, edge_out);
            success = true;
        } else if msec_force >= 0 {
            *msec_result = {
                let cnv = self.vars.cnv.borrow();
                cnv.tc.get_msec_align_from_msec(msec_force as i64) as i32
            };
            let (st, ed) = {
                let cnv = self.vars.cnv.borrow();
                if edge_out == EDGE_RISE || edge_out == EDGE_BOTH {
                    (
                        *msec_result,
                        cnv.tc
                            .get_msec_adjust_frm_from_msec(*msec_result as i64, -1)
                            as i32,
                    )
                } else {
                    (
                        cnv.tc.get_msec_adjust_frm_from_msec(*msec_result as i64, 1) as i32,
                        *msec_result,
                    )
                }
            };
            if cmdarg.get_opt(OptType::FlagForce) > 0 {
                self.ds.insert_scpos(st, ed, -1, PRIOR_DECIDE);
            }
            success = true;
        }
        success
    }

    // ================= arg insertion =================

    /// setArgStateSetCmd: Default in ArgBegin area registers an argument name
    fn set_arg_state_set_cmd(&mut self, cmdarg: &mut CmdArg, state: &mut State) -> bool {
        if cmdarg.cmdsel != CmdType::Default {
            return false;
        }
        if !state.is_arg_area() {
            return false;
        }
        let str_name = cmdarg.get_str_arg(1);
        let mut str_val = self.vars.get_jls_reg_var(&str_name, true).1;
        if str_val.is_none() {
            str_val = Some(cmdarg.get_str_arg(2));
        }
        let str_val = str_val.unwrap_or_default();
        let overwrite = true;
        let flag_local = true;
        let success = self.vars.set_jls_reg_var_with_local(
            &str_name,
            &str_val,
            overwrite,
            flag_local,
            &mut self.ds,
        );
        if success {
            state.add_arg_name(&str_name);
        }
        success
    }

    /// setArgStateInsertReg
    fn set_arg_state_insert_reg(&mut self, state: &mut State) -> bool {
        let mut flag_exe = false;
        let num_arg = state.size_arg_name_list();
        if num_arg > 0 {
            let type_lazy = state.get_lazy_start_type();
            let str_mem_name = if state.is_mem_area() {
                state.get_mem_name()
            } else {
                Vec::new()
            };
            for i in 0..num_arg {
                let name = match state.get_arg_name(i) {
                    Some(n) => n,
                    None => continue,
                };
                let val = self.vars.get_jls_reg_var(&name, true).1;
                if let Some(val) = val {
                    let mut str_buf = b"LocalSet ".to_vec();
                    str_buf.extend_from_slice(&name);
                    str_buf.push(b' ');
                    str_buf.extend_from_slice(&val);
                    if type_lazy != LazyType::None {
                        flag_exe = state.set_lazy_store(&mut self.g, type_lazy, &str_buf);
                    } else if !str_mem_name.is_empty() {
                        flag_exe = state.set_mem_store(&mut self.g, &str_mem_name, &str_buf);
                    }
                }
            }
        }
        flag_exe
    }

    // ================= file/path/env =================

    /// setOptionsGetOne (CLI-shared with the in-script OptSet command)
    pub fn set_options_get_one(
        &mut self,
        argrest: i32,
        strv: &[u8],
        str1: Option<&[u8]>,
        str2: Option<&[u8]>,
        overwrite: bool,
    ) -> i32 {
        if argrest <= 0 {
            return 0;
        }
        let exist2 = argrest >= 2;
        let _exist3 = argrest >= 3;
        let mut numarg = 0i32;
        if strv.first() == Some(&b'-') && strv.len() > 1 {
            if strv.eq_ignore_ascii_case(b"-flags") {
                let Some(f) = str1 else {
                    self.host.stderr_line(b"-flags needs an argument\n");
                    return -1;
                };
                if !self.set_input_flags(f, overwrite) {
                    self.host.stderr_line(b"-flags bad argument\n");
                    return -1;
                }
                numarg = 2;
            } else if strv.eq_ignore_ascii_case(b"-set") {
                let (Some(n), Some(v)) = (str1, str2) else {
                    self.host.stderr_line(b"-set needs two arguments\n");
                    return -1;
                };
                if !self.set_input_reg(n, v, overwrite) {
                    self.host.stderr_line(b"-set bad argument\n");
                    return -1;
                }
                numarg = 3;
            } else if strv.eq_ignore_ascii_case(b"-cutmrgin") {
                if !exist2 {
                    self.host.stderr_line(b"-cutmrgin needs an argument\n");
                    return -1;
                } else if overwrite || self.ds.ext_opt.fix_cut_in == 0 {
                    self.ds.ext_opt.msec_cut_in = crate_timeaxis_cut_mrg(str1.unwrap());
                    self.ds.ext_opt.fix_cut_in = 1;
                }
                numarg = 2;
            } else if strv.eq_ignore_ascii_case(b"-cutmrgout") {
                if !exist2 {
                    self.host.stderr_line(b"-cutmrgout needs an argument\n");
                    return -1;
                } else if overwrite || self.ds.ext_opt.fix_cut_out == 0 {
                    self.ds.ext_opt.msec_cut_out = crate_timeaxis_cut_mrg(str1.unwrap());
                    self.ds.ext_opt.fix_cut_out = 1;
                }
                numarg = 2;
            } else if strv.eq_ignore_ascii_case(b"-cutmrgwi") {
                if !exist2 {
                    self.host.stderr_line(b"-cutmrgwi needs an argument\n");
                    return -1;
                } else if overwrite || self.ds.ext_opt.fix_wid_cut_i == 0 {
                    self.ds.ext_opt.wide_cut_in = stoi_prefix(str1.unwrap());
                    self.ds.ext_opt.fix_wid_cut_i = 1;
                }
                numarg = 2;
            } else if strv.eq_ignore_ascii_case(b"-cutmrgwo") {
                if !exist2 {
                    self.host.stderr_line(b"-cutmrgwo needs an argument\n");
                    return -1;
                } else if overwrite || self.ds.ext_opt.fix_wid_cut_o == 0 {
                    self.ds.ext_opt.wide_cut_out = stoi_prefix(str1.unwrap());
                    self.ds.ext_opt.fix_wid_cut_o = 1;
                }
                numarg = 2;
            } else if strv.eq_ignore_ascii_case(b"-sublist") {
                if !exist2 {
                    self.host.stderr_line(b"-sublist needs an argument\n");
                    return -1;
                } else if overwrite || self.ds.ext_opt.fix_sub_list == 0 {
                    let s1 = str1.unwrap();
                    if s1.first() == Some(&b'+') {
                        let mut tmp = s1[1..].to_vec();
                        tmp.push(b',');
                        tmp.extend_from_slice(&self.ds.ext_opt.sub_list);
                        self.ds.ext_opt.sub_list = tmp;
                    } else {
                        self.ds.ext_opt.sub_list = s1.to_vec();
                    }
                    self.ds.ext_opt.fix_sub_list = 1;
                }
                numarg = 2;
            } else if strv.eq_ignore_ascii_case(b"-subpath") {
                if !exist2 {
                    self.host.stderr_line(b"-subpath needs an argument\n");
                    return -1;
                } else if overwrite || self.ds.ext_opt.fix_sub_path == 0 {
                    self.ds.ext_opt.sub_path = str1.unwrap().to_vec();
                    self.ds.ext_opt.fix_sub_path = 1;
                }
                numarg = 2;
            } else if strv.eq_ignore_ascii_case(b"-vline") {
                if !exist2 {
                    self.host.stderr_line(b"-vline needs an argument\n");
                    return -1;
                }
                if overwrite || self.ds.ext_opt.fix_v_line == 0 {
                    self.ds.ext_opt.v_line = stoi_prefix(str1.unwrap());
                    self.ds.ext_opt.fix_v_line = 1;
                }
                numarg = 2;
            }
        }
        numarg
    }

    /// setInputReg
    pub fn set_input_reg(&mut self, name: &[u8], val: &[u8], overwrite: bool) -> bool {
        self.vars
            .set_jls_reg_var(name, val, overwrite, &mut self.ds)
    }

    /// setInputFlags
    pub fn set_input_flags(&mut self, flags: &[u8], overwrite: bool) -> bool {
        let mut ret = true;
        let mut pos = 0i32;
        loop {
            let mut str_flag: Vec<u8> = Vec::new();
            pos = {
                let cnv = self.vars.cnv.borrow();
                cnv.get_str_word(&mut str_flag, flags, pos)
            };
            if pos < 0 {
                break;
            }
            let (str_name, str_val) = match str_flag.iter().position(|&c| c == b':') {
                Some(n) => (str_flag[..n].to_vec(), str_flag[n + 1..].to_vec()),
                None => (str_flag.clone(), b"1".to_vec()),
            };
            if !self.set_input_reg(&str_name, &str_val, overwrite) {
                ret = false;
            }
        }
        ret
    }

    /// setSystemRegOptions (public for CLI token application)
    pub fn set_system_reg_options_pub(&mut self, buf: &[u8], overwrite: bool) -> bool {
        self.set_system_reg_options(buf, 0, overwrite)
    }

    /// makeFullPath
    pub fn make_full_path(
        &mut self,
        str_full: &mut Vec<u8>,
        str_src: &[u8],
        flag_full: bool,
    ) -> bool {
        let mut str_path_only;

        let str_name: Vec<u8> = if flag_full {
            {
                let mut p = Vec::new();
                let _ = self
                    .vars
                    .cnv
                    .borrow_mut()
                    .get_str_file_path(&mut p, str_src);
                str_path_only = p;
            }
            self.g.set_path_name_jl(&str_path_only);
            *str_full = str_src.to_vec();
            if str_path_only.len() <= str_src.len() {
                str_src[str_path_only.len()..].to_vec()
            } else {
                Vec::new()
            }
        } else {
            str_path_only = self.g.get_path_name_jl().to_vec();
            let mut f = str_path_only.clone();
            f.extend_from_slice(str_src);
            *str_full = f;
            str_src.to_vec()
        };
        let mut decide = self.host.file_exists(str_full);
        if !decide {
            if !self.ds.ext_opt.sub_path.is_empty() {
                let mut p = self.ds.ext_opt.sub_path.clone();
                self.vars.cnv.borrow_mut().get_str_file_all_path(&mut p);
                str_path_only = p;
            }
            let mut remain = self.ds.ext_opt.sub_list.clone();
            remain.push(b',');
            while !remain.is_empty() && !decide {
                let subname;
                if let Some(pos) = index_byte(&remain, b',') {
                    subname = remain[..pos].to_vec();
                    remain = if pos + 1 < remain.len() {
                        remain[pos + 1..].to_vec()
                    } else {
                        Vec::new()
                    };
                } else {
                    subname = remain.clone();
                    remain = Vec::new();
                }
                let mut subname = subname;
                self.vars
                    .cnv
                    .borrow_mut()
                    .get_str_file_all_path(&mut subname);
                let mut str_try = str_path_only.clone();
                str_try.extend_from_slice(&subname);
                str_try.extend_from_slice(&str_name);
                decide = self.host.file_exists(&str_try);
                if decide {
                    *str_full = str_try;
                }
            }
        }
        decide
    }

    // ================= readData family =================

    fn read_data_list(&mut self, cmdarg: &mut CmdArg, fname: &[u8], flag_num: bool) -> bool {
        self.update_result_reg_write(cmdarg);
        let lines = match self.host.read_script_lines(fname) {
            Some(l) => l,
            None => {
                let mut m = b"warning: file not found(".to_vec();
                m.extend_from_slice(fname);
                m.extend_from_slice(b")\n");
                self.host.file_output(&m);
                return false;
            }
        };
        let mut success = false;
        for raw in &lines {
            let sdata = {
                let cnv = self.vars.cnv.borrow();
                let mut d = Vec::new();
                if cnv.get_str_item(&mut d, raw, 0) > 0 {
                    if flag_num {
                        if cnv.get_str_val_num(&d, 0).1.is_some() {
                            Some(d)
                        } else {
                            None
                        }
                    } else {
                        Some(raw.clone())
                    }
                } else if !flag_num {
                    Some(raw.clone())
                } else {
                    None
                }
            };
            if let Some(data) = sdata {
                self.read_data_str_add(cmdarg, &data);
                success = true;
            }
        }
        success
    }

    fn read_data_trim(&mut self, cmdarg: &mut CmdArg, fname: &[u8]) -> bool {
        self.update_result_reg_write(cmdarg);
        let lines = match self.host.read_script_lines(fname) {
            Some(l) => l,
            None => {
                let mut m = b"warning: file not found(".to_vec();
                m.extend_from_slice(fname);
                m.extend_from_slice(b")\n");
                self.host.file_output(&m);
                return false;
            }
        };
        // reference reads with its own line joining; Trim scan is wired when
        // the AVS trim path is exercised (JlsIF-level outputs come later)
        let mut success = false;
        for line in &lines {
            let (st_txt, ed_txt) = match trim_detect(line) {
                Some(v) => v,
                None => continue,
            };
            self.read_data_str_trim(cmdarg, &st_txt, &ed_txt);
            success = true;
        }
        success
    }

    fn read_data_str_trim(&mut self, cmdarg: &mut CmdArg, str_loc_st: &[u8], str_loc_ed: &[u8]) {
        let cnv = self.vars.cnv.clone();
        let b = cnv.borrow();
        let st = (b.get_str_val_msec_from_frm(str_loc_st, 0).1.unwrap_or(0) as f64 / 1000.0).floor()
            as i32
            + 1;
        let mut ed = (b.get_str_val_msec_from_frm(str_loc_ed, 0).1.unwrap_or(0) as f64 / 1000.0)
            .floor() as i32;
        let mut st = st;
        if ed < st {
            ed = st;
        } else {
            st -= 1;
        }
        let s = format!("{},{}", st, ed).into_bytes();
        self.read_data_str_add(cmdarg, &s);
    }

    fn read_data_env_get(&mut self, cmdarg: &mut CmdArg, str_env_name: &[u8]) -> bool {
        let env = self.host.env_var(str_env_name);
        let (success, str_val) = match env {
            Some(v) => (true, v),
            None => (false, Vec::new()),
        };
        let str_name = cmdarg.get_str_opt(OptType::StrRegEnv);
        let overwrite = cmdarg.get_opt(OptType::FlagDefault) == 0;
        let flag_local = cmdarg.get_opt(OptType::FlagLocal) > 0;
        let flag_clear = cmdarg.get_opt(OptType::FlagClear) > 0;
        let mut success = success;
        if success || flag_clear {
            let w = self.vars.set_jls_reg_var_with_local(
                &str_name,
                &str_val,
                overwrite,
                flag_local,
                &mut self.ds,
            );
            if !w {
                success = false;
            }
        }
        success
    }

    fn read_data_str_add(&mut self, cmdarg: &mut CmdArg, sdata: &[u8]) {
        let str_list = cmdarg.get_str_opt(OptType::StrValListW);
        cmdarg.set_str_opt(OptType::StrValListR, str_list);
        cmdarg.set_str_opt(OptType::StrValPosR, sdata.to_vec());
        self.set_result_reg_list_ins(cmdarg, -1);
    }

    /// setLogoDirectString
    fn set_logo_direct_string(&mut self, str_list: &[u8]) {
        let num_list = list_str_size(str_list);
        if self.ds.ext_opt.flag_no_logo != 0 {
            self.ds.ext_opt.flag_no_logo = 0;
            self.set_system_reg_update();
        }
        self.ds.clear_data_logo_all();
        self.ds.ext_opt.flag_direct = 1;
        self.ds.ext_opt.fix_f_direct = 1;
        self.ds.ext_opt.nl_g_exact = 3;
        let mut msec_last = 0i32;
        let mut flag_fall = false;
        for i in 0..num_list {
            let mut flag_valid = false;
            let mut msec_cur = 0i32;
            if let Some(str_cur) = list_str_element(str_list, i + 1) {
                if let Some(v) = self.vars.cnv.borrow().get_str_val_msec(&str_cur, 0).1 {
                    msec_cur = v;
                    flag_valid = true;
                }
            }
            if flag_valid && msec_last <= msec_cur {
                if !flag_fall && i % 2 == 0 {
                    msec_last = msec_cur;
                    flag_fall = true;
                } else if flag_fall && i % 2 != 0 {
                    let mut dt = LogoRecord {
                        rise: msec_last,
                        fall: msec_cur,
                        rise_l: msec_last,
                        rise_r: msec_last,
                        fall_l: msec_cur,
                        fall_r: msec_cur,
                        org_rise: msec_last,
                        org_fall: msec_cur,
                        org_rise_l: msec_last,
                        org_rise_r: msec_last,
                        org_fall_l: msec_cur,
                        org_fall_r: msec_cur,
                        ..Default::default()
                    };
                    dt.unit = 1; // LOGO_UNIT_DIVIDE
                    self.ds.push_logo(dt);
                    msec_last = msec_cur;
                    flag_fall = false;
                }
            }
        }
    }
}

// ================= helpers =================

pub enum Part1 {
    Remain(PendingSub),
    Call(Vec<u8>, PendingSub),
}

pub enum CallAction {
    Run(Vec<u8>),
    Skip,
    Fail,
}

fn lazy_from_i32(v: i32) -> LazyType {
    match v {
        1 => LazyType::Full,
        2 => LazyType::LazyS,
        3 => LazyType::LazyA,
        4 => LazyType::LazyE,
        _ => LazyType::None,
    }
}

fn index_byte(s: &[u8], b: u8) -> Option<usize> {
    s.iter().position(|&x| x == b)
}

fn find_sub(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    hay.windows(needle.len()).position(|w| w == needle)
}

/// std::regex icase "Trim\s*\(\s*(\d+)\s*,\s*(\d+)\s*\)" — manual scan for
/// case-insensitive "trim" then balanced digits; mirrors reference matches.
fn trim_detect(line: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
    let lower: Vec<u8> = line.iter().map(|c| c.to_ascii_lowercase()).collect();
    let mut i = 0;
    while i + 3 < lower.len() {
        if &lower[i..i + 4] == b"trim" {
            let mut j = i + 4;
            while j < line.len() && (line[j] == b' ' || line[j] == b'\t') {
                j += 1;
            }
            if j < line.len() && line[j] == b'(' {
                j += 1;
                while j < line.len() && (line[j] == b' ' || line[j] == b'\t') {
                    j += 1;
                }
                let s0 = j;
                while j < line.len() && line[j].is_ascii_digit() {
                    j += 1;
                }
                if j > s0 {
                    let st = line[s0..j].to_vec();
                    while j < line.len() && (line[j] == b' ' || line[j] == b'\t') {
                        j += 1;
                    }
                    if j < line.len() && line[j] == b',' {
                        j += 1;
                        while j < line.len() && (line[j] == b' ' || line[j] == b'\t') {
                            j += 1;
                        }
                        let e0 = j;
                        while j < line.len() && line[j].is_ascii_digit() {
                            j += 1;
                        }
                        if j > e0 {
                            let ed = line[e0..j].to_vec();
                            while j < line.len() && (line[j] == b' ' || line[j] == b'\t') {
                                j += 1;
                            }
                            if j < line.len() && line[j] == b')' {
                                return Some((st, ed));
                            }
                        }
                    }
                }
            }
        }
        i += 1;
    }
    None
}

/// setOptionsCnvCutMrg wrapper (core::timeaxis canonical 30fps rule)
fn crate_timeaxis_cut_mrg(token: &[u8]) -> i32 {
    let s = String::from_utf8_lossy(token);
    super::super::timeaxis::cnv_cut_mrg(&s) as i32
}

/// atoi semantics: leading integer, no error
fn stoi_prefix(token: &[u8]) -> i32 {
    let s = String::from_utf8_lossy(token);
    let t = s.trim_start();
    let end = t
        .find(|c: char| !(c.is_ascii_digit() || c == '-' || c == '+'))
        .unwrap_or(t.len());
    t[..end].parse::<i32>().unwrap_or(0)
}
