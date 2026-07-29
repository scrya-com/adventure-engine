# NCP → adventure-engine mapping (Cell Block C MVP)

| NCP (`demos/shawshank-pac/room.json`) | Ariadne |
| --- | --- |
| `room` layers / `cellblock_bg.png` | `Scene.rooms.cellblock_c.background` (AssetId of pack PNG path) |
| hotspot `bounds.rect` 2560×1080 | normalized polygon in `Hotspot.polygon` |
| verb `examine` / `talk` / `use` | `HotspotKind` + host `OnClick::Action` |
| `set_flag examined_cell` | tag `State.Flag.ExaminedCell` |
| next day / `andy_arrived` | tag `State.Flag.AndyArrived` (key `N`) |
| `noticed_andy` / `can_talk_andy` | `State.Flag.NoticedAndy` / `CanTalkAndy` |
| `dialogue_andy_first_meeting` | `assets/dialogs/dialogue_andy_first_meeting.dialog.ron` |
| exit `met_andy` + HTML `knows_red_business` | `on_enter` on exit node → both tags |
| `hs_contraband_spot` reveal | host hides until `State.Flag.KnowsRedBusiness` |
| `change_room` red cell interior | **not in MVP** (planned second room) |

HTML pack remains smoke/reference. Golden path: `cargo test -p example-08-shawshank-pac`.


Canonical paths: `adventure_state::flag_paths::shawshank`.
Windowed host draws `examples/06-shawshank-pac/assets` bg + portraits.
