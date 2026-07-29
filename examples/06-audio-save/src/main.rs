//! Phase 6 — Audio + save demo (headless).
//!
//! Demonstrates the Phase 6 exit criteria without a window:
//! 1. Register two music beds + a VO line (synthetic sine PCM).
//! 2. Play music A, crossfade to music B (room change).
//! 3. Snapshot tags/vars/music/bus volumes into a versioned save.
//! 4. Clear state, load the save, restore music + game state.
//!
//! Uses [`NullMixer`] so this runs on CI without ALSA/cpal. For real
//! speakers, swap in `KiraMixer::new()` (enable `adventure-audio/device`).
//!
//! ```text
//! cargo run -p example-06-audio-save
//! ```

use std::path::PathBuf;

use adventure_audio::{
    sine_pcm, AudioEngine, Bus, BusVolumes, ClipId, Curve, MusicSnapshot, NullMixer, Subtitle,
};
use adventure_core::AssetId;
use adventure_save::{
    slot_path, BusVolumes as SaveBusVolumes, Loader, MusicState, SaveBody, SaveVarValue, Saver,
};

fn main() {
    println!("=== Phase 6: audio + save ===\n");

    // ── Clips ────────────────────────────────────────────────────────
    let music_cell = ClipId::from_path("music/cellblock");
    let music_yard = ClipId::from_path("music/yard");
    let vo_red = ClipId::from_path("vo/red_hope");

    let mut audio = NullMixer::new();
    audio
        .register_pcm(music_cell, 44100, &sine_pcm(220.0, 2.0, 44100))
        .expect("register cellblock theme");
    audio
        .register_pcm(music_yard, 44100, &sine_pcm(330.0, 2.0, 44100))
        .expect("register yard theme");
    audio
        .register_pcm(vo_red, 22050, &sine_pcm(440.0, 0.4, 22050))
        .expect("register VO");

    // ── Room A: cellblock music ──────────────────────────────────────
    let mut music = audio
        .play_looping(music_cell, Bus::Music)
        .expect("play cellblock");
    audio.set_bus_volume(Bus::Music, 0.8);
    audio.tick(2.5);
    println!(
        "room=cellblock  music={:?}  pos={:.2}s  music_vol={:.2}",
        audio.music_snapshot().clip,
        audio.music_snapshot().position_secs,
        audio.bus_volumes().music
    );

    // ── Room change → crossfade ──────────────────────────────────────
    music = audio
        .crossfade(music, music_yard, 1.0, Curve::EqualPower)
        .expect("crossfade to yard");
    audio.tick(0.5);
    println!(
        "room=yard       music={:?}  (handlefaded from cellblock)  handle={}",
        audio.music_snapshot().clip,
        music.id()
    );

    // ── VO + subtitle ────────────────────────────────────────────────
    let _vo = audio
        .queue_vo(
            vo_red,
            Subtitle {
                speaker: "Red".into(),
                text: "Hope is a dangerous thing.".into(),
                duration_secs: 2.0,
            },
        )
        .expect("VO");
    if let Some(ev) = audio.poll_subtitle() {
        println!("subtitle     [{}] {}", ev.subtitle.speaker, ev.subtitle.text);
    }

    // ── Build save body from "game state" ────────────────────────────
    let scene = AssetId::from_path("rooms/yard");
    let music_snap: MusicSnapshot = audio.music_snapshot();
    let vols: BusVolumes = audio.bus_volumes();

    let body = SaveBody {
        tags: vec![
            "State.NPC.Red.Met".into(),
            "Quest.Escape.Started".into(),
        ],
        vars: vec![
            ("hope".into(), SaveVarValue::I(1)),
            ("player_name".into(), SaveVarValue::S("Andy".into())),
        ],
        inventory: vec![AssetId::from_path("items/rock_hammer")],
        visited_rooms: vec![
            AssetId::from_path("rooms/cellblock"),
            AssetId::from_path("rooms/yard"),
        ],
        dialog_history: vec![],
        music: music_snap.clip.map(|clip| MusicState {
            clip,
            position_secs: music_snap.position_secs,
        }),
        bus_volumes: SaveBusVolumes {
            master: vols.master,
            music: vols.music,
            sfx: vols.sfx,
            voice: vols.voice,
        },
    };

    let root = std::env::temp_dir().join(format!(
        "adventure-engine-06-{}",
        std::process::id()
    ));
    let slot: PathBuf = slot_path(&root, "slot1");
    Saver::new()
        .save_to_slot(&body, scene, None, 42.0, &slot)
        .expect("save slot");
    println!("saved        {}", slot.display());

    // ── Simulate restart: wipe mixer + load ──────────────────────────
    let mut audio2 = NullMixer::new();
    audio2
        .register_pcm(music_cell, 44100, &sine_pcm(220.0, 2.0, 44100))
        .unwrap();
    audio2
        .register_pcm(music_yard, 44100, &sine_pcm(330.0, 2.0, 44100))
        .unwrap();

    let loaded = Loader::new().load_from_slot(&slot).expect("load slot");
    assert_eq!(loaded.header.scene_id, scene);
    assert!(loaded.body.tags.iter().any(|t| t == "State.NPC.Red.Met"));
    assert_eq!(
        loaded.body.vars.iter().find(|(k, _)| k == "hope").map(|(_, v)| v),
        Some(&SaveVarValue::I(1))
    );

    // Restore bus volumes + music bed
    audio2.set_bus_volume(Bus::Master, loaded.body.bus_volumes.master);
    audio2.set_bus_volume(Bus::Music, loaded.body.bus_volumes.music);
    audio2.set_bus_volume(Bus::Sfx, loaded.body.bus_volumes.sfx);
    audio2.set_bus_volume(Bus::Voice, loaded.body.bus_volumes.voice);

    if let Some(ref m) = loaded.body.music {
        let snap = MusicSnapshot {
            clip: Some(m.clip),
            position_secs: m.position_secs,
        };
        audio2.restore_music(&snap).expect("restore music");
    }

    let restored = audio2.music_snapshot();
    println!(
        "restored     scene={:?}  music={:?}  pos={:.2}s  tags={}  inv={}",
        loaded.header.scene_id,
        restored.clip,
        restored.position_secs,
        loaded.body.tags.len(),
        loaded.body.inventory.len()
    );

    assert_eq!(restored.clip, Some(music_yard));
    assert!((audio2.bus_volumes().music - 0.8).abs() < 1e-3);

    // Cleanup temp saves
    let _ = std::fs::remove_dir_all(&root);

    println!("\n✓ Phase 6 exit criteria met:");
    println!("  - music crossfade on room change");
    println!("  - save + restart restores tags/vars/music/bus volumes");
}
