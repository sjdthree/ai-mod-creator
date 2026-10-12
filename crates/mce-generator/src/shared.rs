//! Loader-agnostic generation (`SPEC-003 §9`).
//!
//! Everything here is identical between mod loaders: the vanilla **data-pack** output (models,
//! blockstates, loot tables, lang, recipes), placeholder textures, the shared GameTest structure
//! template, the committed Gradle wrapper, and the write/prune/manifest `materialize` step. The
//! per-loader modules (`neoforge`, `fabric`) start from [`insert_shared_files`] and add only their
//! mod metadata, Gradle build, and Java sources.
//!
//! Moved here verbatim from the original single-loader generator, so NeoForge output stays
//! byte-identical (`SPEC-005 §22`).

use std::collections::BTreeMap;
use std::path::Path;

use mce_state::{
    CanonicalState, Concept, ConceptType, Recipe, RecipeType, Reference, ReferenceType,
};

use crate::manifest::GenerationManifest;
use crate::texture::{self, Palette};
use crate::{Generated, GENERATOR_VERSION};

// The full Gradle wrapper, embedded so a generated project is self-contained: it builds with only a
// JDK (the wrapper downloads Gradle + the loader itself). No system `gradle` needed on the host.
// Loader-agnostic — both NeoForge and Fabric build through this same Gradle wrapper.
const GRADLEW: &[u8] = include_bytes!("../assets/gradlew");
const GRADLEW_BAT: &[u8] = include_bytes!("../assets/gradlew.bat");
const GRADLE_WRAPPER_JAR: &[u8] = include_bytes!("../assets/gradle/wrapper/gradle-wrapper.jar");

/// The source roots the generator fully owns and re-materializes every run. Only these are pruned of
/// stale files — everything else under the project (build output, Gradle caches, the wrapper JAR) is
/// off-limits.
const MANAGED_ROOTS: &[&str] = &["src/main/java", "src/main/resources"];
/// Also owned by the generator: Fabric puts its client entrypoint under `src/client/java`.
const MANAGED_ROOTS_EXTRA: &[&str] = &["src/client/java"];

// ---- shared file set ------------------------------------------------------------------------

