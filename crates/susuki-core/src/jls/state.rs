//! Derived from join_logo_scp (https://github.com/tobitti0/join_logo_scp,
//! commit d1a24d758f8b3c04352b82ff6756fdf3d56729b3), Copyright (C) tobitti0,
//! GPL-2.0-or-later. This port is a GPL derivative work, redistributed here
//! under GPL-3.0-or-later (design §10.6).
//! Port of JlsScriptState.{hpp,cpp}: per-Call script control state
//! (If / Repeat nesting, lazy & mem execution queues).

use std::collections::VecDeque;

use super::gstate::GState;
use super::tables::{CmdCat, CmdType};
use super::types::{CacheExeType, LazyType};

const SIZE_REPLINE: usize = 4096; // JlsNameSpace.hpp
const SIZE_MEMLINE: usize = 8192; // JlsNameSpace.hpp

#[derive(Clone, Copy, PartialEq, Eq)]
enum CondIfState {
    Finished,
    Prepare,
    Running,
}

#[derive(Clone, Copy)]
struct RepDepthHold {
    line_start: i32,
    count_loop: i32,
    ext_line_end: i32,
    ext_line_ret: i32,
    exe_type: CacheExeType,
    ext_flag_nest: bool,
}

pub struct State {
    // --- IF control ---
    if_skip: bool,
    list_if_state: Vec<CondIfState>,
    // --- Repeat control ---
    rep_skip: bool,
    rep_line_read_cache: i32,
    list_rep_cmd_cache: Vec<Vec<u8>>,
    list_rep_depth: Vec<RepDepthHold>,
    rep_line_ext_r_cache: i32,
    list_rep_ext_cache: Vec<Vec<u8>>,
    // --- return ---
    flag_return: bool,
    // --- lazy control ---
    type_cache_exe: CacheExeType,
    flag_cache_rep_ext: bool,
    lazy_auto: bool,
    lazy_start_type: LazyType,
    // --- mem control ---
    mem_area: bool,
    mem_name: Vec<u8>,
    mem_dupe: bool,
    mem_skip: bool,
    // --- mem/lazy control ---
    mem_expand: bool,
    // --- arg name list ---
    arg_area: bool,
    list_arg_name: Vec<Vec<u8>>,
    arg_ins_ready: bool,
    // --- lazy/mem execution queues ---
    cache_exe_lazy_s: VecDeque<Vec<u8>>,
    cache_exe_lazy_a: VecDeque<Vec<u8>>,
    cache_exe_lazy_e: VecDeque<Vec<u8>>,
    cache_exe_lazy_o: VecDeque<Vec<u8>>,
    cache_exe_mem: VecDeque<Vec<u8>>,
}

impl State {
    pub fn new() -> Self {
        let mut s = State {
            if_skip: false,
            list_if_state: Vec::new(),
            rep_skip: false,
            rep_line_read_cache: -1,
            list_rep_cmd_cache: Vec::new(),
            list_rep_depth: Vec::new(),
            rep_line_ext_r_cache: -1,
            list_rep_ext_cache: Vec::new(),
            flag_return: false,
            type_cache_exe: CacheExeType::None,
            flag_cache_rep_ext: false,
            lazy_auto: false,
            lazy_start_type: LazyType::None,
            mem_area: false,
            mem_name: Vec::new(),
            mem_dupe: false,
            mem_skip: false,
            mem_expand: false,
            arg_area: false,
            list_arg_name: Vec::new(),
            arg_ins_ready: false,
            cache_exe_lazy_s: VecDeque::new(),
            cache_exe_lazy_a: VecDeque::new(),
            cache_exe_lazy_e: VecDeque::new(),
            cache_exe_lazy_o: VecDeque::new(),
            cache_exe_mem: VecDeque::new(),
        };
        s.clear();
        s
    }

