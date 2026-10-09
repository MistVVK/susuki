# LICENSES

Susuki is distributed under **GPL-3.0-or-later**. This directory holds the
license texts and the provenance of every upstream work this tool is built
from or links against (design doc §10.4 / §10.6).

## License texts in this directory

| File | Applies to |
|------|------------|
| `GPL-3.0.txt` | Susuki itself (this repository's `LICENSE`) |
| `GPL-2.0.txt` | GPL-2.0-or-later reference implementations (see below) |
| `LGPL-2.1.txt` | FFmpeg libraries (LGPL build) |
| `join_logo_scp.LICENSE` | REF-JLS upstream license (GPLv2+, as shipped upstream) |
| `chapter_exe.LICENSE` | REF-CHAPTER upstream license (GPLv2+) |
| `logoframe.LICENSE` | REF-LOGO upstream license (GPLv2+) |
| `libaribtlv.LICENSE` | REF-ARIBTLV upstream license (MIT) |
| `ffmpeg-libaribtlv.LICENSE` | REF-FFMPEG-TLV patch set upstream license (MIT) |

## Derivation (§10.6)

Parts of `crates/susuki-core` (notably the `jls` module and the scene /
logo detectors) are strict ports of the reference implementations below.
Those works are GPL-2.0-or-later; the ported sources are derivative works
and are redistributed here as GPL-3.0-or-later, which is permitted by the
"or any later version" clause of GPLv2+.

| Upstream | Repository | Pinned commit | Role |
|----------|------------|---------------|------|
| REF-JLS | https://github.com/tobitti0/join_logo_scp | `d1a24d758f8b3c04352b82ff6756fdf3d56729b3` | composition estimation, CM judgement, JLS script language |
| REF-CHAPTER | https://github.com/rigaya/chapter_exe | `32880d45f088e574285a101e6a49b032bb04f6ea` | silence / scene-change detection |
| REF-LOGO | https://github.com/tobitti0/logoframe | `c93aec0c89838e9b4dd3a9a3d5496dfaaa41311d` | logo interval detection, logo file format |

Ported source files carry a header naming the upstream file, commit and
license. Files without such a header are original to Susuki.

The embedded script `crates/susuki/resources/JL_標準.txt` is a byte-exact
copy of `JL/JL_標準.txt` from the REF-JLS pinned commit (verified by SHA256
in `crates/susuki/build.rs`, design §7.5 / §10.3).

## Linked components

| Component | Repository | Pinned | License | Notes |
|-----------|------------|--------|---------|-------|
| FFmpeg 8.1.2 | https://github.com/FFmpeg/FFmpeg | tag `n8.1.2` (commit `38b88335f99e76ed89ff3c93f877fdefce736c13`) | LGPL-2.1-or-later | built **LGPL configuration** (`--disable-gpl`, see `scripts/ffmpeg-build.sh`); demuxers/decoders only, shared libraries |
| libaribtlv v0.2.0 | https://github.com/makeding/libaribtlv | `91bafe32479141c660a334cc03aed1f9869a18a5` | MIT | MMT/TLV demuxer dependency |
| ffmpeg-libaribtlv patches | https://github.com/makeding/ffmpeg-libaribtlv | `29cc14822fae9cb8cd06bbfdd584d06aa7a6a1fa` | MIT | `vendor/patches/ffmpeg-8.1.2/0001..0003`, SHA256 pinned in `ffmpeg.lock` |
| ffnvcodec headers | https://github.com/FFmpeg/nv-codec-headers | see `ffmpeg.lock` | MIT | NVDEC build headers only |

## Copyleft obligations / source availability

- `scripts/ffmpeg-build.sh`, `ffmpeg.lock`, `vendor/patches/` and the full
  Cargo workspace are included in this repository and in every source
  distribution, so the work can be rebuilt (§10.4).
- Binary distributions ship this `LICENSES/` directory and `NOTICE` inside
  the archive, together with `lib/` (the shared FFmpeg/libaribtlv
  libraries).
- For the LGPL-built FFmpeg shared libraries, dynamic linking is used and
  relinking is possible by replacing files under `lib/`.
