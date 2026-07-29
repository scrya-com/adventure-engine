//! 4-bus audio engine (Master/Music/Sfx/Voice) backed by kira.
//!
//! Mirrors UE's [`USoundClass`](Engine/Classes/Sound/SoundClass.h) +
//! [`USoundMix`](Engine/Classes/Sound/SoundMix.h) crossfade primitive.
//! See `docs/AUDIO.md`.
//!
//! # Backends
//!
//! - [`NullMixer`] — no device; records play/stop for tests and headless CI.
//! - [`KiraMixer`] — real output via kira's default backend (cpal).
//!
//! Both implement [`AudioEngine`].

#![deny(missing_docs)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use adventure_core::AssetId;
use crossbeam_channel::{Receiver, Sender, TryRecvError};

// ── Types ────────────────────────────────────────────────────────────────

/// Mixer bus (UE SoundClass grouping).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bus {
    /// Global volume multiplier applied to all other buses.
    Master,
    /// Background music (usually looping).
    Music,
    /// One-shot sound effects.
    Sfx,
    /// Voice-over dialog.
    Voice,
}

impl Bus {
    /// All non-master buses (routing targets for clips).
    pub const ROUTABLE: [Bus; 3] = [Bus::Music, Bus::Sfx, Bus::Voice];
}

/// Fade / crossfade curve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Curve {
    /// Constant speed.
    #[default]
    Linear,
    /// Logarithmic-ish (kira `InPowf(2)`).
    Logarithmic,
    /// Equal-power (sine-ish) — preferred for music crossfades.
    EqualPower,
}

/// Clip identifier (path-hash [`AssetId`]).
pub type ClipId = AssetId;

/// Opaque playback handle returned by looping / VO / music plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Handle(u64);

impl Handle {
    /// Raw id (for debugging / save correlation).
    pub fn id(self) -> u64 {
        self.0
    }
}

/// Subtitle attached to a voice-over line.
#[derive(Debug, Clone, PartialEq)]
pub struct Subtitle {
    /// Speaker label (tag string or display name).
    pub speaker: String,
    /// Line text.
    pub text: String,
    /// How long the subtitle should remain visible.
    pub duration_secs: f32,
}

/// Event pushed when VO starts (UI consumes for subtitle rendering).
#[derive(Debug, Clone, PartialEq)]
pub struct SubtitleEvent {
    /// Handle of the VO playback.
    pub handle: Handle,
    /// Subtitle payload.
    pub subtitle: Subtitle,
}

/// Snapshot of music for save games.
#[derive(Debug, Clone, PartialEq)]
pub struct MusicSnapshot {
    /// Clip currently playing (if any).
    pub clip: Option<ClipId>,
    /// Playback position in seconds (best-effort; 0 if unknown).
    pub position_secs: f64,
}

/// Bus volumes (0.0–1.0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BusVolumes {
    /// Master.
    pub master: f32,
    /// Music.
    pub music: f32,
    /// Sfx.
    pub sfx: f32,
    /// Voice.
    pub voice: f32,
}

impl Default for BusVolumes {
    fn default() -> Self {
        Self {
            master: 1.0,
            music: 1.0,
            sfx: 1.0,
            voice: 1.0,
        }
    }
}

impl BusVolumes {
    /// Volume for a bus.
    pub fn get(self, bus: Bus) -> f32 {
        match bus {
            Bus::Master => self.master,
            Bus::Music => self.music,
            Bus::Sfx => self.sfx,
            Bus::Voice => self.voice,
        }
    }

    /// Effective gain for a routable bus (master × bus).
    pub fn effective(self, bus: Bus) -> f32 {
        match bus {
            Bus::Master => self.master,
            other => self.master * self.get(other),
        }
    }

    /// Set a bus volume (clamped 0–1).
    pub fn set(&mut self, bus: Bus, vol: f32) {
        let v = vol.clamp(0.0, 1.0);
        match bus {
            Bus::Master => self.master = v,
            Bus::Music => self.music = v,
            Bus::Sfx => self.sfx = v,
            Bus::Voice => self.voice = v,
        }
    }
}