/// Insert every loader-agnostic file into `files`: the Gradle wrapper, all per-concept resources
/// (models, blockstates, textures, loot, lang), the GameTest structure template, and the recipe data
/// pack. The caller (a loader generator) then adds its metadata + Gradle build + Java.
pub(crate) fn insert_shared_files(state: &CanonicalState, files: &mut BTreeMap<String, Vec<u8>>) {
    let modid = &state.namespace;

    // --- Gradle wrapper (shared: both loaders build through Gradle) ---
    files.insert("gradle.properties".into(), gradle_properties().into_bytes());
    files.insert(
        "gradle/wrapper/gradle-wrapper.properties".into(),
        gradle_wrapper_properties().into_bytes(),
    );
    files.insert("gradlew".into(), GRADLEW.to_vec());
    files.insert("gradlew.bat".into(), GRADLEW_BAT.to_vec());
    files.insert(
        "gradle/wrapper/gradle-wrapper.jar".into(),
        GRADLE_WRAPPER_JAR.to_vec(),
    );

    // --- per-concept resources + textures (vanilla data pack, loader-agnostic) ---
    let mut lang = BTreeMap::<String, String>::new();
    for concept in sorted_concepts(state) {
        let name = registry_name(concept);
        let seed = seed_from(&concept.namespace_id);
        if concept.concept_type == ConceptType::Block {
            // A placeable block needs a blockstate, a block model, an item model that reuses the
            // block model, a block texture, and a loot table so breaking it drops itself.
            files.insert(
                format!("src/main/resources/assets/{modid}/blockstates/{name}.json"),
                blockstate_json(modid, &name).into_bytes(),
            );
            files.insert(
                format!("src/main/resources/assets/{modid}/models/block/{name}.json"),
                block_model_json(modid, &name).into_bytes(),
            );
            files.insert(
                format!("src/main/resources/assets/{modid}/models/item/{name}.json"),
                block_item_model_json(modid, &name).into_bytes(),
            );
            files.insert(
                format!("src/main/resources/assets/{modid}/textures/block/{name}.png"),
                texture::placeholder_png(seed, &Palette::default_for(seed)),
            );
            files.insert(
                format!("src/main/resources/data/{modid}/loot_table/blocks/{name}.json"),
                block_loot_table_json(modid, &name).into_bytes(),
            );
            lang.insert(
                format!("block.{modid}.{name}"),
                concept.display_name.clone(),
            );
        } else {
            files.insert(
                format!("src/main/resources/assets/{modid}/models/item/{name}.json"),
                item_model_json(modid, &name, concept.concept_type).into_bytes(),
            );
            // A shaped silhouette (sword/pickaxe/apple/…) so the item reads as what it is, not a
            // featureless square.
            files.insert(
                format!("src/main/resources/assets/{modid}/textures/item/{name}.png"),
                texture::sprite_png(seed, &Palette::default_for(seed), shape_for(concept)),
            );
            lang.insert(format!("item.{modid}.{name}"), concept.display_name.clone());
        }
    }
    files.insert(
        format!("src/main/resources/assets/{modid}/lang/en_us.json"),
        lang_json(&lang).into_bytes(),
    );

    // --- GameTest structure template (shared empty 3x3x3, ADR-0004) ---
    // Minecraft 1.21 singularized data-pack folders (structures -> structure), matching the recipe
    // change. Verified: `runGameTestServer` loads the template from the singular `structure/` path.
    files.insert(
        format!("src/main/resources/data/{modid}/structure/empty.nbt"),
        crate::structure::empty_structure_nbt(),
    );

    // --- recipes (data pack, vanilla format) ---
    for recipe in sorted_recipes(state) {
        let name = path_of(&recipe.namespace_id);
        files.insert(
            format!("src/main/resources/data/{modid}/recipe/{name}.json"),
            recipe_json(state, recipe).into_bytes(),
        );
    }
}

/// Write `files` to `output_root`, prune stale generator-owned files, and build the manifest for
/// `target`. Shared by every loader — the write/prune/manifest logic is identical.
pub(crate) fn materialize(
    files: &BTreeMap<String, Vec<u8>>,
    output_root: &Path,
    target: &str,
) -> mce_core::Result<Generated> {
    for (rel, bytes) in files {
        // Every generated path must stay inside the project. Paths are built from state values
        // (namespace, ids), so refuse anything that could climb out of `output_root`.
        let rel_path = Path::new(rel);
        if !rel_path
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
        {
            return Err(mce_core::MceError::validation(
                mce_core::Component::Generator,
                format!("refusing to write `{rel}`: generated paths must stay inside the project"),
            ));
        }
        let path = output_root.join(rel_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, bytes)?;
        // The wrapper script must be executable to run as `./gradlew`.
        #[cfg(unix)]
        if rel == "gradlew" {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
        }
    }

    // Generation is a full re-materialization: the generator owns `src/main/java`,
    // `src/main/resources`, and (Fabric) `src/client/java` outright, so any file there that isn't in
    // the freshly-built set is a leftover from a prior state (a removed recipe, a deleted concept, a
    // renamed concept's old name) and must be pruned — otherwise Gradle bundles it into the JAR and
    // the change never reaches the game. Restricted to those roots so Gradle's own trees (`build/`,
    // `.gradle/`, the wrapper under `gradle/`) are left untouched.
    prune_stale(output_root, files)?;

    let manifest = GenerationManifest::build(GENERATOR_VERSION, target, files);
    Ok(Generated {
        output_root: output_root.to_path_buf(),
        manifest,
    })
}

