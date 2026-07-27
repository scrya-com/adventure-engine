# Save System — Versioned Header + Migrations

> Deep dive on `crates/save/`. See `docs/DESIGN.md` for the high-level rationale.

## UE 5.8 reference

Unreal's `USaveGame` (Engine/Classes/GameFramework/SaveGame.h) + the save header (Engine/Private/GameplayStatics.cpp:89-103):

```
FSaveGameHeader {
    FileTypeTag         = 0x53415647;  // 'SAVG'
    SaveGameFileVersion;
    PackageFileUEVersion;
    SavedEngineVersion;
    CustomVersionFormat;
    CustomVersions;     // FCustomVersionContainer — per-subsystem version GUIDs
    SaveGameClassName;  // stored so loader can LoadObject<UClass> the right subclass
}
```

`SaveGameToSlot` / `LoadGameFromSlot` / `SaveGameToMemory` (GameplayStatics.h:1127-1211).

**What we keep:** magic + integer bumps + `FCustomVersionContainer` for per-subsystem migration + schema name.

**What we skip:** UE version compatibility surface, `UClass` reflection-driven field serialization.

## Header

```rust
use serde::{Serialize, Deserialize};
use smallvec::SmallVec;

/// File type magic. ASCII 'SAVG' as little-endian u32.
pub const SAVE_MAGIC: u32 = 0x53415647;

/// Schema name. Bumped when the on-disk layout changes incompatibly.
/// Old saves with a different schema name are rejected.
pub const SCHEMA_NAME: &str = "adventure-save-v1";

/// GUID for each subsystem that has independent versioning needs.
/// Bump the version number when the subsystem's persisted shape changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CustomVersionGuid([u8; 16]);

pub const GUID_TAGS:        CustomVersionGuid = CustomVersionGuid(uuid::uuid!("..."));
pub const GUID_INVENTORY:   CustomVersionGuid = CustomVersionGuid(uuid::uuid!("..."));
pub const GUID_VARS:        CustomVersionGuid = CustomVersionGuid(uuid::uuid!("..."));
pub const GUID_DIALOG:      CustomVersionGuid = CustomVersionGuid(uuid::uuid!("..."));
pub const GUID_SCRIPTING:   CustomVersionGuid = CustomVersionGuid(uuid::uuid!("..."));

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveHeader {
    pub magic: u32,
    pub engine_semver: (u16, u16, u16),
    pub content_version: u32,
    pub schema_name: SmolStr,
    pub custom_versions: SmallVec<[(CustomVersionGuid, u32); 8]>,
    pub saved_at_unix: i64,
    pub playtime_secs: f64,
    pub scene_id: AssetId,
    pub node_id: Option<u32>,
}
```

## Body

Serialized with `rmp-serde` (MessagePack). Binary, compact (~10× smaller than JSON), schema-flexible.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveBody {
    pub tags: Vec<String>,                          // hierarchical tag strings
    pub vars: Vec<(String, VarValue)>,              // typed variable table
    pub inventory: Vec<AssetId>,                    // item IDs
    pub visited_rooms: Vec<AssetId>,                // for "have been here before" checks
    pub dialog_history: Vec<DialogHistoryEntry>,    // last N lines
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t", content = "v")]
pub enum VarValue {
    I(i64),
    F(f64),
    S(String),
    B(bool),
    Asset(AssetId),
}
```

## File layout

```
+----------------------+
| SaveHeader (msgpack) |
+----------------------+
| SaveBody    (msgpack)|
+----------------------+
| optional thumb PNG   |  <- screenshot for save slot UI
+----------------------+
| footer: SHA1 of above|  <- integrity check
+----------------------+
```

## Migrations

When `SaveBody`'s shape changes, we add a migration function:

```rust
// crates/save/src/migrations.rs
pub fn migrate(
    header: &SaveHeader,
    body: &mut serde_json::Value,  // migrate via JSON intermediate
) -> Result<(), SaveError> {
    let tags_v = header.custom_version_of(GUID_TAGS);
    if tags_v < 2 {
        migrate_tags_v1_to_v2(body)?;
    }
    let inv_v = header.custom_version_of(GUID_INVENTORY);
    if inv_v < 3 {
        migrate_inventory_v2_to_v3(body)?;
    }
    Ok(())
}
```

The migration functions take a JSON-intermediate representation (decoded from msgpack) and patch it up. After migration, re-serialize to the current `SaveBody` Rust struct.

## Public API

```rust
pub struct Saver {
    engine_semver: (u16, u16, u16),
    custom_versions: SmallVec<[(CustomVersionGuid, u32); 8]>,
}

impl Saver {
    pub fn save(&self, snapshot: &SaveBody, scene: AssetId, node: Option<u32>) -> Result<Vec<u8>, SaveError>;
    pub fn save_to_slot(&self, snapshot: &SaveBody, slot: &Path) -> Result<(), SaveError>;
}

pub struct Loader {
    engine_semver_compatible: fn((u16, u16, u16)) -> bool,
}

impl Loader {
    pub fn load(&self, bytes: &[u8]) -> Result<LoadedSave, SaveError>;
}

pub struct LoadedSave {
    pub header: SaveHeader,
    pub body: SaveBody,
}

#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error("wrong magic: {0:#x}")]
    BadMagic(u32),
    #[error("schema mismatch: file has {file}, engine expects {engine}")]
    SchemaMismatch { file: String, engine: String },
    #[error("engine version too old: file {file:?}, engine supports up to {engine:?}")]
    EngineTooOld { file: (u16, u16, u16), engine: (u16, u16, u16) },
    #[error("migration failed at subsystem {subsystem:?} from v{from} to v{to}: {reason}")]
    MigrationFailed { subsystem: CustomVersionGuid, from: u32, to: u32, reason: String },
    #[error("deserialize error: {0}")]
    Deserialize(#[from] rmp_serde::decode::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("sha1 mismatch")]
    Corrupted,
}
```

## What we save (and don't)

**Save:**
- Tags (state flags)
- Inventory items
- Variable table (integers, floats, strings, bools, asset refs)
- Current scene_id + dialog node_id
- Playtime accumulated
- Save timestamp
- Optional screenshot thumbnail

**Don't save (regenerate from scene on load):**
- Walker positions (re-derived from spawn + saved scene state)
- Animation state (re-derived from walker)
- Currently playing SFX (one-shots, by definition)
- Loaded assets (loader re-resolves)
- Cursor state (UI re-initializes)

**Special-case music:** Music state IS saved (current track + position + bus volumes) so the player doesn't get a jarring silence on load.

## Slot layout

```
saves/
├── slot1.bin
├── slot1.png          <- thumbnail
├── slot1.meta.json    <- human-readable metadata for save browser UI
├── slot2.bin
├── ...
└── autosave.bin
```

The `.meta.json` is a small sidecar so the save browser UI doesn't need to deserialize the full save just to show "Slot 1 — Forest Clearing — 2:34:15".

## Versioning discipline

- **Bump `engine_semver`** on every engine release (per semver rules).
- **Bump a `custom_versions` entry** when the corresponding subsystem's persisted shape changes. Write a migration function in the same PR.
- **Bump `SCHEMA_NAME`** only when an incompatible top-level change happens (rare — should be accompanied by an ADR).

## What we skip

- UObject reflection-driven field serialization (we use explicit serde structs)
- Per-platform save packaging (one format, all platforms)
- `FCustomVersionContainer`'s full GUID registry (we have 5 subsystems, not 50)
- Save game class name field (we don't have UClass)
