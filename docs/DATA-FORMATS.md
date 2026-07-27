# Data Formats — RON Schemas

> Authoritative schemas for all data files. RON is the *only* format for authored data (scenes, dialog, manifest). MessagePack is used only for `save/` (binary, compact). JSON is supported as a load-time fallback for fixtures only.

## Why RON

- **Readability** — designers edit it in any text editor.
- **Comments** — `// explain what this hotspot does`.
- **Trailing commas** — diff-friendly.
- **Typed** — Rust `serde` derives round-trip cleanly.
- **Hot-reloadable** — `notify` watches the file; on save, re-parse and swap.

JSON lacks comments and trailing commas. YAML's whitespace-significance makes large diffs painful. TOML is great for config but bad for nested structures. RON wins for game data.

## Top-level schemas

### Scene — `assets/scenes/<name>.scene.ron`

```ron
Scene((
    name: "forest_clearing",
    entry_room: "clearing",
    rooms: {
        "clearing": Room((
            background: "bg/clearing.webp",
            walk_graph: WalkGraph(/* see below */),
            hotspots: [
                Hotspot((
                    id: "hotspot_door",
                    kind: WalkTo,                     // WalkTo | Examine | Use | Talk | Pickup
                    polygon: [(0.4, 0.6), (0.5, 0.6), (0.5, 0.8), (0.4, 0.8)],
                    cursor: "door",                   // optional cursor override
                    facing: East,                     // required for WalkTo
                    on_click: Action("enter_cottage"),
                )),
                Hotspot((
                    id: "hotspot_sign",
                    kind: Examine,
                    polygon: [(0.1, 0.4), (0.15, 0.4), (0.15, 0.5), (0.1, 0.5)],
                    on_click: Action("examine_sign"),
                )),
            ],
            props: [
                Prop((
                    id: "prop_barrel",
                    sprite: "props/barrel",
                    transform: Transform2D((pos: (0.3, 0.7), scale: (1.0, 1.0))),
                    layer: 10,
                )),
            ],
            spawns: {
                "player": Spawn(( point: "entry", facing: East )),
            },
            ambient_music: Some("music/forest_ambient"),
            ambient_sfx: Some("sfx/forest_birds"),
        )),
        "cottage": Room((/* ... */)),
    },
    transitions: {
        // (room_from, hotspot_id) -> (room_to, spawn_point)
        ("clearing", "enter_cottage"): Transition(( to: "cottage", spawn: "cottage_entry" )),
    },
))
```

### WalkGraph — embedded in Scene

Coordinates are normalized (0..1, y-down). `depth` is 0 near, 1 far — used for perspective scale.

```ron
WalkGraph((
    nodes: [
        Node(( id: "entry",       pos: (0.2, 0.9), depth: 0.0, kind: Floor )),
        Node(( id: "door_pad",    pos: (0.45, 0.7), depth: 0.3, kind: Approach(target_hotspot: "hotspot_door") )),
        Node(( id: "sign_pad",    pos: (0.125, 0.45), depth: 0.5, kind: Approach(target_hotspot: "hotspot_sign") )),
        Node(( id: "barrel_pad",  pos: (0.3, 0.7), depth: 0.3, kind: Approach(target_prop: "prop_barrel") )),
        Node(( id: "center",      pos: (0.5, 0.85), depth: 0.05, kind: Floor )),
    ],
    edges: [
        Edge(( a: "entry", b: "center" )),
        Edge(( a: "center", b: "door_pad" )),
        Edge(( a: "center", b: "sign_pad" )),
        Edge(( a: "center", b: "barrel_pad" )),
    ],
))
```

This maps directly to `crates/locomotion/src/walk_graph.rs::WalkGraph`.

### Dialog tree — `assets/dialogs/<name>.dialog.ron`

