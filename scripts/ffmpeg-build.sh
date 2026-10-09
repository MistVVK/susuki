#!/usr/bin/env bash
# 専用 FFmpeg ソースビルド (設計書 §10.2、REF-FFMPEG-TLV build.sh ベース + Kikyo
# build-cm-analysis.sh の CM 解析設定を基礎に §10.2 の不要機能除外を反映)。
# システム FFmpeg へのリンク分岐は設けない (§10.2 / REQ-BD-001)。
# ビルドは LGPL 構成固定 (--disable-gpl)。設計 §10.2 の GPL 構成許可は本ツールの
# 必須コーデックに GPL コンポーネントが必要な場合のみ使うため、v1 では GPL を
# 有効化しない。GPL 有効化時 (将来) は enable 部品のライセンスヘッダを v2-only
# 混入チェックする。ビルド前に config.h で GPL/NONFREE を検証し失敗させる。
# TARGET_OS=windows は Linux 上の MinGW-w64 で Windows x86_64 向け DLL を
# クロスビルドする。GPU デコードは D3D11VA のみ (NVDEC/VAAPI なし)、圧縮
# ライブラリは ffmpeg.lock の固定 commit から静的ライブラリとして組み込む。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck disable=SC1091  # ffmpeg.lock is data-only kv; -x tracking not needed
source "$ROOT/ffmpeg.lock"

BUILD_DIR="${BUILD_DIR:-$ROOT/.ffmpeg-build}"
PREFIX="${PREFIX:-$ROOT/dist/ffmpeg-$RELEASE_TAG}"
JOBS="${JOBS:-4}"
TARGET_OS="${TARGET_OS:-linux}"
CROSS=x86_64-w64-mingw32-

case "$TARGET_OS" in
    linux) tools=(git cmake make pkg-config gcc readelf) ;;
    windows) tools=(git cmake make pkg-config "${CROSS}gcc" "${CROSS}g++" "${CROSS}objdump") ;;
    *) echo "error: unknown TARGET_OS '$TARGET_OS' (linux|windows)" >&2; exit 1 ;;
esac
for tool in "${tools[@]}"; do
    command -v "$tool" >/dev/null || { echo "error: missing tool: $tool" >&2; exit 1; }
done
if [[ -e "$BUILD_DIR" ]]; then
    echo "error: build directory already exists: $BUILD_DIR" >&2
    exit 1
fi
mkdir -p "$BUILD_DIR" "$PREFIX"

fetch_pinned() { # repo tag commit destdir
    git clone --branch "$2" --depth 1 "$1" "$4"
    [[ "$(git -C "$4" rev-parse HEAD)" == "$3" ]] || {
        echo "error: $1 commit mismatch (expected $3)" >&2
        exit 1
    }
}

