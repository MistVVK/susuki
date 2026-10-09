//! Derived from join_logo_scp (https://github.com/tobitti0/join_logo_scp,
//! commit d1a24d758f8b3c04352b82ff6756fdf3d56729b3), Copyright (C) tobitti0,
//! GPL-2.0-or-later. This port is a GPL derivative work, redistributed here
//! under GPL-3.0-or-later (design §10.6).
//! Port of JlsScrReg.{hpp,cpp} (register storage: layered local + global).
//! Reference: join_logo_scp d1a24d75... src/JlsScrReg.cpp
//! Strings are CP932 byte vectors; comparisons follow strcasecmp/strncasecmp
//! semantics from compat.h (ASCII case-insensitive on bytes).

const SIZE_VARNUM_MAX: usize = 2048; // JlsNameSpace.hpp

fn eq_ignore_case(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.eq_ignore_ascii_case(y))
}

/// strncasecmp(a, b, n) == 0
fn nicmp_eq(a: &[u8], b: &[u8], n: usize) -> bool {
    for i in 0..n {
        let ca = a.get(i).copied();
        let cb = b.get(i).copied();
        match (ca, cb) {
            (None, None) => return true, // both terminated together
            (Some(x), Some(y)) => {
                if !x.eq_ignore_ascii_case(&y) {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
}

pub struct RegFile {
    vars: Vec<Vec<u8>>, // "name:value" pairs
    msg_err: Vec<u8>,
}

impl RegFile {
    pub fn new() -> Self {
        RegFile {
            vars: Vec::new(),
            msg_err: Vec::new(),
        }
    }

    /// getRegNameVal: split at first ':'; returns n = position (i32, -1 if none).
    /// name/val are only produced when n > 0 (stale-value semantics preserved).
    fn get_reg_name_val(pair: &[u8]) -> (i32, Option<Vec<u8>>, Option<Vec<u8>>) {
        let n = match pair.iter().position(|&c| c == b':') {
            Some(p) => p as i32,
            None => -1,
        };
        if n > 0 {
            let name = pair[..n as usize].to_vec();
            let val = if (n as usize) + 1 < pair.len() {
                pair[(n as usize + 1)..].to_vec()
            } else {
                Vec::new()
            };
            (n, Some(name), Some(val))
        } else {
            (n, None, None)
        }
    }

    pub fn set_reg_var(&mut self, name: &[u8], val: &[u8], overwrite: bool) -> bool {
        let nlenvar = name.len() as i32;
        let mut nloc: i32 = -1;
        let nmaxlist = self.vars.len();
        for i in 0..nmaxlist {
            let (n, str_org_name, _) = Self::get_reg_name_val(&self.vars[i]);
            if nlenvar == n {
                if let Some(org) = str_org_name {
                    if eq_ignore_case(name, &org) {
                        nloc = i as i32;
                    }
                }
            }
        }
        let mut str_pair = name.to_vec();
        str_pair.push(b':');
        str_pair.extend_from_slice(val);
        if nloc >= 0 {
            if overwrite {
                self.vars[nloc as usize] = str_pair;
            }
        } else if nmaxlist < SIZE_VARNUM_MAX {
            self.vars.push(str_pair);
        } else {
            return false;
        }
        true
    }

    /// Returns (nmatch, Some(val)) on hit; (nmatch=0, None) otherwise.
    /// val is only produced on match (caller keeps old buffer otherwise).
    pub fn get_reg_var(&mut self, cand: &[u8], exact: bool) -> (i32, Option<Vec<u8>>) {
        let nlencand = cand.len() as i32;
        let mut nmatch: i32 = 0;
        let mut nloc: i32 = -1;
        let nmaxlist = self.vars.len();
        for i in 0..nmaxlist {
            let (n, str_org_name, _) = Self::get_reg_name_val(&self.vars[i]);
            if nmatch < n {
                let hit = match &str_org_name {
                    Some(org) => nicmp_eq(cand, org, n as usize) && (n == nlencand || !exact),
                    None => false,
                };
                if hit {
                    nloc = i as i32;
                    nmatch = n;
                }
            }
        }
        if nloc >= 0 {
            let (n, str_org_name, val) = Self::get_reg_name_val(&self.vars[nloc as usize]);
            if let (Some(org), Some(v)) = (str_org_name, val) {
                let prefix: Vec<u8> = cand[..n as usize].to_vec();
                if org != prefix {
                    self.msg_err
                        .extend_from_slice(b"warning : mismatch capital letter of register name(");
                    self.msg_err.extend_from_slice(&prefix);
                    self.msg_err.push(b' ');
                    self.msg_err.extend_from_slice(&org);
                    self.msg_err.extend_from_slice(b")\n");
                }
                return (nmatch, Some(v));
            }
        }
        (nmatch, None)
    }

    pub fn pop_msg_error(&mut self) -> Option<Vec<u8>> {
        if self.msg_err.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.msg_err))
        }
    }
}

impl Default for RegFile {
    fn default() -> Self {
        Self::new()
    }
}

struct RegLayer {
    base: bool, // stop searching upper layers
    regfile: RegFile,
}

pub struct ScrReg {
    layers: Vec<RegLayer>,  // layered local variables
    global_reg: RegFile,    // global variables
    list_arg: Vec<Vec<u8>>, // Call arguments (name,val pairs flat)
    only_local: bool,       // hide global on read
    msg_err: Vec<u8>,
}

impl ScrReg {
    pub fn new() -> Self {
        ScrReg {
            layers: Vec::new(),
            global_reg: RegFile::new(),
            list_arg: Vec::new(),
            only_local: false,
            msg_err: Vec::new(),
        }
    }

    /// Returns created layer count (0 on failure).
    pub fn create_local(&mut self, flag_base: bool) -> i32 {
        self.layers.push(RegLayer {
            base: flag_base,
            regfile: RegFile::new(),
        });
        if flag_base {
            self.set_reg_from_arg();
        } else {
            self.list_arg.clear(); // clearArgReg
        }
        self.layers.len() as i32
    }

    /// releaseLocal quirk preserved: numLayer is first set to the last index
    /// (so a refused release of a base layer "succeeds" returning that index
    /// without popping), and only becomes size+1 when actually released.
    pub fn release_local(&mut self, flag_base: bool) -> i32 {
        let mut num_layer = 0i32;
        if !self.layers.is_empty() {
            num_layer = self.layers.len() as i32 - 1;
            let last = num_layer as usize;
            if flag_base || !self.layers[last].base {
                self.layers.pop();
                self.list_arg.clear();
                num_layer = (self.layers.len() + 1) as i32;
            }
        }
        if num_layer == 0 {
            self.msg_err
                .extend_from_slice(b"error:too many release local-register layer\n");
            return 0;
        }
        num_layer
    }

    pub fn set_local_reg_var(&mut self, name: &[u8], val: &[u8], overwrite: bool) -> bool {
        if self.layers.is_empty() {
            self.msg_err
                .extend_from_slice(b"error:internal setting(empty local-register layer)\n");
            return false;
        }
        if self.check_err_reg_name(name) {
            return false;
        }
        let last = self.layers.len() - 1;
        self.layers[last].regfile.set_reg_var(name, val, overwrite)
    }

    /// set into existing local layer if any, else into global.
    pub fn set_reg_var(&mut self, name: &[u8], val: &[u8], overwrite: bool) -> bool {
        if self.check_err_reg_name(name) {
            return false;
        }
        let (num_layer, _) = self.get_layer_reg_var(name, true);
        if num_layer >= 0 {
            self.layers[num_layer as usize]
                .regfile
                .set_reg_var(name, val, overwrite)
        } else {
            self.global_reg.set_reg_var(name, val, overwrite)
        }
    }

    /// Returns (nmatch, Some(val)) on hit.
    pub fn get_reg_var(&mut self, cand: &[u8], exact: bool) -> (i32, Option<Vec<u8>>) {
        let (num_layer, found) = self.get_layer_reg_var(cand, exact);
        if num_layer < 0 && !self.only_local {
            let (nmatch, val) = self.global_reg.get_reg_var(cand, exact);
            if nmatch > 0 {
                self.pop_err_lower_global();
            }
            (nmatch, val)
        } else {
            (if num_layer >= 0 { found.1 } else { 0 }, found.0)
        }
    }

    /// local-layer lookup: returns (numLayer, (Option<val>, nmatch))
    fn get_layer_reg_var(&mut self, cand: &[u8], exact: bool) -> (i32, (Option<Vec<u8>>, i32)) {
        let mut num_layer: i32 = -1;
        let mut result = (None, 0i32);
        if !self.layers.is_empty() {
            let mut n = self.layers.len() as i32;
            let mut scope = true;
            while scope && n > 0 {
                n -= 1;
                let (nmatch, str_val) = self.layers[n as usize].regfile.get_reg_var(cand, exact);
                if nmatch > 0 {
                    num_layer = n;
                    result = (str_val, nmatch);
                    scope = false;
                    let msg = std::mem::take(&mut self.layers[n as usize].regfile.msg_err);
                    if !msg.is_empty() {
                        self.msg_err.extend_from_slice(&msg);
                    }
                } else if self.layers[n as usize].base {
                    scope = false;
                }
            }
        }
        (num_layer, result)
    }

    pub fn set_arg_reg(&mut self, name: &[u8], val: &[u8]) -> bool {
        self.list_arg.push(name.to_vec());
        self.list_arg.push(val.to_vec());
        true
    }

    pub fn set_local_only(&mut self, flag: bool) {
        self.only_local = flag;
    }

    pub fn pop_msg_error(&mut self) -> Option<Vec<u8>> {
        if self.msg_err.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.msg_err))
        }
    }

    fn set_reg_from_arg(&mut self) {
        let size_list = self.list_arg.len();
        if size_list > 0 {
            let overwrite = true;
            let args = std::mem::take(&mut self.list_arg);
            let mut i = 0;
            while i + 1 < size_list {
                let _ = self.set_local_reg_var(&args[i], &args[i + 1], overwrite);
                i += 2;
            }
        }
    }

    /// checkErrRegName: returns true when invalid.
    fn check_err_reg_name(&mut self, name: &[u8]) -> bool {
        const CHECK_FULL: &[u8] = b"!#$%&'()*+,-./:;<=>?";
        let mut check_first = CHECK_FULL.to_vec();
        check_first.extend_from_slice(b"0123456789");
        let first = if name.is_empty() { None } else { Some(name[0]) };
        if let Some(c) = first {
            if check_first.contains(&c) {
                self.msg_err
                    .extend_from_slice(b"error: register setting, invalid first char(");
                self.msg_err.extend_from_slice(name);
                self.msg_err.extend_from_slice(b")\n");
                return true;
            }
        }
        for &c in CHECK_FULL {
            if name.contains(&c) {
                self.msg_err
                    .extend_from_slice(b"error: register setting, bad char exist(");
                self.msg_err.extend_from_slice(name);
                self.msg_err.extend_from_slice(b")\n");
                return true;
            }
        }
        false
    }

    fn pop_err_lower_global(&mut self) {
        let msg = self.global_reg.pop_msg_error();
        if let Some(m) = msg {
            self.msg_err.extend_from_slice(&m);
        }
    }
}

impl Default for ScrReg {
    fn default() -> Self {
        Self::new()
    }
}