/// Delete files under the generator-owned roots that aren't in the freshly-built set, then remove any
/// directories left empty. This is what makes a removal/rename actually reach the game: without it a
/// prior generation's recipe/asset/Java file lingers in the source tree and Gradle re-bundles it.
fn prune_stale(output_root: &Path, files: &BTreeMap<String, Vec<u8>>) -> mce_core::Result<()> {
    let keep: std::collections::HashSet<std::path::PathBuf> =
        files.keys().map(|rel| output_root.join(rel)).collect();
    for root in MANAGED_ROOTS.iter().chain(MANAGED_ROOTS_EXTRA) {
        let dir = output_root.join(root);
        if dir.exists() {
            prune_dir(&dir, &keep)?;
        }
    }
    Ok(())
}

/// Recursively remove files not in `keep`, and directories that end up empty. Returns whether `dir`
/// is now empty (so the caller can remove it too).
fn prune_dir(
    dir: &Path,
    keep: &std::collections::HashSet<std::path::PathBuf>,
) -> mce_core::Result<bool> {
    let mut empty = true;
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            if prune_dir(&path, keep)? {
                std::fs::remove_dir(&path)?;
            } else {
                empty = false;
            }
        } else if keep.contains(&path) {
            empty = false;
        } else {
            std::fs::remove_file(&path)?;
        }
    }
    Ok(empty)
}

// ---- concept helpers ------------------------------------------------------------------------

pub(crate) fn sorted_concepts(state: &CanonicalState) -> Vec<&Concept> {
    let mut v: Vec<&Concept> = state.active_concepts().collect();
    v.sort_by(|a, b| a.namespace_id.cmp(&b.namespace_id));
    v
}

/// The registry path (the part after `namespace:`).
pub(crate) fn registry_name(concept: &Concept) -> String {
    path_of(&concept.namespace_id)
}

pub(crate) fn path_of(namespace_id: &str) -> String {
    namespace_id
        .split_once(':')
        .map(|(_, p)| p.to_string())
        .unwrap_or_default()
}

pub(crate) fn sorted_recipes(state: &CanonicalState) -> Vec<&Recipe> {
    let mut v: Vec<&Recipe> = state.active_recipes().collect();
    v.sort_by(|a, b| a.namespace_id.cmp(&b.namespace_id));
    v
}

/// Resolve a canonical reference to its Minecraft registry id for recipe JSON.
fn ref_item_id(state: &CanonicalState, reference: &Reference) -> String {
    match reference.reference_type {
        ReferenceType::ConceptId => state
            .concepts
            .get(&reference.value)
            .map(|c| c.namespace_id.clone())
            .unwrap_or_else(|| reference.value.clone()),
        // minecraft_id / namespace_id are already registry ids.
        _ => reference.value.clone(),
    }
}

/// Generate vanilla recipe JSON (`data/<modid>/recipe/<name>.json`, `SPEC-004 §15`). Loader-agnostic.
fn recipe_json(state: &CanonicalState, recipe: &Recipe) -> String {
    use serde_json::json;
    let result_id = ref_item_id(state, &recipe.result);
    let result = json!({ "id": result_id, "count": recipe.count });

    let value = match recipe.recipe_type {
        RecipeType::Shaped => {
            let key: serde_json::Map<String, serde_json::Value> = recipe
                .key
                .iter()
                .map(|(sym, r)| (sym.clone(), json!({ "item": ref_item_id(state, r) })))
                .collect();
            json!({
                "type": "minecraft:crafting_shaped",
                "pattern": recipe.pattern,
                "key": key,
                "result": result,
            })
        }
        RecipeType::Shapeless => {
            let ingredients: Vec<serde_json::Value> = recipe
                .ingredients
                .iter()
                .map(|r| json!({ "item": ref_item_id(state, r) }))
                .collect();
            json!({
                "type": "minecraft:crafting_shapeless",
                "ingredients": ingredients,
                "result": result,
            })
        }
        RecipeType::Smelting | RecipeType::Blasting => {
            let ty = if recipe.recipe_type == RecipeType::Smelting {
                "smelting"
            } else {
                "blasting"
            };
            let ingredient = recipe
                .ingredient
                .as_ref()
                .map(|r| json!({ "item": ref_item_id(state, r) }))
                .unwrap_or(json!({}));
            json!({
                "type": format!("minecraft:{ty}"),
                "ingredient": ingredient,
                "result": result,
                "experience": recipe.experience.unwrap_or(0.0),
                "cookingtime": recipe.cooking_time_ticks.unwrap_or(200),
            })
        }
    };
    // serde_json Objects are key-sorted (no preserve_order feature), so this is deterministic.
    format!(
        "{}\n",
        serde_json::to_string_pretty(&value).expect("recipe serializes")
    )
}

