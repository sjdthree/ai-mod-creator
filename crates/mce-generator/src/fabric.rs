//! The **Fabric** generator (ADR-0007) — *scaffold*.
//!
//! Emits a Fabric Loom / fabric-api 1.21.1 project using **Mojang mappings** (`officialMojangMappings`),
//! so the `net.minecraft.*` game classes match the NeoForge generator's and the loader-agnostic
//! resource/recipe/texture half (from [`crate::shared`]) is reused verbatim. Only the mod metadata
//! (`fabric.mod.json`), the Gradle build (`fabric-loom`), and the Java registration surface
//! (`ModInitializer` + `Registry.register` + `ItemGroupEvents`) differ from NeoForge.
//!
//! **Status:** verified against a real Loom build (JDK 21, 2026-07-22) — the generated project
//! compiles, packages a remapped jar, and passes `runGametest` (6/6 GameTests). Three
//! NeoForge→vanilla divergences were found by real builds and fixed here:
//!   1. `SwordItem.createAttributes` takes an **int** damage bonus (NeoForge patched it to float).
//!   2. `fabric-gametest` **instantiates** the entrypoint class → it needs a **public** constructor
//!      (NeoForge scans statically, so a private ctor is fine there).
//!   3. Fabric has no `@GameTestHolder`, so the GameTest `template` must be **namespace-qualified**
//!      (`<modid>:empty`) — a bare `"empty"` resolves to `minecraft:empty` and 404s.
//!
//! Still to exercise before full promotion: the client smoke (`runClient`) and CI matrix (ADR-0007).

use std::collections::BTreeMap;
use std::path::Path;

use mce_core::Result;
use mce_state::{CanonicalState, Concept, ConceptType};

use crate::shared::{
    self, concept_konst, jfloat, pascal_case, prop_f64, prop_i64, registry_name, sorted_concepts,
    tool_class, tool_defaults, vanilla_tab,
};
use crate::{Generated, Generator};

/// The pinned target this generator emits for (ADR-0007), recorded in the manifest.
pub const MINECRAFT_TARGET: &str = "fabric-1.21.1";
// Pinned Fabric toolchain (1.21.1 line). Exact patch versions are locked here per ADR-0007.
const MINECRAFT_VERSION: &str = "1.21.1";
const FABRIC_LOOM_VERSION: &str = "1.7-SNAPSHOT";
const FABRIC_LOADER_VERSION: &str = "0.16.5";
// fabric-api build for 1.21.1.
const FABRIC_API_VERSION: &str = "0.102.0+1.21.1";
const YARN_JAVA_VERSION: u32 = 21;

/// The Fabric generator (scaffold).
#[derive(Debug, Default)]
pub struct FabricGenerator;

impl Generator for FabricGenerator {
    fn version(&self) -> &str {
        crate::GENERATOR_VERSION
    }

    fn target(&self) -> &str {
        MINECRAFT_TARGET
    }

    fn generate(&self, state: &CanonicalState, output_root: &Path) -> Result<Generated> {
        let files = build_files(state);
        shared::materialize(&files, output_root, MINECRAFT_TARGET)
    }
}

/// Build the full generated Fabric file set in memory (path → bytes), deterministically.
pub fn build_files(state: &CanonicalState) -> BTreeMap<String, Vec<u8>> {
    let modid = &state.namespace;
    let mod_class = pascal_case(modid);
    let pkg_path = format!("mce/{modid}");
    let has_blocks = sorted_concepts(state)
        .iter()
        .any(|c| c.concept_type == ConceptType::Block);
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();

    // Loader-agnostic half: resources, recipes, textures, structure, Gradle wrapper.
    shared::insert_shared_files(state, &mut files);

    // --- Fabric project skeleton ---
    files.insert(
        "settings.gradle".into(),
        settings_gradle(modid).into_bytes(),
    );
    files.insert("build.gradle".into(), build_gradle(modid).into_bytes());
    files.insert(
        "src/main/resources/fabric.mod.json".into(),
        fabric_mod_json(modid, &state.name, &mod_class).into_bytes(),
    );

    // --- Java ---
    files.insert(
        format!("src/main/java/{pkg_path}/{mod_class}.java"),
        main_class_java(state, &mod_class, has_blocks).into_bytes(),
    );
    files.insert(
        format!("src/main/java/{pkg_path}/ModItems.java"),
        mod_items_java(state, &mod_class).into_bytes(),
    );
    if has_blocks {
        files.insert(
            format!("src/main/java/{pkg_path}/ModBlocks.java"),
            mod_blocks_java(state, &mod_class).into_bytes(),
        );
    }
    files.insert(
        format!("src/main/java/{pkg_path}/ModGameTests.java"),
        mod_gametests_java(state, &mod_class).into_bytes(),
    );

    files
}

