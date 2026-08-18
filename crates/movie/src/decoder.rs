//! ffmpeg-pipe decoder: `.webm` → tightly packed RGBA frames.
//!
//! Uses the system `ffmpeg` / `ffprobe` binaries (default:
//! `/home/johndpope/miniconda3/bin/ffmpeg`). Silent skip is not a
//! success path — [`MovieDecoder::open`] returns [`MovieError`] if the
//! file cannot be probed or the decode process fails to start.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Decode target: 1080p RGBA is 8 MiB/frame; the game window is 1280×720.
const MAX_DECODE_W: u32 = 1280;
const MAX_DECODE_H: u32 = 720;

/// Default ffmpeg used by HarmonyHaven / this machine.
pub const DEFAULT_FFMPEG: &str = "/home/johndpope/miniconda3/bin/ffmpeg";

/// Failed to probe or decode a movie.
#[derive(Debug, thiserror::Error)]
pub enum MovieError {
    /// File is missing.
    #[error("movie not found: {0}")]
    NotFound(String),
    /// ffprobe / ffmpeg failed to describe the stream.
    #[error("probe {path}: {detail}")]
    Probe {
        /// Movie path.
        path: String,
        /// ffprobe stderr / parse error.
        detail: String,
    },
    /// ffmpeg process failed to spawn or exited early.
    #[error("decode {path}: {detail}")]
    Decode {
        /// Movie path.
        path: String,
        /// Spawn / IO error.
        detail: String,
    },
}

/// Probed stream info used to size the RGBA pipe.
#[derive(Clone, Debug, PartialEq)]
pub struct VideoInfo {
    /// Pixel width.
    pub width: u32,
    /// Pixel height.
    pub height: u32,
    /// Frames per second (used to pace [`MovieDecoder::poll_frame`]).
    pub fps: f64,
}

impl VideoInfo {
    /// Tightly packed RGBA byte count for one frame.
    pub fn frame_bytes(&self) -> usize {
        (self.width as usize)
            .saturating_mul(self.height as usize)
            .saturating_mul(4)
    }
}

/// Locate `ffmpeg`. `FFMPEG` env, then [`DEFAULT_FFMPEG`], then `PATH`.
pub fn find_ffmpeg() -> PathBuf {
    if let Some(p) = std::env::var_os("FFMPEG") {
        return PathBuf::from(p);
    }
    let default = PathBuf::from(DEFAULT_FFMPEG);
    if default.is_file() {
        return default;
    }
    PathBuf::from("ffmpeg")
}

/// Locate `ffprobe` next to [`find_ffmpeg`], else `PATH`.
pub fn find_ffprobe() -> PathBuf {
    if let Some(p) = std::env::var_os("FFPROBE") {
        return PathBuf::from(p);
    }
    let ffmpeg = find_ffmpeg();
    if let Some(dir) = ffmpeg.parent() {
        let probe = dir.join("ffprobe");
        if probe.is_file() {
            return probe;
        }
    }
    PathBuf::from("ffprobe")
}

/// Read width / height / fps via ffprobe (falls back to `ffmpeg -i`).
pub fn probe_video(path: &Path) -> Result<VideoInfo, MovieError> {
    if !path.is_file() {
        return Err(MovieError::NotFound(path.display().to_string()));
    }
    let display = path.display().to_string();
    let out = Command::new(find_ffprobe())
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height,avg_frame_rate",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
        .map_err(|e| MovieError::Probe {
            path: display.clone(),
            detail: e.to_string(),
        })?;
    if out.status.success() {
        if let Some(info) = parse_ffprobe_csv(&String::from_utf8_lossy(&out.stdout)) {
            return Ok(info);
        }
    }
    // ffmpeg -i prints stream info on stderr.
    let probe = Command::new(find_ffmpeg())
        .args(["-hide_banner", "-i"])
        .arg(path)
        .output()
        .map_err(|e| MovieError::Probe {
            path: display.clone(),
            detail: e.to_string(),
        })?;
    parse_ffmpeg_i_stderr(&String::from_utf8_lossy(&probe.stderr)).ok_or(MovieError::Probe {
        path: display,
        detail: format!(
            "ffprobe: {} ffmpeg: {}",
            String::from_utf8_lossy(&out.stderr).trim(),
            String::from_utf8_lossy(&probe.stderr).trim()
        ),
    })
}