```ron
DialogTree((
    root: "intro",
    nodes: {
        "intro": DialogNode((
            speaker: "bob",
            portrait: Some("portraits/bob_happy"),
            text: "Hello there, stranger.",
            choices: [
                Choice((
                    text: "Who are you?",
                    next: "intro_name",
                    when: "has_tag(\"State.Player.KnowsBob\") == false",
                )),
                Choice((
                    text: "Goodbye.",
                    next: "exit",
                )),
            ],
            on_enter: [
                "set_var(\"met_bob\", true)",
                "give_tag(\"State.NPC.Bob.Met\")",
            ],
        )),
        "intro_name": DialogNode((
            speaker: "bob",
            text: "Name's Bob. I work the mill.",
            next: "intro",
            on_enter: ["give_tag(\"State.Player.KnowsBob\")"],
        )),
        "exit": DialogNode((
            speaker: "bob",
            text: "Take care.",
            terminal: true,
        )),
    },
))
```

See `docs/SCRIPTING.md` for the Rhai integration.

### Items — `assets/items/<name>.item.ron`

```ron
Item((
    id: "key_cellar",
    display_name: "Cellar Key",
    description: "A heavy iron key, cold to the touch.",
    icon: "icons/key_cellar",
    verbs: [
        Verb(( kind: Look,  text: "Examine",      action: "examine_key_cellar" )),
        Verb(( kind: Use,   text: "Use on",       requires_target: true )),
        Verb(( kind: UseOn, matches: "lock_cellar", action: "unlock_cellar" )),
    ],
))
```

### Manifest — `assets/manifest.toml` (dev) or `manifest.bin` (ship)

Dev form is TOML for git-friendliness. Ship form is the same data serialized via `bincode`.

```toml
# Generated by tools/packer at cook time, or hand-maintained in dev.
[assets.backgrounds]
"bg/clearing" = { path = "backgrounds/clearing.webp", sha1 = "...", size = 1048576 }

[assets.sprites]
"props/barrel" = { path = "sprites/barrel.webp", sha1 = "...", size = 32768, deps = [] }

[assets.audio]
"music/forest_ambient" = { path = "audio/forest_ambient.ogg", sha1 = "...", size = 2097152 }

[assets.scenes]
"scenes/clearing" = { path = "scenes/clearing.scene.ron", sha1 = "...", deps = [
    "bg/clearing", "props/barrel", "music/forest_ambient",
]}
```

### Save — `saves/slot<N>.bin`

Binary, not RON. See `docs/SAVE.md`.

## Naming conventions

- **Asset IDs** are path-like: `bg/clearing`, `sprites/walk_n`, `sfx/door_open`. Always lowercase, underscores, no spaces, no extension (extension implied by kind).
- **Tags** are hierarchical, dot-separated, PascalCase per segment: `State.NPC.Bob.Met`, `State.Door.Cellar.Locked`.
- **Scene IDs** are asset IDs: `scenes/clearing`.
- **Dialog IDs** within a tree are snake_case: `intro`, `intro_name`, `ask_about_mill`.
- **Hotspot IDs** within a room are snake_case with `hotspot_` prefix: `hotspot_door`, `hotspot_sign`.

## Coordinate system

- **Background space** (in scenes): `(x, y)` both in `[0.0, 1.0]`, `y` down. `depth` in `[0.0, 1.0]`, 0 near camera, 1 far.
- **Screen space** (in render2d): pixels, `y` down, origin top-left. `(0, 0)` is top-left of window.
- **World space** (in ECS transforms): same as background space by convention.
- **Atlas UVs**: `[0.0, 1.0]`, origin top-left of atlas image.

Conversion: `screen = background * window_size`. Perspective scale: `scale = mix(1.0, 0.5, depth)`.

## Validation

`crates/assets/` includes a validator that runs at cook time and on every manifest update:

- All `AssetId` references resolve.
- All polygons have at least 3 points and are non-degenerate.
- All `WalkGraphNode::Approach` targets exist.
- All `Transition` targets reference valid rooms + spawns.
- All Rhai `when` and `on_enter` snippets compile.
- All dialog `next` references point to existing nodes.

Errors block the cook or trigger a watcher notification.

## Migration discipline

When a schema changes:
1. Add a new optional field (default value at deserialize time).
2. Bump the corresponding `custom_version` GUID in `crates/save/`.
3. Write a migration in `crates/save/src/migrations.rs` for the rare case where saves need transforming.
4. Update `tools/packer` if the manifest format changes.

Schema changes are recorded in `docs/DECISIONS/` if they break existing scenes.
