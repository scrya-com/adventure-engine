//! Ren'Py image/audio name → filesystem path.
//!
//! HarmonyHaven (and other extracted Ren'Py games) author `scene bgn1` /
//! `play music spark` / `image anima1 = Movie(...)`. The player has to
//! turn those names into files under `images/`, `images/animations/`, and
//! `audio/`. Dest (cooked / installed copy) is probed first, then the
//! extract tree.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Default HarmonyHaven extract used when `HARMONYHAVEN_EXTRACT` is unset.
pub const DEFAULT_EXTRACT: &str = "/home/johndpope/Desktop/HarmonyHaven-extracted";

/// Image / movie extensions probed for a bare visual name, in order.
pub const VISUAL_EXTS: &[&str] = &["jpg", "png", "webp", "webm"];

/// Audio extensions probed for a bare audio name, in order.
pub const AUDIO_EXTS: &[&str] = &["mp3", "wav", "ogg", "flac", "opus"];

/// Dest + extract roots plus the `visuals.rpy` Movie() map.
#[derive(Clone, Debug)]
pub struct AssetResolver {
    dest: PathBuf,
    extract: PathBuf,
    /// `anima1` → `images/animations/anima1.webm` (relative).
    movies: BTreeMap<String, PathBuf>,
}

impl AssetResolver {
    /// Build a resolver. Loads `scripts/visuals.rpy` from dest, then extract
    /// (dest entries win on name collision).
    pub fn new(dest: impl Into<PathBuf>, extract: impl Into<PathBuf>) -> Self {
        let dest = dest.into();
        let extract = extract.into();
        let mut movies = BTreeMap::new();
        for root in [&extract, &dest] {
            let rpy = root.join("scripts/visuals.rpy");
            if let Ok(src) = std::fs::read_to_string(&rpy) {
                for (name, rel) in parse_visuals_rpy(&src) {
                    movies.insert(name, rel);
                }
            }
        }
        Self {
            dest,
            extract,
            movies,
        }
    }