// ---- Fabric metadata + Gradle ---------------------------------------------------------------

fn settings_gradle(modid: &str) -> String {
    format!(
        "pluginManagement {{\n    repositories {{\n        \
         maven {{ name = 'Fabric'; url = 'https://maven.fabricmc.net/' }}\n        \
         gradlePluginPortal()\n    }}\n}}\n\n\
         rootProject.name = '{modid}'\n"
    )
}

fn build_gradle(modid: &str) -> String {
    // Mojang mappings so game-class names match the NeoForge generator (ADR-0007). The `gametest`
    // run declares the fabric-api gametest system property; the resulting task is `runGametest`
    // (see `Loader::gametest_task`).
    format!(
        "plugins {{\n    id 'fabric-loom' version '{FABRIC_LOOM_VERSION}'\n}}\n\n\
         version = '0.1.0'\ngroup = 'mce.{modid}'\n\n\
         repositories {{\n    mavenCentral()\n}}\n\n\
         dependencies {{\n    \
         minecraft \"com.mojang:minecraft:{MINECRAFT_VERSION}\"\n    \
         mappings loom.officialMojangMappings()\n    \
         modImplementation \"net.fabricmc:fabric-loader:{FABRIC_LOADER_VERSION}\"\n    \
         modImplementation \"net.fabricmc.fabric-api:fabric-api:{FABRIC_API_VERSION}\"\n}}\n\n\
         loom {{\n    runs {{\n        \
         gametest {{\n            server()\n            name = 'Game Test'\n            \
         source sourceSets.main\n            \
         property 'fabric-api.gametest'\n        }}\n    }}\n}}\n\n\
         java {{\n    toolchain.languageVersion = JavaLanguageVersion.of({YARN_JAVA_VERSION})\n}}\n"
    )
}

fn fabric_mod_json(modid: &str, display_name: &str, mod_class: &str) -> String {
    // Hand-formatted for deterministic byte output (key order fixed). Entrypoints: the main
    // initializer plus the fabric-gametest discovery hook.
    let dn = display_name.replace('\\', "\\\\").replace('"', "\\\"");
    format!(
        "{{\n  \"schemaVersion\": 1,\n  \"id\": \"{modid}\",\n  \"version\": \"0.1.0\",\n  \
         \"name\": \"{dn}\",\n  \"description\": \"Made with AI-mod creator.\",\n  \
         \"license\": \"MIT\",\n  \"environment\": \"*\",\n  \"entrypoints\": {{\n    \
         \"main\": [\n      \"mce.{modid}.{mod_class}\"\n    ],\n    \
         \"fabric-gametest\": [\n      \"mce.{modid}.ModGameTests\"\n    ]\n  }},\n  \
         \"depends\": {{\n    \"fabricloader\": \">=0.16.0\",\n    \"minecraft\": \"~1.21.1\",\n    \
         \"java\": \">=21\",\n    \"fabric-api\": \"*\"\n  }}\n}}\n"
    )
}

// ---- Fabric Java ----------------------------------------------------------------------------

/// The Java expression yielding a concept's item constant on Fabric. Unlike NeoForge's deferred
/// registers, Fabric registrations are plain `Item`/`BlockItem` statics — no `.get()`. A block
/// concept's inventory form is a `BlockItem` registered in `ModItems`, so all accessors are uniform.
fn item_accessor(concept: &Concept) -> String {
    format!("ModItems.{}", concept_konst(concept))
}

