//! Phase 3 spike helper — generate the full Ruby Blade project (weapon + ingredient + shaped
//! recipe) to a directory, so it can be built with Gradle against NeoForge.
//!
//! Usage: `cargo run -p mce-generator --example ruby_world -- <output_dir>`

use std::path::Path;

use mce_generator::{Generator, NeoForgeGenerator};
use mce_state::ProjectStateManager;
use serde_json::json;

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "ruby_world_out".to_string());

    let mut sm = ProjectStateManager::new("Ruby World", "mce_ruby_world");
    let tx = sm
        .open_candidate(&[
            json!({"operation":"create_concept","payload":{
                "concept_type":"item","subtype":"ingredient",
                "display_name":"Ruby","namespace_hint":"ruby"}}),
            json!({"operation":"create_concept","payload":{
                "concept_type":"weapon","subtype":"sword",
                "display_name":"Ruby Blade","namespace_hint":"ruby_blade",
                "properties":{"attack_damage":9.0,"attack_speed":-2.4,"durability":1200,"enchantability":14}}}),
            json!({"operation":"add_recipe","payload":{"recipe":{
                "recipe_type":"shaped","pattern":[" R "," R "," S "],
                "key":{"R":{"reference_type":"display_name","value":"Ruby"},
                       "S":{"reference_type":"minecraft_id","value":"minecraft:stick"}},
                "result":{"reference_type":"display_name","value":"Ruby Blade"},"count":1}}}),
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
    println!("output_hash: {}", generated.output_hash());
}