    /// Dest = `HARMONYHAVEN_DEST` or cwd; extract = `HARMONYHAVEN_EXTRACT`
    /// or [`DEFAULT_EXTRACT`].
    pub fn from_env() -> Self {
        let dest = std::env::var_os("HARMONYHAVEN_DEST")
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let extract = std::env::var_os("HARMONYHAVEN_EXTRACT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_EXTRACT));
        Self::new(dest, extract)
    }

    /// Dest root (probed first).
    pub fn dest(&self) -> &Path {
        &self.dest
    }

    /// Extract root (probed second).
    pub fn extract(&self) -> &Path {
        &self.extract
    }

    /// Movie() declarations parsed from `visuals.rpy`.
    pub fn movies(&self) -> &BTreeMap<String, PathBuf> {
        &self.movies
    }

    /// Insert or override a Movie() mapping (tests / extra scripts).
    pub fn insert_movie(&mut self, name: impl Into<String>, rel: impl Into<PathBuf>) {
        self.movies.insert(name.into(), rel.into());
    }

    /// Resolve a Ren'Py image / movie name to an existing file.
    ///
    /// Probe order (dest, then extract, for each candidate):
    /// 1. `visuals.rpy` Movie() path
    /// 2. authored path as-is (if it contains `/` or an extension)
    /// 3. `images/<name>.{jpg,png,webp,webm}`
    /// 4. `images/animations/<name>.{jpg,png,webp,webm}`
    pub fn resolve_visual(&self, name: &str) -> Option<PathBuf> {
        let name = name.trim();
        if name.is_empty() {
            return None;
        }
        if let Some(rel) = self.movies.get(name) {
            if let Some(p) = self.probe(rel) {
                return Some(p);
            }
        }
        if looks_like_path(name) {
            if let Some(p) = self.probe(Path::new(name)) {
                return Some(p);
            }
        }
        let stem = strip_known_ext(name, VISUAL_EXTS);
        for ext in VISUAL_EXTS {
            if let Some(p) = self.probe(Path::new(&format!("images/{stem}.{ext}"))) {
                return Some(p);
            }
        }
        for ext in VISUAL_EXTS {
            if let Some(p) = self.probe(Path::new(&format!("images/animations/{stem}.{ext}"))) {
                return Some(p);
            }
        }
        None
    }

    /// Resolve a Ren'Py audio name (`spark`, `spark.mp3`, `audio/spark.mp3`).
    pub fn resolve_audio(&self, name: &str) -> Option<PathBuf> {
        let name = name.trim();
        if name.is_empty() {
            return None;
        }
        if looks_like_path(name) {
            if let Some(p) = self.probe(Path::new(name)) {
                return Some(p);
            }
        }
        let file = name.rsplit('/').next().unwrap_or(name);
        if has_known_ext(file, AUDIO_EXTS) {
            if let Some(p) = self.probe(Path::new(&format!("audio/{file}"))) {
                return Some(p);
            }
        }
        let stem = strip_known_ext(file, AUDIO_EXTS);
        for ext in AUDIO_EXTS {
            if let Some(p) = self.probe(Path::new(&format!("audio/{stem}.{ext}"))) {
                return Some(p);
            }
        }
        None
    }

    /// Visual or audio, based on the name/extension.
    pub fn resolve(&self, name: &str) -> Option<PathBuf> {
        if is_audio_name(name) {
            self.resolve_audio(name)
        } else {
            self.resolve_visual(name)
                .or_else(|| self.resolve_audio(name))
        }
    }

    /// Dest-relative first, then extract-relative.
    fn probe(&self, rel: &Path) -> Option<PathBuf> {
        let dest = self.dest.join(rel);
        if dest.is_file() {
            return Some(dest);
        }
        let extract = self.extract.join(rel);
        if extract.is_file() {
            return Some(extract);
        }
        None
    }
}

/// Parse `image NAME = Movie(play="images/animations/NAME.webm")` lines.
///
/// Tolerates missing spaces around `=` (seen in HarmonyHaven `visuals.rpy`).
pub fn parse_visuals_rpy(src: &str) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    for raw in src.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some(rest) = line.strip_prefix("image") else {
            continue;
        };
        let rest = rest.trim_start();
        let Some(eq) = rest.find('=') else {
            continue;
        };
        let name = rest[..eq].trim();
        if name.is_empty() {
            continue;
        }
        let rhs = rest[eq + 1..].trim();
        if !rhs.starts_with("Movie") {
            continue;
        }
        let Some(play) = movie_play_path(rhs) else {
            continue;
        };
        out.push((name.to_string(), PathBuf::from(play)));
    }
    out
}

fn movie_play_path(rhs: &str) -> Option<&str> {
    let play = rhs.find("play")?;
    let after = rhs[play + 4..].trim_start();
    let after = after.strip_prefix('=')?.trim_start();
    let quote = after.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let body = &after[1..];
    let end = body.find(quote)?;
    Some(&body[..end])
}

fn looks_like_path(name: &str) -> bool {
    name.contains('/') || name.contains('\\') || Path::new(name).extension().is_some()
}

fn has_known_ext(name: &str, exts: &[&str]) -> bool {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| exts.iter().any(|k| e.eq_ignore_ascii_case(k)))
        .unwrap_or(false)
}

fn strip_known_ext<'a>(name: &'a str, exts: &[&str]) -> &'a str {
    if has_known_ext(name, exts) {
        Path::new(name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(name)
    } else {
        name
    }
}

fn is_audio_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("audio/") || has_known_ext(name, AUDIO_EXTS)
}

/// Resolve a visual name with dest = cwd, extract = default HarmonyHaven tree.
pub fn resolve_visual(name: &str) -> Option<PathBuf> {
    AssetResolver::from_env().resolve_visual(name)
}

