//! Looping Ren'Py `Movie()` playback for HarmonyHaven `.webm` files.
//!
//! Decode path: spawn `ffmpeg` (default `/home/johndpope/miniconda3/bin/ffmpeg`)
//! as a raw RGBA pipe, then upload each frame to a wgpu texture via
//! [`adventure_render2d::WgpuRenderer::update_texture`]. A missing or
//! unreadable movie is a hard error — callers must not log-and-skip.
//!
//! The player resolves authored names with [`resolve_visual`]:
//!
//! ```ignore
//! if let Some(path) = resolve_visual("anima1") {
//!     let movie = MoviePlayer::start_looping(renderer, &path)?;
//!     movie.push_fullscreen(batcher, width, height);
//! }
//! ```

#![deny(missing_docs)]

pub mod decoder;
pub mod present;

pub use adventure_assets::{
    is_movie_path, parse_visuals_rpy, resolve_audio, resolve_visual, AssetResolver,
};
pub use decoder::{find_ffmpeg, find_ffprobe, probe_video, MovieDecoder, MovieError, VideoInfo};
pub use present::{fullscreen_quad, MoviePlayer};

/// True when a Scene / Show path should be presented as a looping movie.
pub fn path_is_movie(path: &str) -> bool {
    let p = path.trim();
    let lower = p.to_ascii_lowercase();
    lower.ends_with(".webm") || lower.ends_with(".mp4")
}