cmake_target=()
if [[ "$TARGET_OS" == windows ]]; then
    # 圧縮ライブラリ (静的) の置き場。配布物には入らず FFmpeg の DLL に取り込まれる
    DEPS="$BUILD_DIR/deps"
    mkdir -p "$DEPS/include" "$DEPS/lib"
    cmake_target=(
        -DCMAKE_SYSTEM_NAME=Windows
        -DCMAKE_SYSTEM_PROCESSOR=x86_64
        "-DCMAKE_C_COMPILER=${CROSS}gcc"
        "-DCMAKE_CXX_COMPILER=${CROSS}g++"
        "-DCMAKE_RC_COMPILER=${CROSS}windres"
        "-DCMAKE_FIND_ROOT_PATH=$DEPS;/usr/x86_64-w64-mingw32"
        -DCMAKE_FIND_ROOT_PATH_MODE_PROGRAM=NEVER
        -DCMAKE_FIND_ROOT_PATH_MODE_LIBRARY=ONLY
        -DCMAKE_FIND_ROOT_PATH_MODE_INCLUDE=ONLY
        -DCMAKE_FIND_ROOT_PATH_MODE_PACKAGE=ONLY
    )

    # 0) zlib / bzip2 / xz (liblzma)。Linux ではディストリビューションの開発パッケージを使う
    fetch_pinned "$ZLIB_REPO" "$ZLIB_TAG" "$ZLIB_COMMIT" "$BUILD_DIR/zlib"
    make -C "$BUILD_DIR/zlib" -f win32/Makefile.gcc PREFIX="$CROSS" -j "$JOBS" libz.a
    cp "$BUILD_DIR/zlib/zlib.h" "$BUILD_DIR/zlib/zconf.h" "$DEPS/include/"
    cp "$BUILD_DIR/zlib/libz.a" "$DEPS/lib/"

    fetch_pinned "$BZIP2_REPO" "$BZIP2_TAG" "$BZIP2_COMMIT" "$BUILD_DIR/bzip2"
    make -C "$BUILD_DIR/bzip2" -j "$JOBS" libbz2.a \
        CC="${CROSS}gcc" AR="${CROSS}ar" RANLIB="${CROSS}ranlib" \
        CFLAGS="-O2 -D_FILE_OFFSET_BITS=64"
    cp "$BUILD_DIR/bzip2/bzlib.h" "$DEPS/include/"
    cp "$BUILD_DIR/bzip2/libbz2.a" "$DEPS/lib/"

    fetch_pinned "$XZ_REPO" "$XZ_TAG" "$XZ_COMMIT" "$BUILD_DIR/xz"
    cmake -S "$BUILD_DIR/xz" -B "$BUILD_DIR/xz-build" "${cmake_target[@]}" \
        -DCMAKE_BUILD_TYPE=Release \
        -DCMAKE_INSTALL_PREFIX="$DEPS" \
        -DCMAKE_INSTALL_LIBDIR=lib \
        -DBUILD_SHARED_LIBS=OFF \
        -DBUILD_TESTING=OFF \
        -DXZ_NLS=OFF \
        -DXZ_DOC=OFF \
        -DXZ_TOOL_XZ=OFF \
        -DXZ_TOOL_XZDEC=OFF \
        -DXZ_TOOL_LZMADEC=OFF \
        -DXZ_TOOL_LZMAINFO=OFF \
        -DXZ_TOOL_SCRIPTS=OFF
    cmake --build "$BUILD_DIR/xz-build" --parallel "$JOBS"
    cmake --install "$BUILD_DIR/xz-build"

    # libaribtlv.pc の -lstdc++ は gcc ドライバでのリンクなので -static-libstdc++ が
    # 効かない。ld はディレクトリ順に探すので、先に検索される $DEPS/lib に静的版
    # だけを置き、libstdc++-6.dll への依存を出さない
    cp "$("${CROSS}g++" -print-file-name=libstdc++.a)" "$DEPS/lib/"
fi

# 1) libaribtlv (静的、FFmpeg に組み込み。MIT)
fetch_pinned "$LIBARIBTLV_REPO" "$LIBARIBTLV_TAG" "$LIBARIBTLV_COMMIT" "$BUILD_DIR/libaribtlv"
cmake -S "$BUILD_DIR/libaribtlv" -B "$BUILD_DIR/libaribtlv-build" ${cmake_target[@]+"${cmake_target[@]}"} \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_CXX_FLAGS="-fPIC" \
    -DCMAKE_POSITION_INDEPENDENT_CODE=ON \
    -DCMAKE_INSTALL_PREFIX="$PREFIX" \
    -DCMAKE_INSTALL_LIBDIR=lib \
    -DBUILD_SHARED_LIBS=OFF \
    -DBUILD_TESTING=OFF
cmake --build "$BUILD_DIR/libaribtlv-build" --parallel "$JOBS"
cmake --install "$BUILD_DIR/libaribtlv-build"

