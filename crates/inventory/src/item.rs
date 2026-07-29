//! [`Item`] definition + per-item verb bindings.
//!
//! Authored as `assets/items/<name>.item.ron` (see `docs/DATA-FORMATS.md`).

use adventure_core::{AssetId, SmolStr};
use serde::{Deserialize, Serialize};

use crate::error::InventoryError;
use crate::verb::VerbKind;

/// One verb entry on an item (Look / Use / UseOn / …).
///
/// Matches the DATA-FORMATS nested `Verb(( kind: Look, text: "...", ... ))`
/// shape. Unknown optional fields default sensibly.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemVerb {
    /// Which verb this entry describes.
    pub kind: VerbKind,
    /// Player-facing label override (defaults to [`VerbKind::label`]).
    #[serde(default)]
    pub text: Option<SmolStr>,
    /// Named action id dispatched when this verb fires (no target).
    #[serde(default)]
    pub action: Option<SmolStr>,
    /// When true, Use requires a second click target (item / hotspot).
    #[serde(default)]
    pub requires_target: bool,
    /// For [`VerbKind::UseOn`]: the target id this verb matches.
    #[serde(default)]
    pub matches: Option<SmolStr>,
}

impl ItemVerb {
    /// Build a simple Look / action verb.
    pub fn look(action: impl Into<SmolStr>) -> Self {
        Self {
            kind: VerbKind::Look,
            text: Some("Examine".into()),
            action: Some(action.into()),
            requires_target: false,
            matches: None,
        }
    }

    /// Build a Use that requires a target.
    pub fn use_with_target() -> Self {
        Self {
            kind: VerbKind::Use,
            text: Some("Use on".into()),
            action: None,
            requires_target: true,
            matches: None,
        }
    }

    /// Build an explicit UseOn binding for a target id.
    pub fn use_on(target: impl Into<SmolStr>, action: impl Into<SmolStr>) -> Self {
        Self {
            kind: VerbKind::UseOn,
            text: None,
            action: Some(action.into()),
            requires_target: true,
            matches: Some(target.into()),
        }
    }

    /// Display label (override or verb default).
    pub fn label(&self) -> &str {
        self.text
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or_else(|| self.kind.label())
    }
}

/// An inventory / world item definition.
///
/// The `id` is the stable authoring key (e.g. `"key_cellar"`). Soft
/// asset references use [`Item::asset_id`] / [`Item::icon_id`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    /// Stable item id (path-like, no extension): `"key_cellar"`.
    pub id: SmolStr,
    /// Player-facing name.
    pub display_name: SmolStr,
    /// Examine text (Look verb default).
    pub description: SmolStr,
    /// Icon asset path (e.g. `"icons/key_cellar"`), optional.
    #[serde(default)]
    pub icon: Option<SmolStr>,
    /// Gameplay tags attached to this definition (not inventory state).
    #[serde(default)]
    pub tags: Vec<SmolStr>,
    /// Available verbs / actions.
    #[serde(default)]
    pub verbs: Vec<ItemVerb>,
    /// Whether this item may appear as a combine source/target.
    #[serde(default = "default_true")]
    pub can_combine: bool,
    /// Whether the world prop can be picked up.
    #[serde(default = "default_true")]
    pub can_pickup: bool,
    /// Whether multiple copies stack in one slot.
    #[serde(default)]
    pub stackable: bool,
    /// Max stack size when stackable (ignored when not stackable; treated as 1).
    #[serde(default = "default_one")]
    pub max_stack: u32,
}

fn default_true() -> bool {
    true
}

fn default_one() -> u32 {
    1
}

impl Item {
    /// Construct a minimal item (Look-only, pickupable, combinable).
    pub fn new(
        id: impl Into<SmolStr>,
        display_name: impl Into<SmolStr>,
        description: impl Into<SmolStr>,
    ) -> Self {
        let id = id.into();
        Self {
            id: id.clone(),
            display_name: display_name.into(),
            description: description.into(),
            icon: None,
            tags: Vec::new(),
            verbs: vec![ItemVerb::look(format!("examine_{id}"))],
            can_combine: true,
            can_pickup: true,
            stackable: false,
            max_stack: 1,
        }
    }

    /// Soft asset id for this item (`items/<id>`).
    pub fn asset_id(&self) -> AssetId {
        AssetId::from_path(&format!("items/{}", self.id))
    }

    /// Soft asset id for the icon, if any.
    pub fn icon_id(&self) -> Option<AssetId> {
        self.icon.as_ref().map(|p| AssetId::from_path(p.as_str()))
    }

    /// Effective max stack (always ≥ 1).
    pub fn effective_max_stack(&self) -> u32 {
        if self.stackable {
            self.max_stack.max(1)
        } else {
            1
        }
    }

