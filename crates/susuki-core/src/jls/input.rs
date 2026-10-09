//! Derived from join_logo_scp (https://github.com/tobitti0/join_logo_scp,
//! commit d1a24d758f8b3c04352b82ff6756fdf3d56729b3), Copyright (C) tobitti0,
//! GPL-2.0-or-later. This port is a GPL derivative work, redistributed here
//! under GPL-3.0-or-later (design §10.6).
//! Port of JlsIF.cpp readLogoframe/readScpos (input text parsers) plus the
//! design-6.4 internal entry `scpos_from_records` sharing the merge core.
//! Reference file is UTF-8 encoded; "＿" mark bytes are verbatim (JlsIF:452).

use super::cnvstr::Cnv;
use super::dataset::{Dataset, LogoRecord, ScpRecord};

const MARK_FULLWIDTH_UNDER: &[u8] = "＿".as_bytes(); // EF BC BF (UTF-8 source)

/// The internal LogoFind/merge entry has the same inclusive fall endpoint and
/// original-edge fields as readLogoframe, without serializing result text.
pub fn logos_from_records(cnv: &Cnv, ds: &mut Dataset, records: &[crate::logo_find::OutSub]) {
    for r in records {
        let ms = |frame| cnv.tc.get_msec_from_frm(frame) as i32;
        let rise = ms(r.frm_rise);
        let fall = ms(r.frm_fall);
        let rise_l = ms(r.frm_rise_l);
        let rise_r = ms(r.frm_rise_r);
        let fall_l = ms(r.frm_fall_l);
        let fall_r = ms(r.frm_fall_r);
        ds.push_logo(LogoRecord {
            rise,
            fall,
            rise_l,
            rise_r,
            fall_l,
            fall_r,
            org_rise: rise,
            org_fall: fall,
            org_rise_l: rise_l,
            org_rise_r: rise_r,
            org_fall_l: fall_l,
            org_fall_r: fall_r,
            fade_rise: i32::from(r.fade_rise),
            fade_fall: i32::from(r.fade_fall),
            intl_rise: i32::from(r.intl_rise),
            intl_fall: i32::from(r.intl_fall),
            ..Default::default()
        });
    }
}

/// readLogoframe: returns 2 on open failure; errors appended to `err`
pub fn read_logoframe(
    cnv: &Cnv,
    ds: &mut Dataset,
    text: Option<Vec<u8>>,
    fname: &[u8],
    err: &mut Vec<u8>,
) -> i32 {
    let data = match text {
        Some(d) => d,
        None => {
            let mut m = b"error: failed to open ".to_vec();
            m.extend_from_slice(fname);
            m.push(b'\n');
            err.extend_from_slice(&m);
            return 2;
        }
    };
    let lines = split_lines(&data);

    let mut set_rise = 0i32;
    let mut line_no = 0i32;
    let mut dtlogo = LogoRecord::default();
    reset_logo_if(&mut dtlogo);
    let mut str_word: Vec<u8> = Vec::new();

    for str_buf in lines {
        line_no += 1;
        if str_buf.first() == Some(&b'#') {
            continue;
        }
        let mut pos = 0i32;
        let (p, n1v) = cnv.get_str_val_msec_from_frm(&str_buf, pos);
        pos = p;
        if pos >= 0 {
            let n1 = n1v.unwrap_or(0);
            let mut fade = 0i32;
            let mut interlace = 0i32;
            let mut n2 = n1;
            let mut n3 = n1;
            // get S or E
            {
                let p2 = cnv.get_str_item(&mut str_word, &str_buf, pos);
                if p2 < 0 {
                    str_word = if set_rise == 0 {
                        b"S".to_vec()
                    } else {
                        b"E".to_vec()
                    };
                }
                pos = p2;
            }
            let flag_start = if str_word.first() == Some(&b'S') {
                1
            } else if str_word.first() == Some(&b'E') {
                0
            } else {
                -1
            };
            // get fade (C++ leaves fade unchanged on parse failure)
            let (p3, fv) = cnv.get_str_val_num(&str_buf, pos);
            if p3 >= 0 {
                fade = fv.unwrap_or(fade);
            }
            pos = p3;
            // get interlace
            {
                let mut w2: Vec<u8> = Vec::new();
                let p4 = cnv.get_str_item(&mut w2, &str_buf, pos);
                if p4 >= 0 {
                    match w2.first() {
                        Some(b'T') => interlace = 1,
                        Some(b'B') => interlace = 2,
                        _ => {}
                    }
                }
                pos = p4;
            }
            // frame-left / frame-right
            let (p5, v5) = cnv.get_str_val_msec_from_frm(&str_buf, pos);
            if p5 >= 0 {
                n2 = v5.unwrap_or(n2);
            }
            pos = p5;
            let (p6, v6) = cnv.get_str_val_msec_from_frm(&str_buf, pos);
            if p6 >= 0 {
                n3 = v6.unwrap_or(n3);
            }

            if flag_start > 0 && set_rise == 0 {
                dtlogo.rise = n1;
                dtlogo.rise_l = n2;
                dtlogo.rise_r = n3;
                dtlogo.fade_rise = fade;
                dtlogo.intl_rise = interlace;
                set_rise = 1;
            } else if flag_start == 0 && set_rise > 0 {
                dtlogo.fall = n1;
                dtlogo.fall_l = n2;
                dtlogo.fall_r = n3;
                dtlogo.fade_fall = fade;
                dtlogo.intl_fall = interlace;
                set_rise = 0;
                let rec = LogoRecord {
                    rise: dtlogo.rise,
                    fall: dtlogo.fall,
                    rise_l: dtlogo.rise_l,
                    rise_r: dtlogo.rise_r,
                    fall_l: dtlogo.fall_l,
                    fall_r: dtlogo.fall_r,
                    org_rise: dtlogo.rise,
                    org_fall: dtlogo.fall,
                    org_rise_l: dtlogo.rise_l,
                    org_rise_r: dtlogo.rise_r,
                    org_fall_l: dtlogo.fall_l,
                    org_fall_r: dtlogo.fall_r,
                    fade_rise: dtlogo.fade_rise,
                    fade_fall: dtlogo.fade_fall,
                    intl_rise: dtlogo.intl_rise,
                    intl_fall: dtlogo.intl_fall,
                    ..Default::default()
                };
                ds.push_logo(rec);
                reset_logo_if(&mut dtlogo);
            } else {
                let mut m = b"error:ignored line".to_vec();
                m.extend_from_slice(line_no.to_string().as_bytes());
                m.extend_from_slice(b":'");
                m.extend_from_slice(&str_buf);
                m.extend_from_slice(b"'\n");
                err.extend_from_slice(&m);
            }
            let _ = n1;
        }
    }
    if set_rise > 0 && !ds.empty_logo() {
        ds.pop_logo();
    }
    if ds.empty_logo() {
        let mut m = b"warning: no logo information found in '".to_vec();
        m.extend_from_slice(fname);
        m.extend_from_slice(b"'\n");
        err.extend_from_slice(&m);
    }
    0
}