    pub fn clear(&mut self) {
        self.if_skip = false;
        self.list_if_state.clear();
        self.rep_skip = false;
        self.rep_line_read_cache = -1;
        self.list_rep_cmd_cache.clear();
        self.list_rep_depth.clear();
        self.rep_line_ext_r_cache = -1;
        self.list_rep_ext_cache.clear();
        self.flag_return = false;
        self.type_cache_exe = CacheExeType::None;
        self.flag_cache_rep_ext = false;
        self.lazy_auto = false;
        self.lazy_start_type = LazyType::None;
        self.mem_area = false;
        self.mem_name = Vec::new();
        self.mem_dupe = false;
        self.mem_skip = false;
        self.mem_expand = false;
        self.arg_area = false;
        self.list_arg_name.clear();
        self.arg_ins_ready = false;
        self.cache_exe_lazy_s.clear();
        self.cache_exe_lazy_a.clear();
        self.cache_exe_lazy_e.clear();
        self.cache_exe_lazy_o.clear();
        self.cache_exe_mem.clear();
    }

    // ================= If =================

    pub fn if_begin(&mut self, flag_cond: bool) -> i32 {
        let stat = if self.if_skip {
            self.if_skip = true;
            CondIfState::Finished
        } else if !flag_cond {
            self.if_skip = true;
            CondIfState::Prepare
        } else {
            self.if_skip = false;
            CondIfState::Running
        };
        self.list_if_state.push(stat);
        0
    }

    pub fn if_end(&mut self) -> i32 {
        let depth = self.list_if_state.len() as i32;
        if depth <= 0 {
            return 1;
        }
        self.list_if_state.pop();
        let depth = depth - 1;
        if depth <= 0 {
            self.if_skip = false;
        } else {
            self.if_skip = self.list_if_state[(depth - 1) as usize] != CondIfState::Running;
        }
        0
    }

    pub fn if_else(&mut self, flag_cond: bool) -> i32 {
        let depth = self.list_if_state.len() as i32;
        if depth <= 0 {
            return 1;
        }
        let stat_cur = self.list_if_state[(depth - 1) as usize];
        let stat_nxt;
        match stat_cur {
            CondIfState::Finished | CondIfState::Running => {
                self.if_skip = true;
                stat_nxt = CondIfState::Finished;
            }
            CondIfState::Prepare => {
                if flag_cond {
                    self.if_skip = false;
                    stat_nxt = CondIfState::Running;
                } else {
                    self.if_skip = true;
                    stat_nxt = CondIfState::Prepare;
                }
            }
        }
        self.list_if_state[(depth - 1) as usize] = stat_nxt;
        0
    }

    // ================= Repeat =================

    pub fn repeat_begin(&mut self, g: &mut GState, num: i32) -> i32 {
        if self.list_rep_depth.is_empty() {
            self.rep_skip = false;
            self.rep_line_read_cache = -1;
        }
        let str_cmd_repeat = format!("Repeat {}", num).into_bytes();
        let num = if num <= 0 || self.rep_skip {
            self.rep_skip = true;
            -1
        } else {
            num
        };
        let mut holdval = RepDepthHold {
            line_start: 0,
            count_loop: num,
            ext_line_end: 0,
            ext_line_ret: 0,
            exe_type: CacheExeType::None,
            ext_flag_nest: false,
        };
        let errval;
        if self.is_lazy_exe() || self.is_mem_exe() {
            if self.flag_cache_rep_ext {
                errval = self.repeat_begin_ext_nest(&mut holdval);
            } else {
                errval = self.repeat_begin_extend(g, &mut holdval, &str_cmd_repeat);
            }
        } else {
            errval = self.repeat_begin_normal(&mut holdval, &str_cmd_repeat);
        }
        if errval == 0 {
            self.list_rep_depth.push(holdval);
        }
        errval
    }

    fn repeat_begin_normal(&mut self, holdval: &mut RepDepthHold, str_cmd_repeat: &[u8]) -> i32 {
        let size_line = self.list_rep_cmd_cache.len() as i32;
        if size_line as usize >= SIZE_REPLINE {
            return 1;
        }
        let mut size_line = size_line;
        if size_line == 0 {
            self.list_rep_cmd_cache.push(str_cmd_repeat.to_vec());
            size_line += 1;
        }
        let mut line_start = self.rep_line_read_cache;
        if line_start < 0 {
            line_start = size_line;
        }
        holdval.line_start = line_start;
        holdval.ext_line_end = 0;
        holdval.ext_line_ret = 0;
        holdval.exe_type = CacheExeType::None;
        holdval.ext_flag_nest = false;
        0
    }

