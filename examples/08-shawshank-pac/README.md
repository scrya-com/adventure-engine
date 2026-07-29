# Example 08 — Shawshank PAC (Cell Block C)

Playable chrome: **status strip**, **LOOK / TALK / USE** verb bar, **NEXT DAY**,
face-cropped hub portraits, door-aligned hotspots, dialog text.

<p align="center">
  <img src="../../docs/assets/readme/shawshank-pac.jpg" alt="Shawshank PAC — Cell Block C with LOOK/TALK/USE chrome" width="100%"/>
</p>

```bash
cargo test -p example-08-shawshank-pac
cargo run -p example-08-shawshank-pac -- --headless
cargo run -p example-08-shawshank-pac -- --playtest /tmp/score.json
cargo run -p example-08-shawshank-pac   # windowed
```

### Play (window)

1. **LOOK** (or `L`) → click **Red's cell** (mugshot / door face)  
2. **NEXT DAY** (or `N`)  
3. **LOOK** → click **Andy's cell**  
4. **TALK** → click Andy → dialog choices  
5. **USE** → **loose stone** (gold glint under Red's door) after dialog  

Shortcuts: `1`–`4` force spine steps. Esc quit.

See `NCP_MAPPING.md`, `PLAYTEST.md` (oracle score + **Holo 3.1** CUA notes).  
Stills: `examples/06-shawshank-pac/assets/`.