pub(crate) fn prop_i64(concept: &Concept, key: &str, default: i64) -> i64 {
    concept
        .properties
        .get(key)
        .and_then(|v| v.as_i64())
        .unwrap_or(default)
}

pub(crate) fn prop_f64(concept: &Concept, key: &str, default: f64) -> f64 {
    concept
        .properties
        .get(key)
        .and_then(|v| v.as_f64())
        .unwrap_or(default)
}

// ---- vanilla resource templates (loader-agnostic) -------------------------------------------

fn gradle_properties() -> String {
    "org.gradle.jvmargs=-Xmx2G\norg.gradle.daemon=false\n".to_string()
}

fn gradle_wrapper_properties() -> String {
    // The wrapper JAR itself is binary and is materialized once via `gradle wrapper` (Phase 3
    // spike 0000); these properties pin the Gradle distribution used by both loaders.
    "distributionBase=GRADLE_USER_HOME\n\
     distributionPath=wrapper/dists\n\
     distributionUrl=https\\://services.gradle.org/distributions/gradle-8.10.2-bin.zip\n\
     networkTimeout=10000\n\
     validateDistributionUrl=true\n\
     zipStoreBase=GRADLE_USER_HOME\n\
     zipStorePath=wrapper/dists\n"
        .to_string()
}

fn item_model_json(modid: &str, name: &str, concept_type: ConceptType) -> String {
    // Weapons and tools are held in a "handheld" pose; other items use the flat generated model.
    let parent = if matches!(concept_type, ConceptType::Weapon | ConceptType::Tool) {
        "item/handheld"
    } else {
        "item/generated"
    };
    format!(
        "{{\n  \"parent\": \"{parent}\",\n  \"textures\": {{\n    \"layer0\": \"{modid}:item/{name}\"\n  }}\n}}\n"
    )
}

/// Blockstate mapping the single default variant to the block's model.
fn blockstate_json(modid: &str, name: &str) -> String {
    format!(
        "{{\n  \"variants\": {{\n    \"\": {{ \"model\": \"{modid}:block/{name}\" }}\n  }}\n}}\n"
    )
}

/// A full-cube block model textured on all faces with the block's placeholder texture.
fn block_model_json(modid: &str, name: &str) -> String {
    format!(
        "{{\n  \"parent\": \"minecraft:block/cube_all\",\n  \"textures\": {{\n    \
         \"all\": \"{modid}:block/{name}\"\n  }}\n}}\n"
    )
}

/// The block's inventory item model simply reuses the block model.
fn block_item_model_json(modid: &str, name: &str) -> String {
    format!("{{\n  \"parent\": \"{modid}:block/{name}\"\n}}\n")
}

/// A "drop itself" loot table so breaking the block yields its item (1.21 singular `loot_table`).
fn block_loot_table_json(modid: &str, name: &str) -> String {
    format!(
        "{{\n  \"type\": \"minecraft:block\",\n  \"pools\": [\n    {{\n      \
         \"rolls\": 1,\n      \"entries\": [\n        {{ \"type\": \"minecraft:item\", \
         \"name\": \"{modid}:{name}\" }}\n      ]\n    }}\n  ]\n}}\n"
    )
}

