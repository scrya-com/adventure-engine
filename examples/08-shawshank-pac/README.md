# Example 08 — Shawshank PAC (Cell Block C)

Playable chrome: **status strip**, **LOOK / TALK / USE** verb bar, **NEXT DAY**,
bright hotspots with hover labels, portraits, dialog text.

```bash
cargo test -p example-08-shawshank-pac
cargo run -p example-08-shawshank-pac -- --headless
cargo run -p example-08-shawshank-pac -- --playtest /tmp/score.json
cargo run -p example-08-shawshank-pac   # windowed
```

### Play (window)

1. **LOOK** (or `L`) → click **Red's cell** (blue glow)  
2. **NEXT DAY** (or `N`)  
3. **LOOK** → click **Andy's cell**  
4. **TALK** → click Andy → dialog choices  
5. **USE** → **loose stone** (gold) after dialog  

Shortcuts: `1`–`4` force spine steps. Esc quit.

See `NCP_MAPPING.md`, `PLAYTEST.md` (oracle score + Fara-7B notes).  
Stills: `examples/06-shawshank-pac/assets/`.