    fn repeat_begin_extend(
        &mut self,
        g: &mut GState,
        holdval: &mut RepDepthHold,
        str_cmd_repeat: &[u8],
    ) -> i32 {
        let size_line = self.list_rep_ext_cache.len();
        if size_line >= SIZE_REPLINE {
            return 1;
        }
        self.list_rep_ext_cache.push(str_cmd_repeat.to_vec());
        holdval.line_start = self.list_rep_ext_cache.len() as i32;
        holdval.exe_type = self.get_cache_exe_type();
        holdval.ext_flag_nest = false;
        let mut errval = 1i32;
        let mut q: VecDeque<Vec<u8>> = VecDeque::new();
        let que_slot: &mut VecDeque<Vec<u8>> = match holdval.exe_type {
            CacheExeType::LazyS => &mut self.cache_exe_lazy_s,
            CacheExeType::LazyA => &mut self.cache_exe_lazy_a,
            CacheExeType::LazyE => &mut self.cache_exe_lazy_e,
            CacheExeType::Mem => &mut self.cache_exe_mem,
            CacheExeType::None => &mut q,
        };
        if holdval.exe_type != CacheExeType::None {
            errval = Self::ext_move_queue(g, &mut self.list_rep_ext_cache, que_slot);
        }
        holdval.ext_line_end = self.list_rep_ext_cache.len() as i32 - 1;
        if holdval.line_start > holdval.ext_line_end {
            return 1;
        }
        if errval == 0 {
            holdval.ext_line_ret = self.rep_line_ext_r_cache;
            self.rep_line_ext_r_cache = holdval.line_start;
        }
        errval
    }

    fn repeat_begin_ext_nest(&mut self, holdval: &mut RepDepthHold) -> i32 {
        holdval.line_start = self.rep_line_ext_r_cache;
        holdval.exe_type = self.get_cache_exe_type();
        holdval.ext_line_end = 0;
        holdval.ext_line_ret = 0;
        holdval.ext_flag_nest = true;
        0
    }

    pub fn repeat_end(&mut self, g: &mut GState) -> i32 {
        let mut depth = self.list_rep_depth.len() as i32;
        if depth <= 0 {
            return 1;
        }
        if self.list_rep_depth[(depth - 1) as usize].count_loop > 0 {
            self.list_rep_depth[(depth - 1) as usize].count_loop -= 1;
        }
        let ext_mode = self.is_repeat_ext_type();
        if self.list_rep_depth[(depth - 1) as usize].count_loop > 0 {
            if ext_mode {
                self.rep_line_ext_r_cache = self.list_rep_depth[(depth - 1) as usize].line_start;
            } else {
                self.rep_line_read_cache = self.list_rep_depth[(depth - 1) as usize].line_start;
            }
        } else {
            if ext_mode {
                let hold = self.list_rep_depth[(depth - 1) as usize];
                self.repeat_end_extend(g, &hold);
            }
            self.list_rep_depth.pop();
            depth -= 1;
            if depth == 0 {
                self.list_rep_cmd_cache.clear();
                self.list_rep_ext_cache.clear();
                self.rep_skip = false;
            } else if self.rep_skip && self.list_rep_depth[(depth - 1) as usize].count_loop >= 0 {
                self.rep_skip = false;
            }
        }
        0
    }