fn lang_json(entries: &BTreeMap<String, String>) -> String {
    let mut out = String::from("{\n");
    let mut first = true;
    for (key, value) in entries {
        if !first {
            out.push_str(",\n");
        }
        first = false;
        out.push_str(&format!(
            "  \"{}\": \"{}\"",
            escape_json(key),
            escape_json(value)
        ));
    }
    out.push_str("\n}\n");
    out
}

// ---- small utilities (shared by both loaders' Java generators) ------------------------------

/// Format an f64 as a Java float literal, e.g. `8.0F`, `-2.4F`.
pub(crate) fn jfloat(x: f64) -> String {
    format!("{x:?}F")
}

pub(crate) fn pascal_case(snake: &str) -> String {
    snake
        .split('_')
        .filter(|s| !s.is_empty())
        .map(|s| {
            let mut chars = s.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// FNV-1a 64-bit hash — deterministic across runs (unlike `DefaultHasher`).
fn seed_from(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

/// The placeholder sprite silhouette for a concept, so its icon reads as what it is.
fn shape_for(concept: &Concept) -> texture::Shape {
    use texture::Shape;
    match concept.concept_type {
        ConceptType::Weapon => Shape::Sword,
        ConceptType::Tool => match concept.subtype.as_deref() {
            Some("axe") => Shape::Axe,
            Some("shovel") => Shape::Shovel,
            Some("hoe") => Shape::Hoe,
            _ => Shape::Pickaxe,
        },
        ConceptType::Food => Shape::Food,
        ConceptType::Item => Shape::Gem,
        ConceptType::Block => Shape::Block,
    }
}

/// The vanilla creative tab a concept type belongs in (`net.minecraft.world.item.CreativeModeTabs`).
/// Loader-agnostic under Mojmap — both loaders reference the same `CreativeModeTabs` keys.
pub(crate) fn vanilla_tab(concept_type: ConceptType) -> &'static str {
    match concept_type {
        ConceptType::Weapon => "COMBAT",
        ConceptType::Tool => "TOOLS_AND_UTILITIES",
        ConceptType::Food => "FOOD_AND_DRINKS",
        ConceptType::Block => "BUILDING_BLOCKS",
        ConceptType::Item => "INGREDIENTS",
    }
}

/// The Java constant name for a concept's registered object, e.g. `RUBY_BLADE`.
pub(crate) fn concept_konst(concept: &Concept) -> String {
    registry_name(concept).to_uppercase()
}

/// The Minecraft item class for a tool subtype (defaults to a pickaxe for unknown subtypes). Mojmap
/// names, identical across loaders.
pub(crate) fn tool_class(subtype: Option<&str>) -> &'static str {
    match subtype {
        Some("axe") => "AxeItem",
        Some("shovel") => "ShovelItem",
        Some("hoe") => "HoeItem",
        _ => "PickaxeItem",
    }
}

/// Default (attack_damage bonus, attack_speed) per tool subtype, mirroring vanilla feel.
pub(crate) fn tool_defaults(subtype: Option<&str>) -> (f64, f64) {
    match subtype {
        Some("axe") => (6.0, -3.1),
        Some("shovel") => (1.5, -3.0),
        Some("hoe") => (0.0, -3.0),
        _ => (1.0, -2.8), // pickaxe
    }
}

#[cfg(test)]
mod path_safety_tests {
    use super::*;

    #[test]
    fn materialize_refuses_paths_outside_the_project() {
        let root = std::env::temp_dir().join(format!("mce-materialize-{}", std::process::id()));
        for rel in ["../escape.json", "/abs.json", "a/../../b.json"] {
            let files = BTreeMap::from([(rel.to_string(), b"x".to_vec())]);
            assert!(materialize(&files, &root, "neoforge").is_err(), "{rel}");
        }
        assert!(!root.parent().unwrap().join("escape.json").exists());
        let _ = std::fs::remove_dir_all(root);
    }
}