fn parse_ffprobe_csv(s: &str) -> Option<VideoInfo> {
    let line = s.lines().next()?.trim();
    if line.is_empty() {
        return None;
    }
    let mut parts = line.split(',');
    let width: u32 = parts.next()?.trim().parse().ok()?;
    let height: u32 = parts.next()?.trim().parse().ok()?;
    let fps = parse_rate(parts.next().unwrap_or("30/1")).unwrap_or(30.0);
    if width == 0 || height == 0 {
        return None;
    }
    Some(VideoInfo { width, height, fps })
}

fn parse_rate(s: &str) -> Option<f64> {
    let s = s.trim();
    if let Some((n, d)) = s.split_once('/') {
        let n: f64 = n.parse().ok()?;
        let d: f64 = d.parse().ok()?;
        if d == 0.0 {
            return None;
        }
        Some(n / d)
    } else {
        s.parse().ok()
    }
}

fn parse_ffmpeg_i_stderr(stderr: &str) -> Option<VideoInfo> {
    // Stream #0:0: Video: vp9, yuv420p, 64x48, 10 fps
    for line in stderr.lines() {
        if !line.contains("Video:") {
            continue;
        }
        let mut width = 0u32;
        let mut height = 0u32;
        let mut fps = 30.0;
        for token in line.split([',', ' ']) {
            let token = token.trim();
            if let Some((w, h)) = token.split_once('x') {
                if let (Ok(ww), Ok(hh)) = (w.parse::<u32>(), h.parse::<u32>()) {
                    if ww > 0 && hh > 0 {
                        width = ww;
                        height = hh;
                    }
                }
            }
            if token.eq_ignore_ascii_case("fps") || token.eq_ignore_ascii_case("tbr") {
                // previous numeric already consumed; scan backwards via words
            }
        }
        if let Some(idx) = line.find(" fps") {
            let head = &line[..idx];
            if let Some(num) = head.split_whitespace().last() {
                if let Ok(v) = num.parse::<f64>() {
                    fps = v;
                }
            }
        }
        if width > 0 && height > 0 {
            return Some(VideoInfo { width, height, fps });
        }
    }
    None
}

struct FrameSlot {
    latest: Mutex<Option<Vec<u8>>>,
    err: Mutex<Option<String>>,
    stop: AtomicBool,
}

/// Streaming RGBA decoder. ffmpeg runs on a worker thread so the wgpu
/// frame loop never blocks on 1080p VP9.
pub struct MovieDecoder {
    path: PathBuf,
    info: VideoInfo,
    loop_: bool,
    slot: Arc<FrameSlot>,
    join: Option<JoinHandle<()>>,
    buf: Vec<u8>,
    next_frame_at: Instant,
    frame_dt: Duration,
}

impl std::fmt::Debug for MovieDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MovieDecoder")
            .field("path", &self.path)
            .field("info", &self.info)
            .field("loop_", &self.loop_)
            .finish()
    }
}

impl MovieDecoder {
    /// Open `path` and start decoding. `loop_` matches Ren'Py `Movie()`.
    pub fn open(path: impl AsRef<Path>, loop_: bool) -> Result<Self, MovieError> {
        let path = path.as_ref().to_path_buf();
        let mut info = probe_video(&path)?;
        if info.width > MAX_DECODE_W || info.height > MAX_DECODE_H {
            info.width = MAX_DECODE_W;
            info.height = MAX_DECODE_H;
        }
        let fps = if !info.fps.is_finite() || info.fps < 1.0 {
            24.0
        } else if info.fps > 60.0 {
            30.0
        } else {
            info.fps
        };
        info.fps = fps;
        let frame_dt = Duration::from_secs_f64(1.0 / fps);
        let slot = Arc::new(FrameSlot {
            latest: Mutex::new(None),
            err: Mutex::new(None),
            stop: AtomicBool::new(false),
        });
        let ffmpeg = find_ffmpeg();
        let join = {
            let path_t = path.clone();
            let slot_t = Arc::clone(&slot);
            let info_t = info.clone();
            thread::Builder::new()
                .name("movie-ffmpeg".into())
                .spawn(move || decode_loop(ffmpeg, path_t, info_t, loop_, slot_t))
                .map_err(|e| MovieError::Decode {
                    path: path.display().to_string(),
                    detail: format!("spawn worker: {e}"),
                })?
        };
        let mut dec = Self {
            path,
            info,
            loop_,
            slot,
            join: Some(join),
            buf: Vec::new(),
            next_frame_at: Instant::now(),
            frame_dt,
        };
        dec.wait_first_frame(Duration::from_secs(8))?;
        Ok(dec)
    }