/// Audio subsystem errors.
#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    /// Clip not registered / not loaded.
    #[error("unknown clip: {0}")]
    UnknownClip(ClipId),
    /// Handle not found or already stopped.
    #[error("unknown handle: {0:?}")]
    UnknownHandle(Handle),
    /// Failed to open the audio device / kira manager.
    #[error("device: {0}")]
    Device(String),
    /// Failed to decode / load a clip file.
    #[error("load: {0}")]
    Load(String),
    /// Internal play failure.
    #[error("play: {0}")]
    Play(String),
}

// ── Trait ────────────────────────────────────────────────────────────────

/// Engine-facing audio API (UE `UGameplayStatics` + bus mix surface).
pub trait AudioEngine {
    /// Register a clip from stereo PCM frames `(left, right)` at `sample_rate` Hz.
    fn register_pcm(
        &mut self,
        id: ClipId,
        sample_rate: u32,
        frames: &[(f32, f32)],
    ) -> Result<(), AudioError>;

    /// Register a clip from a file path (ogg/wav/mp3 via kira/symphonia).
    ///
    /// [`NullMixer`] records the path but does not decode.
    fn register_file(&mut self, id: ClipId, path: impl AsRef<Path>) -> Result<(), AudioError>;

    /// Play a clip once on `bus` at relative volume `vol` (0–1, pre-bus).
    fn play_oneshot(&mut self, clip: ClipId, bus: Bus, vol: f32) -> Result<(), AudioError>;

    /// Play a looping clip; returns a handle for stop / crossfade.
    fn play_looping(&mut self, clip: ClipId, bus: Bus) -> Result<Handle, AudioError>;

    /// Crossfade music to a new clip over `secs`.
    ///
    /// `music` is the handle of the current music bed. Returns the new bed handle.
    fn crossfade(
        &mut self,
        music: Handle,
        to: ClipId,
        secs: f32,
        curve: Curve,
    ) -> Result<Handle, AudioError>;

    /// Fade a whole bus to `to_vol` over `secs`.
    fn fade_bus(&mut self, bus: Bus, to_vol: f32, secs: f32);

    /// Immediately set a bus volume (no fade).
    fn set_bus_volume(&mut self, bus: Bus, vol: f32);

    /// Current bus volumes.
    fn bus_volumes(&self) -> BusVolumes;

    /// Play VO with a subtitle; returns handle.
    fn queue_vo(&mut self, clip: ClipId, subtitle: Subtitle) -> Result<Handle, AudioError>;

    /// Stop a handle, optionally fading out.
    fn stop(&mut self, h: Handle, fade_secs: f32) -> Result<(), AudioError>;

    /// Per-frame bookkeeping (subtitle timers, pending fades).
    fn tick(&mut self, dt: f32);

    /// Poll one subtitle event if available.
    fn poll_subtitle(&mut self) -> Option<SubtitleEvent>;

    /// Snapshot of current music for save games.
    fn music_snapshot(&self) -> MusicSnapshot;

    /// Restore music from a save snapshot.
    fn restore_music(&mut self, snap: &MusicSnapshot) -> Result<Option<Handle>, AudioError>;
}

// ── Shared fade bookkeeping ──────────────────────────────────────────────

struct PendingBusFade {
    bus: Bus,
    from: f32,
    to: f32,
    elapsed: f32,
    duration: f32,
}

fn kira_tween(secs: f32, curve: Curve) -> kira::tween::Tween {
    use kira::tween::{Easing, Tween};
    Tween {
        duration: Duration::from_secs_f32(secs.max(0.0)),
        easing: match curve {
            Curve::Linear => Easing::Linear,
            Curve::Logarithmic => Easing::InPowf(2.0),
            Curve::EqualPower => Easing::InOutPowi(2),
        },
        ..Default::default()
    }
}

// ── Null mixer ───────────────────────────────────────────────────────────

/// In-memory record of a play command (for assertions).
#[derive(Debug, Clone, PartialEq)]
pub struct PlayRecord {
    /// Clip.
    pub clip: ClipId,
    /// Bus.
    pub bus: Bus,
    /// Volume at play time.
    pub vol: f32,
    /// Whether looping.
    pub looping: bool,
}

