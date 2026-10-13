//! Generate a project exercising every concept type (item, weapon, tool, food, block) + a recipe +
//! a creative tab, so the generated Java can be compiled against NeoForge to validate all generators.
//!
//! Usage: `cargo run -p amc-generator --example full_demo -- <output_dir>`

use std::path::Path;

use amc_generator::{Generator, NeoForgeGenerator};
use amc_state::ProjectStateManager;
use serde_json::json;

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "full_demo_out".into());
    let mut sm = ProjectStateManager::new("Ruby World", "amc_ruby_full");
    let tx = sm
        .open_candidate(&[
            json!({"operation":"create_concept","payload":{
                "concept_type":"item","subtype":"ingredient","display_name":"Ruby","namespace_hint":"ruby"}}),
            json!({"operation":"create_concept","payload":{
                "concept_type":"weapon","subtype":"sword","display_name":"Ruby Blade","namespace_hint":"ruby_blade",
                "properties":{"attack_damage":9.0,"attack_speed":-2.4,"durability":1200,"enchantability":14}}}),
            json!({"operation":"create_concept","payload":{
                "concept_type":"tool","subtype":"pickaxe","display_name":"Ruby Pickaxe","namespace_hint":"ruby_pickaxe",
                "properties":{"durability":1200,"mining_speed":8.0,"attack_damage":1.0,"attack_speed":-2.8}}}),
            json!({"operation":"create_concept","payload":{
                "concept_type":"food","display_name":"Ruby Apple","namespace_hint":"ruby_apple",
                "properties":{"nutrition":6,"saturation":0.8}}}),
            json!({"operation":"create_concept","payload":{
                "concept_type":"block","subtype":"ore","display_name":"Ruby Block","namespace_hint":"ruby_block",
                "properties":{"hardness":5.0,"blast_resistance":6.0,"light_level":10}}}),
        ])
        .expect("valid operations");
    sm.commit(&tx.transaction_id, true).expect("commit");

    let generated = NeoForgeGenerator
        .generate(sm.canonical(), Path::new(&out))
        .expect("generation");
    println!(
        "generated {} files to {out}",
        generated.manifest.files.len()
    );
}