/// one SCPos row parsed from any source (all values already in ms)
#[derive(Clone)]
pub struct ScpRow {
    pub msec: i32,
    pub msbk: i32,
    pub smute_s: i32,
    pub smute_w: i32,
    pub still: bool,
}

/// readScpos: returns 2 on open failure
pub fn read_scpos(
    cnv: &Cnv,
    ds: &mut Dataset,
    text: Option<Vec<u8>>,
    fname: &[u8],
    err: &mut Vec<u8>,
) -> i32 {
    let data = match text {
        Some(d) => d,
        None => {
            let mut m = b"error: failed to open ".to_vec();
            m.extend_from_slice(fname);
            m.push(b'\n');
            err.extend_from_slice(&m);
            return 2;
        }
    };
    let lines = split_lines(&data);

    // state carried across lines, exactly like the reference locals
    let mut msec_smute_s = -1i32;
    let mut msec_smute_w = -1i32;
    let mut rows: Vec<ScpRow> = Vec::new();

    for str_buf in lines {
        // 無音範囲情報
        if let Some(n) = find_sub(&str_buf, b"NAME=") {
            let (p, v) = cnv.get_str_val_num_head(&str_buf, (n + 5) as i32);
            if p >= 0 {
                let frm = v.unwrap_or(0);
                msec_smute_w = cnv.tc.get_msec_from_frm(frm as i64) as i32;
            }
        } else if let Some(n) = find_sub(&str_buf, b"CHAPTER") {
            let mut pos = (n + 7) as i32;
            while matches!(byte_at(&str_buf, pos), Some(c) if c.is_ascii_digit()) {
                pos += 1;
            }
            if byte_at(&str_buf, pos) == Some(b'=') {
                pos += 1;
                let (p, v) = cnv.get_str_val_msec_from_frm(&str_buf, pos);
                if p >= 0 {
                    msec_smute_s = v.unwrap_or(msec_smute_s);
                }
            }
        }

        // シーンチェンジ情報
        let n = match find_sub(&str_buf, b"SCPos:") {
            Some(n) => n as i32,
            None => continue,
        };
        let (p, mv) = cnv.get_str_val_msec_from_frm(&str_buf, n + 6);
        if p < 0 {
            continue;
        }
        let msec = mv.unwrap_or(0);
        let (p2, bv) = cnv.get_str_val_msec_from_frm(&str_buf, p);
        let msbk = if p2 < 0 {
            cnv.tc.get_msec_adjust_frm_from_msec(msec as i64, -1) as i32
        } else {
            bv.unwrap_or(msec)
        };
        let mut still = false;
        if let Some(n2) = find_sub(&str_buf, MARK_FULLWIDTH_UNDER) {
            if (n2 as i32) < n {
                still = true;
            }
        }
        rows.push(ScpRow {
            msec,
            msbk,
            smute_s: msec_smute_s,
            smute_w: msec_smute_w,
            still,
        });
        msec_smute_s = -1;
        msec_smute_w = -1;
    }
    merge_scpos_rows(cnv, ds, &rows);
    0
}

