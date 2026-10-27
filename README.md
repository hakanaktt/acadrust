# acadrust

[![CI](https://github.com/hakanaktt/acadrust/actions/workflows/ci.yml/badge.svg)](https://github.com/hakanaktt/acadrust/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/acadrust.svg)](https://crates.io/crates/acadrust)
[![Documentation](https://docs.rs/acadrust/badge.svg)](https://docs.rs/acadrust)
[![License: MPL 2.0](https://img.shields.io/badge/License-MPL%202.0-brightgreen.svg)](https://opensource.org/licenses/MPL-2.0)

See [third-party notices](THIRD_PARTY_NOTICES.md) for DWG code provenance and attribution.

**A pure Rust crate for reading, writing, and inspecting CAD files.**

acadrust handles ASCII and binary DXF plus native binary DWG without requiring
an installed CAD application. File support spans DXF and DWG R12 through R2018+.

## Quick Start

```toml
[dependencies]
acadrust = "0.6.8"
```

```rust
use acadrust::{DxfReader, DxfWriter};

fn main() -> acadrust::Result<()> {
    let doc = DxfReader::from_file("input.dxf")?.read()?;
    println!("{} entities", doc.entities().count());

    DxfWriter::new(&doc).write_to_file("output.dxf")?;
    Ok(())
}
```

`DxfReader` detects ASCII and binary input automatically. To produce binary
DXF, use `DxfWriter::new_binary(&doc)`.

### Cargo features

| Feature | Default | Adds |
|---------|---------|------|
| `parallel` | Yes | Rayon parallelism for native DWG reading and writing |
| `serde` | No | `Serialize` and `Deserialize` implementations for document types |
| `import` | No | STL, COLLADA, OBJ, glTF/GLB, and FBX importers |

Enable optional features as needed:

```toml
[dependencies]
acadrust = { version = "0.6.8", features = ["serde", "import"] }
```

To run DWG work sequentially on the calling thread and omit acadrust's Rayon
dependency, disable default features:

```toml
[dependencies]
acadrust = { version = "0.6.8", default-features = false }
```

Cargo features are additive: another dependency enabling `acadrust/parallel`
can enable it for your application too. Existing applications using
`default-features = false` must add `features = ["parallel"]` to retain native
parallelism. WebAssembly remains sequential regardless of this feature.

### Limit DWG concurrency

Native DWG reads and writes use the current Rayon pool, or Rayon's shared
global pool when called outside a pool. To bound concurrency, create an
application-owned pool once and reuse it for all DWG jobs:

```toml
[dependencies]
acadrust = "0.6.8"
rayon = "1"
```

```rust
use acadrust::DwgReader;
use rayon::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = rayon::ThreadPoolBuilder::new().num_threads(4).build()?;
    let paths = ["first.dwg", "second.dwg"];

    let counts: Vec<usize> = pool.install(|| {
        paths.par_iter().map(|path| {
            let mut reader = DwgReader::from_file(path)?;
            let doc = reader.read()?;
            Ok(doc.entities().count())
        }).collect::<acadrust::Result<_>>()
    })?;

    println!("Entity counts: {counts:?}");
    Ok(())
}
```

The batch and acadrust's internal parallel work share these four workers;
reuse the same `pool` for later batches and wrap writes in `pool.install` too.
Separate pools or DWG calls outside this pool are not covered by its limit.
A one-thread pool still executes work on a Rayon worker; disabling `parallel`
keeps acadrust's work on its calling thread.

## Features

- **DXF I/O** — ASCII and binary formats, R12 through R2018+
- **DWG I/O** — Native binary formats, R12 through R2018+
- **Broad entity coverage** — 48 top-level `EntityType` variants covering 2D
  geometry, annotations, dimensions, meshes, underlays, viewports, 3D solids,
  regions, bodies, and native surfaces
- **ACIS modeling data** — SAT/SAB parsing and writing, B-rep topology, solid
  history, and primitive builders
- **Tables and objects** — Layers, linetypes, styles, dictionaries, layouts,
  materials, fields, dynamic blocks, and associative data
- **Resilient reads** — Optional failsafe recovery with bounded, structured
  diagnostics and read statistics
- **Encoding support** — Automatic handling of roughly 40 code pages for
  pre-2007 drawings
- **Optional serialization** — Serde support for document data
- **Optional 3D imports** — STL, COLLADA, OBJ, glTF/GLB, and FBX converted to
  acadrust documents, including node and parent transforms
- **ACAD Arch** — Read and preserve AEC classes, construct straight walls,
  doors, and windows, and edit supported native parameters. See the
  [Arch API](src/classes/arch/API.md)

## File Version Support

| File code | Release era | DXF | DWG |
|-----------|-----------------|-----|-----|
| AC1009 | R11/R12 | R/W | R/W |
| AC1012 | R13 | R/W | R/W |
| AC1014 | R14 | R/W | R/W |
| AC1015 | 2000 | R/W | R/W |
| AC1018 | 2004 | R/W | R/W |
| AC1021 | 2007 | R/W | R/W |
| AC1024 | 2010 | R/W | R/W |
| AC1027 | 2013 | R/W | R/W |
| AC1032 | 2018+ | R/W | R/W |

`R/W` means read and write support. Entity availability varies by file version.
AC1009 is the shared native DWG format used by AutoCAD Release 11 and Release
12. See the [R11/R12 support status](src/docs/ac1009_support.md) for native
fixture coverage and known format limits. DXF export of R11/R12 documents is
promoted to R13 or later.

## Examples

<details>
<summary>DWG Read/Write</summary>

```rust
use acadrust::{CadDocument, Color, DwgReader, DwgWriter, EntityType, Line};

fn main() -> acadrust::Result<()> {
    let mut reader = DwgReader::from_file("drawing.dwg")?;
    let doc = reader.read()?;

    for entity in doc.entities() {
        println!("{:?}", entity);
    }

    let mut doc = CadDocument::new();
    let mut line = Line::from_coords(0.0, 0.0, 0.0, 100.0, 50.0, 0.0);
    line.common.color = Color::RED;
    doc.add_entity(EntityType::Line(line))?;
    DwgWriter::write_to_file("output.dwg", &doc)?;
    Ok(())
}
```
</details>

<details>
<summary>Paper Space Layouts & Viewports</summary>

```rust
use acadrust::{CadDocument, DxfVersion, DxfWriter};
use acadrust::entities::{EntityType, Viewport};
use acadrust::types::Vector3;

fn main() -> acadrust::Result<()> {
    let mut doc = CadDocument::with_version(DxfVersion::AC1027);

    // Add geometry to model space
    let line = acadrust::entities::Line::from_coords(0.0, 0.0, 0.0, 100.0, 100.0, 0.0);
    doc.add_entity(EntityType::Line(line))?;

    // Overall viewport (ID=1) for default Layout1
    let mut overall_vp = Viewport::new();
    overall_vp.id = 1;
    overall_vp.center = Vector3::new(148.5, 105.0, 0.0);
    doc.add_paper_space_entity(EntityType::Viewport(overall_vp))?;

    // Detail viewport using builder pattern
    let mut vp1 = Viewport::new()
        .with_center(Vector3::new(148.5, 105.0, 0.0))
        .with_view_target(Vector3::new(50.0, 50.0, 0.0))
        .with_scale(1.0)
        .with_locked();
    vp1.id = 2;
    doc.add_paper_space_entity(EntityType::Viewport(vp1))?;

    // Create a second layout with its own viewport
    doc.add_layout("Layout2")?;
    let mut vp2 = Viewport::with_size(Vector3::new(200.0, 150.0, 0.0), 400.0, 300.0);
    vp2.id = 2;
    doc.add_entity_to_layout(EntityType::Viewport(vp2), "Layout2")?;

    DxfWriter::new(&doc).write_to_file("layouts.dxf")?;
    Ok(())
}
```
</details>

<details>
<summary>Failsafe Reading and Diagnostics</summary>

```rust
use acadrust::{DxfReader, DxfReaderConfiguration};

fn main() -> acadrust::Result<()> {
    let config = DxfReaderConfiguration {
        failsafe: true,
        ..Default::default()
    };
    let outcome = DxfReader::from_file("drawing.dxf")?
        .with_configuration(config)
        .read_with_stats()?;

    println!("{} entities", outcome.document.entities().count());
    for diagnostic in &outcome.stats.diagnostics {
        eprintln!("{}: {}", diagnostic.code, diagnostic.message);
    }
    Ok(())
}
```
</details>

<details>
<summary>Import a 3D Model</summary>

Requires `features = ["import"]`.

```rust
use acadrust::{import_file, DwgWriter, ImportConfig};

fn main() -> acadrust::Result<()> {
    let doc = import_file("model.glb", &ImportConfig::default())?;
    DwgWriter::write_to_file("model.dwg", &doc)?;
    Ok(())
}
```
</details>

<details>
<summary>Serde / JSON</summary>

```rust
use acadrust::{CadDocument, DxfReader};

fn main() -> acadrust::Result<()> {
    let doc = DxfReader::from_file("drawing.dxf")?.read()?;
    let json = serde_json::to_string_pretty(&doc).unwrap();
    let doc2: CadDocument = serde_json::from_str(&json).unwrap();
    println!("Entities: {}", doc2.entities().count());
    Ok(())
}
```
</details>

## Documentation

- [API documentation](https://docs.rs/acadrust)
- [Arch API](src/classes/arch/API.md)
- [Paper-space viewport example](examples/viewport_layouts.rs)

## Development

```console
cargo test
cargo test --all-features
cargo check --all-targets --all-features
```

GitHub Actions runs these checks with stable Rust on Linux, Windows, and macOS
for pull requests and pushes to `main`. It also checks default and all-feature
library builds for `wasm32-unknown-unknown`.

Tests that need drawings which are not in the repository are `#[ignore]`d,
with the missing file as the reason, so `cargo test` reports them as ignored.
If you have a file, run its tests with `--ignored`, for example
`cargo test --test table_dwg_read -- --ignored`; a test whose file is missing
then fails and names it.

---

## Used By
- [Open CAD Studio](https://github.com/HakanSeven12/OpenCADStudio) An open-source (GPLv3) CAD application that uses acadrust as its core native DWG/DXF engine for read/write operations and 3D modeling.
- [CADCraft](https://github.com/storytold/cadcraft) An open-source, AutoCAD-style drafting application written in Rust that uses acadrust for DWG read/write operations.

## Support & Sponsorship

If you find this project useful and want to support ongoing maintenance and new features, consider buying me a coffee or becoming a patron!

[![Buy Me A Coffee](https://img.shields.io/badge/Buy%20Me%20A%20Coffee-Donate-yellow.svg?style=for-the-badge&logo=buy-me-a-coffee)](https://www.buymeacoffee.com/hakanak)
[![Patreon](https://img.shields.io/badge/Patreon-Support-orange.svg?style=for-the-badge&logo=patreon)](https://www.patreon.com/hakanaktt)

Every contribution helps keep this project active and well-maintained. Thank you!

## License

MPL-2.0 — see [LICENSE](LICENSE).