    fn repeat_end_extend(&mut self, g: &mut GState, holdval: &RepDepthHold) {
        let n_next = self.rep_line_ext_r_cache;
        let n_end = holdval.ext_line_end;
        if holdval.ext_flag_nest {
            return;
        }
        if n_next <= n_end {
            match holdval.exe_type {
                CacheExeType::LazyS => {
                    let mut dq = std::mem::take(&mut self.cache_exe_lazy_s);
                    Self::ext_back_queue(g, &mut dq, &self.list_rep_ext_cache, n_next, n_end);
                    self.cache_exe_lazy_s = dq;
                }
                CacheExeType::LazyA => {
                    let mut dq = std::mem::take(&mut self.cache_exe_lazy_a);
                    Self::ext_back_queue(g, &mut dq, &self.list_rep_ext_cache, n_next, n_end);
                    self.cache_exe_lazy_a = dq;
                }
                CacheExeType::LazyE => {
                    let mut dq = std::mem::take(&mut self.cache_exe_lazy_e);
                    Self::ext_back_queue(g, &mut dq, &self.list_rep_ext_cache, n_next, n_end);
                    self.cache_exe_lazy_e = dq;
                }
                CacheExeType::Mem => {
                    let mut dq = std::mem::take(&mut self.cache_exe_mem);
                    Self::ext_back_queue(g, &mut dq, &self.list_rep_ext_cache, n_next, n_end);
                    self.cache_exe_mem = dq;
                }
                CacheExeType::None => {}
            }
        }
        if (n_end + 1) as usize == self.list_rep_ext_cache.len() {
            for _ in (holdval.line_start - 1)..=n_end {
                if !self.list_rep_ext_cache.is_empty() {
                    self.list_rep_ext_cache.pop();
                }
            }
        }
        self.rep_line_ext_r_cache = holdval.ext_line_ret;
    }

    fn ext_move_queue(
        g: &mut GState,
        list_cache: &mut Vec<Vec<u8>>,
        que_src: &mut VecDeque<Vec<u8>>,
    ) -> i32 {
        if list_cache.len() + que_src.len() >= SIZE_MEMLINE {
            g.add_msg_error(b"error: memory cache for repeat overflow\n");
            return 1;
        }
        while let Some(v) = que_src.pop_front() {
            list_cache.push(v);
        }
        0
    }

    fn ext_back_queue(
        g: &mut GState,
        que_dst: &mut VecDeque<Vec<u8>>,
        list_cache: &[Vec<u8>],
        n_from: i32,
        n_to: i32,
    ) {
        if que_dst.len() + (n_to - n_from + 1) as usize >= SIZE_MEMLINE {
            g.add_msg_error(b"error: memory cache for repeat overflow\n");
            return;
        }
        let mut q = std::mem::take(que_dst);
        for i in n_from..=n_to {
            if (i as usize) < list_cache.len() && i >= 0 {
                que_dst.push_back(list_cache[i as usize].clone());
            }
        }
        que_dst.append(&mut q);
    }

    // ================= lazy / mem store =================

    /// setLazyExe: returns enableExe for the current line
    pub fn set_lazy_exe(&mut self, g: &mut GState, type_lazy: LazyType, str_buf: &[u8]) -> bool {
        let mut enable_exe = true;
        if g.is_lazy_exist(LazyType::Full) {
            let mut que_s: VecDeque<Vec<u8>> = VecDeque::new();
            let mut que_a: VecDeque<Vec<u8>> = VecDeque::new();
            let mut que_e: VecDeque<Vec<u8>> = VecDeque::new();
            match type_lazy {
                LazyType::LazyS => {
                    enable_exe = self.set_lazy_exe_proc_s(g, &mut que_s, str_buf);
                }
                LazyType::LazyA => {
                    enable_exe = self.set_lazy_exe_proc_s(g, &mut que_s, str_buf);
                    g.pop_list_by_lazy(&mut que_a, LazyType::LazyA);
                }
                LazyType::LazyE => {
                    g.pop_list_by_lazy(&mut que_e, LazyType::LazyE);
                }
                LazyType::Full => {
                    enable_exe = self.set_lazy_exe_proc_s(g, &mut que_s, str_buf);
                    g.pop_list_by_lazy(&mut que_a, LazyType::LazyA);
                    g.pop_list_by_lazy(&mut que_e, LazyType::LazyE);
                }
                LazyType::None => {}
            }
            if !que_s.is_empty() {
                self.add_cache_exe_lazy(g, &mut que_s, LazyType::LazyS);
            }
            if !que_a.is_empty() {
                self.add_cache_exe_lazy(g, &mut que_a, LazyType::LazyA);
            }
            if !que_e.is_empty() {
                self.add_cache_exe_lazy(g, &mut que_e, LazyType::LazyE);
            }
        }
        enable_exe
    }

