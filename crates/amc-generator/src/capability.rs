//! Generator capability registry (`SPEC-004 §39`, §9).
//!
//! The generator is authoritative for which concept/subtype and recipe combinations it can
//! actually implement. The validator (`amc-validator`) reads this registry to reject
//! schema-valid-but-unimplementable requests (`SPEC-004 §33` capability stage, MCL-008). Keeping
//! it here — next to the generator — means capabilities and implementation evolve together.

/// The generator version this capability set describes.
pub const GENERATOR_VERSION: &str = "0.1.0";

/// Supported subtypes for a concept type, or `None` if the concept type itself is unsupported.
/// An empty slice means the type is supported with no subtype (e.g. `food`).
pub fn subtypes(concept_type: &str) -> Option<&'static [&'static str]> {
    match concept_type {
        "item" => Some(&["basic", "ingredient", "miscellaneous"]),
        "weapon" => Some(&["sword", "dagger", "axe_weapon"]),
        "tool" => Some(&["pickaxe", "axe", "shovel", "hoe"]),
        "food" => Some(&[]),
        "block" => Some(&["basic", "ore", "decorative"]),
        _ => None,
    }
}

/// Whether the generator can implement this concept type + optional subtype.
pub fn is_concept_supported(concept_type: &str, subtype: Option<&str>) -> bool {
    match subtypes(concept_type) {
        None => false,
        Some(list) => match subtype {
            None => true,
            Some(s) => list.contains(&s),
        },
    }
}

/// Supported recipe types (`SPEC-004 §39`).
pub fn recipe_types() -> &'static [&'static str] {
    &["shaped", "shapeless", "smelting", "blasting"]
}

/// Whether a recipe type is supported.
pub fn is_recipe_type_supported(recipe_type: &str) -> bool {
    recipe_types().contains(&recipe_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_combinations_supported() {
        assert!(is_concept_supported("weapon", Some("sword")));
        assert!(is_concept_supported("weapon", None));
        assert!(is_concept_supported("food", None));
        assert!(is_recipe_type_supported("shaped"));
    }

    #[test]
    fn unknown_combinations_rejected() {
        assert!(!is_concept_supported("weapon", Some("laser")));
        assert!(!is_concept_supported("entity", None));
        assert!(!is_recipe_type_supported("alchemy"));
    }
}
