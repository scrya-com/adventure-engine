# Example 08 — Shawshank PAC (Cell Block C)

Playable **MVP spine** of the NCP point-and-click slice on **adventure-engine** (Ariadne).

## Run

```bash
# from adventure-engine repo root
cargo test -p example-08-shawshank-pac
cargo run -p example-08-shawshank-pac -- --headless   # walkthrough only
cargo run -p example-08-shawshank-pac                   # window + dialog UI
```

## Keys

| Key | Action |
| --- | --- |
| `1` | Examine Red's cell |
| `N` | Next day (Andy arrives) |
| `2` | Examine Andy's cell |
| `3` | Talk to Andy (dialog tree) |
| `4` | Use loose stone (after meeting Andy) |
| Click | Hotspot hit-test (normalized) |
| Esc | Quit |

## Assets

| Path | Role |
| --- | --- |
| `assets/scenes/cellblock_c.scene.ron` | Room + hotspots |
| `assets/dialogs/dialogue_andy_first_meeting.dialog.ron` | First meeting tree |
| `examples/06-shawshank-pac/assets/cellblock_bg.png` | Full-bleed room still (windowed) |
| `examples/06-shawshank-pac/assets/portrait_red.png` | Hub portrait after examine Red |
| `examples/06-shawshank-pac/assets/portrait_andy.png` | Hub portrait after next day |
| `NCP_MAPPING.md` | Flag/verb mapping from NCP JSON |

Still-only content pack `examples/06-shawshank-pac/` stays as art dump; **this** example is the binary.

Windowed mode draws `cellblock_bg` full-bleed and hub portraits at the HTML demo
anchors (`port-red` 18%/28%, `port-andy` 28%/28%, width 11%). Missing PNGs are
skipped with a warning — no panic — so `--headless` / CI stays green.
