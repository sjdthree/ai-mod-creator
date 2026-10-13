//! Minimal empty GameTest structure template (`.nbt`) generation.
//!
//! NeoForge GameTests require a structure template NBT to define the test area (ADR-0004). Rather
//! than depend on NeoForge's heavier test framework (`@EmptyTemplate`), we emit one shared, tiny
//! empty 3×3×3 structure that every generated GameTest references via `@GameTest(template =
//! "empty")`. Minecraft loads structure templates with `NbtIo.readCompressed` (gzip), so the bytes
//! are gzip-compressed NBT.

use std::io::Write;

use flate2::{write::GzEncoder, Compression};

/// Minecraft 1.21.1 data version (for the structure's `DataVersion` tag).
const DATA_VERSION: i32 = 3955;

fn write_name(buf: &mut Vec<u8>, name: &str) {
    buf.extend_from_slice(&(name.len() as u16).to_be_bytes());
    buf.extend_from_slice(name.as_bytes());
}

fn tag_header(buf: &mut Vec<u8>, tag_type: u8, name: &str) {
    buf.push(tag_type);
    write_name(buf, name);
}

/// Build the uncompressed NBT for an empty 3×3×3 structure with a single air block.
fn empty_structure_nbt_uncompressed() -> Vec<u8> {
    const TAG_END: u8 = 0;
    const TAG_INT: u8 = 3;
    const TAG_STRING: u8 = 8;
    const TAG_LIST: u8 = 9;
    const TAG_COMPOUND: u8 = 10;

    let mut b = Vec::new();

    // Root compound (unnamed).
    b.push(TAG_COMPOUND);
    write_name(&mut b, "");

    // DataVersion: Int
    tag_header(&mut b, TAG_INT, "DataVersion");
    b.extend_from_slice(&DATA_VERSION.to_be_bytes());

    // size: List<Int> [3,3,3]
    tag_header(&mut b, TAG_LIST, "size");
    b.push(TAG_INT);
    b.extend_from_slice(&3i32.to_be_bytes());
    for _ in 0..3 {
        b.extend_from_slice(&3i32.to_be_bytes());
    }

    // palette: List<Compound> [ { Name: "minecraft:air" } ]
    tag_header(&mut b, TAG_LIST, "palette");
    b.push(TAG_COMPOUND);
    b.extend_from_slice(&1i32.to_be_bytes());
    tag_header(&mut b, TAG_STRING, "Name");
    write_name(&mut b, "minecraft:air");
    b.push(TAG_END); // end palette[0]

    // blocks: List<Compound> [ { pos: [0,0,0], state: 0 } ]
    tag_header(&mut b, TAG_LIST, "blocks");
    b.push(TAG_COMPOUND);
    b.extend_from_slice(&1i32.to_be_bytes());
    tag_header(&mut b, TAG_LIST, "pos");
    b.push(TAG_INT);
    b.extend_from_slice(&3i32.to_be_bytes());
    for _ in 0..3 {
        b.extend_from_slice(&0i32.to_be_bytes());
    }
    tag_header(&mut b, TAG_INT, "state");
    b.extend_from_slice(&0i32.to_be_bytes());
    b.push(TAG_END); // end blocks[0]

    // entities: empty List<Compound>
    tag_header(&mut b, TAG_LIST, "entities");
    b.push(TAG_COMPOUND);
    b.extend_from_slice(&0i32.to_be_bytes());

    // End root compound.
    b.push(TAG_END);
    b
}

/// The gzip-compressed empty structure template bytes.
pub fn empty_structure_nbt() -> Vec<u8> {
    let raw = empty_structure_nbt_uncompressed();
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&raw).expect("gzip write");
    encoder.finish().expect("gzip finish")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_gzipped_nbt() {
        let bytes = empty_structure_nbt();
        // gzip magic bytes.
        assert_eq!(&bytes[0..2], &[0x1f, 0x8b]);
        assert!(bytes.len() > 20);
    }

    #[test]
    fn uncompressed_starts_with_root_compound() {
        let raw = empty_structure_nbt_uncompressed();
        // TAG_Compound (10), then 2-byte name length 0.
        assert_eq!(raw[0], 10);
        assert_eq!(&raw[1..3], &[0, 0]);
    }
}