    fn set_lazy_exe_proc_s(
        &mut self,
        _g: &mut GState,
        que: &mut VecDeque<Vec<u8>>,
        str_buf: &[u8],
    ) -> bool {
        let mut enable_exe = true;
        let success = _g.pop_list_by_lazy(que, LazyType::LazyS);
        if success {
            enable_exe = false;
            if !str_buf.is_empty() {
                que.push_back(str_buf.to_vec());
            }
        }
        enable_exe
    }

    pub fn set_lazy_flush(&mut self, g: &mut GState) {
        self.set_lazy_exe(g, LazyType::Full, b"");
    }

    pub fn set_mem_call(&mut self, g: &mut GState, str_name: &[u8]) -> bool {
        let mut que: VecDeque<Vec<u8>> = VecDeque::new();
        let enable_exe = g.get_list_by_name(&mut que, str_name);
        self.add_cache_exe_mem(g, &mut que);
        enable_exe
    }

    pub fn set_lazy_store(&mut self, g: &mut GState, type_lazy: LazyType, str_buf: &[u8]) -> bool {
        g.set_lazy_store(type_lazy, str_buf)
    }
    pub fn set_mem_store(&mut self, g: &mut GState, str_name: &[u8], str_buf: &[u8]) -> bool {
        g.set_mem_store(str_name, str_buf)
    }
    pub fn set_mem_erase(&mut self, g: &mut GState, str_name: &[u8]) -> bool {
        g.mem.erase_mem_by_name(str_name)
    }
    pub fn set_mem_copy(&mut self, g: &mut GState, src: &[u8], dst: &[u8]) -> bool {
        g.mem.copy_mem_by_name(src, dst)
    }
    pub fn set_mem_move(&mut self, g: &mut GState, src: &[u8], dst: &[u8]) -> bool {
        g.mem.move_mem_by_name(src, dst)
    }
    pub fn set_mem_append(&mut self, g: &mut GState, src: &[u8], dst: &[u8]) -> bool {
        g.mem.append_mem_by_name(src, dst)
    }

    // ================= execution queues =================

    fn pop_cache_exe_lazy_mem(&mut self, g: &mut GState, str_buf: &mut Vec<u8>) -> bool {
        let type_repeat = self.get_repeat_ext_type();
        let mut flag_ext = false;
        let mut type_read = CacheExeType::None;
        if !self.cache_exe_lazy_s.is_empty() {
            flag_ext = false;
            type_read = CacheExeType::LazyS;
        } else if type_repeat == CacheExeType::LazyS {
            flag_ext = true;
            type_read = CacheExeType::LazyS;
        } else if !self.cache_exe_lazy_a.is_empty() {
            flag_ext = false;
            type_read = CacheExeType::LazyA;
        } else if type_repeat == CacheExeType::LazyA {
            flag_ext = true;
            type_read = CacheExeType::LazyA;
        } else if !self.cache_exe_lazy_e.is_empty() {
            flag_ext = false;
            type_read = CacheExeType::LazyE;
        } else if type_repeat == CacheExeType::LazyE {
            flag_ext = true;
            type_read = CacheExeType::LazyE;
        } else if !self.cache_exe_mem.is_empty() {
            flag_ext = false;
            type_read = CacheExeType::Mem;
        } else if type_repeat == CacheExeType::Mem {
            flag_ext = true;
            type_read = CacheExeType::Mem;
        }
        let mut flag_read = false;
        let mut flag_ext_read = false;
        if type_read != CacheExeType::None {
            if flag_ext {
                flag_read = self.read_repeat_ext_cache(g, str_buf);
                flag_ext_read = true;
            } else {
                let que = match type_read {
                    CacheExeType::LazyS => &mut self.cache_exe_lazy_s,
                    CacheExeType::LazyA => &mut self.cache_exe_lazy_a,
                    CacheExeType::LazyE => &mut self.cache_exe_lazy_e,
                    CacheExeType::Mem => &mut self.cache_exe_mem,
                    CacheExeType::None => &mut self.cache_exe_lazy_o,
                };
                flag_read = Self::pop_queue(str_buf, que);
            }
        }
        self.type_cache_exe = type_read;
        self.flag_cache_rep_ext = flag_ext_read;
        flag_read
    }

