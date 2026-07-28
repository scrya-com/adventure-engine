# Product name: Ariadne

**Public product name:** [Ariadne](https://scrya.com/ariadne)  
**Repo directory (historical):** `adventure-engine`  
**Tagline:** Thread through the labyrinth.

## Why not “adventure-engine”

Generic adventure-game naming doesn’t match Scrya’s monochrome isometric
**goddess craft** brand. Ariadne is the mythic figure who gives the *thread*
that guides Theseus through the maze — the same idea as:

- walk graphs + plant FSM (feet stay on the thread)
- hotspot / flag paths through a room
- dialog trees that branch without losing the way back

## Mapping

| Old / internal | Public |
| --- | --- |
| adventure-engine (folder) | Ariadne |
| example-0N-* | Ariadne examples |
| Flutter `/scene/` | Scrya Scene (web sister) |

Crate names may keep `adventure-*` paths until a coordinated rename; marketing
and docs use **Ariadne** only.

## Live surfaces

- Product docs: https://scrya.com/ariadne  
- Web demo: https://beta.scrya.com/scene/?play=1&room=shawshank  
- Source: https://github.com/scrya-com/adventure-engine  

```bash
git clone git@github.com:scrya-com/adventure-engine.git
cd adventure-engine
cargo build --workspace
cargo test --workspace
cargo run -p example-05-dialog
```

