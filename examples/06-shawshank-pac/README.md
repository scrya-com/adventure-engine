# Example 06 — Shawshank PAC content pack

Hub-style stills for the **Shawshank** point-and-click slice (Cell Block C).

This folder is a **content pack**, not a full Rust binary yet. Verb bar +
inventory land in Phase 7 (`docs/ROADMAP.md`). Until then, the playable demo is
the HTML runner in the NCP repo:

```text
../ncp/demos/shawshank-pac/   (sibling checkout)
```

```bash
cd ../ncp/demos/shawshank-pac   # or your path to johndpope/ncp
python3 -m http.server 8765
# open http://127.0.0.1:8765/
```

## Assets here

| File | Use |
| --- | --- |
| `assets/cellblock_bg.png` | Room background |
| `assets/portrait_red.png` | Red (Ellis) hub portrait |
| `assets/portrait_andy.png` | Andy hub portrait |
| `assets/portrait_warden.png` | Warden hub portrait |

Portraits are **fictional** inmate designs for demo/dev — not celebrity likeness.

## Future Rust example

When Phase 7 verbs land, wire these assets + the dialogue tree from
`ncp/examples/vncp-shawshank-multiformat.json` (`point_and_click` →
`dialogue_andy_first_meeting`) into a `cargo run -p example-06-shawshank-pac`
binary that uses `adventure-dialogue` + hotspot picking (examples 03 + 05).
