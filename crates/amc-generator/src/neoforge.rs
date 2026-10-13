//! The deterministic **NeoForge** generator (ADR-0003).
//!
//! Emits a NeoForge 1.21.1 project: `net.neoforged.moddev` Gradle build, `neoforge.mods.toml`, and
//! Java using `@Mod` / `DeferredRegister` / the mod event bus. The loader-agnostic half (resources,
//! recipes, textures, Gradle wrapper) comes from [`crate::shared`]. Output is byte-identical to the
//! original single-loader generator (`SPEC-005 §22`).

use std::collections::BTreeMap;
use std::path::Path;

use amc_core::Result;
use amc_state::{CanonicalState, Concept, ConceptType};

use crate::shared::{
    self, concept_konst, jfloat, pascal_case, prop_f64, prop_i64, registry_name, sorted_concepts,
    tool_class, tool_defaults, vanilla_tab,
};
use crate::{Generated, Generator};

/// The pinned target this generator emits for (ADR-0003), recorded in the manifest.
pub const MINECRAFT_TARGET: &str = "neoforge-1.21.1";
// Pinned from the official NeoForge 1.21.1 ModDevGradle MDK (ADR-0003).
const NEOFORGE_VERSION: &str = "21.1.235";
const MODDEV_PLUGIN_VERSION: &str = "2.0.141";

/// The deterministic NeoForge generator.
#[derive(Debug, Default)]
pub struct NeoForgeGenerator;