    fn read_repeat_ext_cache(&mut self, g: &mut GState, str_buf: &mut Vec<u8>) -> bool {
        let n_size = self.list_rep_ext_cache.len() as i32;
        if self.rep_line_ext_r_cache >= 0 && self.rep_line_ext_r_cache < n_size {
            let idx = self.rep_line_ext_r_cache as usize;
            *str_buf = self.list_rep_ext_cache[idx].clone();
            self.rep_line_ext_r_cache += 1;
        } else {
            let mut msg = b"error: not found EndRepeat at Lazy/Mem cache Line:".to_vec();
            msg.extend_from_slice(self.rep_line_ext_r_cache.to_string().as_bytes());
            msg.push(b'\n');
            g.add_msg_error(&msg);
        }
        true
    }

    fn add_cache_exe_lazy(&mut self, g: &mut GState, que: &mut VecDeque<Vec<u8>>, t: LazyType) {
        let flag_head = false;
        let type_cache = match t {
            LazyType::LazyS => CacheExeType::LazyS,
            LazyType::LazyA => CacheExeType::LazyA,
            LazyType::LazyE => CacheExeType::LazyE,
            _ => {
                g.add_msg_error(b"error: internal at JlsScriptState::addCacheExeLazy\n");
                CacheExeType::None
            }
        };
        self.add_cache_exe_common(g, que, type_cache, flag_head);
    }

    fn add_cache_exe_mem(&mut self, g: &mut GState, que: &mut VecDeque<Vec<u8>>) {
        if self.is_lazy_exe() {
            let flag_head = true;
            let t = self.type_cache_exe;
            self.add_cache_exe_common(g, que, t, flag_head);
        } else {
            let flag_head = true;
            self.add_cache_exe_common(g, que, CacheExeType::Mem, flag_head);
        }
    }

    fn add_cache_exe_common(
        &mut self,
        g: &mut GState,
        que: &mut VecDeque<Vec<u8>>,
        type_cache: CacheExeType,
        flag_head: bool,
    ) {
        let dst = match type_cache {
            CacheExeType::LazyS => &mut self.cache_exe_lazy_s,
            CacheExeType::LazyA => &mut self.cache_exe_lazy_a,
            CacheExeType::LazyE => &mut self.cache_exe_lazy_e,
            CacheExeType::Mem => &mut self.cache_exe_mem,
            CacheExeType::None => {
                g.add_msg_error(b"error: internal at JlsScriptState::addCacheExeCommon\n");
                return;
            }
        };
        Self::add_queue(dst, que, flag_head, g);
    }

    fn pop_queue(str_buf: &mut Vec<u8>, que: &mut VecDeque<Vec<u8>>) -> bool {
        if que.is_empty() {
            return false;
        }
        *str_buf = que.pop_front().unwrap();
        true
    }

    fn add_queue(
        dst: &mut VecDeque<Vec<u8>>,
        src: &mut VecDeque<Vec<u8>>,
        flag_head: bool,
        g: &mut GState,
    ) {
        if dst.len() + src.len() > SIZE_MEMLINE {
            g.add_msg_error(b"error: memory cache overflow\n");
            dst.clear();
            return;
        }
        if flag_head {
            let old = std::mem::take(dst);
            dst.append(src);
            dst.append(&mut { old });
        } else {
            dst.append(src);
        }
    }

    // ================= cache read/write =================