fn main_class_java(state: &CanonicalState, mod_class: &str, has_blocks: bool) -> String {
    let modid = &state.namespace;

    // Group concepts by their vanilla tab, deterministically (same table as NeoForge).
    let mut by_tab: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
    for c in sorted_concepts(state) {
        by_tab
            .entry(vanilla_tab(c.concept_type))
            .or_default()
            .push(item_accessor(c));
    }
    let mut tab_regs = String::new();
    for (tab, accessors) in &by_tab {
        let mut accepts = String::new();
        for a in accessors {
            accepts.push_str(&format!("            entries.accept({a});\n"));
        }
        tab_regs.push_str(&format!(
            "        ItemGroupEvents.modifyEntriesEvent(CreativeModeTabs.{tab}).register(entries -> {{\n\
             {accepts}            LOGGER.info(\"[MCE] added {n} item(s) to the {tab} creative tab\");\n        }});\n",
            n = accessors.len(),
        ));
    }

    let blocks_init = if has_blocks {
        "        ModBlocks.init();\n"
    } else {
        ""
    };

    format!(
        "package mce.{modid};\n\n\
         import com.mojang.logging.LogUtils;\n\
         import net.fabricmc.api.ModInitializer;\n\
         import net.fabricmc.fabric.api.itemgroup.v1.ItemGroupEvents;\n\
         import net.fabricmc.fabric.api.networking.v1.ServerPlayConnectionEvents;\n\
         import net.fabricmc.loader.api.FabricLoader;\n\
         import net.minecraft.world.item.CreativeModeTabs;\n\
         import net.minecraft.world.level.GameType;\n\
         import org.slf4j.Logger;\n\n\
         public class {mod_class} implements ModInitializer {{\n    \
         public static final String MOD_ID = \"{modid}\";\n    \
         private static final Logger LOGGER = LogUtils.getLogger();\n\n    \
         @Override\n    \
         public void onInitialize() {{\n\
         {blocks_init}        ModItems.init();\n        \
         registerCreativeTabs();\n        \
         registerDevPreview();\n    }}\n\n    \
         /// Drop every generated item into its vanilla creative tab via fabric-api's ItemGroupEvents.\n    \
         private static void registerCreativeTabs() {{\n\
         {tab_regs}    }}\n\n    \
         /// Dev-only: put the player into Creative on join so creations are visible. Inert in a\n    \
         /// packaged mod — gated on the development environment, mirroring NeoForge's ModDevPreview.\n    \
         private static void registerDevPreview() {{\n        \
         if (!FabricLoader.getInstance().isDevelopmentEnvironment()) return;\n        \
         ServerPlayConnectionEvents.JOIN.register((handler, sender, server) -> {{\n            \
         handler.player.setGameMode(GameType.CREATIVE);\n            \
         LOGGER.info(\"[MCE] preview: put {{}} in Creative so your creations are visible (press E)\",\n                \
         handler.player.getName().getString());\n        }});\n    }}\n}}\n"
    )
}

