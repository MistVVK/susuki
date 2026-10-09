//! Build glue for susuki-decode.
//! Design 10.2 / REQ-BD-001: FFmpeg must come from the dedicated self-built
//! dist pointed to by SUSUKI_FFMPEG_PREFIX. There is deliberately no search
//! of, or fallback to, system FFmpeg (design 10.2 forbids such a branch).

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=SUSUKI_FFMPEG_PREFIX");
    println!("cargo:rerun-if-changed=build.rs");
    let prefix = std::env::var("SUSUKI_FFMPEG_PREFIX").expect(
        "SUSUKI_FFMPEG_PREFIX must point to the dedicated FFmpeg dist built by scripts/ffmpeg-build.sh (design 10.2; no system FFmpeg fallback exists)",
    );
    let include = PathBuf::from(&prefix).join("include");
    let libdir = PathBuf::from(&prefix).join("lib");
    assert!(
        include.join("libavformat/avformat.h").exists(),
        "dedicated FFmpeg headers not found under {}",
        include.display()
    );
    println!("cargo:rustc-link-search=native={}", libdir.display());
    for l in ["avformat", "avcodec", "avutil", "swresample"] {
        println!("cargo:rustc-link-lib=dylib={l}");
    }

    let wrapper = r#"
#include <libavformat/avformat.h>
#include <libavcodec/avcodec.h>
#include <libavutil/hwcontext.h>
#include <libavutil/imgutils.h>
#include <libavutil/frame.h>
#include <libavutil/dict.h>
#include <libavutil/error.h>
#include <libavutil/channel_layout.h>
#include <libavutil/rational.h>
#include <libavutil/pixdesc.h>
#include <libavutil/avutil.h>
#include <libavutil/opt.h>
#include <libswresample/swresample.h>
"#
    .to_string();
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("bindings.rs");
    let builder = bindgen::Builder::default()
        .header_contents("susuki-ffmpeg-wrapper.h", &wrapper)
        .clang_arg(format!("-I{}", include.display()))
        .allowlist_type("AV.*")
        .allowlist_type("Swr.*")
        .allowlist_function("av.*")
        .allowlist_function("swr.*")
        .allowlist_var("AV.*")
        .allowlist_var("SWR.*")
        .allowlist_var("FF.*")
        .derive_debug(true)
        .layout_tests(false);
    let bindings = builder
        .generate()
        .expect("bindgen failed against the dedicated FFmpeg headers");
    bindings
        .write_to_file(&out)
        .expect("failed to write bindings");

    // FFERRTAG-style macro error codes are function-macro expressions that bindgen
    // does not emit; export them through a tiny C shim instead.
    cc::Build::new()
        .file("c/averror_shim.c")
        .include(&include)
        .compile("susuki_ffmpeg_shim");
}
