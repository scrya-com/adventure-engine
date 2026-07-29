//! Phase 7 — Inventory + verbs demo (headless).
//!
//! Proves the Phase 7 exit criteria without a window/GPU:
//! 1. Load item RON fixtures + combine table.
//! 2. Pick up items into an inventory bag.
//! 3. Look (examine) an item.
//! 4. Use/combine oil + lamp → lit_lamp (success).
//! 5. Fail-closed combine (rock + oil).
//! 6. Verb coin + inventory bar hit-tests.
//!
//! ```text
//! cargo run -p example-07-inventory
//! ```

use std::path::{Path, PathBuf};

use adventure_core::math::Vec2;
use adventure_inventory::{
    CombineTable, Inventory, InventoryBar, Item, ItemCatalog, VerbCoin, VerbKind,
};

fn workspace_path(rel: &str) -> PathBuf {
    let candidates = [
        PathBuf::from(rel),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../").join(rel),
    ];
    candidates
        .into_iter()
        .find(|p| p.is_file())
        .unwrap_or_else(|| panic!("{rel} not found — run from repo root or workspace"))
}

fn load_item(path: &Path) -> Item {
    let src = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    Item::from_ron(&src).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

fn main() {
    println!("=== Phase 7: inventory + verbs ===\n");

    // ── Catalog from fixtures ────────────────────────────────────────
    let mut catalog = ItemCatalog::new();
    for name in ["key_cellar", "oil", "lamp", "lit_lamp"] {
        let path = workspace_path(&format!("assets/items/{name}.item.ron"));
        let item = load_item(&path);
        println!(
            "loaded item  id={}  name={:?}  verbs={}",
            item.id,
            item.display_name,
            item.verbs.len()
        );
        catalog.insert(item);
    }

    let table_src = std::fs::read_to_string(workspace_path("assets/items/combine_table.ron"))
        .expect("read combine_table.ron");
    let table = CombineTable::from_ron(&table_src).expect("parse combine table");
    println!("loaded combine table  rules={}\n", table.rules.len());

    // ── World props → pickup ─────────────────────────────────────────
    let mut inv = Inventory::with_capacity(8);
    let key = catalog.get("key_cellar").expect("key").clone();
    let oil = catalog.get("oil").expect("oil").clone();
    let lamp = catalog.get("lamp").expect("lamp").clone();

    assert!(key.can_pickup);
    inv.add(&key).expect("pickup key");
    inv.add(&oil).expect("pickup oil");
    inv.add(&lamp).expect("pickup lamp");
    println!(
        "pickup       slots={}  [key, oil, lamp]",
        inv.len()
    );
    assert!(inv.has_any("key_cellar"));
    assert!(inv.has_any("oil"));
    assert!(inv.has_any("lamp"));

    // ── Look ─────────────────────────────────────────────────────────
    let look = key.look_text();
    println!("look key     \"{look}\"");
    assert!(look.contains("iron key"));

    // UseOn binding on key → lock_cellar
    let use_on = key.verb_for_target("lock_cellar").expect("UseOn lock");
    println!(
        "use key on   lock_cellar → action={:?}",
        use_on.action.as_ref().map(|s| s.as_str())
    );
    assert_eq!(use_on.action.as_ref().unwrap().as_str(), "unlock_cellar");

    // ── Combine success ──────────────────────────────────────────────
    let r = table
        .apply(&mut inv, "oil", "lamp", &catalog)
        .expect("combine apply");
    assert!(r.is_success());
    println!(
        "combine ok   oil + lamp → {:?}  msg={:?}",
        r.is_success().then(|| "lit_lamp"),
        r.message()
    );
    assert!(!inv.has_any("oil"));
    assert!(!inv.has_any("lamp"));
    assert!(inv.has_any("lit_lamp"));
    assert!(inv.has_any("key_cellar")); // untouched

    // ── Fail closed ──────────────────────────────────────────────────
    inv.add_id("rock").expect("add rock");
    let fail = table
        .apply(&mut inv, "rock", "key_cellar", &catalog)
        .expect("fail combine");
    assert!(!fail.is_success());
    println!("combine fail rock + key → \"{}\"", fail.message());
    assert!(inv.has_any("rock"));
    assert!(inv.has_any("key_cellar"));

    // ── Verb coin hit-test ───────────────────────────────────────────
    let coin = VerbCoin::standard(Vec2::new(400.0, 300.0));
    let top = Vec2::new(400.0, 300.0 - 40.0);
    let hit = coin.hit_test(top);
    println!(
        "verb coin    center=(400,300)  click top → {:?}",
        hit.map(|v| v.label())
    );
    assert_eq!(hit, Some(VerbKind::Look));
    assert_eq!(coin.hit_test(Vec2::new(400.0, 300.0)), None); // dead zone

    // ── Inventory bar ────────────────────────────────────────────────
    let bar = InventoryBar::new(Vec2::new(16.0, 520.0), 48.0, 8.0, inv.len());
    let first = bar.hit_test(Vec2::new(20.0, 530.0));
    println!(
        "inv bar      slots={}  click first cell → {:?}",
        inv.len(),
        first
    );
    assert_eq!(first, Some(0));

    // ── Final inventory snapshot ─────────────────────────────────────
    print!("inventory    ");
    for (i, slot) in inv.slots().enumerate() {
        if i > 0 {
            print!(", ");
        }
        print!("{}×{}", slot.id, slot.count);
    }
    println!();

    println!("\n✓ Phase 7 exit criteria met:");
    println!("  - pick up items into inventory");
    println!("  - look / examine description");
    println!("  - use/combine oil + lamp → lit_lamp");
    println!("  - fail-closed unknown combine");
    println!("  - verb coin + inventory bar hit-tests");
}
