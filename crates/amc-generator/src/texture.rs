//! Deterministic placeholder textures (`SPEC-004 §17`, `SPEC-005 §22`) — Phase 2 work item
//! `0005`.
//!
//! A pure function of `(asset_seed, palette)` producing a 16×16 RGBA PNG. No wall-clock and no
//! randomness beyond a seeded PRNG, so the same inputs always yield byte-identical output.

const SIZE: usize = 16;

/// An RGB colour.
#[derive(Debug, Clone, Copy)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// The three-colour palette used to fill a placeholder texture.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub primary: Rgb,
    pub secondary: Rgb,
    pub highlight: Rgb,
}

impl Palette {
    /// A neutral default palette used when a concept has no appearance.
    pub fn default_for(seed: u64) -> Self {
        // Derive a stable-but-varied hue from the seed so different concepts look different.
        let base = (seed % 200 + 30) as u8;
        Palette {
            primary: Rgb(base, base / 2, base / 3),
            secondary: Rgb(base / 3, base / 4, base / 5),
            highlight: Rgb(base.saturating_add(40), base / 2 + 20, base / 3 + 20),
        }
    }
}

/// Parse `#RRGGBB` into an [`Rgb`], returning `None` if malformed.
pub fn parse_hex(hex: &str) -> Option<Rgb> {
    let h = hex.strip_prefix('#')?;
    if h.len() != 6 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let r = u8::from_str_radix(&h[0..2], 16).ok()?;
    let g = u8::from_str_radix(&h[2..4], 16).ok()?;
    let b = u8::from_str_radix(&h[4..6], 16).ok()?;
    Some(Rgb(r, g, b))
}

/// A tiny deterministic PRNG (SplitMix64) — no `Math.random`, no wall-clock.
struct Prng(u64);

impl Prng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// The silhouette a placeholder sprite is drawn as, so an item reads as what it is (a sword looks
/// like a sword, not a square). [`Shape::Block`] fills the whole tile (blocks are cubes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Sword,
    Pickaxe,
    Axe,
    Shovel,
    Hoe,
    Food,
    Gem,
    Block,
}

// 16×16 masks. '.' = transparent, 'e' = highlight (bright edge), 'b' = primary (body),
// 'g'/'h' = secondary (guard/handle/outline). Row 0 is the top.
#[rustfmt::skip]
const SWORD: [&str; 16] = [
    "..........ee....",
    ".........ebb....",
    "........ebb.....",
    ".......ebb......",
    "......ebb.......",
    ".....ebb........",
    "....ebb.........",
    "...ebbg.........",
    "..gggbg.........",
    "..ggbgg.........",
    "...hh...........",
    "..hh............",
    "..hh............",
    ".hh.............",
    ".hh.............",
    "................",
];
#[rustfmt::skip]
const PICKAXE: [&str; 16] = [
    ".e..........e...",
    ".eeb......bee...",
    "...eeb..beeb....",
    "....ebeebb......",
    "......ehb.......",
    ".....hbeh.......",
    ".....hbh........",
    "....hbh.........",
    "....hbh.........",
    "...hbh..........",
    "...hbh..........",
    "..hbh...........",
    "..hbh...........",
    ".hbh............",
    ".hh.............",
    "................",
];
#[rustfmt::skip]
const AXE: [&str; 16] = [
    ".....eeee.......",
    "....ebbbbe......",
    "...ebbbbbe......",
    "...ebbbbb.......",
    "...ebbbbh.......",
    "...ebbbh........",
    "....ehbh........",
    ".....hbh........",
    ".....hbh........",
    "....hbh.........",
    "....hbh.........",
    "...hbh..........",
    "...hbh..........",
    "..hbh...........",
    "..hh............",
    "................",
];
#[rustfmt::skip]
const SHOVEL: [&str; 16] = [
    ".........ee.....",
    "........ebbe....",
    "........ebbe....",
    "........ebbe....",
    ".......eebbee...",
    ".......ebbbbe...",
    "........ebbe....",
    ".......hbh......",
    "......hbh.......",
    ".....hbh........",
    ".....hbh........",
    "....hbh.........",
    "...hbh..........",
    "...hbh..........",
    "..hh............",
    "................",
];
#[rustfmt::skip]
const HOE: [&str; 16] = [
    "....eeeee...e...",
    "...ebbbbeebbe...",
    "...ebbbbeebb....",
    "...eeeeh........",
    "......hbh.......",
    ".....hbh........",
    ".....hbh........",
    "....hbh.........",
    "....hbh.........",
    "...hbh..........",
    "...hbh..........",
    "..hbh...........",
    "..hbh...........",
    ".hbh............",
    ".hh.............",
    "................",
];
#[rustfmt::skip]
const FOOD: [&str; 16] = [
    ".......h........",
    "......hg........",
    ".....hg.........",
    "...eebbee.......",
    "..ebbbbbbe......",
    ".ebbbbbbbbe.....",
    ".ebbbbbbbbe.....",
    ".ebbbbbbbbe.....",
    ".ebbbbbbbbe.....",
    ".ebbbbbbbbe.....",
    "..ebbbbbbe......",
    "..ebbbbbbe......",
    "...ebbbbe.......",
    "....eebe........",
    "................",
    "................",
];
#[rustfmt::skip]
const GEM: [&str; 16] = [
    "................",
    "....eeeeee......",
    "...ebbbbbbe.....",
    "..eebbbbbbee....",
    ".ebbbbbbbbbbe...",
    ".ebbbbbbbbbbe...",
    "..ebbbbbbbbe....",
    "...ebbbbbbe.....",
    "...ebbbbbe......",
    "....ebbbe.......",
    ".....ebe........",
    "......e.........",
    "................",
    "................",
    "................",
    "................",
];