    fn wait_first_frame(&mut self, timeout: Duration) -> Result<(), MovieError> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(err) = self.slot.err.lock().unwrap().clone() {
                return Err(MovieError::Decode {
                    path: self.path.display().to_string(),
                    detail: err,
                });
            }
            if self.slot.latest.lock().unwrap().is_some() {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(MovieError::Decode {
                    path: self.path.display().to_string(),
                    detail: "timed out waiting for first ffmpeg frame".into(),
                });
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    /// Looping open (Ren'Py `Movie()` default).
    pub fn open_looping(path: impl AsRef<Path>) -> Result<Self, MovieError> {
        Self::open(path, true)
    }

    /// Source file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Stream info (after optional downscale).
    pub fn info(&self) -> VideoInfo {
        self.info.clone()
    }

    /// Pixel width.
    pub fn width(&self) -> u32 {
        self.info.width
    }

    /// Pixel height.
    pub fn height(&self) -> u32 {
        self.info.height
    }

    /// Whether this decoder restarts at EOF.
    pub fn loops(&self) -> bool {
        self.loop_
    }

    /// Latest decoded frame (RGBA8). Waits briefly so callers can pull
    /// sequential frames; the render loop should use [`Self::poll_frame`].
    pub fn read_frame(&mut self) -> Result<Option<&[u8]>, MovieError> {
        let deadline = Instant::now() + Duration::from_millis(400);
        loop {
            if let Some(err) = self.slot.err.lock().unwrap().clone() {
                return Err(MovieError::Decode {
                    path: self.path.display().to_string(),
                    detail: err,
                });
            }
            if let Some(bytes) = self.slot.latest.lock().unwrap().take() {
                self.buf = bytes;
                return Ok(Some(&self.buf));
            }
            if Instant::now() >= deadline {
                return Ok(None);
            }
            thread::sleep(Duration::from_millis(5));
        }
    }

    /// Newest frame, paced to the clip fps so the window refresh rate
    /// cannot run the movie fast-forward.
    pub fn poll_frame(&mut self, now: Instant) -> Result<Option<&[u8]>, MovieError> {
        if now < self.next_frame_at {
            return Ok(None);
        }
        if let Some(err) = self.slot.err.lock().unwrap().clone() {
            return Err(MovieError::Decode {
                path: self.path.display().to_string(),
                detail: err,
            });
        }
        if let Some(bytes) = self.slot.latest.lock().unwrap().take() {
            self.buf = bytes;
            self.next_frame_at = now + self.frame_dt;
            Ok(Some(&self.buf))
        } else {
            Ok(None)
        }
    }
}

impl Drop for MovieDecoder {
    fn drop(&mut self) {
        self.slot.stop.store(true, Ordering::Relaxed);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn decode_loop(ffmpeg: PathBuf, path: PathBuf, info: VideoInfo, loop_: bool, slot: Arc<FrameSlot>) {
    loop {
        if slot.stop.load(Ordering::Relaxed) {
            return;
        }
        match spawn_ffmpeg(&ffmpeg, &path, &info, loop_) {
            Ok(mut child) => {
                let Some(mut stdout) = child.stdout.take() else {
                    *slot.err.lock().unwrap() = Some("ffmpeg stdout not piped".into());
                    let _ = child.kill();
                    return;
                };
                let n = info.frame_bytes();
                let mut buf = vec![0u8; n];
                let frame_dt = {
                    let fps = if info.fps.is_finite() && info.fps >= 1.0 {
                        info.fps
                    } else {
                        24.0
                    };
                    Duration::from_secs_f64(1.0 / fps)
                };
                let mut due = Instant::now();
                loop {
                    if slot.stop.load(Ordering::Relaxed) {
                        let _ = child.kill();
                        let _ = child.wait();
                        return;
                    }
                    match stdout.read_exact(&mut buf) {
                        Ok(()) => {
                            *slot.latest.lock().unwrap() = Some(buf.clone());
                            due += frame_dt;
                            let now = Instant::now();
                            if due > now {
                                thread::sleep(due - now);
                            } else {
                                // decode fell behind — don't accumulate debt
                                due = now + frame_dt;
                            }
                        }
                        Err(_) => break,
                    }
                }
                let _ = child.kill();
                let _ = child.wait();
                if !loop_ {
                    return;
                }
            }
            Err(e) => {
                *slot.err.lock().unwrap() = Some(e);
                return;
            }
        }
    }
}

fn spawn_ffmpeg(
    ffmpeg: &Path,
    path: &Path,
    info: &VideoInfo,
    loop_: bool,
) -> Result<Child, String> {
    let mut cmd = Command::new(ffmpeg);
    cmd.arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-nostdin");
    if loop_ {
        cmd.arg("-stream_loop").arg("-1");
    }
    cmd.arg("-i").arg(path).args([
        "-map",
        "0:v:0",
        "-vf",
        &format!("scale={}:{}:flags=fast_bilinear", info.width, info.height),
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgba",
        "-an",
        "pipe:1",
    ]);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd.spawn()
        .map_err(|e| format!("spawn {}: {e}", ffmpeg.display()))
}

/// Encode a tiny solid-color VP8 webm (tests).
#[cfg(test)]
pub fn encode_color_webm(
    path: &Path,
    width: u32,
    height: u32,
    frames: u32,
    fps: u32,
) -> Result<(), MovieError> {
    let dur = frames as f64 / fps.max(1) as f64;
    let status = Command::new(find_ffmpeg())
        .args([
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            &format!("color=c=red:s={width}x{height}:d={dur}:r={fps}"),
            "-c:v",
            "libvpx",
            "-auto-alt-ref",
            "0",
            "-b:v",
            "200k",
        ])
        .arg(path)
        .status()
        .map_err(|e| MovieError::Decode {
            path: path.display().to_string(),
            detail: e.to_string(),
        })?;
    if !status.success() {
        return Err(MovieError::Decode {
            path: path.display().to_string(),
            detail: format!("ffmpeg encode exited {status}"),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_probe_csv() {
        let info = parse_ffprobe_csv("64,48,10/1\n").unwrap();
        assert_eq!(info.width, 64);
        assert_eq!(info.height, 48);
        assert!((info.fps - 10.0).abs() < 1e-6);
        assert_eq!(info.frame_bytes(), 64 * 48 * 4);
    }

    #[test]
    fn open_missing_is_error() {
        let err = MovieDecoder::open("/no/such/movie.webm", true).unwrap_err();
        assert!(matches!(err, MovieError::NotFound(_)));
    }

    #[test]
    fn decodes_generated_webm_frames() {
        let dir = std::env::temp_dir().join(format!(
            "ae-movie-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("red.webm");
        encode_color_webm(&path, 64, 48, 6, 10).expect("encode test webm");
        let info = probe_video(&path).expect("probe");
        assert_eq!(info.width, 64);
        assert_eq!(info.height, 48);

        let mut dec = MovieDecoder::open(&path, true).expect("open decoder");
        let mut saw_red = false;
        for _ in 0..4 {
            let frame = dec.read_frame().expect("read").expect("frame");
            assert_eq!(frame.len(), 64 * 48 * 4);
            // VP8 is lossy — look at a mid-pixel for a red-dominant sample.
            let i = ((24 * 64 + 32) * 4) as usize;
            if frame[i] > 180 && frame[i + 1] < 80 && frame[i + 2] < 80 {
                saw_red = true;
            }
        }
        assert!(saw_red, "expected a red-dominant pixel from the test movie");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