/// Headless mixer — no audio device. Implements full [`AudioEngine`] surface.
pub struct NullMixer {
    next_handle: u64,
    volumes: BusVolumes,
    clips: HashMap<ClipId, NullClip>,
    active: HashMap<Handle, ActiveNull>,
    music: Option<(Handle, ClipId)>,
    plays: Vec<PlayRecord>,
    sub_tx: Sender<SubtitleEvent>,
    sub_rx: Receiver<SubtitleEvent>,
    pending_fades: Vec<PendingBusFade>,
}

struct NullClip {
    #[allow(dead_code)]
    path: Option<PathBuf>,
    duration_secs: f32,
}

struct ActiveNull {
    clip: ClipId,
    #[allow(dead_code)]
    bus: Bus,
    looping: bool,
    age: f32,
}

impl Default for NullMixer {
    fn default() -> Self {
        Self::new()
    }
}

impl NullMixer {
    /// Create a null mixer.
    pub fn new() -> Self {
        let (sub_tx, sub_rx) = crossbeam_channel::unbounded();
        Self {
            next_handle: 1,
            volumes: BusVolumes::default(),
            clips: HashMap::new(),
            active: HashMap::new(),
            music: None,
            plays: Vec::new(),
            sub_tx,
            sub_rx,
            pending_fades: Vec::new(),
        }
    }

    /// All play records since construction (test helper).
    pub fn play_log(&self) -> &[PlayRecord] {
        &self.plays
    }

    /// Clear the play log.
    pub fn clear_play_log(&mut self) {
        self.plays.clear();
    }

    fn alloc(&mut self) -> Handle {
        let h = Handle(self.next_handle);
        self.next_handle += 1;
        h
    }

    fn ensure_clip(&self, id: ClipId) -> Result<&NullClip, AudioError> {
        self.clips.get(&id).ok_or(AudioError::UnknownClip(id))
    }
}

impl AudioEngine for NullMixer {
    fn register_pcm(
        &mut self,
        id: ClipId,
        sample_rate: u32,
        frames: &[(f32, f32)],
    ) -> Result<(), AudioError> {
        let duration_secs = if sample_rate == 0 {
            0.0
        } else {
            frames.len() as f32 / sample_rate as f32
        };
        self.clips.insert(
            id,
            NullClip {
                path: None,
                duration_secs,
            },
        );
        Ok(())
    }

    fn register_file(&mut self, id: ClipId, path: impl AsRef<Path>) -> Result<(), AudioError> {
        self.clips.insert(
            id,
            NullClip {
                path: Some(path.as_ref().to_path_buf()),
                duration_secs: 1.0,
            },
        );
        Ok(())
    }

    fn play_oneshot(&mut self, clip: ClipId, bus: Bus, vol: f32) -> Result<(), AudioError> {
        self.ensure_clip(clip)?;
        self.plays.push(PlayRecord {
            clip,
            bus,
            vol,
            looping: false,
        });
        let h = self.alloc();
        self.active.insert(
            h,
            ActiveNull {
                clip,
                bus,
                looping: false,
                age: 0.0,
            },
        );
        Ok(())
    }

    fn play_looping(&mut self, clip: ClipId, bus: Bus) -> Result<Handle, AudioError> {
        self.ensure_clip(clip)?;
        self.plays.push(PlayRecord {
            clip,
            bus,
            vol: 1.0,
            looping: true,
        });
        let h = self.alloc();
        self.active.insert(
            h,
            ActiveNull {
                clip,
                bus,
                looping: true,
                age: 0.0,
            },
        );
        if bus == Bus::Music {
            self.music = Some((h, clip));
        }
        Ok(h)
    }

    fn crossfade(
        &mut self,
        music: Handle,
        to: ClipId,
        _secs: f32,
        _curve: Curve,
    ) -> Result<Handle, AudioError> {
        let _ = self.stop(music, 0.0);
        self.play_looping(to, Bus::Music)
    }