fn mask(shape: Shape) -> &'static [&'static str; 16] {
    match shape {
        Shape::Sword => &SWORD,
        Shape::Pickaxe => &PICKAXE,
        Shape::Axe => &AXE,
        Shape::Shovel => &SHOVEL,
        Shape::Hoe => &HOE,
        Shape::Food => &FOOD,
        Shape::Gem => &GEM,
        Shape::Block => &SWORD, // unused: Block goes through placeholder_png
    }
}

/// Encode a deterministic 16×16 RGBA PNG of a recognizable item silhouette. Blocks fill the whole
/// tile (they render as cubes); everything else is a transparent-background shaped sprite so a sword
/// reads as a sword, a pickaxe as a pickaxe, etc. Deterministic in `(seed, palette, shape)`.
pub fn sprite_png(seed: u64, palette: &Palette, shape: Shape) -> Vec<u8> {
    if shape == Shape::Block {
        return placeholder_png(seed, palette);
    }
    let m = mask(shape);
    let mut pixels = vec![0u8; SIZE * SIZE * 4]; // transparent background
    for (y, row) in m.iter().enumerate() {
        let bytes = row.as_bytes();
        for x in 0..SIZE {
            let (color, alpha) = match bytes.get(x).copied().unwrap_or(b'.') {
                b'e' => (palette.highlight, 255),
                b'b' => (palette.primary, 255),
                b'g' | b'h' => (palette.secondary, 255),
                _ => (Rgb(0, 0, 0), 0),
            };
            let i = (y * SIZE + x) * 4;
            pixels[i] = color.0;
            pixels[i + 1] = color.1;
            pixels[i + 2] = color.2;
            pixels[i + 3] = alpha;
        }
    }
    encode(&pixels)
}

/// Encode a deterministic 16×16 RGBA PNG from a seed and palette.
pub fn placeholder_png(seed: u64, palette: &Palette) -> Vec<u8> {
    let mut prng = Prng(seed.wrapping_add(1));
    let mut pixels = Vec::with_capacity(SIZE * SIZE * 4);

    for y in 0..SIZE {
        for x in 0..SIZE {
            // A 1px border uses the secondary colour to read as an item-ish sprite.
            let border = x == 0 || y == 0 || x == SIZE - 1 || y == SIZE - 1;
            let color = if border {
                palette.secondary
            } else {
                match prng.next() % 100 {
                    0..=59 => palette.primary,
                    60..=89 => palette.secondary,
                    _ => palette.highlight,
                }
            };
            pixels.extend_from_slice(&[color.0, color.1, color.2, 255]);
        }
    }

    encode(&pixels)
}

fn encode(pixels: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, SIZE as u32, SIZE as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("png header");
        writer.write_image_data(pixels).expect("png data");
        writer.finish().expect("png finish");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pal() -> Palette {
        Palette {
            primary: Rgb(122, 11, 24),
            secondary: Rgb(43, 17, 21),
            highlight: Rgb(227, 72, 91),
        }
    }

    #[test]
    fn same_seed_is_byte_identical() {
        assert_eq!(
            placeholder_png(184201, &pal()),
            placeholder_png(184201, &pal())
        );
    }

    #[test]
    fn different_seed_differs() {
        assert_ne!(placeholder_png(1, &pal()), placeholder_png(2, &pal()));
    }

    #[test]
    fn sprite_is_deterministic_and_has_transparency() {
        let a = sprite_png(184201, &pal(), Shape::Sword);
        assert_eq!(a, sprite_png(184201, &pal(), Shape::Sword)); // stable
        assert_ne!(a, sprite_png(184201, &pal(), Shape::Food)); // shape matters
                                                                // A block fills the tile (no transparent background); a sword does not.
        assert_ne!(
            sprite_png(1, &pal(), Shape::Block),
            sprite_png(1, &pal(), Shape::Sword)
        );
    }

    #[test]
    fn parses_hex() {
        let Rgb(r, g, b) = parse_hex("#7A0B18").unwrap();
        assert_eq!((r, g, b), (0x7A, 0x0B, 0x18));
        assert!(parse_hex("7A0B18").is_none());
        assert!(parse_hex("#xyz").is_none());
    }
}