    pub fn read_cmd_cache(&mut self, str_buf: &mut Vec<u8>) -> bool {
        if self.list_rep_depth.is_empty() {
            return false;
        }
        if self.rep_line_read_cache as usize >= self.list_rep_cmd_cache.len() {
            self.rep_line_read_cache = -1;
        }
        if self.rep_line_read_cache < 0 {
            return false;
        }
        let idx = self.rep_line_read_cache as usize;
        *str_buf = self.list_rep_cmd_cache[idx].clone();
        self.rep_line_read_cache += 1;
        true
    }

    pub fn add_cmd_cache(&mut self, str_buf: &[u8]) -> bool {
        if self.list_rep_depth.is_empty() {
            return false;
        }
        if self.list_rep_cmd_cache.len() >= SIZE_REPLINE {
            return false;
        }
        self.list_rep_cmd_cache.push(str_buf.to_vec());
        true
    }

    pub fn read_lazy_mem_next(&mut self, g: &mut GState, str_buf: &mut Vec<u8>) -> bool {
        self.pop_cache_exe_lazy_mem(g, str_buf)
    }

    // ================= state accessors =================

    pub fn is_remain_nest(&self) -> i32 {
        let mut ret = 0;
        if !self.list_if_state.is_empty() {
            ret += 1;
        }
        if !self.list_rep_depth.is_empty() {
            ret += 2;
        }
        if self.is_lazy_area() {
            ret += 4;
        }
        ret
    }

    pub fn set_cmd_return(&mut self, flag: bool) {
        self.flag_return = flag;
    }
    pub fn is_cmd_return_exit(&self, g: &GState) -> bool {
        self.flag_return || g.is_cmd_exit()
    }

    pub fn is_flow_lazy(category: CmdCat) -> bool {
        category == CmdCat::LAZYF || category == CmdCat::MEMLAZYF
    }
    pub fn is_flow_mem(category: CmdCat) -> bool {
        category == CmdCat::MEMF || category == CmdCat::MEMLAZYF
    }

    pub fn is_need_raw(&self, category: CmdCat) -> bool {
        let mut flag_need = false;
        if self.is_lazy_area() && !Self::is_flow_lazy(category) && !self.mem_expand {
            flag_need = true;
        }
        if self.is_mem_area() && !Self::is_flow_mem(category) && !self.mem_expand {
            flag_need = true;
        }
        flag_need
    }

    pub fn is_need_full_decode(&self, cmdsel: CmdType, category: CmdCat) -> bool {
        let mut flag_need = true;
        if self.is_need_raw(category) {
            flag_need = false;
        }
        if self.if_skip {
            if category != CmdCat::COND {
                flag_need = false;
            } else {
                match cmdsel {
                    CmdType::If => flag_need = false,
                    CmdType::ElsIf | CmdType::Else => {
                        let depth = self.list_if_state.len() as i32;
                        if depth <= 1 {
                            flag_need = true;
                        } else {
                            flag_need =
                                self.list_if_state[(depth - 2) as usize] == CondIfState::Running;
                        }
                    }
                    _ => {}
                }
            }
        }
        if self.rep_skip && category != CmdCat::REP {
            flag_need = false;
        }
        flag_need
    }

    pub fn is_skip_cmd(&self) -> bool {
        self.if_skip || self.rep_skip || self.mem_skip
    }

    pub fn is_invalid_cmd_line(&self, category: CmdCat) -> bool {
        let mut flag_invalid = false;
        if self.if_skip && category != CmdCat::COND {
            flag_invalid = true;
        }
        if self.rep_skip && category != CmdCat::REP {
            flag_invalid = true;
        }
        if self.is_lazy_area() && !Self::is_flow_lazy(category) {
            flag_invalid = true;
        }
        if self.is_mem_area() && !Self::is_flow_mem(category) {
            flag_invalid = true;
        }
        flag_invalid
    }

    pub fn is_lazy_exe(&self) -> bool {
        matches!(
            self.type_cache_exe,
            CacheExeType::LazyS | CacheExeType::LazyA | CacheExeType::LazyE
        )
    }
    pub fn get_lazy_exe_type(&self) -> LazyType {
        match self.type_cache_exe {
            CacheExeType::LazyS => LazyType::LazyS,
            CacheExeType::LazyA => LazyType::LazyA,
            CacheExeType::LazyE => LazyType::LazyE,
            _ => LazyType::None,
        }
    }

