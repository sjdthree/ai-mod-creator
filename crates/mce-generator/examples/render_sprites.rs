//! Dev helper: render each placeholder shape to a 16×16 PNG so the art can be eyeballed.
//! Usage: cargo run -p mce-generator --example render_sprites -- <out_dir>

use std::path::Path;

use mce_generator::texture::{sprite_png, Palette, Shape};

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "sprites".into());
    std::fs::create_dir_all(&out).unwrap();
    let pal = Palette::default_for(184_201);
    for (name, shape) in [
        ("sword", Shape::Sword),
        ("pickaxe", Shape::Pickaxe),
        ("axe", Shape::Axe),
        ("shovel", Shape::Shovel),
        ("hoe", Shape::Hoe),
        ("food", Shape::Food),
        ("gem", Shape::Gem),
    ] {
        let png = sprite_png(184_201, &pal, shape);
        std::fs::write(Path::new(&out).join(format!("{name}.png")), png).unwrap();
    }
    println!("wrote sprites to {out}");
}