    fn fade_bus(&mut self, bus: Bus, to_vol: f32, secs: f32) {
        let from = self.volumes.get(bus);
        if secs <= 0.0 {
            self.volumes.set(bus, to_vol);
            return;
        }
        self.pending_fades.push(PendingBusFade {
            bus,
            from,
            to: to_vol.clamp(0.0, 1.0),
            elapsed: 0.0,
            duration: secs,
        });
    }

    fn set_bus_volume(&mut self, bus: Bus, vol: f32) {
        self.volumes.set(bus, vol);
    }

    fn bus_volumes(&self) -> BusVolumes {
        self.volumes
    }

    fn queue_vo(&mut self, clip: ClipId, subtitle: Subtitle) -> Result<Handle, AudioError> {
        self.ensure_clip(clip)?;
        self.plays.push(PlayRecord {
            clip,
            bus: Bus::Voice,
            vol: 1.0,
            looping: false,
        });
        let h = self.alloc();
        self.active.insert(
            h,
            ActiveNull {
                clip,
                bus: Bus::Voice,
                looping: false,
                age: 0.0,
            },
        );
        let _ = self.sub_tx.send(SubtitleEvent {
            handle: h,
            subtitle,
        });
        Ok(h)
    }

    fn stop(&mut self, h: Handle, _fade_secs: f32) -> Result<(), AudioError> {
        if self.active.remove(&h).is_none() {
            return Err(AudioError::UnknownHandle(h));
        }
        if self.music.map(|(mh, _)| mh) == Some(h) {
            self.music = None;
        }
        Ok(())
    }

    fn tick(&mut self, dt: f32) {
        let mut done = Vec::new();
        for (i, f) in self.pending_fades.iter_mut().enumerate() {
            f.elapsed += dt;
            let t = (f.elapsed / f.duration).clamp(0.0, 1.0);
            let v = f.from + (f.to - f.from) * t;
            self.volumes.set(f.bus, v);
            if t >= 1.0 {
                done.push(i);
            }
        }
        for i in done.into_iter().rev() {
            self.pending_fades.remove(i);
        }

        let mut expired = Vec::new();
        for (h, a) in self.active.iter_mut() {
            a.age += dt;
            if !a.looping {
                if let Some(c) = self.clips.get(&a.clip) {
                    if a.age >= c.duration_secs.max(0.05) {
                        expired.push(*h);
                    }
                }
            }
        }
        for h in expired {
            self.active.remove(&h);
        }
    }

    fn poll_subtitle(&mut self) -> Option<SubtitleEvent> {
        match self.sub_rx.try_recv() {
            Ok(e) => Some(e),
            Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => None,
        }
    }

    fn music_snapshot(&self) -> MusicSnapshot {
        MusicSnapshot {
            clip: self.music.map(|(_, c)| c),
            position_secs: self
                .music
                .and_then(|(h, _)| self.active.get(&h).map(|a| a.age as f64))
                .unwrap_or(0.0),
        }
    }

    fn restore_music(&mut self, snap: &MusicSnapshot) -> Result<Option<Handle>, AudioError> {
        if let Some(clip) = snap.clip {
            let h = self.play_looping(clip, Bus::Music)?;
            if let Some(a) = self.active.get_mut(&h) {
                a.age = snap.position_secs as f32;
            }
            Ok(Some(h))
        } else {
            Ok(None)
        }
    }
}

// ── Kira mixer ───────────────────────────────────────────────────────────

/// Real audio mixer using kira 0.9.
pub struct KiraMixer {
    manager: kira::manager::AudioManager<kira::manager::backend::DefaultBackend>,
    tracks: HashMap<Bus, kira::track::TrackHandle>,
    volumes: BusVolumes,
    clips: HashMap<ClipId, kira::sound::static_sound::StaticSoundData>,
    active: HashMap<Handle, ActiveKira>,
    music: Option<(Handle, ClipId)>,
    next_handle: u64,
    sub_tx: Sender<SubtitleEvent>,
    sub_rx: Receiver<SubtitleEvent>,
    pending_fades: Vec<PendingBusFade>,
}

struct ActiveKira {
    handle: kira::sound::static_sound::StaticSoundHandle,
    #[allow(dead_code)]
    clip: ClipId,
    #[allow(dead_code)]
    bus: Bus,
    #[allow(dead_code)]
    looping: bool,
    age: f32,
}

