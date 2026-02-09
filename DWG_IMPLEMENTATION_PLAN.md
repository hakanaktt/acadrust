# DWG Reader/Writer Implementation Plan for acadrust

> **Version:** 1.0  
> **Date:** February 9, 2026  
> **Reference:** ACadSharp C# implementation (~59 files, ~890 KB of source)

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [DWG File Format Overview](#2-dwg-file-format-overview)
3. [Existing Architecture Analysis](#3-existing-architecture-analysis)
4. [Proposed File/Folder Structure](#4-proposed-filefolder-structure)
5. [Module-by-Module Design](#5-module-by-module-design)
6. [Phase 1 — Foundation Layer](#6-phase-1--foundation-layer)
7. [Phase 2 — Bit-Level Stream I/O](#7-phase-2--bit-level-stream-io)
8. [Phase 3 — File Structure & Section Management](#8-phase-3--file-structure--section-management)
9. [Phase 4 — Section Readers](#9-phase-4--section-readers)
10. [Phase 5 — Object Reader (Entities & Objects)](#10-phase-5--object-reader-entities--objects)
11. [Phase 6 — Document Builder](#11-phase-6--document-builder)
12. [Phase 7 — DWG Reader Orchestrator](#12-phase-7--dwg-reader-orchestrator)
13. [Phase 8 — Section Writers](#13-phase-8--section-writers)
14. [Phase 9 — Object Writer (Entities & Objects)](#14-phase-9--object-writer-entities--objects)
15. [Phase 10 — DWG Writer Orchestrator](#15-phase-10--dwg-writer-orchestrator)
16. [Phase 11 — Testing Strategy](#16-phase-11--testing-strategy)
17. [Phase 12 — Performance & Optimization](#17-phase-12--performance--optimization)
18. [Version Support Matrix](#18-version-support-matrix)
19. [Risk Analysis & Mitigations](#19-risk-analysis--mitigations)
20. [Dependencies & Crate Selection](#20-dependencies--crate-selection)
21. [Implementation Priority & Timeline](#21-implementation-priority--timeline)

---

## 1. Executive Summary

This document details the implementation plan for adding DWG (native AutoCAD drawing) read/write support to the `acadrust` library. The plan mirrors the existing DXF reader/writer folder structure and leverages the shared `CadDocument` data model, entity system, table system, and object system.

### Key Differences from DXF

| Aspect | DXF | DWG |
|--------|-----|-----|
| **Data format** | Text or binary code/value pairs | Bit-aligned binary stream |
| **Section access** | Sequential section scanning | Handle-based object map + section locators |
| **Compression** | None | LZ77 (AC18/AC21 variants) |
| **Encryption** | None | XOR-based encryption (AC18+) |
| **Integrity** | None | CRC-8, CRC-32, Adler-32 checksums |
| **Error correction** | None | Reed-Solomon (AC21) |
| **Object discovery** | Sequential reading | Queue-based handle tracing |
| **Multi-stream** | Single stream | 3 sub-streams per object (R2007+) |

### Scope

- **Read support:** R13 (AC1012) through R2018 (AC1032)
- **Write support:** R14 (AC1014) through R2018 (AC1032), excluding R2007 (AC1021)
- **Shared code:** `CadDocument`, all entities, tables, objects, classes, XData, error types, version enum

---

## 2. DWG File Format Overview

### 2.1 File Layout by Version

#### R13–R2000 (AC1012/AC1014/AC1015) — Simple Record Layout

```
┌──────────────────────────────────┐
│ File Header (variable, ~400 B)   │  Version string + section records + CRC8
├──────────────────────────────────┤
│ AcDb:Header                      │  System variables (bit-encoded)
├──────────────────────────────────┤
│ AcDb:Classes                     │  DXF class definitions
├──────────────────────────────────┤
│ AcDb:Handles (Object Map)        │  Delta-encoded handle→offset pairs
├──────────────────────────────────┤
│ AcDb:ObjFreeSpace                │  Free space info
├──────────────────────────────────┤
│ AcDb:Template                    │  Template metadata
├──────────────────────────────────┤
│ AcDb:AuxHeader                   │  Auxiliary header (dates, version stamps)
├──────────────────────────────────┤
│ AcDb:AcDbObjects                 │  All entities/tables/objects (bit-encoded)
├──────────────────────────────────┤
│ AcDb:Preview                     │  Thumbnail image (BMP/WMF)
└──────────────────────────────────┘
```

File header contains 6 section locator records (number + absolute file offset + size).

#### R2004+ (AC1018/AC1024/AC1027/AC1032) — Page-Based Layout

```
┌──────────────────────────────────┐
│ File Header (0x100 bytes)        │  Version string + metadata (plaintext)
├──────────────────────────────────┤
│ Encrypted Header (0x6C bytes)    │  XOR-encrypted via CRC32 LCG
│ @ offset 0x80                    │  Contains page map address, section map ID
├──────────────────────────────────┤
│ Data Pages (0x7400 each)         │  Encrypted page headers + LZ77-compressed
│ ├─ Page Map pages                │  pageNumber → file offset mapping
│ ├─ Section Map pages             │  section descriptors + local page lists
│ ├─ AcDb:Header pages             │
│ ├─ AcDb:Classes pages            │
│ ├─ AcDb:Handles pages            │
│ ├─ AcDb:AcDbObjects pages        │
│ ├─ AcDb:SummaryInfo pages        │
│ ├─ AcDb:Preview pages            │
│ ├─ AcDb:AppInfo pages            │
│ └─ ...other sections             │
└──────────────────────────────────┘
```

Each data page: 8 encrypted int32 fields (type, section#, compSize, pageSize, startOffset, headerCRC, dataCRC, unknown) + compressed payload. Decryption mask: `0x4164536B ^ stream_position`.

#### R2007 (AC1021) — Reed-Solomon + LZ77 AC21

```
┌──────────────────────────────────┐
│ File Header (0x80 bytes)         │  Version string + metadata (plaintext)
├──────────────────────────────────┤
│ RS-Encoded Block (0x400 bytes)   │  Reed-Solomon (255,239) encoded
│ @ offset 0x80                    │  Decoded → LZ77 AC21 → CompressedMetadata
├──────────────────────────────────┤
│ Data Pages @ base 0x480          │  RS-encoded paging + LZ77 AC21
│ ├─ Page Map                      │
│ ├─ Section Map                   │
│ └─ All Data Sections             │
└──────────────────────────────────┘
```

CompressedMetadata (0x110 bytes decoded) contains 34 u64 fields for page/section map offsets, CRCs, seeds, and correction factors.

### 2.2 Bit-Level Data Types

The DWG format is fundamentally **bit-aligned** (not byte-aligned). Every data field uses variable-length encoding:

| Type | Name | Encoding |
|------|------|----------|
| **B** | Bit | 1 bit: 0 or 1 |
| **BB** | 2-Bit Code | 2 bits: entity mode, type selectors |
| **3B** | Bit Triplet | 1–3 bits (R2010+) |
| **BS** | BitShort | 2-bit selector: 00=i16 LE, 01=u8, 10=0, 11=256 |
| **BL** | BitLong | 2-bit selector: 00=i32 LE, 01=u8, 10=0 |
| **BLL** | BitLongLong | 3-bit count + N bytes LE (R2010+) |
| **BD** | BitDouble | 2-bit selector: 00=f64 LE, 01=1.0, 10=0.0 |
| **DD** | BitDouble w/Default | 2-bit selector + partial byte patching |
| **MC** | Modular Char | 7-bit chunks, MSB=continue, bit 6 of last=sign |
| **MS** | Modular Short | 15-bit chunks, MSB=continue |
| **H** | Handle Reference | 4-bit code + 4-bit counter + N handle bytes |
| **T** | Text | BS length + raw chars |
| **TU** | Unicode Text | BS length + 2-byte chars (R2007+) |
| **TV** | Variable Text | T for pre-2007, TU for 2007+ |
| **RC** | Raw Char | 8 bits (may span byte boundary) |
| **RS** | Raw Short | 16 bits LE |
| **RL** | Raw Long | 32 bits LE |
| **RD** | Raw Double | 64 bits LE |
| **SN** | Sentinel | 16 raw bytes |
| **BE** | BitExtrusion | Optimized 3D vector (R2000+: 1 bit, if 0 → (0,0,1)) |
| **BT** | BitThickness | Optimized double (R2000+: 1 bit, if 0 → 0.0) |
| **CMC** | CmColor | Version-dependent color encoding |
| **TC** | TrueColor | Extended color (R2004+: BS idx + BL rgb + name) |
| **OT** | ObjectType | BS for pre-R2010, 2-bit+bytes for R2010+ |

### 2.3 Handle Reference System

Handles are encoded as: `|CODE (4 bits)|COUNTER (4 bits)|HANDLE bytes (N)|`

| Code | Type | Resolution |
|------|------|------------|
| 0x2 | SoftOwnership | Absolute handle |
| 0x3 | HardOwnership | Absolute handle |
| 0x4 | SoftPointer | Absolute handle |
| 0x5 | HardPointer | Absolute handle |
| 0x6 | +1 offset | Reference handle + 1 |
| 0x8 | -1 offset | Reference handle - 1 |
| 0xA | +N offset | Reference handle + offset |
| 0xC | -N offset | Reference handle - offset |

### 2.4 Object Map (AcDb:Handles)

Delta-encoded pairs in chunks (max 2032 bytes per chunk):
- Each pair: `ModularChar(handle_delta)` + `SignedModularChar(offset_delta)`
- Chunk terminated when accumulated size reaches near 2032
- Section terminated by chunk with size == 2
- Result: `HashMap<u64, i64>` (handle → absolute file position)

### 2.5 Per-Object Data Encoding

Each object in AcDb:AcDbObjects:

```
┌─────────────────────────────────────┐
│ MS (Modular Short): object size     │
├─────────────────────────────────────┤  ← R2010+: MC for handle stream size
│ Object Type (BS or OT)              │
├─────────────────────────────────────┤
│ Object-specific bit data            │  Entity data, handle refs, etc.
│ ├─ Common entity header             │  Handle, mode, reactors, layer, etc.
│ ├─ Type-specific fields             │  Geometry, properties, etc.
│ └─ Common entity trailer            │  Remaining handles
├─────────────────────────────────────┤
│ String stream (R2007+)              │  Text data in separate sub-stream
├─────────────────────────────────────┤
│ Handle stream                       │  All handle references
├─────────────────────────────────────┤
│ CRC16                               │  Integrity check
└─────────────────────────────────────┘
```

For R2007+, the object is split into 3 sub-streams accessed through a `DwgMergedReader`:
- **Main data stream** — geometry, flags, numeric values
- **Text/string stream** — all text content (positioned by flag at end)
- **Handle stream** — all handle references (positioned at `end - handle_size`)

### 2.6 Named Sections

| Section Name | Locator (AC15) | Description |
|---|---|---|
| `AcDb:Header` | Record 0 | System variables (~500 fields) |
| `AcDb:Classes` | Record 1 | DXF class definitions |
| `AcDb:Handles` | Record 2 | Object map (handle→offset) |
| `AcDb:ObjFreeSpace` | Record 3 | Free space bookkeeping |
| `AcDb:Template` | Record 4 | Template metadata |
| `AcDb:AuxHeader` | Record 5 | Auxiliary header (dates, version stamps) |
| `AcDb:AcDbObjects` | — (via handles) | All entities/tables/objects |
| `AcDb:SummaryInfo` | — (AC18+ only) | Document metadata (title, author) |
| `AcDb:Preview` | — (address in header) | Thumbnail image |
| `AcDb:AppInfo` | — (AC18+ only) | Application info |
| `AcDb:FileDepList` | — (AC18+ only) | File dependencies |
| `AcDb:RevHistory` | — (AC18+ only) | Revision history |

### 2.7 Sentinel Bytes

16-byte sentinels mark section boundaries:

| Section | Start | End |
|---------|-------|-----|
| Header | `CF 7B 1F 23 FD DE 38 A9 5F 7C 68 B8 4E 6D 33 5F` | `30 84 E0 DC 02 21 C7 56 A0 83 97 47 B1 92 CC A0` |
| Classes | `8D A1 C4 B8 C4 A9 F8 C5 C0 DC F4 5F E7 CF B6 8A` | `72 5E 3B 47 3B 56 07 3A 3F 23 0B A0 18 30 49 75` |
| Preview | `1F 25 6D 07 D4 36 28 28 9D 57 CA 3F 9D 44 10 2B` | `E0 DA 92 F8 2B C9 D7 D7 62 A8 35 C0 62 BB EF D4` |
| File End (AC15) | `95 A0 4E 28 99 82 1A E5 5E 41 E0 5F 9D 3A 4D 00` | — |

### 2.8 Compression & Encryption

| Mechanism | Versions | Purpose |
|-----------|----------|---------|
| **CRC-8** (16-bit) | All | File header, handle sections |
| **CRC-32** | AC18+ | Encrypted metadata blocks |
| **Adler-32** | AC18+ | Data section checksums |
| **XOR LCG encryption** | AC18+ | File header metadata (`seed*0x343FD+0x269EC3`) |
| **XOR position mask** | AC18+ | Data page headers (`0x4164536B ^ position`) |
| **LZ77 AC18** | AC18, AC24+ | Data section compression |
| **LZ77 AC21** | AC21 | Different opcode format for R2007 |
| **Reed-Solomon** | AC21 | File header & page encoding (byte interleaving) |

---

## 3. Existing Architecture Analysis

### 3.1 Shared / Reusable Code (Format-Agnostic)

These modules are **fully reusable** between DXF and DWG:

| Module | Description |
|--------|-------------|
| `src/document.rs` | `CadDocument`, `HeaderVariables` (1222 lines) |
| `src/entities/` | 38 entity types + `Entity` trait + `EntityType` enum |
| `src/tables/` | 9 table types + `TableEntry` trait + `Table<T>` |
| `src/objects/` | 12 object types + `ObjectType` enum |
| `src/classes/` | `DxfClass`, `DxfClassCollection`, `ProxyFlags` |
| `src/xdata/` | `ExtendedData`, `XDataValue`, `ExtendedDataRecord` |
| `src/types/` | `DxfVersion`, `Handle`, `Color`, `Vector2/3`, etc. |
| `src/error.rs` | `DxfError` (already has DWG-relevant variants) |
| `src/notification.rs` | `NotificationCollection` |

### 3.2 DXF I/O Structure (Pattern to Mirror)

```
src/io/
├── mod.rs                    # Re-exports DxfReader, DxfWriter, DwgReader, DwgWriter
└── dxf/
    ├── mod.rs                # Module root, re-exports
    ├── dxf_code.rs           # DXF group code enum
    ├── group_code_value.rs   # Value type mapping
    ├── code_page.rs          # Code page → encoding
    ├── reader.rs             # DxfReader orchestrator (292 lines)
    └── reader/
    │   ├── stream_reader.rs  # DxfStreamReader trait + DxfCodePair (213 lines)
    │   ├── text_reader.rs    # ASCII reader (194 lines)
    │   ├── binary_reader.rs  # Binary reader (256 lines)
    │   └── section_reader.rs # Section reading (4615 lines)
    └── writer/
        ├── mod.rs            # DxfWriter orchestrator (173 lines)
        ├── stream_writer.rs  # DxfStreamWriter trait (107 lines)
        ├── text_writer.rs    # ASCII text writer (157 lines)
        ├── binary_writer.rs  # Binary writer (150 lines)
        └── section_writer.rs # Section writing (3679 lines)
```

### 3.3 Key Architectural Patterns

| Pattern | DXF Implementation | DWG Equivalent |
|---------|-------------------|----------------|
| Stream abstraction | `DxfStreamReader` trait (code/value pairs) | `DwgStreamReader` trait (bit-level ops) |
| Reader polymorphism | `Box<dyn DxfStreamReader>` (text vs binary) | Version-specific readers (AC12/15/18/21/24) |
| Writer polymorphism | `<W: DxfStreamWriter>` generics | Version-specific writers (AC12/15/18/21/24) |
| Section orchestration | `SectionReader<'a>` borrows stream reader | Dedicated section reader structs |
| Document building | Direct field assignment in section reader | Two-pass template system with handle resolution |

---

## 4. Proposed File/Folder Structure

```
src/io/
├── mod.rs                           # Updated: re-exports DxfReader, DxfWriter, DwgReader, DwgWriter
├── dxf/                             # (existing, unchanged)
│   └── ...
└── dwg/
    ├── mod.rs                       # Module root, re-exports DwgReader, DwgWriter
    │
    ├── constants.rs                 # DWG magic numbers, section names, sentinel bytes
    ├── crc.rs                       # CRC-8 (16-bit) and CRC-32 lookup tables + algorithms
    ├── checksum.rs                  # Adler-32 checksum + magic sequence generation
    ├── encryption.rs                # XOR LCG encryption/decryption for AC18+ headers
    ├── compression/
    │   ├── mod.rs                   # Compression trait + re-exports
    │   ├── lz77_ac18.rs             # LZ77 compressor/decompressor for R2004+
    │   └── lz77_ac21.rs             # LZ77 compressor/decompressor for R2007
    ├── reed_solomon.rs              # Reed-Solomon byte interleaving encode/decode
    │
    ├── header_handles.rs            # DwgHeaderHandlesCollection (~50 named handles)
    ├── reference_type.rs            # DwgReferenceType enum + handle resolution helpers
    ├── section_io.rs                # Base section I/O helpers + version flag methods
    │
    ├── file_header/
    │   ├── mod.rs                   # DwgFileHeader trait/enum + factory method
    │   ├── file_header_ac15.rs      # R13–R2000 file header (record-based)
    │   ├── file_header_ac18.rs      # R2004+ file header (page-based, encrypted)
    │   ├── file_header_ac21.rs      # R2007 file header (RS + compressed metadata)
    │   ├── section_definition.rs    # Section name constants + sentinel bytes
    │   ├── section_locator.rs       # DwgSectionLocatorRecord (AC15)
    │   ├── section_descriptor.rs    # DwgSectionDescriptor (AC18+)
    │   ├── local_section_map.rs     # DwgLocalSectionMap (AC18+)
    │   └── compressed_metadata.rs   # Dwg21CompressedMetadata (AC21, 34 fields)
    │
    ├── reader/
    │   ├── mod.rs                   # DwgReader orchestrator
    │   ├── stream_reader.rs         # DwgStreamReader trait (bit-level interface)
    │   ├── stream_reader_base.rs    # Base bit-level reader implementation
    │   ├── stream_reader_ac12.rs    # R13/R14 overrides
    │   ├── stream_reader_ac15.rs    # R2000 overrides (BitExtrusion, BitThickness)
    │   ├── stream_reader_ac18.rs    # R2004 overrides (CmColor, EnColor)
    │   ├── stream_reader_ac21.rs    # R2007 overrides (Unicode text)
    │   ├── stream_reader_ac24.rs    # R2010+ overrides (ObjectType encoding)
    │   ├── merged_reader.rs         # DwgMergedReader (3-stream multiplexer)
    │   ├── header_reader.rs         # AcDb:Header section reader (~1000 lines)
    │   ├── classes_reader.rs        # AcDb:Classes section reader
    │   ├── handle_reader.rs         # AcDb:Handles section reader (object map)
    │   ├── object_reader/
    │   │   ├── mod.rs               # DwgObjectReader orchestrator
    │   │   ├── common.rs            # Common entity/non-entity data reading
    │   │   ├── entities.rs          # Entity reading (all entity types)
    │   │   └── objects.rs           # Non-graphical object reading
    │   ├── summary_info_reader.rs   # AcDb:SummaryInfo reader
    │   ├── preview_reader.rs        # AcDb:Preview reader
    │   └── app_info_reader.rs       # AcDb:AppInfo reader
    │
    ├── writer/
    │   ├── mod.rs                   # DwgWriter orchestrator
    │   ├── stream_writer.rs         # DwgStreamWriter trait (bit-level interface)
    │   ├── stream_writer_base.rs    # Base bit-level writer implementation
    │   ├── stream_writer_ac12.rs    # R13/R14 overrides
    │   ├── stream_writer_ac15.rs    # R2000 overrides
    │   ├── stream_writer_ac18.rs    # R2004 overrides
    │   ├── stream_writer_ac21.rs    # R2007 overrides
    │   ├── stream_writer_ac24.rs    # R2010+ overrides
    │   ├── merged_writer.rs         # DwgMergedStreamWriter (R2007+)
    │   ├── merged_writer_ac14.rs    # Pre-R2007 merged writer (2-stream)
    │   ├── file_header_writer/
    │   │   ├── mod.rs               # IDwgFileHeaderWriter trait + factory
    │   │   ├── header_writer_ac15.rs  # R13–R2000 file header writer
    │   │   ├── header_writer_ac18.rs  # R2004+ file header writer (pages, encryption)
    │   │   └── header_writer_ac21.rs  # R2007 file header writer (stub/partial)
    │   ├── header_writer.rs         # AcDb:Header section writer (~1100 lines)
    │   ├── classes_writer.rs        # AcDb:Classes section writer
    │   ├── handle_writer.rs         # AcDb:Handles section writer (object map)
    │   ├── object_writer/
    │   │   ├── mod.rs               # DwgObjectWriter orchestrator
    │   │   ├── common.rs            # Common entity/non-entity data writing
    │   │   ├── entities.rs          # Entity writing (all entity types)
    │   │   └── objects.rs           # Non-graphical object writing
    │   ├── summary_info_writer.rs   # AcDb:SummaryInfo writer
    │   ├── preview_writer.rs        # AcDb:Preview writer
    │   ├── app_info_writer.rs       # AcDb:AppInfo writer
    │   └── aux_header_writer.rs     # AcDb:AuxHeader writer
    │
    └── builder/
        ├── mod.rs                   # DwgDocumentBuilder
        └── templates.rs             # CadTemplate, ICadObjectTemplate trait
```

**Total estimated new files: ~55 files** (comparable to ACadSharp's 59 DWG files)

---

## 5. Module-by-Module Design

### 5.1 Top-Level Module (`src/io/dwg/mod.rs`)

```rust
//! DWG (Drawing) native AutoCAD file reading and writing

mod constants;
mod crc;
mod checksum;
mod encryption;
mod compression;
mod reed_solomon;
mod header_handles;
mod reference_type;
mod section_io;
mod file_header;
mod reader;
mod writer;
mod builder;

pub use reader::{DwgReader, DwgReaderConfiguration};
pub use writer::{DwgWriter, DwgWriterConfiguration};
pub use reference_type::DwgReferenceType;
```

### 5.2 Update to `src/io/mod.rs`

```rust
//! I/O module for reading and writing CAD files in DXF and DWG formats

pub mod dxf;
pub mod dwg;

pub use dxf::{DxfReader, DxfWriter};
pub use dwg::{DwgReader, DwgWriter};
```

---

## 6. Phase 1 — Foundation Layer

### 6.1 Constants (`constants.rs`)

All magic numbers, section names, and sentinel bytes.

```rust
/// Section name constants (matching ACadSharp DwgSectionDefinition)
pub mod section_names {
    pub const HEADER: &str = "AcDb:Header";
    pub const CLASSES: &str = "AcDb:Classes";
    pub const HANDLES: &str = "AcDb:Handles";
    pub const OBJECTS: &str = "AcDb:AcDbObjects";
    pub const OBJ_FREE_SPACE: &str = "AcDb:ObjFreeSpace";
    pub const TEMPLATE: &str = "AcDb:Template";
    pub const AUX_HEADER: &str = "AcDb:AuxHeader";
    pub const SUMMARY_INFO: &str = "AcDb:SummaryInfo";
    pub const PREVIEW: &str = "AcDb:Preview";
    pub const APP_INFO: &str = "AcDb:AppInfo";
    pub const FILE_DEP_LIST: &str = "AcDb:FileDepList";
    pub const REV_HISTORY: &str = "AcDb:RevHistory";
}

/// Sentinel bytes for section boundaries
pub mod sentinels {
    pub const HEADER_START: [u8; 16] = [0xCF, 0x7B, 0x1F, 0x23, ...];
    pub const HEADER_END: [u8; 16]   = [0x30, 0x84, 0xE0, 0xDC, ...];
    pub const CLASSES_START: [u8; 16] = [...];
    pub const CLASSES_END: [u8; 16]   = [...];
    pub const PREVIEW_START: [u8; 16] = [...];
    pub const PREVIEW_END: [u8; 16]   = [...];
    pub const FILE_HEADER_END_AC15: [u8; 16] = [0x95, 0xA0, 0x4E, 0x28, ...];
}

/// AC18 constants
pub const AC18_ENCRYPTED_HEADER_SIZE: usize = 0x6C; // 108 bytes
pub const AC18_FILE_ID: &[u8] = b"AcFssFcAJMB\0";
pub const AC18_DECRYPTION_MASK: u32 = 0x4164536B;
pub const AC18_MAX_PAGE_SIZE: usize = 0x7400; // 29696 bytes
pub const AC18_PAGE_TYPE_DATA: u32 = 0x4163043B;
pub const AC18_PAGE_TYPE_PAGE_MAP: u32 = 0x41630E3B;
pub const AC18_PAGE_TYPE_SECTION_MAP: u32 = 0x4163003B;

/// AC21 constants
pub const AC21_DATA_PAGE_BASE_OFFSET: u64 = 0x480;
pub const AC21_RS_ENCODED_BLOCK_SIZE: usize = 0x400; // 1024 bytes
pub const AC21_DECOMPRESSED_HEADER_SIZE: usize = 0x110; // 272 bytes
pub const AC21_RS_BLOCK_SIZE: usize = 239; // Reed-Solomon block size

/// Handle section constants
pub const HANDLE_SECTION_MAX_CHUNK_SIZE: usize = 2032;

/// Section locator indices (AC15)
pub const SECTION_HEADER: usize = 0;
pub const SECTION_CLASSES: usize = 1;
pub const SECTION_HANDLES: usize = 2;
pub const SECTION_OBJ_FREE_SPACE: usize = 3;
pub const SECTION_TEMPLATE: usize = 4;
pub const SECTION_AUX_HEADER: usize = 5;
```

### 6.2 CRC (`crc.rs`)

```rust
/// CRC-8 (actually 16-bit) lookup table for DWG file integrity
pub struct Crc8 {
    // 256 x u16 table
}

impl Crc8 {
    pub fn calculate(seed: u16, data: &[u8]) -> u16;
    pub fn calculate_range(seed: u16, data: &[u8], start: usize, end: usize) -> u16;
}

/// CRC-32 lookup table
pub struct Crc32 {
    // 256 x u32 table (polynomial 0xEDB88320)
}

impl Crc32 {
    pub fn calculate(seed: u32, data: &[u8]) -> u32;
}

/// CRC-8 stream handler (wraps a stream, maintains running CRC)
pub struct Crc8StreamHandler<R> { ... }

/// CRC-32 stream handler with XOR decryption capability
pub struct Crc32StreamHandler { ... }
```

### 6.3 Checksum (`checksum.rs`)

```rust
/// Adler-32 variant checksum (modulus 0xFFF1)
pub fn adler32_checksum(seed: u32, data: &[u8]) -> u32;

/// Generate 256-byte magic sequence using LCG
/// seed = seed * 0x343FD + 0x269EC3; byte = (seed >> 16) as u8
pub fn generate_magic_sequence(initial_seed: u32) -> [u8; 256];

/// Calculate compression padding
/// Returns bytes needed to align to 32-byte boundary
pub fn compression_padding(length: usize) -> usize;
```

### 6.4 Encryption (`encryption.rs`)

```rust
/// Decrypt AC18+ file header metadata (0x6C bytes at offset 0x80)
/// Uses CRC32-based LCG XOR stream
pub fn decrypt_header_ac18(encrypted: &[u8]) -> Vec<u8>;

/// Encrypt AC18+ file header metadata
pub fn encrypt_header_ac18(plaintext: &[u8]) -> Vec<u8>;

/// Decrypt AC18+ data section page header (8 x i32)
/// Mask = 0x4164536B ^ stream_position
pub fn decrypt_page_header(data: &mut [u8; 32], stream_position: u64);

/// Encrypt AC18+ data section page header
pub fn encrypt_page_header(data: &mut [u8; 32], stream_position: u64);
```

### 6.5 Compression (`compression/`)

```rust
// compression/mod.rs
pub trait Compressor {
    fn compress(&self, input: &[u8]) -> Result<Vec<u8>>;
}

pub trait Decompressor {
    fn decompress(&self, input: &[u8], expected_size: usize) -> Result<Vec<u8>>;
}

// compression/lz77_ac18.rs
pub struct Lz77Ac18Compressor;
pub struct Lz77Ac18Decompressor;

impl Compressor for Lz77Ac18Compressor {
    fn compress(&self, input: &[u8]) -> Result<Vec<u8>>;
}

impl Decompressor for Lz77Ac18Decompressor {
    fn decompress(&self, input: &[u8], expected_size: usize) -> Result<Vec<u8>>;
}

// compression/lz77_ac21.rs
pub struct Lz77Ac21Compressor;   // May initially be NotImplemented
pub struct Lz77Ac21Decompressor;

impl Decompressor for Lz77Ac21Decompressor {
    fn decompress(&self, input: &[u8], expected_size: usize) -> Result<Vec<u8>>;
}
```

**LZ77 AC18 Decompressor Algorithm:**

Opcodes:
- `0x00–0x0F`: Literal length sequences
- `0x10–0x1F`: Compressed with 3-bit valid, 8-bit offset extension
- `0x11`: Stream terminator
- `0x20+`: Compressed with 5-bit valid
- `0x40+`: High-nibble comp bytes, 2-byte offset

Uses literal count accumulation via 0xFF continuation.

**LZ77 AC21 Decompressor Algorithm:**

Instruction dispatch on `opcode >> 4`:
- Case 0: Long + source byte
- Case 1: Short format
- Case 2: Extended format
- Default (3+): Inline format

Uses optimized 32-byte copy chunks.

### 6.6 Reed-Solomon (`reed_solomon.rs`)

```rust
/// Decode RS-encoded data (byte de-interleaving)
/// For AC21 file header: factor=3, block_size=239
/// For AC21 section pages: factor=compressed_size/251
pub fn decode(encoded: &[u8], factor: usize, block_size: usize) -> Vec<u8>;

/// Encode data with RS byte interleaving
pub fn encode(data: &[u8], factor: usize, block_size: usize) -> Vec<u8>;
```

The "Reed-Solomon" in DWG is actually simple byte interleaving (not full error-correcting RS):
```
// Decode: read every factor-th byte, factor times
for i in 0..factor:
    cindex = i
    for _ in 0..block_size:
        output[index++] = encoded[cindex]
        cindex += factor
```

### 6.7 Reference Type (`reference_type.rs`)

```rust
/// DWG handle reference type (low 2 bits of handle code)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DwgReferenceType {
    Undefined = 0,
    SoftOwnership = 2,   // 0x2, 0x3 (low 2 bits = 0b10, 0b11)
    HardOwnership = 3,
    SoftPointer = 4,     // 0x4, 0x5
    HardPointer = 5,
}

/// A resolved handle reference
#[derive(Debug, Clone, Copy)]
pub struct HandleReference {
    pub code: u8,
    pub reference_type: DwgReferenceType,
    pub handle: u64,
}

impl HandleReference {
    /// Resolve an absolute handle value given a reference handle
    pub fn resolve(&self, reference_handle: u64) -> u64;
}
```

### 6.8 Header Handles (`header_handles.rs`)

```rust
/// Collection of ~50 named handle references found in the DWG header
pub struct DwgHeaderHandlesCollection {
    handles: HashMap<&'static str, Option<u64>>,
}

impl DwgHeaderHandlesCollection {
    // Table control objects
    pub fn block_control_object(&self) -> Option<u64>;
    pub fn layer_control_object(&self) -> Option<u64>;
    pub fn style_control_object(&self) -> Option<u64>;
    pub fn linetype_control_object(&self) -> Option<u64>;
    pub fn view_control_object(&self) -> Option<u64>;
    pub fn ucs_control_object(&self) -> Option<u64>;
    pub fn vport_control_object(&self) -> Option<u64>;
    pub fn appid_control_object(&self) -> Option<u64>;
    pub fn dimstyle_control_object(&self) -> Option<u64>;

    // Named references
    pub fn clayer(&self) -> Option<u64>;
    pub fn celtype(&self) -> Option<u64>;
    pub fn textstyle(&self) -> Option<u64>;
    pub fn dimstyle(&self) -> Option<u64>;
    pub fn cmlstyle(&self) -> Option<u64>;

    // Special blocks
    pub fn model_space(&self) -> Option<u64>;
    pub fn paper_space(&self) -> Option<u64>;

    // Dictionaries
    pub fn dictionary_named_objects(&self) -> Option<u64>;
    pub fn dictionary_layouts(&self) -> Option<u64>;

    // Linetypes
    pub fn bylayer(&self) -> Option<u64>;
    pub fn byblock(&self) -> Option<u64>;
    pub fn continuous(&self) -> Option<u64>;

    // ... ~25 more handles ...

    /// Get all non-None handles for queue seeding
    pub fn get_all_handles(&self) -> Vec<u64>;

    /// Update HeaderVariables by resolving handle names to object names
    pub fn update_header(&self, header: &mut HeaderVariables, objects: &HashMap<u64, ...>);
}
```

### 6.9 Section I/O (`section_io.rs`)

```rust
/// Base helpers for section read/write with version-conditional logic
pub struct SectionIO {
    version: DxfVersion,
}

impl SectionIO {
    pub fn new(version: DxfVersion) -> Self;

    // Version flag queries
    pub fn is_r13_14_only(&self) -> bool;       // AC1012 || AC1014
    pub fn is_r13_15_only(&self) -> bool;       // AC1012..=AC1015
    pub fn is_r2000_plus(&self) -> bool;        // >= AC1015
    pub fn is_r2004_pre(&self) -> bool;         // < AC1018
    pub fn is_r2004_plus(&self) -> bool;        // >= AC1018
    pub fn is_r2007_pre(&self) -> bool;         // < AC1021
    pub fn is_r2007_plus(&self) -> bool;        // >= AC1021
    pub fn is_r2010_plus(&self) -> bool;        // >= AC1024
    pub fn is_r2013_plus(&self) -> bool;        // >= AC1027
    pub fn is_r2018_plus(&self) -> bool;        // >= AC1032

    /// Compare 16-byte sentinels
    pub fn check_sentinel(actual: &[u8; 16], expected: &[u8; 16]) -> Result<()>;
}
```

---

## 7. Phase 2 — Bit-Level Stream I/O

### 7.1 Stream Reader Interface (`reader/stream_reader.rs`)

This is the **most critical abstraction** — the DWG equivalent of `DxfStreamReader`.

```rust
/// Trait for bit-level DWG stream reading
///
/// DWG data is bit-aligned, so every read operation must track
/// the current bit position within the byte stream.
pub trait DwgStreamReader {
    // === Bit-Level Primitives ===

    /// Read a single bit (B)
    fn read_bit(&mut self) -> Result<bool>;

    /// Read a 2-bit code (BB)
    fn read_2bits(&mut self) -> Result<u8>;

    /// Read a 3-bit triplet (3B, R2010+)
    fn read_3bits(&mut self) -> Result<u8>;

    // === Bit-Encoded Integers ===

    /// Read a bitshort (BS): 2-bit selector + 0-2 bytes
    fn read_bit_short(&mut self) -> Result<i16>;

    /// Read a bitlong (BL): 2-bit selector + 0-4 bytes
    fn read_bit_long(&mut self) -> Result<i32>;

    /// Read a bitlonglong (BLL, R2010+): 3-bit count + N bytes
    fn read_bit_long_long(&mut self) -> Result<i64>;

    // === Bit-Encoded Floating Point ===

    /// Read a bitdouble (BD): 2-bit selector + 0-8 bytes
    fn read_bit_double(&mut self) -> Result<f64>;

    /// Read a bitdouble with default value (DD)
    fn read_bit_double_with_default(&mut self, default: f64) -> Result<f64>;

    // === Raw Values (byte-aligned within bit stream) ===

    /// Read a raw byte (RC), respecting bit shift
    fn read_raw_char(&mut self) -> Result<u8>;

    /// Read a raw short (RS), 2 bytes LE
    fn read_raw_short(&mut self) -> Result<i16>;

    /// Read a raw long (RL), 4 bytes LE
    fn read_raw_long(&mut self) -> Result<i32>;

    /// Read a raw double (RD), 8 bytes LE
    fn read_raw_double(&mut self) -> Result<f64>;

    // === Variable-Length Integers ===

    /// Read a modular char (MC): 7-bit chunks, MSB=continue
    fn read_modular_char(&mut self) -> Result<i32>;

    /// Read a signed modular char
    fn read_signed_modular_char(&mut self) -> Result<i32>;

    /// Read a modular short (MS): 15-bit chunks
    fn read_modular_short(&mut self) -> Result<i32>;

    // === Handle References ===

    /// Read a handle reference (H): 4-bit code + 4-bit counter + N bytes
    fn handle_reference(&mut self, reference_handle: u64) -> Result<(u64, DwgReferenceType)>;

    // === Text ===

    /// Read text (T): BS length + char bytes (pre-R2007)
    fn read_text(&mut self) -> Result<String>;

    /// Read Unicode text (TU): BS length + 2-byte chars (R2007+)
    fn read_text_unicode(&mut self) -> Result<String>;

    /// Read version-dependent text (TV)
    fn read_variable_text(&mut self) -> Result<String>;

    // === Geometry Helpers ===

    /// Read a 2D point (pair of BD)
    fn read_2bit_double(&mut self) -> Result<(f64, f64)>;

    /// Read a 3D point (triple of BD)
    fn read_3bit_double(&mut self) -> Result<(f64, f64, f64)>;

    /// Read BitExtrusion (BE): optimized 3D normal vector
    fn read_bit_extrusion(&mut self) -> Result<(f64, f64, f64)>;

    /// Read BitThickness (BT): optimized thickness
    fn read_bit_thickness(&mut self) -> Result<f64>;

    /// Read a 3D point with default (3DD)
    fn read_3bit_double_with_default(&mut self, def: (f64, f64, f64)) -> Result<(f64, f64, f64)>;

    // === Color ===

    /// Read CmColor (version-dependent color)
    fn read_cm_color(&mut self) -> Result<Color>;

    /// Read EnColor (entity color with transparency, R2004+)
    fn read_en_color(&mut self) -> Result<(Color, Transparency, Option<Handle>)>;

    // === Object Type ===

    /// Read object type discriminator (BS for pre-R2010, OT for R2010+)
    fn read_object_type(&mut self) -> Result<i16>;

    // === Positional ===

    /// Read a 16-byte sentinel
    fn read_sentinel(&mut self) -> Result<[u8; 16]>;

    /// Read raw bytes
    fn read_bytes(&mut self, count: usize) -> Result<Vec<u8>>;

    /// Current position in bits
    fn position_in_bits(&self) -> u64;

    /// Set position in bits
    fn set_position_in_bits(&mut self, position: u64);

    /// Advance to byte boundary, read CRC16
    fn reset_shift(&mut self) -> Result<u16>;

    // === Date/Time ===

    /// Read 8-bit Julian date (2 x RL → date/time)
    fn read_8bit_julian_date(&mut self) -> Result<f64>;

    /// Read date/time (2 x BL → Julian)
    fn read_date_time(&mut self) -> Result<f64>;

    /// Read time span (2 x BL → hours + ms)
    fn read_time_span(&mut self) -> Result<f64>;

    // === String Stream (R2007+) ===

    /// Set position for string stream using flag at given bit position
    fn set_position_by_flag(&mut self, offset_in_bits: u64) -> Result<()>;
}
```

### 7.2 Base Stream Reader (`reader/stream_reader_base.rs`)

Core bit-manipulation implementation (~700 lines estimated):

```rust
/// Base implementation of DwgStreamReader with bit-level I/O
pub struct DwgStreamReaderBase<R: Read + Seek> {
    stream: R,
    bit_shift: u8,          // 0-7: current bit position
    last_byte: u8,          // cached last byte read
    encoding: &'static Encoding,
}

impl<R: Read + Seek> DwgStreamReaderBase<R> {
    pub fn new(stream: R, version: DxfVersion) -> Self;

    /// Factory to create version-appropriate reader
    pub fn create_reader(
        stream: R,
        version: DxfVersion,
    ) -> Box<dyn DwgStreamReader>;

    // Internal bit manipulation
    fn read_byte_raw(&mut self) -> Result<u8>;
    fn read_bytes_raw(&mut self, count: usize) -> Result<Vec<u8>>;
}
```

### 7.3 Version-Specific Reader Overrides

Each version override is a small file (~50-150 lines) overriding specific methods:

**`stream_reader_ac12.rs`** (R13/R14): Default behavior, no overrides.

**`stream_reader_ac15.rs`** (R2000): Overrides `read_bit_extrusion` (1-bit flag: if 0 → (0,0,1)) and `read_bit_thickness` (1-bit flag: if 0 → 0.0).

**`stream_reader_ac18.rs`** (R2004): Overrides `read_cm_color` (BS+BL RGB encoding) and `read_en_color` (flags + transparency BL).

**`stream_reader_ac21.rs`** (R2007): Overrides `read_text_unicode` and `read_variable_text` to use 2-byte Unicode chars.

**`stream_reader_ac24.rs`** (R2010+): Overrides `read_object_type` with 2-bit pair + 1-2 byte encoding.

### 7.4 Merged Reader (`reader/merged_reader.rs`)

```rust
/// Multiplexes 3 independent sub-streams for R2007+ object reading
///
/// Routes read operations:
/// - Handle references → handle_reader
/// - Text reads → text_reader (R2007+)
/// - All other reads → main_reader
pub struct DwgMergedReader {
    main_reader: Box<dyn DwgStreamReader>,
    text_reader: Box<dyn DwgStreamReader>,
    handle_reader: Box<dyn DwgStreamReader>,
}

impl DwgStreamReader for DwgMergedReader {
    // Delegates handle_reference() to handle_reader
    // Delegates read_variable_text()/read_text_unicode() to text_reader
    // Delegates everything else to main_reader
    // read_cm_color() needs both main and text readers (special handling)
}
```

### 7.5 Stream Writer Interface (`writer/stream_writer.rs`)

Mirror of the reader interface:

```rust
/// Trait for bit-level DWG stream writing
pub trait DwgStreamWriter {
    fn write_bit(&mut self, value: bool) -> Result<()>;
    fn write_2bits(&mut self, value: u8) -> Result<()>;
    fn write_bit_short(&mut self, value: i16) -> Result<()>;
    fn write_bit_long(&mut self, value: i32) -> Result<()>;
    fn write_bit_long_long(&mut self, value: i64) -> Result<()>;
    fn write_bit_double(&mut self, value: f64) -> Result<()>;
    fn write_bit_double_with_default(&mut self, value: f64, default: f64) -> Result<()>;
    fn write_raw_char(&mut self, value: u8) -> Result<()>;
    fn write_raw_short(&mut self, value: i16) -> Result<()>;
    fn write_raw_long(&mut self, value: i32) -> Result<()>;
    fn write_raw_double(&mut self, value: f64) -> Result<()>;
    fn write_handle_reference(&mut self, ref_type: DwgReferenceType, handle: u64) -> Result<()>;
    fn write_variable_text(&mut self, text: &str) -> Result<()>;
    fn write_text_unicode(&mut self, text: &str) -> Result<()>;
    fn write_cm_color(&mut self, color: &Color) -> Result<()>;
    fn write_en_color(&mut self, color: &Color, transparency: &Transparency) -> Result<()>;
    fn write_bit_extrusion(&mut self, x: f64, y: f64, z: f64) -> Result<()>;
    fn write_bit_thickness(&mut self, value: f64) -> Result<()>;
    fn write_object_type(&mut self, value: i16) -> Result<()>;
    fn write_sentinel(&mut self, sentinel: &[u8; 16]) -> Result<()>;
    fn write_bytes(&mut self, data: &[u8]) -> Result<()>;
    fn write_date_time(&mut self, value: f64) -> Result<()>;

    // Positional
    fn position_in_bits(&self) -> u64;
    fn set_position_in_bits(&mut self, position: u64);
    fn save_position_for_size(&mut self) -> u64;
    fn write_shift_value(&mut self) -> Result<()>;
    fn reset_stream(&mut self) -> Result<()>;
    fn flush(&mut self) -> Result<()>;

    /// Get reference to the underlying stream
    fn stream(&self) -> &[u8];
    fn stream_mut(&mut self) -> &mut Vec<u8>;
}
```

### 7.6 Base Stream Writer (`writer/stream_writer_base.rs`)

Mirror of the reader base (~500 lines estimated):

```rust
pub struct DwgStreamWriterBase {
    buffer: Vec<u8>,
    bit_shift: u8,
    last_byte: u8,
    encoding: &'static Encoding,
}

impl DwgStreamWriterBase {
    pub fn new(version: DxfVersion) -> Self;
    pub fn create_writer(version: DxfVersion) -> Box<dyn DwgStreamWriter>;
}
```

### 7.7 Merged Writers

**`writer/merged_writer.rs`** (R2007+):
```rust
/// Routes text → text_writer, handles → handle_writer, rest → main_writer
/// On flush: appends text stream (with 0x8000 length flag) + handle stream to main
pub struct DwgMergedStreamWriter {
    main_writer: Box<dyn DwgStreamWriter>,
    text_writer: Box<dyn DwgStreamWriter>,
    handle_writer: Box<dyn DwgStreamWriter>,
}
```

**`writer/merged_writer_ac14.rs`** (pre-R2007):
```rust
/// Text goes to main stream; handles accumulate separately
/// On flush: writes size-in-bits position, appends handle bytes, pads to byte boundary
pub struct DwgMergedStreamWriterAC14 {
    main_writer: Box<dyn DwgStreamWriter>,
    handle_writer: Box<dyn DwgStreamWriter>,
}
```

---

## 8. Phase 3 — File Structure & Section Management

### 8.1 File Header Hierarchy (`file_header/`)

```rust
// file_header/mod.rs

/// DWG file header information
pub enum DwgFileHeader {
    AC15(DwgFileHeaderAC15),
    AC18(DwgFileHeaderAC18),
    AC21(DwgFileHeaderAC21),
}

impl DwgFileHeader {
    /// Factory: create appropriate file header for version
    pub fn create(version: DxfVersion) -> Self;

    pub fn version(&self) -> DxfVersion;
    pub fn preview_address(&self) -> i64;
    pub fn code_page(&self) -> u16;
    pub fn maintenance_version(&self) -> u8;
}
```

**`file_header_ac15.rs`** — R13–R2000:
```rust
pub struct DwgFileHeaderAC15 {
    pub version: DxfVersion,
    pub preview_address: i64,
    pub code_page: u16,
    pub maintenance_version: u8,
    pub records: HashMap<usize, DwgSectionLocatorRecord>,
}
```

**`file_header_ac18.rs`** — R2004+:
```rust
pub struct DwgFileHeaderAC18 {
    pub version: DxfVersion,
    pub preview_address: i64,
    pub code_page: u16,
    pub maintenance_version: u8,
    pub dwg_version: u8,
    pub app_release_version: u8,
    pub security_type: u32,
    pub summary_info_address: u32,
    pub vba_project_address: u32,
    pub app_info_address: u32,
    // Encrypted header fields:
    pub root_tree_node_gap: u32,
    pub left_gap: u32,
    pub right_gap: u32,
    pub last_page_id: u32,
    pub last_section_end_address: u64,
    pub second_header_address: u64,
    pub gap_amount: u32,
    pub section_page_amount: u32,
    pub section_page_map_id: u32,
    pub page_map_address: u64,
    pub section_map_id: u32,
    pub section_array_page_size: u32,
    pub gap_array_size: u32,
    pub crc_seed: u32,
    pub descriptors: HashMap<String, DwgSectionDescriptor>,
}
```

**`file_header_ac21.rs`** — R2007:
```rust
pub struct DwgFileHeaderAC21 {
    pub base: DwgFileHeaderAC18,
    pub compressed_metadata: Dwg21CompressedMetadata,
}
```

**`compressed_metadata.rs`**:
```rust
pub struct Dwg21CompressedMetadata {
    pub header_size: u64,           // 0x70
    pub file_size: u64,
    pub pages_map_crc_compressed: u64,
    pub pages_map_correction_factor: u64,
    pub pages_map_crc_seed: u64,
    pub map2_offset: u64,
    pub map2_id: u64,
    pub pages_map_offset: u64,
    pub pages_map_id: u64,
    pub header2_offset: u64,
    pub pages_map_size_compressed: u64,
    pub pages_map_size_uncompressed: u64,
    pub pages_amount: u64,
    pub pages_max_id: u64,
    pub sections_amount: u64,
    pub sections_map_crc_uncompressed: u64,
    pub sections_map_size_compressed: u64,
    pub sections_map_correction_factor: u64,
    pub sections_map_crc_seed: u64,
    pub stream_version: u64,        // 0x60100
    pub crc_seed: u64,
    pub crc_seed_encoded: u64,
    pub random_seed: u64,
    pub header_crc64: u64,
    // ... remaining fields
}
```

**`section_locator.rs`** (AC15):
```rust
pub struct DwgSectionLocatorRecord {
    pub number: Option<usize>,
    pub seeker: i64,   // absolute file offset
    pub size: i64,     // section byte count
}

impl DwgSectionLocatorRecord {
    pub fn is_in_range(&self, position: i64) -> bool;
}
```

**`section_descriptor.rs`** (AC18+):
```rust
pub struct DwgSectionDescriptor {
    pub page_type: u32,             // 0x4163043B
    pub name: String,
    pub compressed_size: u64,
    pub page_count: u32,
    pub decompressed_size: u32,     // 0x7400 default
    pub compressed_code: u32,       // 1=none, 2=compressed
    pub section_id: u32,
    pub encrypted: u32,             // 0=no, 1=yes, 2=unknown
    pub hash_code: u64,             // AC21
    pub encoding: u64,              // AC21
    pub local_sections: Vec<DwgLocalSectionMap>,
}

impl DwgSectionDescriptor {
    pub fn is_compressed(&self) -> bool;
}
```

**`local_section_map.rs`**:
```rust
pub struct DwgLocalSectionMap {
    pub compression: u32,           // default 2
    pub offset: u64,                // offset in decompressed stream
    pub compressed_size: u64,
    pub page_number: u32,
    pub decompressed_size: u64,
    pub seeker: i64,                // absolute file position
    pub size: i64,                  // page size in file
    pub checksum: u32,
    pub crc: u32,
    pub page_size: u32,
}
```

---

## 9. Phase 4 — Section Readers

### 9.1 Header Reader (`reader/header_reader.rs`)

~1000 lines. Reads all system variables from AcDb:Header:

```rust
pub struct DwgHeaderReader<'a> {
    reader: &'a mut dyn DwgStreamReader,
    version: DxfVersion,
}

impl<'a> DwgHeaderReader<'a> {
    pub fn read(
        &mut self,
        header: &mut HeaderVariables,
    ) -> Result<DwgHeaderHandlesCollection>;
}
```

The header reader must read ~500 system variables in the exact order specified by the DWG format, including version-conditional fields. Each version adds/removes variables. The reader returns a `DwgHeaderHandlesCollection` containing all handle references found in the header.

**Key version-conditional blocks:**
- R13/R14 only: `DIMASO`, `DIMSHO`, `PLINEGEN`, `ORTHOMODE`, etc.
- R2000+: `CELWEIGHT`, `ENDCAPS`, `JOINSTYLE`, `SORTENTS`, etc.
- R2004+: `CAMERADISPLAY`, `STEPSPERSEC`, `LENSLENGTH`, etc.
- R2007+: `SHADOWPLANELOCATION`, `INTERFERECOLOR`, etc.
- R2010+: `unknown dimension vars`
- R2013+: `SOLIDHIST`, `SHOWHIST`, `unknown flags`

### 9.2 Classes Reader (`reader/classes_reader.rs`)

~150 lines. Reads DXF class definitions:

```rust
pub struct DwgClassesReader<'a> {
    reader: &'a mut dyn DwgStreamReader,
    version: DxfVersion,
}

impl<'a> DwgClassesReader<'a> {
    pub fn read(&mut self) -> Result<DxfClassCollection>;
}
```

Each class: class_number (BS), proxy_flags (BS), application_name (TV), cpp_class_name (TV), dxf_name (TV), was_zombie (B), item_class_id (BS), instance_count (BL, R2004+), dwg_version (BL, R2004+), maintenance_version (BL, R2004+), unknown1 (BL, R2004+), unknown2 (BL, R2004+).

### 9.3 Handle Reader (`reader/handle_reader.rs`)

~100 lines. Reads the object map:

```rust
pub struct DwgHandleReader<'a> {
    reader: &'a mut dyn DwgStreamReader,
}

impl<'a> DwgHandleReader<'a> {
    /// Read object map, returns handle→file_offset mapping
    pub fn read(&mut self) -> Result<HashMap<u64, i64>>;
}
```

Algorithm:
1. Read section_size (RS)
2. While section_size > 2:
   - Read delta-encoded pairs until section bytes consumed
   - Each pair: MC(handle_delta) + signed MC(offset_delta)
   - Accumulate running handle and offset
   - Validate section CRC (RS at end)
   - Read next section_size

### 9.4 Summary Info Reader (`reader/summary_info_reader.rs`)

~80 lines. Reads document metadata:

```rust
pub struct DwgSummaryInfoReader<'a> { ... }

impl<'a> DwgSummaryInfoReader<'a> {
    pub fn read(&mut self) -> Result<CadSummaryInfo>;
}
```

### 9.5 Preview Reader (`reader/preview_reader.rs`)

~60 lines. Reads thumbnail image:

```rust
pub struct DwgPreviewReader<'a> { ... }

impl<'a> DwgPreviewReader<'a> {
    pub fn read(&mut self) -> Result<Option<DwgPreview>>;
}
```

### 9.6 AppInfo Reader (`reader/app_info_reader.rs`)

~60 lines. Reads application info (test/diagnostic only):

```rust
pub struct DwgAppInfoReader<'a> { ... }

impl<'a> DwgAppInfoReader<'a> {
    pub fn read(&mut self) -> Result<()>; // Mostly diagnostic data
}
```

---

## 10. Phase 5 — Object Reader (Entities & Objects)

This is the **largest single component** (~5000+ lines of Rust), corresponding to ACadSharp's ~7200-line `DwgObjectReader`.

### 10.1 Object Reader Orchestrator (`reader/object_reader/mod.rs`)

```rust
/// Reads all entities, table entries, and objects from AcDb:AcDbObjects
pub struct DwgObjectReader<'a> {
    /// Raw section data
    section_data: &'a [u8],
    /// Object map: handle → file offset
    handle_map: HashMap<u64, i64>,
    /// DWG version
    version: DxfVersion,
    /// Document builder (receives read objects)
    builder: &'a mut DwgDocumentBuilder,
    /// Already-read objects set (prevents duplicates)
    read_objects: HashSet<u64>,
    /// Queue of handles to read
    queue: VecDeque<u64>,
    /// DXF class map (class number → class info)
    class_map: HashMap<i16, DxfClass>,
}

impl<'a> DwgObjectReader<'a> {
    /// Main entry: seed queue from header handles, process until empty
    pub fn read(&mut self) -> Result<()>;

    /// Read a single object at the given handle's offset
    fn read_object(&mut self, handle: u64) -> Result<()>;

    /// Set up sub-streams for an object (main/text/handles)
    fn setup_streams(&self, offset: i64) -> Result<(Box<dyn DwgStreamReader>, i32)>;

    /// Dispatch to type-specific reader
    fn dispatch_object_type(&mut self, reader: &mut dyn DwgStreamReader, object_type: i16) -> Result<()>;
}
```

**Queue-Based Reading Algorithm:**
1. Seed queue from `DwgHeaderHandlesCollection::get_all_handles()`
2. While queue is not empty:
   a. Dequeue handle
   b. Skip if already read
   c. Look up file offset in handle map
   d. Seek to offset, read MS (object size)
   e. For R2010+: read MC (handle stream size), compute sub-stream positions
   f. Create merged reader (3 sub-streams for R2007+, 2 for earlier)
   g. Read object type (BS/OT)
   h. Dispatch to type reader
   i. Pass newly discovered handles to queue
   j. Mark handle as read

### 10.2 Common Entity Data (`reader/object_reader/common.rs`)

```rust
/// Read common entity header data
pub fn read_common_entity_data(
    reader: &mut dyn DwgStreamReader,
    version: DxfVersion,
) -> Result<EntityCommonData>;

/// Read common non-graphical object data
pub fn read_common_object_data(
    reader: &mut dyn DwgStreamReader,
    version: DxfVersion,
) -> Result<ObjectCommonData>;

/// Intermediate structure holding raw handle values before resolution
pub struct EntityCommonData {
    pub handle: u64,
    pub extended_data: ExtendedData,
    pub graphic_image: Option<Vec<u8>>,
    pub entity_mode: u8,       // BB: 00=owner, 01=pspace, 02=mspace
    pub num_reactors: u32,
    pub xdictionary_present: bool,
    pub has_ds_data: bool,      // R2013+
    pub no_links: bool,
    pub color: Color,
    pub linetype_scale: f64,
    pub linetype_flags: u8,
    pub plotstyle_flags: u8,
    pub material_flags: u16,    // R2007+
    pub shadow_flags: u8,       // R2007+
    pub invisible: bool,
    pub line_weight: LineWeight,
    // Handle references (raw, to be resolved):
    pub owner_handle: Option<u64>,
    pub reactor_handles: Vec<u64>,
    pub xdictionary_handle: Option<u64>,
    pub layer_handle: u64,
    pub linetype_handle: Option<u64>,
    pub plotstyle_handle: Option<u64>,
    pub material_handle: Option<u64>,
}
```

### 10.3 Entity Reading (`reader/object_reader/entities.rs`)

This file reads all ~35 entity types. Each entity type has a dedicated function:

```rust
pub fn read_line(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<Line>;
pub fn read_circle(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<Circle>;
pub fn read_arc(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<Arc>;
pub fn read_text(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<Text>;
pub fn read_mtext(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<MText>;
pub fn read_point(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<Point>;
pub fn read_ellipse(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<Ellipse>;
pub fn read_spline(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<Spline>;
pub fn read_insert(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<Insert>;
pub fn read_polyline(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<Polyline>;
pub fn read_lwpolyline(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<LwPolyline>;
pub fn read_hatch(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<Hatch>;
pub fn read_solid(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<Solid>;
pub fn read_face3d(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<Face3D>;
pub fn read_dimension(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<Dimension>;
pub fn read_viewport(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<Viewport>;
pub fn read_leader(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<Leader>;
pub fn read_multileader(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<MultiLeader>;
pub fn read_mline(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<MLine>;
pub fn read_mesh(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<Mesh>;
pub fn read_ray(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<Ray>;
pub fn read_xline(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<XLine>;
pub fn read_solid3d(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<Solid3D>;
pub fn read_region(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<Region>;
pub fn read_body(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<Body>;
pub fn read_tolerance(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<Tolerance>;
pub fn read_shape(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<Shape>;
pub fn read_table_entity(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<TableEntity>;
pub fn read_raster_image(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<RasterImage>;
pub fn read_underlay(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<Underlay>;
pub fn read_wipeout(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<Wipeout>;
pub fn read_ole2frame(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<Ole2Frame>;
// Table entries (read from the same objects section):
pub fn read_layer(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<Layer>;
pub fn read_linetype(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<LineType>;
pub fn read_text_style(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<TextStyle>;
pub fn read_dim_style(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<DimStyle>;
pub fn read_block_record(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<BlockRecord>;
pub fn read_app_id(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<AppId>;
pub fn read_view(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<View>;
pub fn read_vport(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<VPort>;
pub fn read_ucs(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<Ucs>;
```

### 10.4 Non-Graphical Object Reading (`reader/object_reader/objects.rs`)

```rust
pub fn read_dictionary(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<Dictionary>;
pub fn read_xrecord(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<XRecord>;
pub fn read_layout(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<Layout>;
pub fn read_group(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<Group>;
pub fn read_mline_style(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<MLineStyle>;
pub fn read_image_definition(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<ImageDefinition>;
pub fn read_plot_settings(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<PlotSettings>;
pub fn read_multileader_style(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<MultiLeaderStyle>;
pub fn read_scale(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<Scale>;
pub fn read_sort_entities_table(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<SortEntitiesTable>;
pub fn read_table_style(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<TableStyle>;
pub fn read_dictionary_variable(reader: &mut dyn DwgStreamReader, version: DxfVersion) -> Result<DictionaryVariable>;
```

---

## 11. Phase 6 — Document Builder

### 11.1 Template System (`builder/templates.rs`)

Unlike DXF (which directly populates `CadDocument`), DWG uses a **two-pass template system**:

```rust
/// Template for a CAD object with unresolved handle references
pub trait CadObjectTemplate {
    /// Build this template into a real object, resolving handles
    fn build(&self, builder: &DwgDocumentBuilder) -> Result<()>;
}

/// Template for a table entry (Layer, LineType, etc.)
pub struct CadTableEntryTemplate<T: TableEntry> {
    pub entry: T,
    pub owner_handle: Option<u64>,
    pub reactor_handles: Vec<u64>,
    pub xdictionary_handle: Option<u64>,
    // Type-specific handle references:
    pub extra_handles: HashMap<String, u64>,
}

/// Template for an entity
pub struct CadEntityTemplate {
    pub entity: EntityType,
    pub common: EntityCommonData,
    // Handle references to resolve:
    pub layer_handle: u64,
    pub linetype_handle: Option<u64>,
    pub owner_handle: Option<u64>,
}

/// Template for a dictionary
pub struct CadDictionaryTemplate {
    pub dictionary: Dictionary,
    pub entries: Vec<(String, u64)>, // name → handle
}

/// Template for a block record (links block entity to record)
pub struct CadBlockRecordTemplate {
    pub handle: u64,
    pub block_entity_handle: Option<u64>,
    pub end_block_handle: Option<u64>,
    pub entity_handles: Vec<u64>,
}
```

### 11.2 Document Builder (`builder/mod.rs`)

```rust
/// Builds a CadDocument from DWG templates
pub struct DwgDocumentBuilder {
    /// The document being built
    pub document: CadDocument,

    /// Header handles from the header section
    pub header_handles: DwgHeaderHandlesCollection,

    /// Template maps (handle → template)
    pub cad_object_templates: HashMap<u64, Box<dyn CadObjectTemplate>>,
    pub dictionary_templates: HashMap<u64, CadDictionaryTemplate>,
    pub table_entry_templates: HashMap<u64, Box<dyn Any>>,
    pub table_templates: HashMap<u64, Box<dyn Any>>,
    pub block_record_templates: Vec<CadBlockRecordTemplate>,

    /// Resolved objects (handle → Arc<dyn Any>)
    pub cad_objects: HashMap<u64, Box<dyn Any>>,

    /// Entity sets for model/paper space
    pub model_space_entities: Vec<u64>,
    pub paper_space_entities: Vec<u64>,
}

impl DwgDocumentBuilder {
    pub fn new(version: DxfVersion) -> Self;

    /// Add a template to the appropriate collection
    pub fn add_template(&mut self, handle: u64, template: Box<dyn CadObjectTemplate>);

    /// Add a dictionary template
    pub fn add_dictionary_template(&mut self, handle: u64, template: CadDictionaryTemplate);

    /// Main build method — called after all objects are read
    pub fn build_document(self) -> Result<CadDocument>;

    /// Resolve a handle to a typed object
    pub fn try_get_object<T: 'static>(&self, handle: u64) -> Option<&T>;

    // Internal build phases:
    fn create_missing_handles(&mut self);
    fn set_blocks_to_records(&mut self);
    fn register_tables(&mut self);
    fn build_tables(&mut self) -> Result<()>;
    fn build_dictionaries(&mut self) -> Result<()>;
    fn build_entities(&mut self) -> Result<()>;
    fn update_header(&mut self);
}
```

---

## 12. Phase 7 — DWG Reader Orchestrator

### 12.1 DWG Reader (`reader/mod.rs`)

```rust
/// DWG file reader configuration
pub struct DwgReaderConfiguration {
    /// When true, CRC values are validated on read
    pub crc_check: bool,
    /// When true, summary info section is read
    pub read_summary_info: bool,
    /// When true, parse errors are caught as notifications
    pub failsafe: bool,
}

/// Main DWG file reader
pub struct DwgReader<R: Read + Seek> {
    stream: R,
    file_header: DwgFileHeader,
    version: DxfVersion,
    config: DwgReaderConfiguration,
}

impl DwgReader<BufReader<File>> {
    /// Open a DWG file from a path
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self>;
}

impl<R: Read + Seek> DwgReader<R> {
    /// Create from any seekable reader
    pub fn from_reader(reader: R) -> Result<Self>;

    /// Read the DWG file and return a CadDocument
    pub fn read(mut self) -> Result<CadDocument>;

    /// Set reader configuration
    pub fn with_configuration(mut self, config: DwgReaderConfiguration) -> Self;

    // Internal implementation:

    /// Read file header (dispatches by version)
    fn read_file_header(&mut self) -> Result<()>;
    fn read_file_header_ac15(&mut self) -> Result<()>;
    fn read_file_header_ac18(&mut self) -> Result<()>;
    fn read_file_header_ac21(&mut self) -> Result<()>;
    fn read_file_metadata(&mut self) -> Result<()>; // Shared AC18/AC21

    /// Get section buffer (assembled, decompressed, decrypted)
    fn get_section_buffer(&mut self, section_name: &str) -> Result<Vec<u8>>;
    fn get_section_buffer_ac15(&mut self, record_number: usize) -> Result<Vec<u8>>;
    fn get_section_buffer_ac18(&mut self, section_name: &str) -> Result<Vec<u8>>;
    fn get_section_buffer_ac21(&mut self, section_name: &str) -> Result<Vec<u8>>;

    /// Create version-specific stream reader from buffer
    fn create_stream_reader(&self, data: Vec<u8>) -> Box<dyn DwgStreamReader>;

    /// Read individual sections
    fn read_summary_info(&mut self, builder: &mut DwgDocumentBuilder) -> Result<()>;
    fn read_header(&mut self, builder: &mut DwgDocumentBuilder) -> Result<()>;
    fn read_classes(&mut self, builder: &mut DwgDocumentBuilder) -> Result<()>;
    fn read_app_info(&mut self) -> Result<()>;
    fn read_objects(&mut self, builder: &mut DwgDocumentBuilder) -> Result<()>;
}
```

**Read Sequence:**
1. Read file version string (6 bytes ASCII: "AC10XX")
2. Read file header (version-specific)
3. Create `DwgDocumentBuilder`
4. Read `AcDb:SummaryInfo` (if configured)
5. Read `AcDb:Header` → populates `HeaderVariables` + `HeaderHandlesCollection`
6. Read `AcDb:Classes` → populates `DxfClassCollection`
7. Read `AcDb:AppInfo` (diagnostic)
8. Read `AcDb:Handles` → builds object map
9. Read `AcDb:AcDbObjects` (via `DwgObjectReader` with handle queue)
10. `builder.build_document()` → returns `CadDocument`

---

## 13. Phase 8 — Section Writers

### 13.1 Header Writer (`writer/header_writer.rs`)

~1100 lines. Mirror of header reader:

```rust
pub struct DwgHeaderWriter<'a> {
    writer: &'a mut dyn DwgStreamWriter,
    version: DxfVersion,
}

impl<'a> DwgHeaderWriter<'a> {
    pub fn write(
        &mut self,
        header: &HeaderVariables,
        handles: &DwgHeaderHandlesCollection,
    ) -> Result<()>;
}
```

### 13.2 Classes Writer (`writer/classes_writer.rs`)

```rust
pub struct DwgClassesWriter<'a> { ... }

impl<'a> DwgClassesWriter<'a> {
    pub fn write(&mut self, classes: &DxfClassCollection) -> Result<()>;
}
```

### 13.3 Handle Writer (`writer/handle_writer.rs`)

```rust
pub struct DwgHandleWriter<'a> { ... }

impl<'a> DwgHandleWriter<'a> {
    /// Write delta-encoded handle→offset map
    pub fn write(&mut self, map: &BTreeMap<u64, i64>) -> Result<()>;
}
```

### 13.4 File Header Writers (`writer/file_header_writer/`)

```rust
// writer/file_header_writer/mod.rs

/// Interface for version-specific file header writing
pub trait DwgFileHeaderWriter {
    /// Register a section for writing
    fn add_section(&mut self, name: &str, data: Vec<u8>, compressed: bool);

    /// Get the offset where the handle section will be written
    fn handle_section_offset(&self) -> u64;

    /// Write everything to the output stream
    fn write_file<W: Write + Seek>(&self, writer: &mut W) -> Result<()>;
}

/// Factory
pub fn create_file_header_writer(version: DxfVersion) -> Box<dyn DwgFileHeaderWriter>;
```

**`header_writer_ac15.rs`** — Simple sequential layout with record table + CRC8 + sentinel.

**`header_writer_ac18.rs`** — Page-based layout with:
- Encrypted header block generation
- Section descriptor + page map construction
- LZ77 compression of section data
- Page header encryption
- CRC calculations

**`header_writer_ac21.rs`** — Stub/partial (R2007 writing not supported initially, matching ACadSharp).

### 13.5 Other Section Writers

**`writer/summary_info_writer.rs`**: ~60 lines  
**`writer/preview_writer.rs`**: ~60 lines  
**`writer/app_info_writer.rs`**: ~50 lines  
**`writer/aux_header_writer.rs`**: ~120 lines (version stamps, creation dates, etc.)

---

## 14. Phase 9 — Object Writer (Entities & Objects)

### 14.1 Object Writer Orchestrator (`writer/object_writer/mod.rs`)

```rust
/// Writes all entities, table entries, and objects to AcDb:AcDbObjects
pub struct DwgObjectWriter {
    version: DxfVersion,
    /// handle → file offset map (built during writing)
    pub handle_map: BTreeMap<u64, i64>,
}

impl DwgObjectWriter {
    pub fn write(&mut self, document: &CadDocument) -> Result<Vec<u8>>;

    /// Write all table control objects + entries
    fn write_tables(&mut self, document: &CadDocument, writer: &mut dyn DwgStreamWriter) -> Result<()>;

    /// Write all block-contained entities
    fn write_block_entities(&mut self, document: &CadDocument, writer: &mut dyn DwgStreamWriter) -> Result<()>;

    /// Write non-graphical objects (dictionaries, etc.)
    fn write_objects(&mut self, document: &CadDocument, writer: &mut dyn DwgStreamWriter) -> Result<()>;

    /// Write a single object (wrapper with MS size prefix + CRC)
    fn write_object_wrapper<F>(&mut self, handle: u64, writer: &mut dyn DwgStreamWriter, write_fn: F) -> Result<()>
    where F: FnOnce(&mut dyn DwgStreamWriter) -> Result<()>;
}
```

### 14.2 Common Entity Writing (`writer/object_writer/common.rs`)

```rust
/// Write common entity header data
pub fn write_common_entity_data(
    writer: &mut dyn DwgStreamWriter,
    entity: &EntityCommon,
    version: DxfVersion,
) -> Result<()>;

/// Write common non-graphical object data
pub fn write_common_object_data(
    writer: &mut dyn DwgStreamWriter,
    handle: Handle,
    version: DxfVersion,
) -> Result<()>;
```

### 14.3 Entity Writing (`writer/object_writer/entities.rs`)

Mirror of entity reading (~3000 lines):

```rust
pub fn write_line(writer: &mut dyn DwgStreamWriter, line: &Line, version: DxfVersion) -> Result<()>;
pub fn write_circle(writer: &mut dyn DwgStreamWriter, circle: &Circle, version: DxfVersion) -> Result<()>;
pub fn write_arc(writer: &mut dyn DwgStreamWriter, arc: &Arc, version: DxfVersion) -> Result<()>;
// ... all 35+ entity types
```

### 14.4 Non-Graphical Object Writing (`writer/object_writer/objects.rs`)

```rust
pub fn write_dictionary(writer: &mut dyn DwgStreamWriter, dict: &Dictionary, version: DxfVersion) -> Result<()>;
pub fn write_xrecord(writer: &mut dyn DwgStreamWriter, xrec: &XRecord, version: DxfVersion) -> Result<()>;
pub fn write_layout(writer: &mut dyn DwgStreamWriter, layout: &Layout, version: DxfVersion) -> Result<()>;
// ... remaining object types
```

---

## 15. Phase 10 — DWG Writer Orchestrator

### 15.1 DWG Writer (`writer/mod.rs`)

```rust
/// DWG writer configuration
pub struct DwgWriterConfiguration {
    pub close_stream: bool,
}

/// Main DWG file writer
pub struct DwgWriter {
    document: CadDocument,
    config: DwgWriterConfiguration,
}

impl DwgWriter {
    pub fn new(document: CadDocument) -> Self;

    /// Write to a file
    pub fn write_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()>;

    /// Write to any writer
    pub fn write_to_writer<W: Write + Seek>(&self, writer: &mut W) -> Result<()>;

    /// Write to a byte vector
    pub fn write_to_vec(&self) -> Result<Vec<u8>>;
}
```

**Write Sequence:**
1. Validate version (R14+ for write, not R2007)
2. Create `DwgFileHeaderWriter` for version
3. Create stream writer for version
4. Write `AcDb:Header` → add section to file header writer
5. Write `AcDb:Classes` → add section
6. Write `AcDb:SummaryInfo` → add section (AC18+)
7. Write `AcDb:Preview` → add section
8. Write `AcDb:AppInfo` → add section (AC18+)
9. Write `AcDb:FileDepList` → add section (AC18+)
10. Write `AcDb:RevHistory` → add section (AC18+)
11. Write `AcDb:AuxHeader` → add section
12. Write `AcDb:AcDbObjects` (via `DwgObjectWriter`) → add section
13. Write `AcDb:ObjFreeSpace` → add section
14. Write `AcDb:Template` → add section
15. Write `AcDb:Handles` (from `DwgObjectWriter.handle_map`) → add section
16. `file_header_writer.write_file(output)` → assemble final DWG file

---

## 16. Phase 11 — Testing Strategy

### 16.1 Test File Structure

```
tests/
├── dwg_reading_tests.rs          # Read DWG samples, verify entity/table counts
├── dwg_writing_tests.rs          # Write DWG, re-read, compare
├── dwg_roundtrip_tests.rs        # Read → Write → Read → Compare
├── dwg_bit_stream_tests.rs       # Unit tests for bit-level I/O
├── dwg_compression_tests.rs      # LZ77 compress/decompress round-trip
├── dwg_crc_tests.rs              # CRC calculation verification
└── dwg_version_tests.rs          # Version-specific format tests
```

### 16.2 Test Sample Files

ACadSharp provides sample DWG files in multiple versions:

| File | Version |
|------|---------|
| `sample_AC1014.dwg` | R14 |
| `sample_AC1015.dwg` | R2000 |
| `sample_AC1018.dwg` | R2004 |
| `sample_AC1021.dwg` | R2007 |
| `sample_AC1024.dwg` | R2010 |
| `sample_AC1027.dwg` | R2013 |
| `sample_AC1032.dwg` | R2018 |

### 16.3 Test Categories

1. **Unit Tests (bit-level)**
   - BitShort encode/decode round-trip
   - BitLong encode/decode round-trip
   - BitDouble encode/decode round-trip
   - Handle reference encode/decode
   - Modular char encode/decode
   - CRC-8 calculation against known values
   - CRC-32 calculation against known values
   - LZ77 AC18 compress/decompress round-trip
   - LZ77 AC21 decompress known data
   - Reed-Solomon decode known data
   - XOR encryption/decryption round-trip

2. **Integration Tests (section-level)**
   - File header reading for each version
   - Section buffer extraction + decompression
   - Object map reading + handle resolution
   - Header section reading (all system variables)
   - Classes section reading

3. **End-to-End Tests**
   - Read each sample DWG → verify entity counts match expectations
   - Read DWG → verify specific entity properties (coordinates, colors, etc.)
   - Read DWG → Write DXF → compare with original DXF sample
   - Write DWG → Read back → compare document contents

4. **Cross-Format Tests**
   - Read DXF → Write DWG → Read DWG → Compare
   - Read DWG → Write DXF → Read DXF → Compare

5. **Fuzz Testing**
   - Property-based tests for bit encoding (proptest)
   - Corrupted file handling (graceful error reporting)

---

## 17. Phase 12 — Performance & Optimization

### 17.1 Memory Strategy

- Use `Vec<u8>` for section buffers (owned, allocated once per section)
- Use `Cursor<Vec<u8>>` for bit-level stream readers/writers
- Pre-allocate buffers based on section sizes from file header
- Use `BufReader<File>` with large buffer (64KB) for sequential file reading

### 17.2 Parallelism Opportunities

- **Section decompression**: Sections are independent — decompress in parallel using `rayon`
- **Object writing**: Write each block record's entities independently
- **CRC calculation**: Can be done in parallel for multiple sections

### 17.3 Zero-Copy Where Possible

- Section buffers can be sliced for sub-stream creation (avoid copies)
- String handling: use `String::from_utf8_lossy` for non-Unicode content
- Handle map: use `ahash::HashMap` for faster hashing

### 17.4 Estimated Performance Targets

| Operation | Target |
|-----------|--------|
| Read small DWG (<1MB) | <50ms |
| Read medium DWG (1-10MB) | <200ms |
| Read large DWG (>10MB) | <1s |
| Write small DWG | <100ms |
| Write medium DWG | <500ms |
| Bit encode/decode overhead | <5% vs raw bytes |

---

## 18. Version Support Matrix

### Reader Support

| Version | File Header | Sections | Objects | Status |
|---------|-------------|----------|---------|--------|
| AC1012 (R13) | AC15 | Sequential | Standard | Planned |
| AC1014 (R14) | AC15 | Sequential | Standard | Planned |
| AC1015 (R2000) | AC15 | Sequential | Standard | Priority |
| AC1018 (R2004) | AC18 | Paged+Encrypted+LZ77 | Standard | Priority |
| AC1021 (R2007) | AC21 | RS+LZ77 AC21 | 3-stream | Priority |
| AC1024 (R2010) | AC18 | Paged+Encrypted+LZ77 | 3-stream+OT | Priority |
| AC1027 (R2013) | AC18 | Paged+Encrypted+LZ77 | 3-stream+OT | Priority |
| AC1032 (R2018) | AC18 | Paged+Encrypted+LZ77 | 3-stream+OT | Priority |

### Writer Support

| Version | File Header | Compression | Status |
|---------|-------------|-------------|--------|
| AC1012 (R13) | ❌ Not supported | N/A | — |
| AC1014 (R14) | AC15 | None | Planned |
| AC1015 (R2000) | AC15 | None | Priority |
| AC1018 (R2004) | AC18 | LZ77 AC18 | Priority |
| AC1021 (R2007) | ❌ Not supported | LZ77 AC21 N/I | — |
| AC1024 (R2010) | AC18 | LZ77 AC18 | Planned |
| AC1027 (R2013) | AC18 | LZ77 AC18 | Planned |
| AC1032 (R2018) | AC18 | LZ77 AC18 | Planned |

> **Note:** R2007 (AC1021) writing is intentionally excluded (matching ACadSharp) because the LZ77 AC21 compressor is not implemented and the file header format is significantly different. R2007 files can be read and written as R2010+ or R2004.

---

## 19. Risk Analysis & Mitigations

### 19.1 High-Risk Areas

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Bit-alignment bugs | Data corruption | High | Extensive encode/decode round-trip testing |
| Version-conditional field ordering | Wrong data reads | High | Strict ACadSharp reference mapping |
| LZ77 decompression edge cases | Read failures | Medium | Test against all sample files |
| Handle reference resolution | Missing objects | Medium | Validate against DXF equivalent |
| CRC/checksum mismatch | Rejected files | Medium | Test files opened by AutoCAD |
| Page assembly order | Corrupt sections | Medium | Byte-exact comparison with known outputs |
| Reed-Solomon decode errors | R2007 read failure | Low | Simple de-interleave, test known data |

### 19.2 Complexity Estimates (Lines of Rust)

| Component | Estimated Lines | ACadSharp Lines (C#) |
|-----------|----------------|---------------------|
| Constants + CRC + Checksum | ~500 | ~550 |
| Encryption/Compression/RS | ~1,200 | ~1,300 |
| File Headers | ~800 | ~900 |
| Stream Readers (all versions) | ~1,500 | ~1,700 |
| Stream Writers (all versions) | ~1,200 | ~1,400 |
| Merged Reader/Writer | ~400 | ~500 |
| Section Readers | ~1,500 | ~1,600 |
| Object Reader | ~5,000 | ~7,200 |
| Document Builder + Templates | ~800 | ~700 |
| Reader Orchestrator | ~800 | ~1,200 |
| Section Writers | ~1,500 | ~1,700 |
| Object Writer | ~4,000 | ~5,500 |
| File Header Writers | ~800 | ~700 |
| Writer Orchestrator | ~400 | ~400 |
| **Total** | **~20,000** | **~25,000** |

---

## 20. Dependencies & Crate Selection

### 20.1 Existing Dependencies (Already in Cargo.toml)

| Crate | Use for DWG |
|-------|-------------|
| `byteorder` | LE/BE byte reading/writing |
| `encoding_rs` | Code page text encoding |
| `flate2` | Not used (DWG uses LZ77, not deflate) |
| `rayon` | Parallel section decompression |
| `ahash` | Fast HashMap for handle maps |
| `indexmap` | Ordered collections |
| `bitflags` | Flag types |
| `thiserror` | Error types |

### 20.2 New Dependencies Needed

| Crate | Purpose | Notes |
|-------|---------|-------|
| `bitvec` (optional) | Bit-level manipulation | Could use, but hand-rolled may be faster |

### 20.3 No New Dependencies Recommended

The bit-level I/O is best implemented manually (matching ACadSharp's approach) rather than using a generic bit manipulation library. The DWG-specific encoding (BitShort, BitLong, BitDouble, Handle, ModularChar) is too specialized for generic solutions to be efficient.

---

## 21. Implementation Priority & Timeline

### Sprint 1: Foundation (Weeks 1-2)
1. ✅ Create folder structure
2. ✅ `constants.rs` — all magic numbers and sentinels
3. ✅ `crc.rs` — CRC-8 and CRC-32 lookup tables + algorithms
4. ✅ `checksum.rs` — Adler-32 + magic sequence
5. ✅ `encryption.rs` — XOR encryption/decryption
6. ✅ `reference_type.rs` — Handle reference types
7. ✅ `section_io.rs` — Version flags
8. ✅ Unit tests for all foundation components

### Sprint 2: Bit-Level I/O (Weeks 3-4)
1. ✅ `stream_reader.rs` — DwgStreamReader trait
2. ✅ `stream_reader_base.rs` — Core bit reader (~700 lines)
3. ✅ All version-specific reader overrides (ac12–ac24)
4. ✅ `merged_reader.rs` — 3-stream multiplexer
5. ✅ Encode/decode round-trip tests for all bit types

### Sprint 3: Compression & File Structure (Weeks 5-6)
1. ✅ `lz77_ac18.rs` — Compressor + decompressor
2. ✅ `lz77_ac21.rs` — Decompressor (compressor deferred)
3. ✅ `reed_solomon.rs` — Byte interleave decode/encode
4. ✅ All file header types (AC15, AC18, AC21)
5. ✅ Section descriptor, locator record, local section map

### Sprint 4: Reader Core (Weeks 7-9)
1. ✅ `DwgReader` orchestrator — file header reading for all versions
2. ✅ Section buffer extraction + assembly for AC15/AC18/AC21
3. ✅ `handle_reader.rs` — Object map reading
4. ✅ `header_reader.rs` — System variables (~1000 lines)
5. ✅ `classes_reader.rs` — DXF classes
6. ✅ Test: read file headers from all sample DWG files

### Sprint 5: Object Reader — Tables & Simple Entities (Weeks 10-12)
1. ✅ Object reader orchestrator (queue-based)
2. ✅ Common entity data reading
3. ✅ Table entries: Layer, LineType, TextStyle, DimStyle, BlockRecord, AppId, View, VPort, Ucs
4. ✅ Simple entities: Line, Circle, Arc, Point, Ray, XLine, Solid, Face3D
5. ✅ Text entities: Text, MText
6. ✅ Test: read all sample DWG files, verify basic entities

### Sprint 6: Object Reader — Complex Entities (Weeks 13-15)
1. ✅ Polyline variants: Polyline, Polyline2D, Polyline3D, LwPolyline, PolyfaceMesh, PolygonMesh
2. ✅ Ellipse, Spline, Hatch
3. ✅ Insert, Block, Dimension
4. ✅ Leader, MultiLeader, MLine
5. ✅ Viewport, Mesh, Solid3D, Region, Body
6. ✅ Tolerance, Shape, Table, RasterImage, Underlay, Wipeout, Ole2Frame
7. ✅ Test: entity property verification against DXF equivalents

### Sprint 7: Document Builder + Objects (Weeks 16-17)
1. ✅ Template system implementation
2. ✅ Dictionary reading + resolution
3. ✅ Layout, XRecord, Group, MLineStyle, PlotSettings, etc.
4. ✅ Summary info reader, preview reader, app info reader
5. ✅ Full document builder: handle resolution, block linking, table registration
6. ✅ Test: complete Read DWG → Compare with DXF equivalent

### Sprint 8: Writer — Bit-Level I/O (Weeks 18-19)
1. ✅ `stream_writer.rs` — DwgStreamWriter trait
2. ✅ `stream_writer_base.rs` — Core bit writer
3. ✅ All version-specific writer overrides
4. ✅ Merged writers (AC14 and R2007+)
5. ✅ Encode/decode symmetry tests (reader ↔ writer)

### Sprint 9: Writer — Sections (Weeks 20-22)
1. ✅ `header_writer.rs` — System variables
2. ✅ `classes_writer.rs`
3. ✅ `handle_writer.rs` — Object map
4. ✅ `aux_header_writer.rs`
5. ✅ `summary_info_writer.rs`, `preview_writer.rs`, `app_info_writer.rs`
6. ✅ File header writers (AC15, AC18)

### Sprint 10: Object Writer + Full Writer (Weeks 23-26)
1. ✅ Object writer orchestrator
2. ✅ Common entity/object data writing
3. ✅ All entity type writers
4. ✅ All non-graphical object writers
5. ✅ DwgWriter orchestrator — full file assembly
6. ✅ Test: Write DWG → Read back → Verify

### Sprint 11: Integration & Polish (Weeks 27-28)
1. ✅ Cross-format round-trip tests (DXF ↔ DWG)
2. ✅ AutoCAD validation (open written files in AutoCAD)
3. ✅ Fuzz testing with corrupted files
4. ✅ Performance benchmarks
5. ✅ Documentation
6. ✅ Cargo.toml updates (keywords: "dwg", feature flags)

---

## Appendix A: Object Type Numbers

DWG uses numeric type codes (not string names like DXF):

| Code | Entity/Object |
|------|---------------|
| 0x01 | TEXT |
| 0x02 | ATTRIB |
| 0x03 | ATTDEF |
| 0x04 | BLOCK |
| 0x05 | ENDBLK |
| 0x06 | SEQEND |
| 0x07 | INSERT |
| 0x08 | MINSERT |
| 0x0A | VERTEX_2D |
| 0x0B | VERTEX_3D |
| 0x0C | VERTEX_MESH |
| 0x0D | VERTEX_PFACE |
| 0x0E | VERTEX_PFACE_FACE |
| 0x0F | POLYLINE_2D |
| 0x10 | POLYLINE_3D |
| 0x11 | ARC |
| 0x12 | CIRCLE |
| 0x13 | LINE |
| 0x14 | DIMENSION_ORDINATE |
| 0x15 | DIMENSION_LINEAR |
| 0x16 | DIMENSION_ALIGNED |
| 0x17 | DIMENSION_ANG_3PT |
| 0x18 | DIMENSION_ANG_2LN |
| 0x19 | DIMENSION_RADIUS |
| 0x1A | DIMENSION_DIAMETER |
| 0x1B | POINT |
| 0x1C | FACE3D |
| 0x1D | POLYLINE_PFACE |
| 0x1E | POLYLINE_MESH |
| 0x1F | SOLID |
| 0x20 | TRACE |
| 0x21 | SHAPE |
| 0x22 | VIEWPORT |
| 0x23 | ELLIPSE |
| 0x24 | SPLINE |
| 0x25 | REGION |
| 0x26 | SOLID3D |
| 0x27 | BODY |
| 0x28 | RAY |
| 0x29 | XLINE |
| 0x2A | DICTIONARY |
| 0x2C | MTEXT |
| 0x2D | LEADER |
| 0x2E | TOLERANCE |
| 0x2F | MLINE |
| 0x30 | BLOCK_CONTROL |
| 0x31 | BLOCK_HEADER |
| 0x32 | LAYER_CONTROL |
| 0x33 | LAYER |
| 0x34 | STYLE_CONTROL |
| 0x35 | STYLE |
| 0x38 | LTYPE_CONTROL |
| 0x39 | LTYPE |
| 0x3C | VIEW_CONTROL |
| 0x3D | VIEW |
| 0x3E | UCS_CONTROL |
| 0x3F | UCS |
| 0x40 | VPORT_CONTROL |
| 0x41 | VPORT |
| 0x42 | APPID_CONTROL |
| 0x43 | APPID |
| 0x44 | DIMSTYLE_CONTROL |
| 0x45 | DIMSTYLE |
| 0x46 | VP_ENT_HDR_CTRL |
| 0x47 | VP_ENT_HDR |
| 0x48 | GROUP |
| 0x49 | MLINESTYLE |
| 0x4A | OLE2FRAME |
| 0x4C | LONG_TRANSACTION |
| 0x4D | LWPOLYLINE |
| 0x4E | HATCH |
| 0x4F | XRECORD |
| 0x50 | PLACEHOLDER |
| 0x51 | VBA_PROJECT |
| 0x52 | LAYOUT |
| 500+ | Custom class objects (looked up via DxfClassCollection) |

---

## Appendix B: Error Type Updates

The existing `DxfError` enum already covers most DWG needs. Minor additions may include:

```rust
// Potential additions to DxfError:
#[error("Unsupported DWG version for writing: {0}")]
UnsupportedWriteVersion(String),

#[error("Invalid object type: {0}")]
InvalidObjectType(i16),

#[error("Handle not found in object map: {0:#X}")]
HandleNotFound(u64),

#[error("Reed-Solomon decode error: {0}")]
ReedSolomonError(String),

#[error("Page assembly error: {0}")]
PageAssemblyError(String),
```

Consider renaming `DxfError` to `CadError` since it will serve both DXF and DWG, or creating an alias.

---

## Appendix C: Cargo.toml Updates

```toml
[package]
name = "acadrust"
version = "0.2.0"
description = "A pure Rust library for reading and writing CAD files in DXF and DWG formats"
keywords = ["cad", "dxf", "dwg", "autocad", "drawing"]

[features]
default = ["dxf", "dwg"]
dxf = []          # DXF reading/writing
dwg = []          # DWG reading/writing (depends on compression, CRC)
```

---

## Appendix D: Public API Examples

### Reading a DWG File

```rust
use acadrust::{CadDocument, io::dwg::DwgReader};

// Read a DWG file
let doc = DwgReader::from_file("drawing.dwg")?.read()?;

// Access entities
for entity in doc.entities() {
    println!("Entity: {:?}", entity);
}

// Same CadDocument works with DXF writer
use acadrust::io::dxf::DxfWriter;
DxfWriter::new(doc.clone()).write_to_file("output.dxf")?;
```

### Writing a DWG File

```rust
use acadrust::{CadDocument, io::dwg::DwgWriter};

// Build a document
let mut doc = CadDocument::new();
doc.version = DxfVersion::AC1024; // R2010
doc.add_entity(EntityType::Line(Line { /* ... */ }));

// Write to DWG
DwgWriter::new(doc).write_to_file("output.dwg")?;
```

### Cross-Format Conversion

```rust
use acadrust::io::{dxf::DxfReader, dwg::DwgWriter};

// Read DXF → Write DWG
let doc = DxfReader::from_file("input.dxf")?.read()?;
DwgWriter::new(doc).write_to_file("output.dwg")?;

// Read DWG → Write DXF
let doc = DwgReader::from_file("input.dwg")?.read()?;
DxfWriter::new(doc).write_to_file("output.dxf")?;
```
