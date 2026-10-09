//! Derived from join_logo_scp (https://github.com/tobitti0/join_logo_scp,
//! commit d1a24d758f8b3c04352b82ff6756fdf3d56729b3), Copyright (C) tobitti0,
//! GPL-2.0-or-later. This port is a GPL derivative work, redistributed here
//! under GPL-3.0-or-later (design §10.6).
//! Design 7.5 static check: v1 forbids file/external-input JLS commands.
//! Scans the entry script and everything reachable via Call (recursively,
//! cycle-safe) for the forbidden command names as the first token of a line.

/// Recursive script reader: (path, dir) -> Some((content, resolved_path)) or None.
pub type ReadCall<'a> = &'a mut dyn FnMut(&[u8], &[u8]) -> Option<(&'static [u8], Vec<u8>)>;

use super::cnvstr::Cnv;

const FORBIDDEN: [&[u8]; 7] = [
    b"FileOpen",
    b"FileAppend",
    b"FileClose",
    b"ReadData",
    b"ReadTrim",
    b"ReadString",
    b"EnvGet",
];

/// Returns Some(line) for the first forbidden token found (1-based line number
/// of the scanned file) plus the offending command name.
pub fn scan_forbidden(
    cnv: &Cnv,
    entry: &[u8],
    entry_text: &[u8],
    read_call: ReadCall<'_>,
) -> Option<(Vec<u8>, u32, &'static [u8])> {
    let mut seen: Vec<Vec<u8>> = vec![entry.to_vec()];
    let mut queue: Vec<(Vec<u8>, Vec<u8>)> = vec![(entry.to_vec(), entry_text.to_vec())];
    while let Some((path, text)) = queue.pop() {
        for (idx, line) in text.split(|&c| c == b'\n').enumerate() {
            let mut tok: Vec<u8> = Vec::new();
            cnv.get_str_item(&mut tok, line, 0);
            for f in FORBIDDEN.iter() {
                if tok.eq_ignore_ascii_case(f) {
                    return Some((path.clone(), (idx + 1) as u32, f));
                }
            }
            if tok.eq_ignore_ascii_case(b"Call") {
                let mut arg: Vec<u8> = Vec::new();
                let p = cnv.get_str_item(&mut arg, line, 0);
                if p >= 0 {
                    cnv.get_str_item(&mut arg, line, p);
                    if !arg.is_empty() {
                        if let Some((canon, body)) = read_call(&path, &arg) {
                            if !seen.iter().any(|s| s == canon) {
                                seen.push(canon.to_vec());
                                queue.push((canon.to_vec(), body));
                            }
                        }
                    }
                }
            }
        }
    }
    None
}