impl KiraMixer {
    /// Open the default audio device and create Music/Sfx/Voice sub-tracks.
    pub fn new() -> Result<Self, AudioError> {
        use kira::manager::{AudioManager, AudioManagerSettings};
        use kira::track::TrackBuilder;

        let mut manager = AudioManager::<kira::manager::backend::DefaultBackend>::new(
            AudioManagerSettings::default(),
        )
        .map_err(|e| AudioError::Device(format!("{e:?}")))?;

        let mut tracks = HashMap::new();
        for bus in Bus::ROUTABLE {
            let track = manager
                .add_sub_track(TrackBuilder::new())
                .map_err(|e| AudioError::Device(format!("{e:?}")))?;
            tracks.insert(bus, track);
        }

        let (sub_tx, sub_rx) = crossbeam_channel::unbounded();
        Ok(Self {
            manager,
            tracks,
            volumes: BusVolumes::default(),
            clips: HashMap::new(),
            active: HashMap::new(),
            music: None,
            next_handle: 1,
            sub_tx,
            sub_rx,
            pending_fades: Vec::new(),
        })
    }

    fn alloc(&mut self) -> Handle {
        let h = Handle(self.next_handle);
        self.next_handle += 1;
        h
    }

    fn apply_track_volume(&mut self, bus: Bus) {
        if bus == Bus::Master {
            for b in Bus::ROUTABLE {
                self.apply_track_volume(b);
            }
            return;
        }
        let gain = self.volumes.effective(bus) as f64;
        if let Some(track) = self.tracks.get_mut(&bus) {
            let _ = track.set_volume(
                kira::Volume::Amplitude(gain),
                kira::tween::Tween::default(),
            );
        }
    }

    fn play_data(
        &mut self,
        clip: ClipId,
        bus: Bus,
        looping: bool,
        vol: f32,
        fade_in: f32,
        curve: Curve,
        start_position: f64,
    ) -> Result<Handle, AudioError> {
        use kira::sound::static_sound::StaticSoundSettings;

        let data = self
            .clips
            .get(&clip)
            .cloned()
            .ok_or(AudioError::UnknownClip(clip))?;

        let track = self
            .tracks
            .get(&bus)
            .ok_or_else(|| AudioError::Device(format!("no track for {bus:?}")))?;

        let mut settings = StaticSoundSettings::new()
            .output_destination(track)
            .volume(kira::Volume::Amplitude(
                (vol.clamp(0.0, 1.0) as f64) * self.volumes.effective(bus) as f64,
            ));
        if looping {
            settings = settings.loop_region(0.0..);
        }
        if fade_in > 0.0 {
            settings = settings.fade_in_tween(kira_tween(fade_in, curve));
        }
        if start_position > 0.0 {
            settings = settings.start_position(start_position);
        }

        let sound = self
            .manager
            .play(data.with_settings(settings))
            .map_err(|e| AudioError::Play(e.to_string()))?;

        let h = self.alloc();
        self.active.insert(
            h,
            ActiveKira {
                handle: sound,
                clip,
                bus,
                looping,
                age: start_position as f32,
            },
        );
        if bus == Bus::Music && looping {
            self.music = Some((h, clip));
        }
        Ok(h)
    }
}

impl AudioEngine for KiraMixer {
    fn register_pcm(
        &mut self,
        id: ClipId,
        sample_rate: u32,
        frames: &[(f32, f32)],
    ) -> Result<(), AudioError> {
        use kira::sound::static_sound::StaticSoundData;
        use kira::Frame;

        let kframes: Arc<[Frame]> = frames
            .iter()
            .map(|(l, r)| Frame::new(*l, *r))
            .collect::<Vec<_>>()
            .into();
        let data = StaticSoundData {
            sample_rate,
            frames: kframes,
            settings: Default::default(),
            slice: None,
        };
        self.clips.insert(id, data);
        Ok(())
    }