/// Resolve an audio name with dest = cwd, extract = default HarmonyHaven tree.
pub fn resolve_audio(name: &str) -> Option<PathBuf> {
    AssetResolver::from_env().resolve_audio(name)
}

/// True when a resolved path should be presented as a looping Movie().
pub fn is_movie_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("webm") || e.eq_ignore_ascii_case("mp4"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_file(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, b"x").unwrap();
    }

    fn unique_root(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "ae-assets-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn fixture_resolver() -> (PathBuf, AssetResolver) {
        let root = unique_root("fix");
        fs::create_dir_all(&root).unwrap();
        let dest = root.join("dest");
        let extract = root.join("extract");
        write_file(&extract.join("images/bgn1.jpg"));
        write_file(&extract.join("images/ch1.png"));
        write_file(&extract.join("images/animations/anima1.webm"));
        write_file(&extract.join("audio/spark.mp3"));
        fs::write(
            extract.join("scripts/visuals.rpy"),
            r#"
image anima1 = Movie(play="images/animations/anima1.webm")
image anima2 = Movie(play="images/animations/anima2.webm")
image zkbsanim3= Movie(play="images/animations/zkbsanim3.webm")
"#,
        )
        .unwrap();
        write_file(&dest.join("images/bgn1.jpg"));
        let resolver = AssetResolver::new(&dest, &extract);
        (root, resolver)
    }

    #[test]
    fn parse_visuals_rpy_movie_lines() {
        let src = r#"
image anima1 = Movie(play="images/animations/anima1.webm")
image zkbsanim3= Movie(play="images/animations/zkbsanim3.webm")
# comment
image bg kitchen = "images/bgn1.jpg"
"#;
        let map = parse_visuals_rpy(src);
        assert_eq!(map.len(), 2);
        assert_eq!(map[0].0, "anima1");
        assert_eq!(map[0].1, PathBuf::from("images/animations/anima1.webm"));
        assert_eq!(map[1].0, "zkbsanim3");
    }

    #[test]
    fn table_resolves_bgn1_ch1_anima1_spark() {
        let extract_real = PathBuf::from(DEFAULT_EXTRACT);
        let (_hold, resolver) = if extract_real.join("images/bgn1.jpg").is_file() {
            let dest = unique_root("empty-dest");
            fs::create_dir_all(&dest).ok();
            (dest.clone(), AssetResolver::new(&dest, &extract_real))
        } else {
            fixture_resolver()
        };

        let cases: &[(&str, &str)] = &[
            ("bgn1", "images/bgn1.jpg"),
            ("ch1", "images/ch1.png"),
            ("anima1", "images/animations/anima1.webm"),
            ("spark.mp3", "audio/spark.mp3"),
        ];
        for (name, expect_suffix) in cases {
            let got = if *name == "spark.mp3" {
                resolver.resolve_audio(name)
            } else {
                resolver.resolve_visual(name)
            };
            let got = got.unwrap_or_else(|| panic!("unresolved {name}"));
            let got_s = got.to_string_lossy().replace('\\', "/");
            assert!(
                got_s.ends_with(expect_suffix),
                "{name}: expected suffix {expect_suffix}, got {got_s}"
            );
            assert!(got.is_file(), "{name}: {} is not a file", got.display());
        }
        assert!(is_movie_path(&resolver.resolve_visual("anima1").unwrap()));
    }

    #[test]
    fn dest_wins_over_extract() {
        let root = std::env::temp_dir().join(format!(
            "ae-dest-win-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let dest = root.join("dest");
        let extract = root.join("extract");
        write_file(&dest.join("images/bgn1.jpg"));
        write_file(&extract.join("images/bgn1.jpg"));
        let r = AssetResolver::new(&dest, &extract);
        let got = r.resolve_visual("bgn1").unwrap();
        assert!(got.starts_with(&dest), "dest should win: {}", got.display());
        let _ = fs::remove_dir_all(&root);
    }
}