fn mod_items_java(state: &CanonicalState, mod_class: &str) -> String {
    let mut regs = String::new();
    for concept in sorted_concepts(state) {
        let name = registry_name(concept);
        let konst = concept_konst(concept);
        match concept.concept_type {
            ConceptType::Weapon => {
                let durability = prop_i64(concept, "durability", 250);
                let enchantability = prop_i64(concept, "enchantability", 14);
                let attack_damage = prop_f64(concept, "attack_damage", 6.0);
                let attack_speed = prop_f64(concept, "attack_speed", -2.4);
                // Vanilla `SwordItem.createAttributes(Tier, int, float)` — the damage bonus is an
                // **int** (NeoForge patched it to a float; Fabric/vanilla did not). Sword base damage
                // is integer in vanilla, so round the bonus. createAttributes adds the player base 1.
                let bonus = (attack_damage - 1.0).round() as i64;
                regs.push_str(&format!(
                    "    public static final Tier {konst}_TIER = tier({durability}, 1.0F, 0.0F, {enchantability});\n    \
                     public static final SwordItem {konst} = register(\"{name}\",\n        \
                     new SwordItem({konst}_TIER, new Item.Properties().attributes(\n            \
                     SwordItem.createAttributes({konst}_TIER, {bonus}, {speed}))));\n\n",
                    speed = jfloat(attack_speed),
                ));
            }
            ConceptType::Tool => {
                let class = tool_class(concept.subtype.as_deref());
                let (def_dmg, def_speed) = tool_defaults(concept.subtype.as_deref());
                let durability = prop_i64(concept, "durability", 250);
                let mining_speed = prop_f64(concept, "mining_speed", 6.0);
                let enchantability = prop_i64(concept, "enchantability", 14);
                let attack_damage = prop_f64(concept, "attack_damage", def_dmg);
                let attack_speed = prop_f64(concept, "attack_speed", def_speed);
                regs.push_str(&format!(
                    "    public static final Tier {konst}_TIER = tier({durability}, {mining}, 0.0F, {enchantability});\n    \
                     public static final {class} {konst} = register(\"{name}\",\n        \
                     new {class}({konst}_TIER, new Item.Properties().attributes(\n            \
                     {class}.createAttributes({konst}_TIER, {dmg}, {speed}))));\n\n",
                    mining = jfloat(mining_speed),
                    dmg = jfloat(attack_damage),
                    speed = jfloat(attack_speed),
                ));
            }
            ConceptType::Food => {
                let nutrition = prop_i64(concept, "nutrition", 4);
                let saturation = prop_f64(concept, "saturation", 0.3);
                regs.push_str(&format!(
                    "    public static final Item {konst} = register(\"{name}\",\n        \
                     new Item(new Item.Properties().food(\n            \
                     new FoodProperties.Builder().nutrition({nutrition}).saturationModifier({sat}).build())));\n\n",
                    sat = jfloat(saturation),
                ));
            }
            ConceptType::Block => {
                // The block lives in ModBlocks; register its inventory item form here.
                regs.push_str(&format!(
                    "    public static final BlockItem {konst} =\n        \
                     register(\"{name}\", new BlockItem(ModBlocks.{konst}, new Item.Properties()));\n\n"
                ));
            }
            ConceptType::Item => {
                regs.push_str(&format!(
                    "    public static final Item {konst} =\n        \
                     register(\"{name}\", new Item(new Item.Properties()));\n\n"
                ));
            }
        }
    }

    format!(
        "package mce.{modid};\n\n\
         import net.minecraft.core.Registry;\n\
         import net.minecraft.core.registries.BuiltInRegistries;\n\
         import net.minecraft.resources.ResourceLocation;\n\
         import net.minecraft.tags.BlockTags;\n\
         import net.minecraft.tags.TagKey;\n\
         import net.minecraft.world.food.FoodProperties;\n\
         import net.minecraft.world.item.AxeItem;\n\
         import net.minecraft.world.item.BlockItem;\n\
         import net.minecraft.world.item.HoeItem;\n\
         import net.minecraft.world.item.Item;\n\
         import net.minecraft.world.item.PickaxeItem;\n\
         import net.minecraft.world.item.ShovelItem;\n\
         import net.minecraft.world.item.SwordItem;\n\
         import net.minecraft.world.item.Tier;\n\
         import net.minecraft.world.item.crafting.Ingredient;\n\
         import net.minecraft.world.level.block.Block;\n\n\
         public final class ModItems {{\n\
         {regs}    /// A one-off vanilla Tier (Fabric has no NeoForge SimpleTier).\n    \
         private static Tier tier(int uses, float speed, float damage, int enchant) {{\n        \
         return new Tier() {{\n            \
         @Override public int getUses() {{ return uses; }}\n            \
         @Override public float getSpeed() {{ return speed; }}\n            \
         @Override public float getAttackDamageBonus() {{ return damage; }}\n            \
         @Override public TagKey<Block> getIncorrectBlocksForDrops() {{ return BlockTags.INCORRECT_FOR_STONE_TOOL; }}\n            \
         @Override public int getEnchantmentValue() {{ return enchant; }}\n            \
         @Override public Ingredient getRepairIngredient() {{ return Ingredient.EMPTY; }}\n        \
         }};\n    }}\n\n    \
         private static <T extends Item> T register(String name, T item) {{\n        \
         return Registry.register(BuiltInRegistries.ITEM,\n            \
         ResourceLocation.fromNamespaceAndPath({mod_class}.MOD_ID, name), item);\n    }}\n\n    \
         /// Force class-load so the static registrations above run.\n    \
         public static void init() {{}}\n\n    \
         private ModItems() {{}}\n}}\n",
        modid = state.namespace,
    )
}