    fn register_file(&mut self, id: ClipId, path: impl AsRef<Path>) -> Result<(), AudioError> {
        use kira::sound::static_sound::StaticSoundData;
        let data = StaticSoundData::from_file(path.as_ref())
            .map_err(|e| AudioError::Load(e.to_string()))?;
        self.clips.insert(id, data);
        Ok(())
    }

    fn play_oneshot(&mut self, clip: ClipId, bus: Bus, vol: f32) -> Result<(), AudioError> {
        let _ = self.play_data(clip, bus, false, vol, 0.0, Curve::Linear, 0.0)?;
        Ok(())
    }

    fn play_looping(&mut self, clip: ClipId, bus: Bus) -> Result<Handle, AudioError> {
        self.play_data(clip, bus, true, 1.0, 0.0, Curve::Linear, 0.0)
    }

    fn crossfade(
        &mut self,
        music: Handle,
        to: ClipId,
        secs: f32,
        curve: Curve,
    ) -> Result<Handle, AudioError> {
        if let Some(mut active) = self.active.remove(&music) {
            active.handle.stop(kira_tween(secs, curve));
        }
        if self.music.map(|(h, _)| h) == Some(music) {
            self.music = None;
        }
        self.play_data(to, Bus::Music, true, 1.0, secs, curve, 0.0)
    }

    fn fade_bus(&mut self, bus: Bus, to_vol: f32, secs: f32) {
        let from = self.volumes.get(bus);
        if secs <= 0.0 {
            self.volumes.set(bus, to_vol);
            self.apply_track_volume(bus);
            return;
        }
        self.pending_fades.push(PendingBusFade {
            bus,
            from,
            to: to_vol.clamp(0.0, 1.0),
            elapsed: 0.0,
            duration: secs,
        });
    }

    fn set_bus_volume(&mut self, bus: Bus, vol: f32) {
        self.volumes.set(bus, vol);
        self.apply_track_volume(bus);
    }

    fn bus_volumes(&self) -> BusVolumes {
        self.volumes
    }

    fn queue_vo(&mut self, clip: ClipId, subtitle: Subtitle) -> Result<Handle, AudioError> {
        let h = self.play_data(clip, Bus::Voice, false, 1.0, 0.0, Curve::Linear, 0.0)?;
        let _ = self.sub_tx.send(SubtitleEvent {
            handle: h,
            subtitle,
        });
        Ok(h)
    }

    fn stop(&mut self, h: Handle, fade_secs: f32) -> Result<(), AudioError> {
        let mut active = self
            .active
            .remove(&h)
            .ok_or(AudioError::UnknownHandle(h))?;
        active.handle.stop(kira_tween(fade_secs, Curve::Linear));
        if self.music.map(|(mh, _)| mh) == Some(h) {
            self.music = None;
        }
        Ok(())
    }

    fn tick(&mut self, dt: f32) {
        let mut done = Vec::new();
        for (i, f) in self.pending_fades.iter_mut().enumerate() {
            f.elapsed += dt;
            let t = (f.elapsed / f.duration).clamp(0.0, 1.0);
            let v = f.from + (f.to - f.from) * t;
            self.volumes.set(f.bus, v);
            if t >= 1.0 {
                done.push(i);
            }
        }
        for i in done.into_iter().rev() {
            let f = self.pending_fades.remove(i);
            self.apply_track_volume(f.bus);
        }
        for f in &self.pending_fades {
            let gain = self.volumes.effective(f.bus) as f64;
            if let Some(track) = self.tracks.get_mut(&f.bus) {
                let _ = track.set_volume(
                    kira::Volume::Amplitude(gain),
                    kira::tween::Tween::default(),
                );
            }
        }
        for a in self.active.values_mut() {
            a.age += dt;
        }
    }

    fn poll_subtitle(&mut self) -> Option<SubtitleEvent> {
        self.sub_rx.try_recv().ok()
    }

    fn music_snapshot(&self) -> MusicSnapshot {
        MusicSnapshot {
            clip: self.music.map(|(_, c)| c),
            position_secs: self
                .music
                .and_then(|(h, _)| self.active.get(&h).map(|a| a.age as f64))
                .unwrap_or(0.0),
        }
    }

