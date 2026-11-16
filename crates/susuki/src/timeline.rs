//! 映像と音声の PTS を解析フレーム軸に揃える。

use std::collections::VecDeque;

use susuki_core::scene::MuteDetector;
use susuki_decode::frame::AudioChunk;

type TimelineResult = Result<(), (String, u8)>;

/// フレーム格子の量子化やコンテナの時間基準による端数を許容する。
const TIMESTAMP_TOLERANCE_US: i128 = 2_000;
/// 映像の開始前に大量の音声が並ぶ異常入力でメモリを使い尽くさない。
const MAX_PENDING_SAMPLES: usize = 8_000_000;
const MAX_PENDING_CHUNKS: usize = 16_384;

pub struct Timeline {
    fps_num: i64,
    fps_den: i64,
    video_origin_us: Option<i64>,
    pending: VecDeque<AudioChunk>,
    pending_samples: usize,
    audio_cursor_ns: i128,
    audio_started: bool,
}

impl Timeline {
    pub fn new(fps_num: i64, fps_den: i64) -> Self {
        Self {
            fps_num,
            fps_den,
            video_origin_us: None,
            pending: VecDeque::new(),
            pending_samples: 0,
            audio_cursor_ns: 0,
            audio_started: false,
        }
    }

    pub fn video(
        &mut self,
        index: i64,
        pts_us: Option<i64>,
        mute: &mut MuteDetector,
    ) -> TimelineResult {
        let pts = pts_us.ok_or_else(|| ("video frame has no usable PTS".into(), 5))?;
        if let Some(origin) = self.video_origin_us {
            // JLS は一定 fps の格子を使う。VFR や欠落をその格子に載せると
            // CM の位置がずれるため、対応できるまでは結果を公開しない。
            let actual = (i128::from(pts) - i128::from(origin)) * i128::from(self.fps_num);
            let expected = i128::from(index) * i128::from(self.fps_den) * 1_000_000;
            if (actual - expected).abs() > TIMESTAMP_TOLERANCE_US * i128::from(self.fps_num) {
                return Err((
                    "video timestamps do not match the nominal frame rate".into(),
                    5,
                ));
            }
        } else {
            self.video_origin_us = Some(pts);
            // demux 順によっては映像より音声が先に復号される。基準時刻が確定して
            // から消費することで、先頭音声を消失させずに位置合わせする。
            while let Some(chunk) = self.pending.pop_front() {
                self.align_audio(chunk, mute)?;
            }
            self.pending_samples = 0;
        }
        Ok(())
    }

    /// 選択音声が映像の大部分を覆わなければ、途中の PID 交代などを見落とす。
    pub fn validate_audio_end(&self, video_frames: i64, has_audio_stream: bool) -> TimelineResult {
        if has_audio_stream {
            if !self.audio_started {
                return Err((
                    "selected audio has no samples on the video timeline".into(),
                    5,
                ));
            }
            let video_end_ns = i128::from(video_frames) * i128::from(self.fps_den) * 1_000_000_000
                / i128::from(self.fps_num);
            // AAC 等の端数・入力末尾の短い差は許容する。長い欠落は正常扱いしない。
            if video_end_ns - self.audio_cursor_ns > 1_000_000_000 {
                return Err(("selected audio ends before the video".into(), 5));
            }
        }
        Ok(())
    }

    /// 出力する秒数の原点。解析成功時には先頭映像で必ず確定する。
    pub fn video_origin_us(&self) -> Option<i64> {
        self.video_origin_us
    }

    pub fn audio(&mut self, chunk: AudioChunk, mute: &mut MuteDetector) -> TimelineResult {
        // サンプルのないフレームは時間位置やバッファを進めない。
        if chunk.peaks.is_empty() {
            return Ok(());
        }
        if self.video_origin_us.is_none() {
            if self.pending.len() >= MAX_PENDING_CHUNKS {
                return Err((
                    "too many audio frames before the first video frame".into(),
                    5,
                ));
            }
            self.pending_samples = self
                .pending_samples
                .checked_add(chunk.peaks.len())
                .ok_or_else(|| ("too much audio before the first video frame".into(), 5))?;
            if self.pending_samples > MAX_PENDING_SAMPLES {
                return Err(("too much audio before the first video frame".into(), 5));
            }
            self.pending.push_back(chunk);
            return Ok(());
        }
        self.align_audio(chunk, mute)
    }

    fn align_audio(&mut self, chunk: AudioChunk, mute: &mut MuteDetector) -> TimelineResult {
        let pts = chunk
            .pts_us
            .ok_or_else(|| ("audio frame has no usable PTS".into(), 5))?;
        let origin = self
            .video_origin_us
            .expect("video origin must already be set");
        let rate = i128::from(chunk.sample_rate);
        // 不正に大きい周波数で、時間差の穴埋めが無制限に増えるのを防ぐ。
        if rate == 0 || rate > 768_000 {
            return Err(("invalid audio sample rate".into(), 5));
        }
        let start_ns = (i128::from(pts) - i128::from(origin)) * 1_000;
        let mut delta_ns = start_ns - self.audio_cursor_ns;
        if self.audio_started && delta_ns.abs() <= TIMESTAMP_TOLERANCE_US * 1_000 {
            // 時間基準の粗いコンテナによる丸めは、サンプル数の積算で吸収する。
            delta_ns = 0;
        }
        let limit_ns = if self.audio_started {
            5_000_000_000
        } else {
            60_000_000_000
        };
        if delta_ns.abs() > limit_ns {
            return Err(("audio timestamp discontinuity is too large".into(), 5));
        }
        let offset = (delta_ns.abs() * rate + 500_000_000) / 1_000_000_000;
        let mut skip = 0usize;
        if delta_ns > 0 {
            // 音声が存在しない区間は無音ではない。高い振幅を与えて誤検出を防ぐ。
            let filler = [u16::MAX; 4096];
            let mut remaining =
                usize::try_from(offset).map_err(|_| ("audio gap overflow".into(), 5))?;
            while remaining > 0 {
                let size = remaining.min(filler.len());
                mute.push(&filler[..size], chunk.sample_rate)
                    .map_err(|e| (e.into(), 5))?;
                self.audio_cursor_ns += size as i128 * 1_000_000_000 / rate;
                remaining -= size;
            }
        } else if delta_ns < 0 {
            skip = usize::try_from(offset)
                .map_err(|_| ("audio overlap overflow".into(), 5))?
                .min(chunk.peaks.len());
        }
        let samples = &chunk.peaks[skip..];
        if !samples.is_empty() {
            mute.push(samples, chunk.sample_rate)
                .map_err(|e| (e.into(), 5))?;
            self.audio_cursor_ns += samples.len() as i128 * 1_000_000_000 / rate;
            self.audio_started = true;
        }
        Ok(())
    }
}