    /// Whether this item offers the given verb kind.
    pub fn has_verb(&self, kind: VerbKind) -> bool {
        self.verbs.iter().any(|v| v.kind == kind)
            || matches!(kind, VerbKind::Look) // Look always available via description
    }

    /// First verb entry matching `kind`, if any.
    pub fn verb(&self, kind: VerbKind) -> Option<&ItemVerb> {
        self.verbs.iter().find(|v| v.kind == kind)
    }

    /// Resolve UseOn / Use-with-target against a target id.
    ///
    /// Returns the matching [`ItemVerb`] when an explicit `UseOn` matches,
    /// or a `Use` entry with `requires_target` when no UseOn matches but
    /// generic Use is allowed (caller still needs a combine table for recipes).
    pub fn verb_for_target(&self, target_id: &str) -> Option<&ItemVerb> {
        self.verbs
            .iter()
            .find(|v| {
                v.kind == VerbKind::UseOn
                    && v.matches
                        .as_ref()
                        .map(|m| m.as_str() == target_id)
                        .unwrap_or(false)
            })
            .or_else(|| {
                self.verbs
                    .iter()
                    .find(|v| v.kind == VerbKind::Use && v.requires_target)
            })
    }

    /// Look text: description field.
    pub fn look_text(&self) -> &str {
        self.description.as_str()
    }

    /// Parse a RON string into an [`Item`].
    ///
    /// Accepts bare `(...)` (dialogue-style) or the DATA-FORMATS
    /// newtype form `Item(( ... ))`.
    ///
    /// # Errors
    ///
    /// Returns [`InventoryError::Ron`] on parse failure.
    pub fn from_ron(s: &str) -> Result<Self, InventoryError> {
        // Bare struct: ( id: "...", ... )
        if let Ok(item) = ron::from_str::<Item>(s) {
            return Ok(item);
        }
        // DATA-FORMATS newtype: Item(( id: "...", ... ))
        #[derive(Deserialize)]
        #[serde(rename = "Item")]
        struct ItemNewtype(Item);
        ron::from_str::<ItemNewtype>(s)
            .map(|w| w.0)
            .map_err(|e| InventoryError::Ron(e.to_string()))
    }

    /// Serialize to a pretty RON string.
    ///
    /// # Errors
    ///
    /// Returns [`InventoryError::Serialize`] on failure.
    pub fn to_ron(&self) -> Result<String, InventoryError> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|e| InventoryError::Serialize(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cellar_key() -> Item {
        Item {
            id: "key_cellar".into(),
            display_name: "Cellar Key".into(),
            description: "A heavy iron key, cold to the touch.".into(),
            icon: Some("icons/key_cellar".into()),
            tags: vec!["Item.Key".into()],
            verbs: vec![
                ItemVerb::look("examine_key_cellar"),
                ItemVerb::use_with_target(),
                ItemVerb::use_on("lock_cellar", "unlock_cellar"),
            ],
            can_combine: true,
            can_pickup: true,
            stackable: false,
            max_stack: 1,
        }
    }

    #[test]
    fn ron_round_trip() {
        let item = cellar_key();
        let ron_str = item.to_ron().unwrap();
        let back = Item::from_ron(&ron_str).unwrap();
        assert_eq!(item, back);
    }

    #[test]
    fn data_formats_style_wrapper() {
        let src = r#"
Item((
    id: "key_cellar",
    display_name: "Cellar Key",
    description: "A heavy iron key, cold to the touch.",
    icon: Some("icons/key_cellar"),
    verbs: [
        ( kind: Look,  text: Some("Examine"), action: Some("examine_key_cellar") ),
        ( kind: Use,   text: Some("Use on"),  requires_target: true ),
        ( kind: UseOn, matches: Some("lock_cellar"), action: Some("unlock_cellar") ),
    ],
))
"#;
        let item = Item::from_ron(src).expect("parse DATA-FORMATS style");
        assert_eq!(item.id.as_str(), "key_cellar");
        assert_eq!(item.verbs.len(), 3);
        assert!(item.verb_for_target("lock_cellar").unwrap().action.as_ref().unwrap() == "unlock_cellar");
    }

    #[test]
    fn look_always_has_text() {
        let i = Item::new("rock", "Rock", "A grey rock.");
        assert_eq!(i.look_text(), "A grey rock.");
        assert!(i.has_verb(VerbKind::Look));
    }

    #[test]
    fn asset_id_stable() {
        let i = Item::new("key_cellar", "Key", "key");
        assert_eq!(i.asset_id(), AssetId::from_path("items/key_cellar"));
    }

    #[test]
    fn load_key_cellar_fixture() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/items/key_cellar.item.ron");
        let src = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let item = Item::from_ron(&src).expect("parse key_cellar fixture");
        assert_eq!(item.id.as_str(), "key_cellar");
        assert!(item.verb_for_target("lock_cellar").is_some());
    }
}