impl Generator for NeoForgeGenerator {
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

/// Build the full generated NeoForge file set in memory (path → bytes), deterministically.
///
/// Exposed for tests and for callers that want to hash without writing to disk.
pub fn build_files(state: &CanonicalState) -> BTreeMap<String, Vec<u8>> {
    let modid = &state.namespace;
    let mod_class = pascal_case(modid);
    let pkg_path = format!("amc/{modid}");
    let has_blocks = sorted_concepts(state)
        .iter()
        .any(|c| c.concept_type == ConceptType::Block);
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();

    // Loader-agnostic half: resources, recipes, textures, structure, Gradle wrapper.
    shared::insert_shared_files(state, &mut files);

    // --- NeoForge project skeleton ---
    files.insert(
        "settings.gradle".into(),
        settings_gradle(modid).into_bytes(),
    );
    files.insert("build.gradle".into(), build_gradle(modid).into_bytes());
    files.insert(
        "src/main/resources/META-INF/neoforge.mods.toml".into(),
        mods_toml(modid, &state.name).into_bytes(),
    );

    // --- Java ---
    files.insert(
        format!("src/main/java/{pkg_path}/{mod_class}.java"),
        main_class_java(modid, &mod_class, has_blocks).into_bytes(),
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
        format!("src/main/java/{pkg_path}/ModCreativeTabs.java"),
        mod_creative_tabs_java(state, &mod_class).into_bytes(),
    );
    files.insert(
        format!("src/main/java/{pkg_path}/ModDevPreview.java"),
        mod_dev_preview_java(state, &mod_class).into_bytes(),
    );
    files.insert(
        format!("src/main/java/{pkg_path}/ModGameTests.java"),
        mod_gametests_java(state, &mod_class).into_bytes(),
    );

    files
}

// ---- NeoForge text templates ----------------------------------------------------------------

fn settings_gradle(modid: &str) -> String {
    format!(
        "pluginManagement {{\n    repositories {{\n        gradlePluginPortal()\n        \
         maven {{ url = 'https://maven.neoforged.net/releases' }}\n    }}\n}}\n\n\
         rootProject.name = '{modid}'\n"
    )
}

fn build_gradle(modid: &str) -> String {
    format!(
        "plugins {{\n    id 'net.neoforged.moddev' version '{MODDEV_PLUGIN_VERSION}'\n}}\n\n\
         version = '0.1.0'\ngroup = 'amc.{modid}'\n\n\
         repositories {{\n    mavenCentral()\n}}\n\n\
         neoForge {{\n    version = '{NEOFORGE_VERSION}'\n    mods {{\n        {modid} {{\n            \
         sourceSet sourceSets.main\n        }}\n    }}\n    runs {{\n        \
         client {{ client() }}\n        \
         server {{ server() }}\n        \
         gameTestServer {{\n            type = \"gameTestServer\"\n            \
         systemProperty 'neoforge.enabledGameTestNamespaces', '{modid}'\n        }}\n    }}\n}}\n\n\
         java {{\n    toolchain.languageVersion = JavaLanguageVersion.of(21)\n}}\n"
    )
}

fn mods_toml(modid: &str, display_name: &str) -> String {
    format!(
        "modLoader = \"javafml\"\nloaderVersion = \"[1,)\"\nlicense = \"MIT\"\n\n\
         [[mods]]\nmodId = \"{modid}\"\nversion = \"0.1.0\"\ndisplayName = \"{display_name}\"\n\
         description = \"Made with AI-mod creator.\"\n\n\
         [[dependencies.{modid}]]\nmodId = \"neoforge\"\ntype = \"required\"\n\
         versionRange = \"[21.1,)\"\nordering = \"NONE\"\nside = \"BOTH\"\n\n\
         [[dependencies.{modid}]]\nmodId = \"minecraft\"\ntype = \"required\"\n\
         versionRange = \"[1.21.1,1.22)\"\nordering = \"NONE\"\nside = \"BOTH\"\n"
    )
}

fn main_class_java(modid: &str, mod_class: &str, has_blocks: bool) -> String {
    // Register every DeferredRegister the project uses to the mod event bus, plus the listener that
    // drops our items into the vanilla creative tabs.
    let mut regs = String::new();
    if has_blocks {
        regs.push_str("        ModBlocks.BLOCKS.register(modEventBus);\n");
    }
    regs.push_str("        ModItems.ITEMS.register(modEventBus);\n");
    regs.push_str("        modEventBus.addListener(ModCreativeTabs::buildContents);\n");
    format!(
        "package amc.{modid};\n\n\
         import net.neoforged.bus.api.IEventBus;\n\
         import net.neoforged.fml.common.Mod;\n\n\
         @Mod({mod_class}.MOD_ID)\n\
         public class {mod_class} {{\n    \
         public static final String MOD_ID = \"{modid}\";\n\n    \
         public {mod_class}(IEventBus modEventBus) {{\n{regs}    }}\n}}\n"
    )
}

/// Add every generated item to the matching **vanilla** creative tab (sword → Combat, block →
/// Building Blocks, food → Food & Drinks, …) via `BuildCreativeModeTabContentsEvent`, so a player
/// finds their creations in the normal creative categories they already know — no custom tab to
/// hunt for. The listener is added on the mod event bus in the mod constructor and fires client-side.
/// Logs a one-line confirmation per populated tab so the placement is verifiable in the client log.
fn mod_creative_tabs_java(state: &CanonicalState, _mod_class: &str) -> String {
    let modid = &state.namespace;
    // Group concepts by their vanilla tab, deterministically.
    let mut by_tab: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
    for c in sorted_concepts(state) {
        by_tab
            .entry(vanilla_tab(c.concept_type))
            .or_default()
            .push(item_accessor(c));
    }
    let mut blocks = String::new();
    for (tab, accessors) in &by_tab {
        let mut accepts = String::new();
        for a in accessors {
            accepts.push_str(&format!("            event.accept({a});\n"));
        }
        blocks.push_str(&format!(
            "        if (event.getTabKey() == CreativeModeTabs.{tab}) {{\n{accepts}            \
             LOGGER.info(\"[AMC] added {n} item(s) to the {tab} creative tab\");\n        }}\n",
            n = accessors.len(),
        ));
    }
    format!(
        "package amc.{modid};\n\n\
         import com.mojang.logging.LogUtils;\n\
         import net.minecraft.world.item.CreativeModeTabs;\n\
         import net.neoforged.neoforge.event.BuildCreativeModeTabContentsEvent;\n\
         import org.slf4j.Logger;\n\n\
         public final class ModCreativeTabs {{\n    \
         private static final Logger LOGGER = LogUtils.getLogger();\n\n    \
         /// Fired on the mod event bus (client only) once per creative tab.\n    \
         public static void buildContents(BuildCreativeModeTabContentsEvent event) {{\n\
         {blocks}    }}\n\n    \
         private ModCreativeTabs() {{}}\n}}\n"
    )
}

/// A dev/preview-only convenience: in the `amc play` dev client, drop the player straight into
/// **Creative** the moment they enter a world, so their creations are visible in the creative
/// inventory with zero fiddling. Gated on `!FMLEnvironment.production`, so it is completely inert in
/// a packaged/installed mod — a distributed jar never forces anyone's game mode.
fn mod_dev_preview_java(state: &CanonicalState, mod_class: &str) -> String {
    let modid = &state.namespace;
    format!(
        "package amc.{modid};\n\n\
         import com.mojang.logging.LogUtils;\n\
         import net.minecraft.server.level.ServerPlayer;\n\
         import net.minecraft.world.level.GameType;\n\
         import net.neoforged.bus.api.SubscribeEvent;\n\
         import net.neoforged.fml.common.EventBusSubscriber;\n\
         import net.neoforged.fml.loading.FMLEnvironment;\n\
         import net.neoforged.neoforge.event.entity.player.PlayerEvent;\n\
         import org.slf4j.Logger;\n\n\
         @EventBusSubscriber(modid = {mod_class}.MOD_ID)\n\
         public final class ModDevPreview {{\n    \
         private static final Logger LOGGER = LogUtils.getLogger();\n\n    \
         @SubscribeEvent\n    \
         public static void onPlayerJoin(PlayerEvent.PlayerLoggedInEvent event) {{\n        \
         if (!FMLEnvironment.production && event.getEntity() instanceof ServerPlayer player) {{\n            \
         player.setGameMode(GameType.CREATIVE);\n            \
         LOGGER.info(\"[AMC] preview: put {{}} in Creative so your creations are visible (press E)\", \
         player.getName().getString());\n        }}\n    }}\n\n    \
         private ModDevPreview() {{}}\n}}\n"
    )
}

/// The Java expression yielding a concept's `ItemLike` for the creative tab / recipes: block
/// concepts expose their block (an `ItemLike`) from `ModBlocks`, everything else its item.
fn item_accessor(concept: &Concept) -> String {
    let konst = concept_konst(concept);
    if concept.concept_type == ConceptType::Block {
        format!("ModBlocks.{konst}.get()")
    } else {
        format!("ModItems.{konst}.get()")
    }
}

/// Generate a GameTest class asserting each concept's item is registered (`registry_exists`,
/// `SPEC-004 §32`). Uses the vanilla `@GameTest` framework with a shared empty structure template
/// (`@PrefixGameTestTemplate(false)` + `template = "empty"` → `data/<modid>/structures/empty.nbt`)
/// and only guaranteed-stable APIs (throw-on-failure + `helper.succeed()`).
fn mod_gametests_java(state: &CanonicalState, mod_class: &str) -> String {
    let modid = &state.namespace;
    let mut methods = String::new();
    for concept in sorted_concepts(state) {
        let name = registry_name(concept);
        let konst = concept_konst(concept);
        // Every concept: it is actually registered in the game (FR-005 level: registry).
        methods.push_str(&format!(
            "    @GameTest(template = \"empty\")\n    \
             public static void {name}_registered(GameTestHelper helper) {{\n        \
             ResourceLocation id = ResourceLocation.fromNamespaceAndPath(\"{modid}\", \"{name}\");\n        \
             if (!BuiltInRegistries.ITEM.containsKey(id)) \
             throw new GameTestAssertException(\"{name} is not registered\");\n        \
             helper.succeed();\n    }}\n\n"
        ));
        // Behavioural checks: the concept doesn't just exist, it does its job (FR-005).
        match concept.concept_type {
            ConceptType::Block => methods.push_str(&format!(
                "    @GameTest(template = \"empty\")\n    \
                 public static void {name}_places(GameTestHelper helper) {{\n        \
                 BlockPos pos = new BlockPos(1, 1, 1);\n        \
                 helper.setBlock(pos, ModBlocks.{konst}.get());\n        \
                 helper.assertBlockPresent(ModBlocks.{konst}.get(), pos);\n        \
                 helper.succeed();\n    }}\n\n"
            )),
            ConceptType::Weapon => methods.push_str(&format!(
                "    @GameTest(template = \"empty\")\n    \
                 public static void {name}_is_weapon(GameTestHelper helper) {{\n        \
                 if (!(ModItems.{konst}.get() instanceof SwordItem)) \
                 throw new GameTestAssertException(\"{name} is not a functional weapon\");\n        \
                 helper.succeed();\n    }}\n\n"
            )),
            ConceptType::Tool => methods.push_str(&format!(
                "    @GameTest(template = \"empty\")\n    \
                 public static void {name}_is_tool(GameTestHelper helper) {{\n        \
                 if (!(ModItems.{konst}.get() instanceof DiggerItem)) \
                 throw new GameTestAssertException(\"{name} is not a functional tool\");\n        \
                 helper.succeed();\n    }}\n\n"
            )),
            ConceptType::Food => methods.push_str(&format!(
                "    @GameTest(template = \"empty\")\n    \
                 public static void {name}_is_edible(GameTestHelper helper) {{\n        \
                 if (!ModItems.{konst}.get().getDefaultInstance().has(DataComponents.FOOD)) \
                 throw new GameTestAssertException(\"{name} is not edible\");\n        \
                 helper.succeed();\n    }}\n\n"
            )),
            ConceptType::Item => {}
        }
    }
    // Import broadly; Java tolerates unused imports and the referenced Mod* classes always exist
    // when a concept of that type is present.
    format!(
        "package amc.{modid};\n\n\
         import net.minecraft.core.BlockPos;\n\
         import net.minecraft.core.component.DataComponents;\n\
         import net.minecraft.core.registries.BuiltInRegistries;\n\
         import net.minecraft.gametest.framework.GameTest;\n\
         import net.minecraft.gametest.framework.GameTestAssertException;\n\
         import net.minecraft.gametest.framework.GameTestHelper;\n\
         import net.minecraft.resources.ResourceLocation;\n\
         import net.minecraft.world.item.DiggerItem;\n\
         import net.minecraft.world.item.SwordItem;\n\
         import net.neoforged.neoforge.gametest.GameTestHolder;\n\
         import net.neoforged.neoforge.gametest.PrefixGameTestTemplate;\n\n\
         @GameTestHolder({mod_class}.MOD_ID)\n\
         @PrefixGameTestTemplate(false)\n\
         public final class ModGameTests {{\n\n\
         {methods}    private ModGameTests() {{}}\n}}\n"
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
                // createAttributes adds 1 (player base); pass attack_damage - 1 as the bonus.
                let bonus = attack_damage - 1.0;
                regs.push_str(&format!(
                    "    public static final Tier {konst}_TIER = new SimpleTier(\n        \
                     BlockTags.INCORRECT_FOR_STONE_TOOL, {durability}, 1.0F, 0.0F, \
                     {enchantability}, () -> Ingredient.EMPTY);\n    \
                     public static final DeferredItem<SwordItem> {konst} = ITEMS.register(\"{name}\",\n        \
                     () -> new SwordItem({konst}_TIER, new Item.Properties().attributes(\n            \
                     SwordItem.createAttributes({konst}_TIER, {bonus}, {speed}))));\n\n",
                    bonus = jfloat(bonus),
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
                    "    public static final Tier {konst}_TIER = new SimpleTier(\n        \
                     BlockTags.INCORRECT_FOR_STONE_TOOL, {durability}, {mining}, 0.0F, \
                     {enchantability}, () -> Ingredient.EMPTY);\n    \
                     public static final DeferredItem<{class}> {konst} = ITEMS.register(\"{name}\",\n        \
                     () -> new {class}({konst}_TIER, new Item.Properties().attributes(\n            \
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
                    "    public static final DeferredItem<Item> {konst} = ITEMS.register(\"{name}\",\n        \
                     () -> new Item(new Item.Properties().food(\n            \
                     new FoodProperties.Builder().nutrition({nutrition}).saturationModifier({sat}).build())));\n\n",
                    sat = jfloat(saturation),
                ));
            }
            ConceptType::Block => {
                // The block itself lives in ModBlocks; here we register its inventory item form so
                // it can be held, crafted, and shown in the creative tab.
                regs.push_str(&format!(
                    "    public static final DeferredItem<BlockItem> {konst} =\n        \
                     ITEMS.registerSimpleBlockItem(ModBlocks.{konst});\n\n"
                ));
            }
            ConceptType::Item => {
                regs.push_str(&format!(
                    "    public static final DeferredItem<Item> {konst} = \
                     ITEMS.registerSimpleItem(\"{name}\", new Item.Properties());\n\n"
                ));
            }
        }
    }

    // Import every item type we might emit; Java tolerates unused imports (unlike Rust).
    format!(
        "package amc.{modid};\n\n\
         import net.minecraft.tags.BlockTags;\n\
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
         import net.neoforged.neoforge.common.SimpleTier;\n\
         import net.neoforged.neoforge.registries.DeferredItem;\n\
         import net.neoforged.neoforge.registries.DeferredRegister;\n\n\
         public final class ModItems {{\n    \
         public static final DeferredRegister.Items ITEMS = DeferredRegister.createItems({mod_class}.MOD_ID);\n\n\
         {regs}    private ModItems() {{}}\n}}\n",
        modid = state.namespace,
    )
}

