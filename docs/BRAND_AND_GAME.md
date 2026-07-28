# Ariadne — brand & flagship game brief

**Source conversation:** [Grok share](https://grok.com/share/bGVnYWN5_f2bf511a-b1b4-4210-bb58-9757556edea8)  
**Product site:** https://scrya.com/ariadne  
**Repo:** https://github.com/scrya-com/adventure-engine  
**Status:** brand + game concept locked from voice session (2026-07); implement in slices

---

## Pronunciation

**Ariadne** → *air-ee-AD-nee* (four syllables; stress on **AD**).

Not “AVAL” (that’s the motion format). Ariadne is the **engine** and the **hero brand**.

---

## Myth core (keep this simple)

| Element | Meaning for Scrya |
| --- | --- |
| Ariadne | Daughter of Minos; Cretan root ~ “utterly pure”; **weaver / guide** |
| The thread | Path through chaos — walk graphs, flags, dialog branches, player agency |
| The Labyrinth | The world players must navigate (room graph, parkour maze, society’s maze) |
| Not just helper | **Divine feminine** craft: intuition, clarity, sovereignty — not a sidekick tool |

**One-line brand:** *The thread that weaves worlds.*

**Alternate taglines:**
- Thread through the labyrinth.
- Weave order from chaos.
- Your path. Your world.

---

## Brand voice & visual (from session)

### Voice
- Mythic but modern — no dusty textbook Greek.
- Empowering, not preachy: self-sovereignty, craft over gatekeepers.
- Feminine divine as **strength of clarity**, not soft-only aesthetics.
- Chaos is raw material; Ariadne **organizes** it into a playable path.

### Visual system
| Token | Direction |
| --- | --- |
| Primary motif | Glowing **thread** that forms a stylized **A** |
| Secondary | Labyrinth geometry, pixel nodes on the thread (ties to Scene/PAC) |
| Palette | Deep teal + gold (mythic) on charcoal (Scrya monochrome base) |
| Logo idea | Thread → A; optional labyrinth watermark; works 1-color |
| Sister brand | Scrya crystal / goddess craft stays; Ariadne is the **runtime / game** face |

### Logo production notes (for Imagine / designer)
- Glowing golden thread coiling into letter **A**
- Optional subtle maze in negative space
- Teal ambient glow, gold core line
- Flat + isometric variants for site and engine docs
- No celebrity faces, no temple-stock clichés unless intentional Santorini set

---

## Flagship game: **Ariadne’s Thread**

### Elevator pitch
Temple Run–style endless / staged **parkour chase**, but you play **Ariadne the weaver-goddess**.  
The Labyrinth is a living maze of power and illusion. Your **thread** is both movement tool and **truth-vision** (*They Live* filter): hold it and the world’s hidden commands appear — OBEY, CONSUME, SUBMIT — so parkour is also **revelation**.

### Fantasy stack (locked preferences from session)
1. **Parkour / mountaineering** — use existing **mountaineering action pack** (vault, slide, climb, leap).
2. **Truth-reveal** — thread-vision reveals tyrant script on walls/NPCs.
3. **Greek sacred geography** — lean **Santorini / Cyclades / Delphi**: whitewashed cliffs, deep blue sea, cave oracles, sun-bleached temples — *not* generic data-center cyber (that was a draft only).
4. **Lara Croft agility × divine weaver** — explorer body language, goddess purpose.

### Core loop
```
Sprint / parkour through stage
  → weave thread (place path, swing, reveal)
  → chase pressure (drones / beasts / enforcers)
  → optional oracle shrines (story beats, upgrades)
  → stage exit / boss thread duel
```

### Stage seeds (from session — rewrite outdoor/Greek)

| Stage | Working title | Beat |
| --- | --- | --- |
| 1 | **Cliff of the Watchers** (was Surveillance Spire) | Neon → **sun-bleached cliff cameras** as “eyes of Minos”; vault under collapsing white walls; first thread-vision reveal |
| 2 | **Oracle Vault** (was Fractured Archive) | Cave shrine under Santorini cliff; falling **marble / data-stele** hybrid pillars; **echo thread** (ghost of your last path) |
| 3 | **Overgrown Labyrinth** | Outdoor vine-maze on ancient stone; beasts as tyrant-minions; open sky + vertical vines |

### Systems to wire (engine + Scrya)

| System | How |
| --- | --- |
| Character | Hub → AVAL isometric / full-body action pack (mountaineering) |
| Locomotion | Ariadne walk graphs + parkour states (vault, slide, climb) |
| Thread ability | Meta-action / item: place path, swing, **vision mode** shader/overlay |
| Verbs later | Look/Use on oracles, pickups (thread reels, amulets) |
| Web slice | `/scene/?play=1` room demos; later dedicated `ariadnes-thread` route |
| Native | `scrya-com/adventure-engine` examples |

### What we are **not**
- Not Crystal Skull Run / generic skull flash games  
- Not pure cyberpunk data-center (optional skin only)  
- Not renaming AVAL — AVAL stays the **motion format**; Ariadne is engine + game brand  

---

## Scrya platform alignment

```
Scrya (platform)     companions, packs, SuperGrok hub workflow
   │
   ├─ AVAL           motion format (.avl)
   ├─ Scene (web)    isometric playable rooms today
   └─ Ariadne        native Rust adventure runtime + game brand
          └─ Ariadne’s Thread   flagship parkour/adventure IP
```

**Story sell:** divine feminine craft + self-sovereignty = why creators *and* players care.  
**Tech sell:** hub → packs → AVAL → Ariadne/Scene = the pipeline we already ship pieces of.

---

## Implementation slices (recommended order)

1. **Brand page polish** — pronunciation, tagline, palette, logo brief on scrya.com/ariadne  
2. **Logo / OG art** — thread-A in teal+gold (Imagine + brand pack)  
3. **Vertical slice prototype** — one Santorini cliff corridor in Scene or Ariadne example: sprint + 2 parkour states + thread-vision toggle (even if vision is caption/UI first)  
4. **Mountaineering pack → AVAL** — real vault/slide/climb clips on a Greek explorer hub (not celebrity)  
5. **Stage 1 design doc** — prop list, flag list, chase timing, oracle shrine  
6. **Audio mood** — lyre-electronic hybrid, wind + surf + distant sirens  

---

## Voice checklist (copy)

- Prefer: thread, weave, labyrinth, path, clarity, sovereignty, oracle, cliff, sea  
- Avoid: “just another game engine,” “Unreal killer,” cynical techbro tone  
- Product honesty: SuperGrok for Imagine; Scrya for pipeline + runtime  

---

## Open decisions

| Decision | Options | Lean |
| --- | --- | --- |
| Game moniker | *Ariadne’s Thread* vs *Thread* vs *Labyrinth* | **Ariadne’s Thread** |
| Perspective | 3rd-person runner vs isometric PAC chase | Hybrid: **isometric PAC for demos**, runner camera later |
| Multiplayer | none / async ghosts (echo thread) | **Echo thread** single-player first |
| Monetization | free demo + Developer tools | Demo free; tools via Scrya Developer |

---

*Follow-up from Grok voice brand session. Update this file when logo and Stage 1 art lock.*