fn mod_blocks_java(state: &CanonicalState, mod_class: &str) -> String {
    let mut regs = String::new();
    for concept in sorted_concepts(state) {
        if concept.concept_type != ConceptType::Block {
            continue;
        }
        let name = registry_name(concept);
        let konst = concept_konst(concept);
        let hardness = prop_f64(concept, "hardness", 3.0);
        let resistance = prop_f64(concept, "blast_resistance", hardness.max(3.0));
        let light = prop_i64(concept, "light_level", 0).clamp(0, 15);
        let light_call = if light > 0 {
            format!(".lightLevel(s -> {light})")
        } else {
            String::new()
        };
        regs.push_str(&format!(
            "    public static final Block {konst} = register(\"{name}\",\n        \
             new Block(BlockBehaviour.Properties.of().strength({hardness}, {resistance}){light_call}));\n\n",
            hardness = jfloat(hardness),
            resistance = jfloat(resistance),
        ));
    }
    format!(
        "package mce.{modid};\n\n\
         import net.minecraft.core.Registry;\n\
         import net.minecraft.core.registries.BuiltInRegistries;\n\
         import net.minecraft.resources.ResourceLocation;\n\
         import net.minecraft.world.level.block.Block;\n\
         import net.minecraft.world.level.block.state.BlockBehaviour;\n\n\
         public final class ModBlocks {{\n\
         {regs}    private static Block register(String name, Block block) {{\n        \
         return Registry.register(BuiltInRegistries.BLOCK,\n            \
         ResourceLocation.fromNamespaceAndPath({mod_class}.MOD_ID, name), block);\n    }}\n\n    \
         /// Force class-load so the static registrations above run.\n    \
         public static void init() {{}}\n\n    \
         private ModBlocks() {{}}\n}}\n",
        modid = state.namespace,
    )
}

