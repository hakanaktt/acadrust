# acadrust

[![Crates.io](https://img.shields.io/crates/v/acadrust.svg)](https://crates.io/crates/acadrust)
[![Documentation](https://docs.rs/acadrust/badge.svg)](https://docs.rs/acadrust)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-2021-orange.svg)](https://www.rust-lang.org/)

**A pure Rust library for reading and writing CAD drawing exchange files (DXF).**

acadrust provides comprehensive support for the DXF file format with a focus on correctness, type safety, and completeness. Inspired by [ACadSharp](https://github.com/DomCR/ACadSharp), this library brings full-featured DXF file manipulation to the Rust ecosystem.

---

## ✨ Features

### Core Capabilities

- **📖 Read & Write** — Full support for both ASCII and Binary DXF formats
- **🔒 Type Safe** — Leverages Rust's type system with strongly-typed entities, tables, and objects
- **🌐 Encoding Support** — Automatic code page detection and character encoding for pre-2007 files (~40 code pages via `encoding_rs`)
- **🛡️ Failsafe Mode** — Optional error-tolerant parsing that collects diagnostics instead of aborting
- **📋 Notifications** — Structured diagnostic system reporting unsupported elements, warnings, and errors
- **🔗 Handle Resolution** — Automatic owner handle assignment and handle tracking after read
- **❓ Unknown Entity Preservation** — Unrecognized entity types are preserved as `UnknownEntity` with common fields intact

### File Version Support

| Version Code | AutoCAD Version |
|-------------|-----------------|
| AC1012 | R13 |
| AC1014 | R14 |
| AC1015 | 2000 |
| AC1018 | 2004 |
| AC1021 | 2007 (UTF-8) |
| AC1024 | 2010 |
| AC1027 | 2013 |
| AC1032 | 2018+ |

### Supported Entity Types (38)

<details>
<summary>Click to expand full entity list</summary>

#### Basic Entities
- **Point** — Single point in 3D space
- **Line** — Line segment between two points
- **Circle** — Circle defined by center and radius
- **Arc** — Circular arc with start and end angles
- **Ellipse** — Ellipse or elliptical arc

#### Polylines
- **Polyline** — 2D polyline with optional bulge
- **Polyline3D** — 3D polyline
- **LwPolyline** — Lightweight polyline (optimized 2D)
- **PolyfaceMesh** — 3D mesh defined by vertices and faces
- **PolygonMesh** — 3D polygon surface mesh (M×N grid)

#### Text & Annotations
- **Text** — Single-line text
- **MText** — Multi-line formatted text
- **AttributeDefinition** — Block attribute template
- **AttributeEntity** — Block attribute instance
- **Tolerance** — Geometric tolerancing symbols

#### Dimensions & Leaders
- **Dimension** — Various dimension types (linear, angular, radial, etc.)
- **Leader** — Leader line with annotation
- **MultiLeader** — Modern multi-leader with advanced formatting
- **Table** — Table with cells, rows, and columns

#### Complex Entities
- **Spline** — NURBS curve
- **Hatch** — Filled region with pattern
- **Solid** — 2D filled polygon
- **Face3D** — 3D triangular/quadrilateral face
- **Mesh** — Subdivision mesh surface

#### Blocks & References
- **Block** / **BlockEnd** — Block definition markers
- **Insert** — Block reference (instance)
- **Seqend** — Sequence end marker for complex entities

#### Construction Geometry
- **Ray** — Semi-infinite line
- **XLine** — Infinite construction line

#### Advanced Entities
- **Viewport** — Paper space viewport
- **RasterImage** — Embedded or linked raster image
- **Solid3D** — 3D solid with ACIS data
- **Region** — 2D region with ACIS data
- **Body** — 3D body with ACIS data
- **MLine** — Multi-line with style
- **Wipeout** — Masking region
- **Shape** — Shape reference
- **Underlay** — PDF/DWF/DGN underlay reference
- **Ole2Frame** — OLE 2.0 embedded object
- **UnknownEntity** — Preserves common fields for unrecognized entity types

</details>

### Table System

Complete support for all standard tables:

| Table | Description |
|-------|-------------|
| **Layer** | Drawing layers with color, linetype, and visibility |
| **LineType** | Line patterns and dash definitions |
| **TextStyle** | Font and text formatting settings |
| **DimStyle** | Dimension appearance and behavior |
| **BlockRecord** | Block definition registry |
| **AppId** | Application identifier registry |
| **View** | Named view configurations |
| **VPort** | Viewport configurations |
| **UCS** | User coordinate system definitions |

### Objects (Non-Graphical Elements)

- **Dictionary** / **DictionaryWithDefault** — Key-value storage for objects
- **DictionaryVariable** — Named variable in a dictionary
- **Group** — Named entity collections
- **Layout** — Model/paper space layout definitions
- **MLineStyle** — Multi-line style definitions
- **MultiLeaderStyle** — Multi-leader style definitions
- **TableStyle** — Table formatting styles
- **PlotSettings** — Print/plot configurations
- **Scale** — Annotation scale definitions
- **ImageDefinition** / **ImageDefinitionReactor** — Raster image definitions and reactors
- **XRecord** — Extended data records
- **SortEntitiesTable** — Entity draw order
- **VisualStyle** — 3D visual style definitions
- **Material** — Material definitions
- **GeoData** — Geolocation data
- **SpatialFilter** — Spatial clipping filter
- **RasterVariables** — Raster display settings
- **BookColor** — Color book (DBCOLOR) entries
- **PlaceHolder** — Placeholder objects
- **WipeoutVariables** — Wipeout display settings

### CLASSES Section

Full support for the CLASSES section — reading, storing, and writing DXF class definitions with all standard fields (class name, DXF name, application name, proxy flags, instance count).

### Extended Data (XData)

Full support for application-specific extended data:

- String, binary, and numeric values
- 3D points, directions, and displacements
- Layer references and database handles
- Nested data structures with control strings

### Reactors & Extension Dictionaries

Full support for entity/object reactor chains (group code 102 `{ACAD_REACTORS}`) and extension dictionaries (`{ACAD_XDICTIONARY}`), read and written for all entity and object types.

---

## 📦 Installation

Add acadrust to your `Cargo.toml`:

```toml
[dependencies]
acadrust = "0.1.6"
```

Or install via cargo:

```bash
cargo add acadrust
```

---

## 🚀 Quick Start

### Reading a DXF File

```rust
use acadrust::{CadDocument, DxfReader};

fn main() -> acadrust::Result<()> {
    // Open and read a DXF file
    let doc = DxfReader::from_file("drawing.dxf")?.read()?;
    
    // Access document properties
    println!("Version: {:?}", doc.header().version);
    
    // Iterate over entities in model space
    for entity in doc.entities() {
        println!("Entity: {:?}", entity);
    }
    
    // Check parse notifications
    for note in doc.notifications.iter() {
        println!("[{:?}] {}", note.level, note.message);
    }
    
    Ok(())
}
```

### Reading with Failsafe Mode

```rust
use acadrust::{DxfReader};
use acadrust::io::dxf::DxfReaderConfiguration;

fn main() -> acadrust::Result<()> {
    let config = DxfReaderConfiguration { failsafe: true };
    let doc = DxfReader::from_file("drawing.dxf")?
        .with_configuration(config)
        .read()?;
    
    // Even if some sections had errors, the document is partially populated
    println!("Entities read: {}", doc.entities().len());
    println!("Notifications: {}", doc.notifications.len());
    
    Ok(())
}
```

### Writing a DXF File

```rust
use acadrust::{CadDocument, DxfWriter, Line, Layer, Vector3};

fn main() -> acadrust::Result<()> {
    // Create a new document
    let mut doc = CadDocument::new();
    
    // Add a layer
    let layer = Layer::new("MyLayer");
    doc.layers_mut().add(layer)?;
    
    // Create and add a line
    let line = Line {
        start: Vector3::new(0.0, 0.0, 0.0),
        end: Vector3::new(100.0, 100.0, 0.0),
        ..Default::default()
    };
    doc.add_entity(line);
    
    // Write to file
    DxfWriter::new(&doc).write_to_file("output.dxf")?;
    
    Ok(())
}
```

### Working with Layers

```rust
use acadrust::{CadDocument, Layer, Color};

fn main() -> acadrust::Result<()> {
    let mut doc = CadDocument::new();
    
    // Create a custom layer
    let mut layer = Layer::new("Annotations");
    layer.color = Color::from_index(1); // Red
    layer.is_frozen = false;
    layer.is_locked = false;
    
    doc.layers_mut().add(layer)?;
    
    // Access existing layers
    if let Some(layer) = doc.layers().get("0") {
        println!("Default layer color: {:?}", layer.color);
    }
    
    Ok(())
}
```

### Creating Complex Entities

```rust
use acadrust::{CadDocument, LwPolyline, LwVertex, Vector2, Circle, Arc};

fn main() -> acadrust::Result<()> {
    let mut doc = CadDocument::new();
    
    // Create a rectangle using LwPolyline
    let mut polyline = LwPolyline::new();
    polyline.vertices = vec![
        LwVertex { position: Vector2::new(0.0, 0.0), ..Default::default() },
        LwVertex { position: Vector2::new(100.0, 0.0), ..Default::default() },
        LwVertex { position: Vector2::new(100.0, 50.0), ..Default::default() },
        LwVertex { position: Vector2::new(0.0, 50.0), ..Default::default() },
    ];
    polyline.is_closed = true;
    doc.add_entity(polyline);
    
    // Create a circle
    let circle = Circle {
        center: Vector3::new(50.0, 25.0, 0.0),
        radius: 10.0,
        ..Default::default()
    };
    doc.add_entity(circle);
    
    Ok(())
}
```

---

## 🏗️ Architecture

acadrust uses a trait-based design for maximum flexibility and extensibility:

```
┌──────────────────────────────────────────────────────────────┐
│                       CadDocument                            │
├──────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌──────────────┐  ┌─────────────────────┐ │
│  │   Header    │  │    Tables    │  │      Entities       │ │
│  │  Variables  │  │              │  │                     │ │
│  └─────────────┘  │ - Layers     │  │ - Lines, Circles    │ │
│                   │ - LineTypes  │  │ - Polylines, Arcs   │ │
│  ┌─────────────┐  │ - Styles     │  │ - Text, Dimensions  │ │
│  │   Blocks    │  │ - DimStyles  │  │ - Hatches, Splines  │ │
│  │             │  │ - VPorts     │  │ - 3D, Mesh, Images  │ │
│  └─────────────┘  └──────────────┘  └─────────────────────┘ │
│                                                              │
│  ┌──────────────────────────────────┐  ┌──────────────────┐ │
│  │            Objects               │  │  Notifications   │ │
│  │  Dictionaries, Groups, Styles,   │  │  Warnings, Errors│ │
│  │  Layouts, XRecords, Materials    │  │  Diagnostics     │ │
│  └──────────────────────────────────┘  └──────────────────┘ │
│                                                              │
│  ┌──────────────────────────────────────────────────────────┐│
│  │                      Classes                             ││
│  │  DXF class definitions (name, app, proxy flags, count)   ││
│  └──────────────────────────────────────────────────────────┘│
└──────────────────────────────────────────────────────────────┘
```

### Core Traits

| Trait | Purpose |
|-------|---------|
| `Entity` | Base trait for all graphical entities |
| `TableEntry` | Base trait for table entries (layers, styles, etc.) |
| `CadObject` | Common interface for all CAD objects |

### Key Types

| Type | Description |
|------|-------------|
| `CadDocument` | Central document container |
| `DxfReader` | DXF file reader (ASCII and binary) |
| `DxfWriter` | DXF file writer |
| `DxfReaderConfiguration` | Reader options (failsafe mode) |
| `Handle` | Unique object identifier |
| `Vector2` / `Vector3` | 2D and 3D coordinate types |
| `Color` | CAD color (indexed or true color) |
| `LineWeight` | Line thickness enumeration |
| `Transform` | Transformation matrices |
| `NotificationCollection` | Parse diagnostics and warnings |

---

## ⚙️ Dependencies

acadrust is built on a foundation of high-quality Rust crates:

| Crate | Purpose |
|-------|---------|
| `thiserror` / `anyhow` | Error handling |
| `nom` | Parser combinators for binary parsing |
| `byteorder` | Cross-platform byte order handling |
| `flate2` | Compression/decompression |
| `nalgebra` | Linear algebra and transformations |
| `indexmap` | Ordered hash maps |
| `rayon` | Parallel iterators |
| `encoding_rs` | Character encoding support |
| `bitflags` | Type-safe bitflags |

---

## 🧪 Testing

Run the test suite:

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_read_minimal_dxf
```

Run benchmarks:

```bash
cargo bench
```

---

## ️ Roadmap

- [x] ASCII DXF read/write
- [x] Binary DXF read/write
- [x] Full entity, table, and object coverage
- [x] CLASSES section support
- [x] Character encoding / code page support
- [x] Failsafe (error-tolerant) reading mode
- [x] Unknown entity preservation
- [ ] Full DWG binary format support
- [ ] Geometric operations (offset, trim, extend)
- [ ] SVG/PDF export
- [ ] Spatial indexing for large drawings

---

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

### Development Setup

```bash
# Clone the repository
git clone https://github.com/hakanaktt/acadrust.git
cd acadrust

# Build the project
cargo build

# Run tests
cargo test

# Check formatting
cargo fmt --check

# Run clippy
cargo clippy
```

---

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

## 🙏 Acknowledgments

- [ACadSharp](https://github.com/DomCR/ACadSharp) - The C# library that inspired this project
- The Rust community for excellent tooling and libraries
- All contributors who help improve this library

---

## 📞 Support

- **Issues**: [GitHub Issues](https://github.com/hakanaktt/acadrust/issues)
- **Discussions**: [GitHub Discussions](https://github.com/hakanaktt/acadrust/discussions)

---

<p align="center">
  Made with ❤️ in Rust
</p>

