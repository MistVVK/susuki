/* Tiny shim exposing FFmpeg function-macro error constants to Rust FFI. */
#include <errno.h>
#include <libavutil/error.h>

int susuki_averror_eof(void)      { return AVERROR_EOF; }
int susuki_averror_eagain(void)   { return AVERROR(EAGAIN); }
int susuki_averror_enosys(void)   { return AVERROR(ENOSYS); }
int susuki_averror_enomem(void)   { return AVERROR(ENOMEM); }
int susuki_averror_einval(void)   { return AVERROR(EINVAL); }
int susuki_averror_invaliddata(void) { return AVERROR_INVALIDDATA; }
int susuki_averror_errno(int e)   { return AVERROR(e); }
