# Audio — kira 4-Bus Model

> Deep dive on `crates/audio/`. See `docs/DESIGN.md` for the high-level rationale.

## UE 5.8 reference

Unreal's audio stack (analyzed across Engine/Source/Runtime/{AudioMixer,Engine/Classes/Sound/}/):

**Three-layer routing:**

1. **Assets** — `USoundWave` (raw clip + compressed chunks), `USoundCue` (graph-based SFX), `UDialogueWave` (context → soundwave map with subtitle text).
2. **Buses / classes / submixes:**
   - `USoundClass` — volume grouping (`Volume`, `Pitch`, `bIsMusic`, `bIsUISound`)
   - `USoundSubmix` — DSP graph (EQ/reverb/send levels)
   - `UAudioBus` — patch-cord between sources
3. **Playback** — `UAudioComponent`: `Play()`, `Stop()`, `FadeIn(dur, level, start, EAudioFaderCurve)`, `FadeOut(...)`, with state machine `EAudioComponentPlayState { Playing, Stopped, Paused, FadingIn, FadingOut }` and curves `Linear / Logarithmic / SCurve / Sin(Equal-Power)`.

**What we keep:** SoundClass (buses) + UAudioComponent's Play/Stop/FadeIn/FadeOut + USoundMix (crossfade primitive) + UDialogueWave (VO with subtitles).

**What we skip:** Submix DSP graph (no reverb sends in point-and-click), MetaSounds, attenuation, spatialization, 3D listener.

## Buses

Four buses cover point-and-click:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bus {
    Master,  // global volume multiplier
    Music,   // background music
    Sfx,     // one-shot sound effects (clicks, doors, footsteps)
    Voice,   // voice-over dialog
}
```

Each bus has its own volume + mute. Bus volumes are multiplied — `Master * Music` is the effective music volume. Bus volume changes fade smoothly over a configurable duration to avoid clicks.

## Public API

```rust
pub trait AudioEngine {
    /// Play a clip once. Fire-and-forget.
    /// Equivalent: UGameplayStatics::PlaySound2D
    fn play_oneshot(&mut self, clip: ClipId, bus: Bus, vol: f32);

    /// Play a clip that loops until stopped. Returns a handle.
    /// Equivalent: UAudioComponent + bLooping=true
    fn play_looping(&mut self, clip: ClipId, bus: Bus) -> Handle;

    /// Crossfade a music handle to a new clip over `secs` seconds.
    /// Uses equal-power curve by default (avoids mid-crossfade dip).
    fn crossfade(&mut self, music: Handle, to: ClipId, secs: f32, curve: Curve);

    /// Fade a whole bus to a target volume over `secs` seconds.
    /// Equivalent: pushing a USoundMix with FSoundClassAdjuster
    fn fade_bus(&mut self, bus: Bus, to_vol: f32, secs: f32);

    /// Play a voice-over clip with an associated subtitle.
    /// Equivalent: UDialogueWave + FSubtitleManager::QueueSubtitles
    fn queue_vo(&mut self, clip: ClipId, subtitle: Subtitle) -> Handle;

    /// Stop a handle, optionally fading out.
    fn stop(&mut self, h: Handle, fade_secs: f32);

    /// Per-frame: pump subtitle queue, advance fades.
    fn tick(&mut self, dt: f32);
}

#[derive(Debug, Clone, Copy)]
pub enum Curve {
    Linear,
    Logarithmic,
    EqualPower,  // Sin curve — preferred for crossfades
}

pub struct Subtitle {
    pub speaker: Tag,
    pub text: String,
    pub duration_secs: f32,
}
```

## Backend: kira

[`kira`](https://crates.io/crates/kira) provides:
- Manager + track tree (we use 4 tracks for our 4 buses)
- Built-in fade curves (linear, sine, logarithm)
- Region loops, parameter automation
- Sound send levels (for bus → bus sends if needed)

```rust
// crates/audio/src/backend.rs
use kira::{
    manager::{AudioManager, AudioManagerSettings, backend::cpal::CpalBackend},
    track::{TrackBuilder, TrackIndex, TrackHandle},
    sound::static_sound::{StaticSoundData, StaticSoundHandle},
    tween::Tween,
    Volume,
};

pub struct KiraBackend {
    manager: AudioManager<CpalBackend>,
    buses: HashMap<Bus, TrackHandle>,
}

impl KiraBackend {
    pub fn new() -> Result<Self, AudioError> {
        let mut manager = AudioManager::<CpalBackend>::new(AudioManagerSettings::default())?;
        let mut buses = HashMap::new();
        // Master is the main track
        buses.insert(Bus::Master, manager.track(TrackBuilder::default())?);
        // Music/Sfx/Voice are sub-tracks of Master
        for bus in [Bus::Music, Bus::Sfx, Bus::Voice] {
            let h = manager.track(TrackBuilder::new().parent(buses[&Bus::Master].index()))?;
            buses.insert(bus, h);
        }
        Ok(Self { manager, buses })
    }
}
```

## Codecs

| Format | Use | Crate |
|---|---|---|
| Ogg/Vorbis | Music | `lewton` (kira-native) |
| Opus | Voice-over (low bitrate) | `audiopus_lite` or pre-decode to WAV at build time |
| WAV | SFX (short clicks) | kira built-in |

**Pre-decode voice-over at build time** if you want zero runtime Opus dependency. The `tools/packer` can transcode `.opus` → `.wav` during cooking (adventure game VO is short lines, not 30-minute audio books — disk size is fine).

Skip: ADPCM (proprietary/irrelevant), Bink (proprietary), RadAudioCodec.

## Save integration

Music state is saved:
- Current playing music clip AssetId
- Playback position
- Bus volumes

On load, restart the music at the saved position. SFX are not saved (one-shots by definition).

## Subtitle flow

```
queue_vo(clip, subtitle)
        ↓
play clip on Voice bus
        ↓
push subtitle onto UI subtitle queue with start_time + duration
        ↓
UI's subtitle renderer picks it up next frame
        ↓
on clip end OR duration expiry → pop subtitle
```

The audio engine emits a `SubtitleEvent` (via `crossbeam::channel` or `tokio::sync::mpsc`) that the UI consumes. No direct audio → UI dependency.

## What we skip

| Thing | Why |
|---|---|
| Sound attenuation (FSoundAttenuationSettings) | 2D — no listener, no panner |
| Spatialization | Same |
| Submix DSP graph (USoundSubmix) | Point-and-click doesn't need reverb sends |
| MetaSounds | Overkill — procedural audio not needed |
| SoundCue graph | Replaced by Rhai `play_sound("...")` calls |
| 3D HRTF | No |
| Audio effects (chorus, flange, etc.) | Not needed |