/// Blocks + their default properties. Block items are registered in [`mod_items_java`].
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
            "    public static final DeferredBlock<Block> {konst} = BLOCKS.registerSimpleBlock(\"{name}\",\n        \
             BlockBehaviour.Properties.of().strength({hardness}, {resistance}){light_call});\n\n",
            hardness = jfloat(hardness),
            resistance = jfloat(resistance),
        ));
    }
    format!(
        "package amc.{modid};\n\n\
         import net.minecraft.world.level.block.Block;\n\
         import net.minecraft.world.level.block.state.BlockBehaviour;\n\
         import net.neoforged.neoforge.registries.DeferredBlock;\n\
         import net.neoforged.neoforge.registries.DeferredRegister;\n\n\
         public final class ModBlocks {{\n    \
         public static final DeferredRegister.Blocks BLOCKS = DeferredRegister.createBlocks({mod_class}.MOD_ID);\n\n\
         {regs}    private ModBlocks() {{}}\n}}\n",
        modid = state.namespace,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest;
    use amc_state::ProjectStateManager;
    use serde_json::json;

    fn ruby_world() -> CanonicalState {
        let mut sm = ProjectStateManager::new("Ruby World", "amc_ruby_world");
        let tx = sm
            .open_candidate(&[
                json!({
                    "operation": "create_concept",
                    "payload": {
                        "concept_type": "item", "subtype": "ingredient", "display_name": "Ruby",
                        "namespace_hint": "ruby"
                    }
                }),
                json!({
                    "operation": "create_concept",
                    "payload": {
                        "concept_type": "weapon", "subtype": "sword", "display_name": "Ruby Blade",
                        "namespace_hint": "ruby_blade",
                        "properties": { "attack_damage": 9.0, "attack_speed": -2.4, "durability": 1200 }
                    }
                }),
            ])
            .unwrap();
        sm.commit(&tx.transaction_id, true).unwrap();
        sm.canonical().clone()
    }

    #[test]
    fn generates_expected_file_tree() {
        let files = build_files(&ruby_world());
        assert!(files.contains_key("build.gradle"));
        assert!(files.contains_key("src/main/resources/META-INF/neoforge.mods.toml"));
        assert!(files.contains_key("src/main/java/amc/amc_ruby_world/AmcRubyWorld.java"));
        assert!(files.contains_key("src/main/java/amc/amc_ruby_world/ModItems.java"));
        assert!(files
            .contains_key("src/main/resources/assets/amc_ruby_world/models/item/ruby_blade.json"));
        assert!(files
            .contains_key("src/main/resources/assets/amc_ruby_world/textures/item/ruby_blade.png"));
    }

    #[test]
    fn weapon_java_uses_sword_and_tier() {
        let files = build_files(&ruby_world());
        let java =
            String::from_utf8(files["src/main/java/amc/amc_ruby_world/ModItems.java"].clone())
                .unwrap();
        assert!(java.contains("RUBY_BLADE_TIER = new SimpleTier"));
        assert!(java.contains("new SwordItem(RUBY_BLADE_TIER"));
        // attack_damage 9.0 -> createAttributes bonus 8.0F
        assert!(java.contains("8.0F"));
        // basic item uses registerSimpleItem
        assert!(java.contains("registerSimpleItem(\"ruby\""));
    }

    #[test]
    fn weapon_model_uses_handheld_parent() {
        let files = build_files(&ruby_world());
        let model = String::from_utf8(
            files["src/main/resources/assets/amc_ruby_world/models/item/ruby_blade.json"].clone(),
        )
        .unwrap();
        assert!(model.contains("item/handheld"));
    }

    fn ruby_world_with_recipe() -> CanonicalState {
        let mut sm = ProjectStateManager::new("Ruby World", "amc_ruby_world");
        let tx = sm
            .open_candidate(&[
                json!({"operation":"create_concept","payload":{"concept_type":"item","display_name":"Ruby","namespace_hint":"ruby"}}),
                json!({"operation":"create_concept","payload":{"concept_type":"weapon","subtype":"sword","display_name":"Ruby Blade","namespace_hint":"ruby_blade","properties":{"attack_damage":9.0,"durability":1200}}}),
                json!({"operation":"add_recipe","payload":{"recipe":{
                    "recipe_type":"shaped","pattern":[" R "," R "," S "],
                    "key":{"R":{"reference_type":"display_name","value":"Ruby"},"S":{"reference_type":"minecraft_id","value":"minecraft:stick"}},
                    "result":{"reference_type":"display_name","value":"Ruby Blade"},"count":1}}}),
            ])
            .unwrap();
        sm.commit(&tx.transaction_id, true).unwrap();
        sm.canonical().clone()
    }

    #[test]
    fn generates_shaped_recipe_json() {
        let files = build_files(&ruby_world_with_recipe());
        let path = "src/main/resources/data/amc_ruby_world/recipe/ruby_blade.json";
        let recipe = String::from_utf8(files[path].clone()).unwrap();
        assert!(recipe.contains("minecraft:crafting_shaped"));
        assert!(recipe.contains("amc_ruby_world:ruby")); // ruby ingredient resolved to registry id
        assert!(recipe.contains("minecraft:stick")); // vanilla passthrough
        assert!(recipe.contains("amc_ruby_world:ruby_blade")); // result
    }

    fn all_types_world() -> CanonicalState {
        let mut sm = ProjectStateManager::new("Ruby World", "amc_ruby_full");
        let tx = sm
            .open_candidate(&[
                json!({"operation":"create_concept","payload":{"concept_type":"tool","subtype":"pickaxe","display_name":"Ruby Pickaxe","namespace_hint":"ruby_pickaxe","properties":{"durability":1200,"mining_speed":8.0}}}),
                json!({"operation":"create_concept","payload":{"concept_type":"food","display_name":"Ruby Apple","namespace_hint":"ruby_apple","properties":{"nutrition":6,"saturation":0.8}}}),
                json!({"operation":"create_concept","payload":{"concept_type":"block","subtype":"ore","display_name":"Ruby Block","namespace_hint":"ruby_block","properties":{"hardness":5.0,"blast_resistance":6.0,"light_level":10}}}),
            ])
            .unwrap();
        sm.commit(&tx.transaction_id, true).unwrap();
        sm.canonical().clone()
    }

    #[test]
    fn tool_uses_digger_class_and_food_uses_food_properties() {
        let files = build_files(&all_types_world());
        let items =
            String::from_utf8(files["src/main/java/amc/amc_ruby_full/ModItems.java"].clone())
                .unwrap();
        assert!(items.contains("new PickaxeItem(RUBY_PICKAXE_TIER"));
        assert!(items.contains("PickaxeItem.createAttributes"));
        assert!(items.contains("FoodProperties.Builder().nutrition(6).saturationModifier(0.8F)"));
        // The block's inventory item form is registered from ModBlocks.
        assert!(items.contains("registerSimpleBlockItem(ModBlocks.RUBY_BLOCK)"));
    }

    #[test]
    fn block_generates_block_class_and_placement_resources() {
        let files = build_files(&all_types_world());
        let blocks =
            String::from_utf8(files["src/main/java/amc/amc_ruby_full/ModBlocks.java"].clone())
                .unwrap();
        assert!(blocks.contains("registerSimpleBlock(\"ruby_block\""));
        assert!(blocks.contains("strength(5.0F, 6.0F)"));
        assert!(blocks.contains(".lightLevel"));
        // Placement + drop resources exist.
        assert!(files
            .contains_key("src/main/resources/assets/amc_ruby_full/blockstates/ruby_block.json"));
        assert!(files
            .contains_key("src/main/resources/assets/amc_ruby_full/models/block/ruby_block.json"));
        assert!(files.contains_key(
            "src/main/resources/data/amc_ruby_full/loot_table/blocks/ruby_block.json"
        ));
        assert!(files
            .contains_key("src/main/resources/assets/amc_ruby_full/textures/block/ruby_block.png"));
    }

    #[test]
    fn creative_tab_adds_each_item_to_its_vanilla_tab() {
        let files = build_files(&all_types_world());
        let tab = String::from_utf8(
            files["src/main/java/amc/amc_ruby_full/ModCreativeTabs.java"].clone(),
        )
        .unwrap();
        // Items go into the standard vanilla tabs via BuildCreativeModeTabContentsEvent.
        assert!(tab.contains("BuildCreativeModeTabContentsEvent event"));
        assert!(tab.contains("event.getTabKey() == CreativeModeTabs.TOOLS_AND_UTILITIES"));
        assert!(tab.contains("event.accept(ModItems.RUBY_PICKAXE.get())"));
        assert!(tab.contains("event.getTabKey() == CreativeModeTabs.FOOD_AND_DRINKS"));
        assert!(tab.contains("event.accept(ModItems.RUBY_APPLE.get())"));
        assert!(tab.contains("event.getTabKey() == CreativeModeTabs.BUILDING_BLOCKS"));
        assert!(tab.contains("event.accept(ModBlocks.RUBY_BLOCK.get())")); // block via its ItemLike
                                                                           // The listener is wired onto the mod event bus.
        let main =
            String::from_utf8(files["src/main/java/amc/amc_ruby_full/AmcRubyFull.java"].clone())
                .unwrap();
        assert!(main.contains("ModBlocks.BLOCKS.register"));
        assert!(main.contains("modEventBus.addListener(ModCreativeTabs::buildContents)"));
    }

    #[test]
    fn gametests_include_behavioral_checks_not_just_registration() {
        let files = build_files(&all_types_world());
        let tests =
            String::from_utf8(files["src/main/java/amc/amc_ruby_full/ModGameTests.java"].clone())
                .unwrap();
        // Registration for every concept, plus behaviour per type (FR-005 depth).
        assert!(tests.contains("ruby_block_registered"));
        assert!(tests.contains("ruby_block_places"));
        assert!(tests.contains("helper.setBlock(pos, ModBlocks.RUBY_BLOCK.get())"));
        assert!(tests.contains("ruby_pickaxe_is_tool"));
        assert!(tests.contains("instanceof DiggerItem"));
        assert!(tests.contains("ruby_apple_is_edible"));
        assert!(tests.contains("has(DataComponents.FOOD)"));
    }

    #[test]
    fn dev_preview_forces_creative_only_outside_production() {
        let files = build_files(&all_types_world());
        let preview =
            String::from_utf8(files["src/main/java/amc/amc_ruby_full/ModDevPreview.java"].clone())
                .unwrap();
        assert!(preview.contains("PlayerEvent.PlayerLoggedInEvent"));
        assert!(preview.contains("player.setGameMode(GameType.CREATIVE)"));
        // Guarded so a distributed/installed mod never forces anyone's game mode.
        assert!(preview.contains("!FMLEnvironment.production"));
    }

    #[test]
    fn output_hash_is_deterministic() {
        // COMP-GEN-009: identical state + generator version -> identical output hash.
        let state = ruby_world();
        let a = manifest::GenerationManifest::build(
            crate::GENERATOR_VERSION,
            MINECRAFT_TARGET,
            &build_files(&state),
        );
        let b = manifest::GenerationManifest::build(
            crate::GENERATOR_VERSION,
            MINECRAFT_TARGET,
            &build_files(&state),
        );
        assert_eq!(a.output_hash, b.output_hash);
    }

    #[test]
    fn generate_writes_files_and_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let gen = NeoForgeGenerator;
        let generated = gen.generate(&ruby_world(), dir.path()).unwrap();
        assert!(dir.path().join("build.gradle").exists());
        assert!(generated.output_hash().starts_with("sha256:"));
        assert!(!generated.manifest.files.is_empty());
    }

    #[test]
    fn regeneration_prunes_a_removed_recipe_and_a_deleted_concept() {
        let dir = tempfile::tempdir().unwrap();
        let gen = NeoForgeGenerator;
        let mut sm = ProjectStateManager::new("Ruby World", "amc_ruby_world");
        let tx = sm
            .open_candidate(&[
                json!({"operation":"create_concept","payload":{"concept_type":"item","display_name":"Ruby","namespace_hint":"ruby"}}),
                json!({"operation":"create_concept","payload":{"concept_type":"weapon","subtype":"sword","display_name":"Ruby Blade","namespace_hint":"ruby_blade","properties":{"attack_damage":9.0,"durability":1200}}}),
                json!({"operation":"add_recipe","payload":{"recipe":{"recipe_type":"shapeless","ingredients":[{"reference_type":"minecraft_id","value":"minecraft:stick"}],"result":{"reference_type":"display_name","value":"Ruby Blade"},"count":1}}}),
            ])
            .unwrap();
        sm.commit(&tx.transaction_id, true).unwrap();
        gen.generate(sm.canonical(), dir.path()).unwrap();
        let recipe = dir
            .path()
            .join("src/main/resources/data/amc_ruby_world/recipe/ruby_blade.json");
        let ruby_model = dir
            .path()
            .join("src/main/resources/assets/amc_ruby_world/models/item/ruby.json");
        assert!(recipe.exists(), "recipe should be generated first");
        assert!(ruby_model.exists(), "Ruby's item model should be generated");

        // Remove the recipe AND delete the Ruby concept, then regenerate to the SAME dir.
        let tx = sm
            .open_candidate(&[
                json!({"operation":"remove_recipe","payload":{"target":{"reference_type":"display_name","value":"Ruby Blade"}}}),
                json!({"operation":"delete_concept","payload":{"target":{"reference_type":"display_name","value":"Ruby"}}}),
            ])
            .unwrap();
        sm.commit(&tx.transaction_id, true).unwrap();
        gen.generate(sm.canonical(), dir.path()).unwrap();

        // The removed recipe's file and the deleted concept's model are pruned; the now-empty recipe
        // directory is gone too. The still-active Ruby Blade weapon is untouched.
        assert!(!recipe.exists(), "removed recipe file must be pruned");
        assert!(
            !recipe.parent().unwrap().exists(),
            "empty recipe directory must be pruned"
        );
        assert!(
            !ruby_model.exists(),
            "deleted concept's model must be pruned"
        );
        assert!(dir
            .path()
            .join("src/main/resources/assets/amc_ruby_world/models/item/ruby_blade.json")
            .exists());
    }
}