/// Generate a Fabric GameTest class. Fabric discovers `@GameTest` methods through the
/// `fabric-gametest` entrypoint (declared in `fabric.mod.json`); templates load from
/// `data/<modid>/structure/empty.nbt`. The assertions mirror the NeoForge generator's, using
/// Fabric's plain-static accessors (no `.get()`).
fn mod_gametests_java(state: &CanonicalState, _mod_class: &str) -> String {
    let modid = &state.namespace;
    let mut methods = String::new();
    for concept in sorted_concepts(state) {
        let name = registry_name(concept);
        let konst = concept_konst(concept);
        methods.push_str(&format!(
            "    @GameTest(template = \"{modid}:empty\")\n    \
             public static void {name}_registered(GameTestHelper helper) {{\n        \
             ResourceLocation id = ResourceLocation.fromNamespaceAndPath(\"{modid}\", \"{name}\");\n        \
             if (!BuiltInRegistries.ITEM.containsKey(id)) \
             throw new GameTestAssertException(\"{name} is not registered\");\n        \
             helper.succeed();\n    }}\n\n"
        ));
        match concept.concept_type {
            ConceptType::Block => methods.push_str(&format!(
                "    @GameTest(template = \"{modid}:empty\")\n    \
                 public static void {name}_places(GameTestHelper helper) {{\n        \
                 BlockPos pos = new BlockPos(1, 1, 1);\n        \
                 helper.setBlock(pos, ModBlocks.{konst});\n        \
                 helper.assertBlockPresent(ModBlocks.{konst}, pos);\n        \
                 helper.succeed();\n    }}\n\n"
            )),
            ConceptType::Weapon => methods.push_str(&format!(
                "    @GameTest(template = \"{modid}:empty\")\n    \
                 public static void {name}_is_weapon(GameTestHelper helper) {{\n        \
                 if (!(ModItems.{konst} instanceof SwordItem)) \
                 throw new GameTestAssertException(\"{name} is not a functional weapon\");\n        \
                 helper.succeed();\n    }}\n\n"
            )),
            ConceptType::Tool => methods.push_str(&format!(
                "    @GameTest(template = \"{modid}:empty\")\n    \
                 public static void {name}_is_tool(GameTestHelper helper) {{\n        \
                 if (!(ModItems.{konst} instanceof DiggerItem)) \
                 throw new GameTestAssertException(\"{name} is not a functional tool\");\n        \
                 helper.succeed();\n    }}\n\n"
            )),
            ConceptType::Food => methods.push_str(&format!(
                "    @GameTest(template = \"{modid}:empty\")\n    \
                 public static void {name}_is_edible(GameTestHelper helper) {{\n        \
                 if (!ModItems.{konst}.getDefaultInstance().has(DataComponents.FOOD)) \
                 throw new GameTestAssertException(\"{name} is not edible\");\n        \
                 helper.succeed();\n    }}\n\n"
            )),
            ConceptType::Item => {}
        }
    }
    format!(
        "package mce.{modid};\n\n\
         import net.minecraft.core.BlockPos;\n\
         import net.minecraft.core.component.DataComponents;\n\
         import net.minecraft.core.registries.BuiltInRegistries;\n\
         import net.minecraft.gametest.framework.GameTest;\n\
         import net.minecraft.gametest.framework.GameTestAssertException;\n\
         import net.minecraft.gametest.framework.GameTestHelper;\n\
         import net.minecraft.resources.ResourceLocation;\n\
         import net.minecraft.world.item.DiggerItem;\n\
         import net.minecraft.world.item.SwordItem;\n\n\
         // fabric-gametest-api-v1 discovers @GameTest static methods by INSTANTIATING this\n\
         // entrypoint class (declared under `fabric-gametest` in fabric.mod.json), so it needs a\n\
         // public no-arg constructor — a private one throws IllegalAccessException at server boot.\n\
         // (Confirmed against a real `runGametest` run — this is where NeoForge's static-scan style\n\
         // diverges from Fabric's.)\n\
         public final class ModGameTests {{\n\n\
         {methods}    public ModGameTests() {{}}\n}}\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use mce_state::ProjectStateManager;
    use serde_json::json;

    fn all_types_world() -> CanonicalState {
        let mut sm = ProjectStateManager::new("Ruby World", "mce_ruby_full");
        let tx = sm
            .open_candidate(&[
                json!({"operation":"create_concept","payload":{"concept_type":"item","display_name":"Ruby","namespace_hint":"ruby"}}),
                json!({"operation":"create_concept","payload":{"concept_type":"weapon","subtype":"sword","display_name":"Ruby Blade","namespace_hint":"ruby_blade","properties":{"attack_damage":9.0,"attack_speed":-2.4,"durability":1200}}}),
                json!({"operation":"create_concept","payload":{"concept_type":"tool","subtype":"pickaxe","display_name":"Ruby Pickaxe","namespace_hint":"ruby_pickaxe","properties":{"durability":1200,"mining_speed":8.0}}}),
                json!({"operation":"create_concept","payload":{"concept_type":"food","display_name":"Ruby Apple","namespace_hint":"ruby_apple","properties":{"nutrition":6,"saturation":0.8}}}),
                json!({"operation":"create_concept","payload":{"concept_type":"block","subtype":"ore","display_name":"Ruby Block","namespace_hint":"ruby_block","properties":{"hardness":5.0,"blast_resistance":6.0,"light_level":10}}}),
            ])
            .unwrap();
        sm.commit(&tx.transaction_id, true).unwrap();
        sm.canonical().clone()
    }

    #[test]
    fn emits_fabric_metadata_and_loom_build_not_neoforge() {
        let files = build_files(&all_types_world());
        // Fabric metadata + build, and none of the NeoForge markers.
        assert!(files.contains_key("src/main/resources/fabric.mod.json"));
        assert!(!files.contains_key("src/main/resources/META-INF/neoforge.mods.toml"));
        let build = String::from_utf8(files["build.gradle"].clone()).unwrap();
        assert!(build.contains("fabric-loom"));
        assert!(build.contains("officialMojangMappings"));
        assert!(!build.contains("net.neoforged.moddev"));
        let meta = String::from_utf8(files["src/main/resources/fabric.mod.json"].clone()).unwrap();
        assert!(meta.contains("\"fabric-gametest\""));
        assert!(meta.contains("mce.mce_ruby_full.MceRubyFull"));
    }

    #[test]
    fn shared_vanilla_resources_are_identical_to_neoforge() {
        // The loader-agnostic half must be byte-for-byte identical between loaders (the whole point
        // of the shared seam): models, textures, recipes, lang, loot, structure.
        let state = all_types_world();
        let fab = build_files(&state);
        let neo = crate::neoforge::build_files(&state);
        for key in [
            "src/main/resources/assets/mce_ruby_full/models/item/ruby_blade.json",
            "src/main/resources/assets/mce_ruby_full/blockstates/ruby_block.json",
            "src/main/resources/data/mce_ruby_full/loot_table/blocks/ruby_block.json",
            "src/main/resources/assets/mce_ruby_full/lang/en_us.json",
            "src/main/resources/data/mce_ruby_full/structure/empty.nbt",
            "gradlew",
        ] {
            assert_eq!(fab.get(key), neo.get(key), "shared file diverged: {key}");
        }
    }

    #[test]
    fn items_use_fabric_registry_registration() {
        let files = build_files(&all_types_world());
        let items =
            String::from_utf8(files["src/main/java/mce/mce_ruby_full/ModItems.java"].clone())
                .unwrap();
        assert!(items.contains("Registry.register(BuiltInRegistries.ITEM"));
        assert!(items.contains("new SwordItem(RUBY_BLADE_TIER"));
        // Vanilla `SwordItem.createAttributes` takes an `int` damage bonus (NeoForge uses float);
        // emit an int literal, not `8.0F`, or javac rejects it (confirmed against a real Loom build).
        assert!(items.contains("SwordItem.createAttributes(RUBY_BLADE_TIER, 8, "));
        assert!(!items.contains("createAttributes(RUBY_BLADE_TIER, 8.0F"));
        assert!(items.contains("new PickaxeItem(RUBY_PICKAXE_TIER"));
        assert!(items.contains("FoodProperties.Builder().nutrition(6).saturationModifier(0.8F)"));
        assert!(items.contains("new BlockItem(ModBlocks.RUBY_BLOCK"));
        // Fabric accessors have no NeoForge deferred-register `.get()`.
        assert!(!items.contains(".get()"));
        assert!(!items.contains("DeferredRegister"));
    }

    #[test]
    fn creative_tabs_use_item_group_events() {
        let files = build_files(&all_types_world());
        let main =
            String::from_utf8(files["src/main/java/mce/mce_ruby_full/MceRubyFull.java"].clone())
                .unwrap();
        assert!(main.contains("implements ModInitializer"));
        assert!(main
            .contains("ItemGroupEvents.modifyEntriesEvent(CreativeModeTabs.TOOLS_AND_UTILITIES)"));
        assert!(main.contains("entries.accept(ModItems.RUBY_PICKAXE)"));
        assert!(main.contains("entries.accept(ModItems.RUBY_BLOCK)")); // block via its BlockItem
                                                                       // Dev preview gated on the development environment.
        assert!(main.contains("isDevelopmentEnvironment()"));
        assert!(main.contains("setGameMode(GameType.CREATIVE)"));
    }

    #[test]
    fn gametests_mirror_neoforge_assertions_without_get() {
        let files = build_files(&all_types_world());
        let tests =
            String::from_utf8(files["src/main/java/mce/mce_ruby_full/ModGameTests.java"].clone())
                .unwrap();
        assert!(tests.contains("ruby_block_places"));
        assert!(tests.contains("helper.setBlock(pos, ModBlocks.RUBY_BLOCK)"));
        assert!(tests.contains("ruby_pickaxe_is_tool"));
        assert!(tests.contains("ModItems.RUBY_PICKAXE instanceof DiggerItem"));
        assert!(tests.contains("ruby_apple_is_edible"));
        // fabric-gametest instantiates the entrypoint class → it needs a PUBLIC ctor, not the
        // private one NeoForge's static-scan style uses (confirmed against a real runGametest run).
        assert!(tests.contains("public ModGameTests()"));
        assert!(!tests.contains("private ModGameTests()"));
    }

    #[test]
    fn output_is_deterministic() {
        let state = all_types_world();
        let a = crate::manifest::GenerationManifest::build(
            crate::GENERATOR_VERSION,
            MINECRAFT_TARGET,
            &build_files(&state),
        );
        let b = crate::manifest::GenerationManifest::build(
            crate::GENERATOR_VERSION,
            MINECRAFT_TARGET,
            &build_files(&state),
        );
        assert_eq!(a.output_hash, b.output_hash);
    }

    #[test]
    fn generate_writes_a_fabric_project() {
        let dir = tempfile::tempdir().unwrap();
        let generated = FabricGenerator
            .generate(&all_types_world(), dir.path())
            .unwrap();
        assert!(dir.path().join("build.gradle").exists());
        assert!(dir
            .path()
            .join("src/main/resources/fabric.mod.json")
            .exists());
        assert_eq!(generated.manifest.minecraft_target, MINECRAFT_TARGET);
    }
}