    fn is_repeat_ext_type(&self) -> bool {
        self.get_repeat_ext_type() != CacheExeType::None
    }
    fn get_repeat_ext_type(&self) -> CacheExeType {
        let depth = self.list_rep_depth.len() as i32;
        if depth <= 0 {
            return CacheExeType::None;
        }
        self.list_rep_depth[(depth - 1) as usize].exe_type
    }
    fn get_cache_exe_type(&self) -> CacheExeType {
        self.type_cache_exe
    }
    pub fn is_mem_exe(&self) -> bool {
        self.type_cache_exe == CacheExeType::Mem
    }

    pub fn set_lazy_start_type(&mut self, type_lazy: LazyType) {
        self.lazy_start_type = type_lazy;
    }
    pub fn get_lazy_start_type(&self) -> LazyType {
        self.lazy_start_type
    }
    pub fn is_lazy_area(&self) -> bool {
        self.lazy_start_type != LazyType::None
    }
    pub fn set_lazy_auto(&mut self, flag: bool) {
        self.lazy_auto = flag;
    }
    pub fn is_lazy_auto(&self) -> bool {
        self.lazy_auto
    }

    pub fn set_lazy_state_ini_auto(&mut self, g: &mut GState, flag: bool) {
        g.set_lazy_state_ini_auto(flag);
    }
    pub fn is_lazy_state_ini_auto(&self, g: &GState) -> bool {
        g.is_lazy_state_ini_auto()
    }
    pub fn set_lazy_store_g(&mut self, g: &mut GState, t: LazyType, buf: &[u8]) -> bool {
        self.set_lazy_store(g, t, buf)
    }

    pub fn start_mem_area(&mut self, str_name: &[u8]) {
        self.mem_area = true;
        self.mem_name = str_name.to_vec();
        self.mem_skip = self.mem_dupe;
    }
    pub fn end_mem_area(&mut self) {
        self.mem_area = false;
        self.mem_skip = false;
    }
    pub fn is_mem_area(&self) -> bool {
        self.mem_area
    }
    pub fn get_mem_name(&self) -> Vec<u8> {
        self.mem_name.clone()
    }
    pub fn set_mem_dupe(&mut self, flag: bool) {
        self.mem_dupe = flag;
    }
    pub fn set_mem_expand(&mut self, flag: bool) {
        self.mem_expand = flag;
    }

    pub fn set_arg_area(&mut self, flag: bool) {
        self.arg_area = flag;
    }
    pub fn is_arg_area(&self) -> bool {
        self.arg_area
    }
    pub fn add_arg_name(&mut self, str_name: &[u8]) {
        if self.list_arg_name.len() < i32::MAX as usize / 4 {
            self.list_arg_name.push(str_name.to_vec());
        }
    }
    pub fn size_arg_name_list(&self) -> i32 {
        self.list_arg_name.len() as i32
    }
    pub fn get_arg_name(&self, num: i32) -> Option<Vec<u8>> {
        if num >= 0 && num < self.list_arg_name.len() as i32 {
            let name = self.list_arg_name[num as usize].clone();
            if !name.is_empty() {
                return Some(name);
            }
        }
        None
    }

    pub fn check_arg_reg_insert(&mut self, cmdsel: CmdType) -> bool {
        if self.size_arg_name_list() == 0 {
            return false;
        }
        if self.is_lazy_exe() || self.is_mem_exe() {
            return false;
        }
        let mut need_ins = false;
        match cmdsel {
            CmdType::LazyStart | CmdType::Memory => {
                self.arg_ins_ready = true;
            }
            CmdType::LocalSt => {
                if self.arg_ins_ready {
                    need_ins = true;
                    self.arg_ins_ready = false;
                }
            }
            CmdType::LocalEd | CmdType::EndLazy | CmdType::EndMemory => {
                self.arg_ins_ready = false;
            }
            _ => {}
        }
        need_ins
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}
