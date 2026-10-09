//! --help text (§3.2). stdout only, exit code 0 (§3.5/§3.6).

// The decode backends differ per platform (Unix: NVDEC/VAAPI, Windows:
// D3D11VA); everything else is shared text.
macro_rules! usage {
    ($backend:literal, $device:literal) => {
        concat!(
            r#"susuki - CM interval detection CLI

USAGE:
    susuki [OPTIONS] --logo PATH [--logo PATH ...] [--output-json PATH] [--output-chapter PATH] INPUT

INPUT:
    INPUT                        analyzed video file (local file only)

INPUT OPTIONS:
    --logo PATH                  candidate logo file (repeatable, order = logo number)
    --no-logo                    skip logo analysis (exclusive with --logo)
    --script PATH                JLS script (default: embedded standard script)
    --video-stream SPEC          v:N (0-based video ordinal) or absolute stream index
    --audio-stream SPEC          a:N, absolute index, or none (disables mute detection)
    --tlv-service N              service_id to analyze in TLV/MMT input

DECODE OPTIONS:
    --backend NAME               "#,
            $backend,
            r#" (default: auto, see design 4.3)
    --device SPEC                "#,
            $device,
            r#"
    --analysis-backend NAME      auto|cpu|gpu (analysis compute, see design 6.5)

SCENE PARAMETERS (chapter_exe derived):
    --mute-threshold N           mute amplitude threshold 1..32767 (default: 50)
    --mute-min-frames N          min consecutive mute frames (default: 10)
    --mute-search-gap N          mute scene search gap frames (default: 60)
    --mute-extend N              extend around mute interval (default: 1)

LOGO PARAMETERS (logoframe derived):
    --logo-param NAME=VALUE      repeatable; NAME one of:
                                 fadein,fadeout,mrgleft,mrgright,onwidth,onlevel,offwidth,
                                 yedge,yoffedg,ymax,ymin,ydif,ysetdif,areaset,clrrate

JUDGE PARAMETERS (join_logo_scp derived):
    --jls-set NAME=VALUE         pre-set script variable (repeatable)
    --jls-flags LIST             pre-set script flags (comma separated)
    --cut-margin-in FRAMES       start margin, 30fps frames (decimals allowed)
    --cut-margin-out FRAMES      end margin, 30fps frames (decimals allowed)
    --cut-margin-wide-in N       wide start margin (integer)
    --cut-margin-wide-out N      wide end margin (integer)
    --lastcut FRAMES             frames excluded from the tail (default: 0)

OUTPUT OPTIONS:
    --output-json PATH           JSON output (design 9.2)
    --output-chapter PATH        .chapter.txt output (design 9.3)
    --output-div PATH            split positions (design 9.4)
    --output-stdout FORMAT       json|chapter to stdout
    --overwrite                  allow overwriting existing output files

OTHER:
    --help                       show this usage (exit 0)
    --version                    show version and component versions (exit 0)

EXIT CODES: 0 ok | 2 argument error | 3 io error | 4 input unsupported |
5 decode/analysis failure | 6 script error | 130 interrupted
"#
        )
    };
}

#[cfg(unix)]
pub const USAGE: &str = usage!(
    "auto|nvdec|vaapi|cpu",
    "VAAPI: DRM render node; NVDEC: CUDA device number"
);
#[cfg(windows)]
pub const USAGE: &str = usage!("auto|d3d11va|cpu", "D3D11VA: DXGI adapter number");
