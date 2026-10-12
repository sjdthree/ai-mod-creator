# 0006 — Generation manifest & output hashing

- **Phase:** 2
- **Workstream:** core-engine / qa
- **Effort:** Medium
- **Owner:** unassigned
- **Status:** done

## Objective

Emit a generation manifest listing every generated file with its content hash, and a single
stable `output_hash` proving byte-level reproducibility.

## Background

`SPEC-004 §31` (`generator_version`), `SPEC-005 §22` (generator determinism), `SPEC-003 §9`
(generation manifest + output hashes).

## Dependencies

`phase2/0002`, `phase2/0003`, `phase2/0005`.

## Implementation Notes

- Build files in memory as `path -> bytes`, sorted by path; write to `output_root`.
- Manifest: `{ generator_version, files: [{path, sha256}], output_hash }`. `output_hash` =
  canonical hash over the sorted `(path, sha256)` list.
- No timestamps or absolute paths in hashed content.

## Acceptance Tests

- `COMP-GEN-009`: identical state + generator version → identical `output_hash`.
- The manifest lists every written file with a matching sha256.

## Benchmark Coverage

`COMP-GEN-009`.

## Traceability

- Product requirement(s): `FR-007` (repeatability).
- Component(s): `mce-generator`.