# 2) nv-codec-headers (ヘッダのみ、MIT。CUDA Toolkit 依存禁止 §10.7)。NVDEC は Linux のみ
if [[ "$TARGET_OS" == linux ]]; then
    fetch_pinned "$NVCODEC_REPO" "$NVCODEC_TAG" "$NVCODEC_COMMIT" "$BUILD_DIR/nv-codec-headers"
    make -C "$BUILD_DIR/nv-codec-headers" install PREFIX="$PREFIX"
fi

# 3) FFmpeg + REF-FFMPEG-TLV パッチ (SHA256 検証)
fetch_pinned "$FFMPEG_REPO" "$FFMPEG_TAG" "$FFMPEG_COMMIT" "$BUILD_DIR/ffmpeg"
( cd "$ROOT/vendor/patches/ffmpeg-$FFMPEG_VERSION" && sha256sum -c - <<EOF
$PATCH0001_SHA256  0001-Add-ARIB-MMT-TLV-demuxer-support-via-libaribtlv.patch
$PATCH0002_SHA256  0002-avformat-libaribtlv-report-recording-duration.patch
$PATCH0003_SHA256  0003-avformat-libaribtlv-support-timestamp-seeking.patch
EOF
)
for p in "$ROOT"/vendor/patches/ffmpeg-"$FFMPEG_VERSION"/*.patch; do
    git -C "$BUILD_DIR/ffmpeg" apply "$p"
done

# 各ライブラリは互いを自分と同じディレクトリから解決する (例: libavcodec ->
# libswresample) ため、RUNPATH はリテラルの $ORIGIN でなければならない。
# このフラグは configure の eval、make、ffbuild/library.mak の $(eval)、
# レシピの shell で順に展開されるので、'$' 4 個と引用符で全段を通す。
# shellcheck disable=SC2016  # '$' は configure/make 向けで、この shell では展開しない
rpath_origin='\$\$\$\$ORIGIN'
# shellcheck disable=SC2054  # '-Wl,....' contains an intended comma, not a separator
configure_args=(
    "--prefix=$PREFIX"
    "--extra-version=snapshot-libaribtlv-$LIBARIBTLV_VERSION"
    --enable-libaribtlv
    --pkg-config-flags=--static
    # 基盤設定 (Kikyo build-cm-analysis.sh の CM 解析設定を基礎)
    --disable-autodetect
    --disable-debug
    --disable-doc
    --disable-programs          # fftools 除外 §10.2
    --disable-network           # ローカルファイルのみ §3.4/§10.2
    --disable-devices           # avdevice 除外 §10.2
    --disable-encoders          # encoders 除外 §10.2
    --disable-muxers            # muxers 除外 §10.2
    --disable-filters           # filters 除外 §10.2 (libavfilter v1 不使用 §4.2)
    --disable-swscale           # libswscale 除外 §10.2/§6.1 (縮小は自前)
    --disable-static
    --enable-shared
    --enable-pic
    --enable-decoders
    --enable-libaribtlv
    # §4.2: Intel QSV (libvpl) は v1 不使用、AMD AMF は Linux デコード機能なしのため除外
    --enable-zlib --enable-bzlib --enable-lzma   # コンテナ圧縮
    # demuxer §10.2
    --enable-demuxer=mpegts
    --enable-demuxer=mpegtsraw
    --enable-demuxer=mxf
    --enable-demuxer=matroska
    --enable-demuxer=ogg
    --enable-demuxer=mov
    --enable-demuxer=libaribtlv
    # decoder §10.2 (映像)
    --enable-decoder=mpeg1video
    --enable-decoder=mpeg2video
    --enable-decoder=mpeg4
    --enable-decoder=h264
    --enable-decoder=hevc
    --enable-decoder=av1
    --enable-decoder=vp9
    --enable-decoder=theora
    # decoder §10.2 (音声)
    --enable-decoder=aac
    --enable-decoder=mp3
    --enable-decoder=ac3
    --enable-decoder=alac
    --enable-decoder=vorbis
    --enable-decoder=opus
    # decoder (PCM 系。mxf/ts の可逆音声。pcm_* ファミリの主要)
    --enable-decoder=pcm_s16le
    --enable-decoder=pcm_s16be
    --enable-decoder=pcm_s24le
    --enable-decoder=pcm_s24be
    --enable-decoder=pcm_s32le
    --enable-decoder=pcm_s32be
    --enable-decoder=pcm_u8
)
# hwaccel は §4.2 の入力コーデック範囲 (h264/hevc/av1/vp9)
if [[ "$TARGET_OS" == linux ]]; then
    configure_args+=(
        --enable-vaapi                           # Intel/AMD VAAPI §4.2 (libva は DFSG フリー)
        --enable-ffnvcodec                       # NVDEC は ffnvcodec ヘッダのみ §10.7
        --enable-nvdec
        --enable-hwaccel=h264_nvdec
        --enable-hwaccel=hevc_nvdec
        --enable-hwaccel=av1_nvdec
        --enable-hwaccel=vp9_nvdec
        --enable-hwaccel=h264_vaapi
        --enable-hwaccel=hevc_vaapi
        --enable-hwaccel=av1_vaapi
        --enable-hwaccel=vp9_vaapi
        "--extra-ldflags=-Wl,-rpath,'$rpath_origin'"
    )
    hwaccel_components="H264_NVDEC_HWACCEL HEVC_NVDEC_HWACCEL AV1_NVDEC_HWACCEL VP9_NVDEC_HWACCEL
        H264_VAAPI_HWACCEL HEVC_VAAPI_HWACCEL AV1_VAAPI_HWACCEL VP9_VAAPI_HWACCEL"
else
    # D3D11VA は MinGW のヘッダだけで足り、d3d11.dll は実行時に動的ロードされる。
    # hw_device_ctx 経由 (AV_PIX_FMT_D3D11) で使うのは *_d3d11va2 の方
    configure_args+=(
        --target-os=mingw32
        --arch=x86_64
        "--cross-prefix=$CROSS"
        --pkg-config=pkg-config                  # 既定の $CROSS-pkg-config は無い。.pc は PKG_CONFIG_LIBDIR で限定
        --enable-w32threads
        --enable-d3d11va
        --enable-hwaccel=h264_d3d11va2
        --enable-hwaccel=hevc_d3d11va2
        --enable-hwaccel=av1_d3d11va2
        --enable-hwaccel=vp9_d3d11va2
        # libgcc を DLL に静的に取り込み、MinGW ランタイム DLL への依存を出さない
        # (libstdc++ は下の $DEPS/lib のコピーで同様に静的になる)
        "--extra-ldflags=-L$DEPS/lib -static-libgcc"
    )
    hwaccel_components="H264_D3D11VA2_HWACCEL HEVC_D3D11VA2_HWACCEL AV1_D3D11VA2_HWACCEL VP9_D3D11VA2_HWACCEL"
fi

(
    cd "$BUILD_DIR/ffmpeg"
    if [[ "$TARGET_OS" == linux ]]; then
        export PKG_CONFIG_PATH="$PREFIX/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
        export CFLAGS="-I$PREFIX/include"
    else
        # ホストの .pc を拾わない。libaribtlv と liblzma は静的リンクなので、
        # ヘッダの dllimport を外すマクロを渡す (libaribtlv.pc の Cflags には無い)
        export PKG_CONFIG_LIBDIR="$PREFIX/lib/pkgconfig"
        unset PKG_CONFIG_PATH
        export CFLAGS="-I$PREFIX/include -I$DEPS/include -DARIBTLV_STATIC -DLZMA_API_STATIC"
    fi
    ./configure "${configure_args[@]}"
    # LGPL 構成検証 (§10.7: proprietary/GPL なしでビルド可能)
    grep -Fxq '#define CONFIG_GPL 0' config.h
    grep -Fxq '#define CONFIG_VERSION3 0' config.h
    grep -Fxq '#define CONFIG_NONFREE 0' config.h
    # ライブラリ有効検証 (libマクロは config.h)
    for lib in AVCODEC AVFORMAT AVUTIL SWRESAMPLE; do
        grep -Fxq "#define CONFIG_$lib 1" config.h || {
            echo "error: required library disabled: $lib" >&2
            exit 1
        }
    done
    # §10.2 必須コンポーネントの実在検証 (Kikyo verify-ffmpeg-component 相当)。
    # FFmpeg 8.x はコンポーネント定義を config_components.h に分割している。
    for component in \
        FILE_PROTOCOL \
        MPEGTS_DEMUXER MPEGTSRAW_DEMUXER MXF_DEMUXER LIBARIBTLV_DEMUXER \
        MATROSKA_DEMUXER OGG_DEMUXER MOV_DEMUXER \
        MPEG1VIDEO_DECODER MPEG2VIDEO_DECODER MPEG4_DECODER H264_DECODER HEVC_DECODER \
        AV1_DECODER VP9_DECODER THEORA_DECODER \
        AAC_DECODER MP3FLOAT_DECODER AC3_DECODER ALAC_DECODER VORBIS_DECODER OPUS_DECODER \
        PCM_S16LE_DECODER PCM_S16BE_DECODER PCM_S24LE_DECODER PCM_S24BE_DECODER \
        PCM_S32LE_DECODER PCM_S32BE_DECODER PCM_U8_DECODER \
        $hwaccel_components; do
        grep -Fxq "#define CONFIG_$component 1" config_components.h || {
            echo "error: required FFmpeg component disabled: $component" >&2
            exit 1
        }
    done
    make -j "$JOBS"
    make install
)

# 本ツールは ffprobe を使わない (§4.1 fftools 不使用)。ライブラリ実在のみ確認。
if [[ "$TARGET_OS" == linux ]]; then
    for lib in libavcodec libavformat libavutil libswresample; do
        compgen -G "$PREFIX/lib/$lib.so*" > /dev/null || { echo "error: missing $lib" >&2; exit 1; }
        runpath="$(readelf -d "$PREFIX/lib/$lib.so" | sed -n 's/.*(RUNPATH).*\[\(.*\)\]/\1/p')"
        [[ "$runpath" == "\$ORIGIN" ]] || { echo "error: $lib RUNPATH is '$runpath', expected \$ORIGIN" >&2; exit 1; }
    done
else
    # DLL は bin/、リンク用の import library は lib/ に入る。DLL が読み込む DLL は
    # 同梱の FFmpeg DLL か Windows のシステム DLL だけにする。システム DLL は
    # lib/zlib で始まらないので、それらの名前は MinGW ランタイムか圧縮ライブラリの
    # DLL への依存が漏れたことを示す
    for lib in avcodec avformat avutil swresample; do
        compgen -G "$PREFIX/bin/$lib-*.dll" > /dev/null || { echo "error: missing $lib DLL" >&2; exit 1; }
        [[ -f "$PREFIX/lib/lib$lib.dll.a" ]] || { echo "error: missing lib$lib.dll.a" >&2; exit 1; }
    done
    for dll in "$PREFIX"/bin/*.dll; do
        leaked="$("${CROSS}objdump" -p "$dll" | sed -n 's/^[[:space:]]*DLL Name: //p' | grep -Ei '^(lib|zlib)' || true)"
        [[ -z "$leaked" ]] || { echo "error: $(basename "$dll") imports non-system DLLs: $leaked" >&2; exit 1; }
    done
fi
echo "built FFmpeg $RELEASE_TAG (commit $FFMPEG_COMMIT + libaribtlv $LIBARIBTLV_COMMIT) in $PREFIX"