    fn restore_music(&mut self, snap: &MusicSnapshot) -> Result<Option<Handle>, AudioError> {
        if let Some(clip) = snap.clip {
            let h = self.play_data(
                clip,
                Bus::Music,
                true,
                1.0,
                0.0,
                Curve::Linear,
                snap.position_secs,
            )?;
            Ok(Some(h))
        } else {
            Ok(None)
        }
    }
}

/// Build a simple sine tone as stereo PCM (demos / tests without asset files).
pub fn sine_pcm(freq_hz: f32, duration_secs: f32, sample_rate: u32) -> Vec<(f32, f32)> {
    let n = (duration_secs * sample_rate as f32) as usize;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sample_rate as f32;
        let env = if t < 0.01 {
            t / 0.01
        } else if t > duration_secs - 0.05 {
            ((duration_secs - t) / 0.05).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let s = (t * freq_hz * std::f32::consts::TAU).sin() * 0.25 * env;
        out.push((s, s));
    }
    out
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_play_loop_crossfade_and_save_snapshot() {
        let mut m = NullMixer::new();
        let theme_a = ClipId::from_path("music/cellblock");
        let theme_b = ClipId::from_path("music/yard");
        m.register_pcm(theme_a, 44100, &sine_pcm(220.0, 0.5, 44100))
            .unwrap();
        m.register_pcm(theme_b, 44100, &sine_pcm(330.0, 0.5, 44100))
            .unwrap();

        let h = m.play_looping(theme_a, Bus::Music).unwrap();
        m.tick(1.5);
        let snap = m.music_snapshot();
        assert_eq!(snap.clip, Some(theme_a));
        assert!((snap.position_secs - 1.5).abs() < 1e-3);

        let h2 = m
            .crossfade(h, theme_b, 1.0, Curve::EqualPower)
            .unwrap();
        assert_ne!(h, h2);
        assert_eq!(m.music_snapshot().clip, Some(theme_b));

        // Room-change narrative: two looping plays logged.
        assert!(m.play_log().iter().filter(|p| p.looping).count() >= 2);
    }

    #[test]
    fn null_vo_emits_subtitle() {
        let mut m = NullMixer::new();
        let vo = ClipId::from_path("vo/red_hope");
        m.register_pcm(vo, 22050, &sine_pcm(440.0, 0.2, 22050))
            .unwrap();
        let _h = m
            .queue_vo(
                vo,
                Subtitle {
                    speaker: "Red".into(),
                    text: "Hope is a dangerous thing.".into(),
                    duration_secs: 2.0,
                },
            )
            .unwrap();
        let ev = m.poll_subtitle().expect("subtitle event");
        assert_eq!(ev.subtitle.speaker, "Red");
        assert!(m.poll_subtitle().is_none());
    }

    #[test]
    fn null_bus_fade() {
        let mut m = NullMixer::new();
        m.fade_bus(Bus::Music, 0.0, 1.0);
        m.tick(0.5);
        let v = m.bus_volumes().music;
        assert!((v - 0.5).abs() < 0.05, "mid-fade music vol={v}");
        m.tick(0.6);
        assert!((m.bus_volumes().music - 0.0).abs() < 1e-3);
    }

    #[test]
    fn null_restore_music() {
        let mut m = NullMixer::new();
        let theme = ClipId::from_path("music/theme");
        m.register_pcm(theme, 44100, &sine_pcm(200.0, 1.0, 44100))
            .unwrap();
        let snap = MusicSnapshot {
            clip: Some(theme),
            position_secs: 3.0,
        };
        let h = m.restore_music(&snap).unwrap().unwrap();
        assert_eq!(m.music_snapshot().clip, Some(theme));
        assert!((m.music_snapshot().position_secs - 3.0).abs() < 1e-3);
        m.stop(h, 0.0).unwrap();
        assert!(m.music_snapshot().clip.is_none());
    }

    #[test]
    fn bus_volumes_effective() {
        let mut v = BusVolumes::default();
        v.set(Bus::Master, 0.5);
        v.set(Bus::Music, 0.5);
        assert!((v.effective(Bus::Music) - 0.25).abs() < 1e-6);
    }
}