/// design 6.4 internal entry: rows converted from frame numbers (§5.2)
/// feed the same merge core as the text parser.
pub fn scpos_from_records(cnv: &Cnv, ds: &mut Dataset, rows: &[ScpRow]) {
    merge_scpos_rows(cnv, ds, rows);
}

/// shared merge core (read_scpos lines loop body + final fixup)
fn merge_scpos_rows(cnv: &Cnv, ds: &mut Dataset, rows: &[ScpRow]) {
    // 最初は0フレームとする
    ds.push_scp(ScpRecord::default());

    for row in rows {
        let mut dtscp = ScpRecord {
            msec: row.msec,
            msbk: row.msbk,
            still: row.still,
            ..Default::default()
        };

        // 無音範囲情報設定
        if row.smute_s >= 0 && row.smute_w >= 0 {
            dtscp.msmute_s = row.smute_s;
            dtscp.msmute_e = cnv
                .tc
                .get_msec_align_from_msec((row.smute_s + row.smute_w) as i64)
                as i32;
            let msmute_s_prev = cnv.tc.get_msec_adjust_frm_from_msec(row.smute_s as i64, -1) as i32;
            let mut j = ds.size_scp() - 1;
            while j > 0 {
                let mut dttmp = ds.scp[j as usize];
                if dttmp.msmute_e >= msmute_s_prev {
                    if dttmp.msmute_s < dtscp.msmute_s {
                        dtscp.msmute_s = dttmp.msmute_s;
                    }
                    if dttmp.msmute_e < dtscp.msmute_e {
                        dttmp.msmute_e = dtscp.msmute_e;
                        ds.set_scp(dttmp, j);
                    }
                } else {
                    j = 0;
                }
                j -= 1;
            }
        }

        // シーンチェンジ重複時の処理
        let mut flag_insert = true;
        let j = ds.size_scp() - 1;
        if j > 0 && dtscp.msbk != dtscp.msec {
            let mut dttmp = ds.scp[j as usize];
            if (dttmp.msbk <= dtscp.msbk && dttmp.msec >= dtscp.msbk)
                || (dttmp.msbk <= dtscp.msec && dttmp.msec >= dtscp.msec)
            {
                if dttmp.msbk < dtscp.msbk {
                    dttmp.msbk = dtscp.msbk;
                    dttmp.msec = dtscp.msec;
                    ds.set_scp(dttmp, j);
                }
                if dttmp.msec > dtscp.msec {
                    dttmp.msec = dtscp.msec;
                    ds.set_scp(dttmp, j);
                }
                if dttmp.msbk < dtscp.msbk {
                    dttmp.msbk = dtscp.msbk;
                    ds.set_scp(dttmp, j);
                }
                flag_insert = false;
            }
        }
        if flag_insert {
            ds.push_scp(dtscp);
        }
    }

    // 最終シーンチェンジ補正
    let jmax = ds.size_scp() - 1;
    if jmax < 0 {
        return;
    }
    let mut dttmp = ds.scp[jmax as usize];
    ds.msec_total_max = dttmp.msec;
    if jmax > 1 {
        let frm_lastcut = ds.ext_opt.frm_lastcut;
        dttmp.msec = cnv
            .tc
            .get_msec_adjust_frm_from_msec(dttmp.msec as i64, -(frm_lastcut as i64))
            as i32;
        dttmp.msbk = cnv
            .tc
            .get_msec_adjust_frm_from_msec(dttmp.msbk as i64, -(frm_lastcut as i64))
            as i32;
        if dttmp.msmute_s < 0 && dttmp.msmute_e < 0 {
            dttmp.msmute_s = dttmp.msec;
            dttmp.msmute_e = dttmp.msec;
        }
        ds.msec_total_max = dttmp.msec;
        ds.set_scp(dttmp, jmax);
        // reference loop `for(j=1; j<0; j++)` never executes
    }
}

fn reset_logo_if(l: &mut LogoRecord) {
    l.rise = 0;
    l.fall = 0;
    l.rise_l = 0;
    l.rise_r = 0;
    l.fall_l = 0;
    l.fall_r = 0;
    l.fade_rise = 0;
    l.fade_fall = 0;
    l.intl_rise = 0;
    l.intl_fall = 0;
}

fn split_lines(data: &[u8]) -> Vec<Vec<u8>> {
    let mut lines: Vec<Vec<u8>> = data.split(|&c| c == b'\n').map(|l| l.to_vec()).collect();
    if lines.last().map(|l| l.is_empty()).unwrap_or(false) {
        lines.pop();
    }
    lines
}

fn byte_at(s: &[u8], i: i32) -> Option<u8> {
    if i < 0 {
        None
    } else {
        s.get(i as usize).copied()
    }
}

fn find_sub(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    hay.windows(needle.len()).position(|w| w == needle)
}
