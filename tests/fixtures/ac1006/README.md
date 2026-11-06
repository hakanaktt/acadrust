# AC1006 (AutoCAD R10) samples

The 32 drawings in this folder are copied unchanged from
[CAD-Format-DWG-AC1006](https://github.com/michal-josef-spacek/CAD-Format-DWG-AC1006)
by Michal Josef Špaček, `t/data` at commit
`5f490a103a147f53776dcb433867e35cac848938`, and keep that folder layout. They
are distributed under the BSD-2-Clause licence in `LICENSE` next to this file.

Each file is a small R10 drawing with one entity type or one non-default
header setting. The expected values in `tests/dwg_ac1006_read.rs` come from
the upstream Perl tests (`t/CAD-Format-DWG-AC1006/*.t`) for the same files.

| File | SHA-256 |
|---|---|
| `blank/BLANK.DWG` | `8e76b67b7cc072235f91bb4e1e122cf902b7d12712e69572aa879a709d59c9b7` |
| `entity/line/LINE1.DWG` | `29586c80cfe26346f3004e7f81b2612732ecb99bef4d7739073133af84a349f8` |
