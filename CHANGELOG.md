## Changelog

### 0.6.3

- **DXF color interoperability** - Layers now emit the nearest ACI fallback
  alongside their exact true color.
- **DWG transparency interoperability** - Explicit transparency uses the
  standard `kByAlpha` method byte while legacy method-3 values remain readable.

### 0.6.2

- **AC1009/R12 DWG support** - Added native pre-R13 DWG read/write support,
  including fixed-layout headers, symbol tables, entity records, CRC checks,
  XDATA, compound sequences, and exhaustive entity coverage.

### 0.6.1

- **Fields and tables** - Added sheet-set, count, object-property, and table
  formula support, including numeric cell rendering and refresh-safe table
  formatting.
- **DWG/DXF fidelity** - Improved associative data, ACIS/SAB, summary info,
  table styles, legacy text, viewport metadata, spatial filters, and retained
  TrueType font flags and object references.
- **Round-trip reliability** - Preserved opaque records and extension data,
  repaired empty associative values, table-cell overrides, and legacy border
  visibility across conversions.

### 0.6.0

- **DXF color and class parsing** - Preserved unsigned `AcCmColor` values and
  accepted padded class object numbers.
- **DXF entity references** - Repaired AC1009 references, ATTRIB rotation and
  oblique-angle units, omitted DIMSTYLE defaults, and constant-time entity
  counting.
- **DXF AutoCAD compatibility** - Fixed output rejected by AutoCAD, including
  handle ownership, plot-view pointers, DATATABLE encodings, xref linetype
  localization, legacy escape handling, and raw-all geometry exclusions.
- **DXF metadata** - Preserved polyline vertex tangent angles, spline
  construction flags, hatch origin and pattern placement, current transparency,
  and standard parametric objects.
- **Import and copy behavior** - Localized nested-copy symbols, normalized
  named default layers and implicit Global materials, and exposed standard
  extension-dictionary creation.
- **DWG record fidelity** - Retained section object entities, source entity
  records, source class tables, and raw objects. Untouched records can be
  written verbatim, including records whose class table is renumbered.
- **DWG class and reference handling** - Preserved legacy class mappings,
  viewport layer overrides, duplicate layer records, ACAD_MLEADERSTYLE, and
  entity layer handles. An unresolved layer reference is now distinguishable
  from a real layer `0` reference (issue #113).
- **DWG object and visitor output** - Added `DwgReader::read_visiting`, made
  visitor commits deterministic, indexed owner-to-extension-dictionary lookup,
  and preserved object ordering by handle.
- **DWG modeler data** - Fixed external modeler blob extraction, 3DSOLID data,
  explicit spline numeric SAB roles, native constraint graphs and groups,
  standard spline constraint nodes, constrained ellipse fields, and native
  surface and region fixtures.
- **DWG header and string decoding** - Repaired lineweight and measurement
  round-trips and UTF-16 template parsing. The bit reader now decodes raw
  doubles without a heap allocation.
- **DWG AutoCAD round-trips** - Fixed hard-pointer plot-view references and
  multiple records that caused AutoCAD to reject or repair the output.
- **Spline behavior** - Clamped generated knot degrees to the control-point
  count. Splines now retain their DWG storage scenario so fit-point records
  expose when `closed` or `periodic` was not stated (issue #114).
- **MLINE behavior** - Preserved closed state and cap-suppression bits, and
  normalized MLINE flags across legacy escape handling.
- **MINSERT and colors** - Avoided oversized MINSERT reservations and color
  index overflow.
- **Block names** - De-duplicated DWG block names case-insensitively and
  numbered anonymous blocks in live block-control order.
- **Multiline attributes and MLEADER** - Repaired multiline attribute style
  handle ordering and made MLEADER explode include the full leader path,
  landing, and arrowhead.
- **Dimensions and transparency** - Fixed dimension affine transforms and
  quantized explicit transparency to match packed opacity. Unsupported current
  transparency is rejected before mutation.
- **Layer and hatch state** - Added layer descriptions and persisted the
  default hatch origin in drawing variables.
- **Materials and helixes** - Decoded procedural checker material maps and
  clarified persisted helix radius semantics.
- **Testing and tooling** - Added semantic document inventory, entity-atlas
  generation, portable AutoCAD and native-surface fixtures, raw-record
  regressions, and coverage for all repaired round-trip paths.

### 0.5.5

- **Cross-application compatibility atlas** — Added a 71-case entity atlas,
  version-aware fixtures, isolated-case validation, and generated audit matrices
  for AutoCAD and BricsCAD across AC1012 through AC1032.
- **DWG compatibility** — Improved AC1021 Reed-Solomon handling, legacy viewport
  records, class metadata, table styles, annotative context data, arc-aligned
  text, and complete database-record output.
- **ACIS and native surfaces** — Preserved solid-history edits, pcurves, NURBS
  data, SAT/SAB tokens, multi-body datastores, and lofted, revolved, swept, and
  extruded surface construction data across round trips.
- **DXF fidelity** — Corrected binary group-code widths, legacy space aliases,
  handle allocation, typed raw-record output, and audit failures in native
  surface records.
- **Stable editing and output** — Preserved opaque source data when editing
  known records and ordered written objects by handle for deterministic output.

### 0.5.4

- **DXF handle integrity (issue #64)** — Symbol-table control handles are preserved on read, so tables written by files with non-default handles no longer collide with file-sourced records on round-trip (LAYER and BLOCK_RECORD previously shared handle #1).
- **Root dictionary resolution (issue #64)** — The reader now resolves the NAMED OBJECTS DICTIONARY after reading the OBJECTS section. The writer emits the real root dictionary first, so consumers no longer audit away dictionaries, layouts, and materials as orphans.
### 0.5.3

- **DIMSTYLE text-style regression, part 2 (issue #64)** - The reader now re-points stale DIMSTYLE text-style handles at the file's same-named text style, so a ByBlock linetype handle can no longer leak into group 340.
- **MLine::close** - The CLOSED flag is set regardless of vertex count (rebuild geometry guards degenerate cases), restoring the API contract.
- **Table bounding box** - Unit test updated to the rotated-table-aware semantics: rows flow downward from the top-left insertion point.
- **DWG handle-less table entries** - Entries added without handles (e.g. Layer::new + layers.add) are assigned fresh handles on DWG save on a cloned document, so they no longer disappear from re-opened drawings and the handle map stays consistent.
### 0.5.2

- **DIMSTYLE text-style regression (issue #64)** - Fixed a 0.5.1 regression where a DXF round-trip wrote a ByBlock linetype handle as the DIMSTYLE text style (group 340): when the input file replaced the default Standard text style at a colliding handle, the surviving default DIMSTYLE kept pointing at the stale numeric handle. Stale text-style references are now re-pointed at the file's same-named text style on read.
- **Dimension fidelity** - Angular dimension quadrant measurements, radius/diameter point semantics, ordinate datum references, and dimension geometry/override consistency are preserved through round-trips.
- **ACIS enhancements** - Exact spline geometry is written, parametric pcurves decode, cone history radius semantics are defined, and solid boundary geometry is corrected.
- **Table entities** - Structural integrity is preserved and header suppression overrides are honored.
- **MText / Tolerance** - MText columns are encoded safely and preserved in DXF; geometric tolerance orientation and style identity are preserved.
- **Curves and surfaces** - Spline transformed geometry with tangents and tolerances, helix transform precision, world-space circle and arc bounds, circle thickness in bounds, and persistent center mark / centerline association metadata.
- **Hatch / Wipeout / Sketch** - Hatch boundary flags are preserved, wipeout geometry and display data survive with frames visible by default, and freehand sketch settings round-trip.
- **Layers and color books** - Layer color book identity survives DWG, DXF, and layer-state round-trips; layer rename references are preserved; the current layer state is restored on DWG save and child entity ownership is kept.
- **Transparency** - The inheritance method is preserved with a serde-friendly enum shape.
- **DWG saves** - AC1015 records are preserved, saved viewport cameras are restored, and earlier round-trip regressions are repaired.

### 0.5.1

- **Docs cleanup** - Removed the stale "IFCCAD foundation" feature bullet from the README (the feature never shipped in the published crate).

### 0.5.0

- **BricsCAD / AutoCAD 2026 compatibility** — Round-tripped DXF files now open without the recovery prompt: EED handles are decoded/encoded big-endian, `$PSTYLEMODE` is written as a boolean (290) so layer plot-style references validate, the ACAD RegApp record is emitted first, built-in materials use the minimal form, unrestorable associative-framework objects and empty map file names are dropped, dictionary-with-default records match BricsCAD's export, and `$CELWEIGHT` is sanitized to a valid lineweight.

- **DXF read/write cycle stability** — Handle-less records (R12-era files) receive handles, colliding defaults are re-handled, `DictionaryWithDefault::default_handle` follows remapped objects, MLINESTYLE angles round-trip in degrees/radians symmetrically, and NOD entry ownership matches what the writer emits. Duplicate handles and dangling references that made CAD applications discard drawings are gone.

- **DWG decoding fixes** — AC15 (R13–R2000) files whose AuxHeader sits behind the Handles section now read fully (AcDbObjects inferred from the Classes-to-Handles gap); code-page strings and MIF `\U+XXXX` escapes decode correctly, with unmappable characters written as MIF escapes.

- **AC1032 writer fixes (issue #45)** — `$ACADMAINTVER` uses group code 90, manually added layers get real non-zero handles below `$HANDSEED`, and LAYER records carry the required 390 plot-style pointer.

- **LWPOLYLINE down-save** — Plain 2D polylines are written as LWPOLYLINE for R2000+ output instead of the legacy POLYLINE/VERTEX/SEQEND form, matching what CAD applications expect (issue #63 follow-up).

- **DXF hard-owner alignment (issue #63)** — `ACAD_FIELD` is written as a hard owner (360) so applications no longer erase it; remaining NOD entries match BricsCAD's own export.

- **Community contributions** — Solid history graph management, exact ACIS spline topology, lump/shell chain traversal, block record base points, viewport shadow layer normalization, case-insensitive space records, entity extension-dictionary remapping, large mesh face stream preservation, and dimension definition point fixes.


### 0.4.1

- **MTEXT formatting** — Added a structured MTEXT format parser with richer control-code handling, including escaped semicolons, caret codes, legacy `%%u`/`%%o`/`%%nnn` text codes, line-spacing style, and relative-vs-absolute height scalars.

- **Expanded entity coverage** — Added read/write and round-trip support for `HELIX`, `ACAD_TABLE` cell content, PDF/DWF/DGN underlay references and definitions, ACAD surface entities, SPATIAL_FILTER clip boundaries, complex linetype shapes/text, and additional surface/body/history fields.

- **ACIS and 3D solid reliability** — Improved planar B-rep and NURBS spline-surface output, transformed ACIS body geometry correctly, linked R2013+ 3DSOLID/REGION/BODY geometry to AcDs SAB blobs, and fixed several AcDs record pairing/search/index layouts.

- **DWG/DXF interoperability fixes** — Tightened DWG writer conformance for AutoCAD round-trips, R2018 MLEADER/MTEXT column handling, viewport and plot settings persistence, xref block preservation, ENC color/transparency decoding, spline scenario detection, dimension angles/group codes, TEXT thickness/generation flags, and POLYLINE routing by flags.

- **Performance and security** — Removed quadratic AcDs SAB scans, made `SatDocument::record()` O(1), bounded SAB end-marker searches, and added a JSON recursion depth limit to prevent stack-overflow denial of service in glTF import.


### 0.4.0

- **Annotative styles** — `TextStyle`, `DimStyle`, and `TableStyle` now carry an `annotative` flag, persisted the standard way via `AcadAnnotative` XDATA/EED in both DXF and DWG.

- **AcDbGeoData decode** — DWG reader now decodes the `AcDbGeoData` coordinate-system definition.

- **CANNOSCALE header vars** — Read/write support for the `CANNOSCALE` and `CANNOSCALEVALUE` header variables in DXF.

- **VPORT visual style** — Render mode / visual style is persisted through both DXF and DWG; duplicate and tiled `*Active` VPORT entries are preserved instead of being collapsed.

- **Layout paper dimensions** — Paper size and plot rotation are exposed on `Layout`.

- **DWG reader robustness** — Hatch boundary-handle counts capped with `safe_count`; raster-image / wipeout clip-boundary vertices retained; 3DFACE corners 2–4 always decode Z with BD-default; invalid page offsets from gap entries no longer computed.

- **DXF reader fixes** — Improved 3D-point header parsing, null entity-handle allocation, BlockRecord initialization ordering, and configurable default encoding. Mirrored explode now produces correct arc/ellipse handedness and OCS centers.

- **DWG roundtrip** — Roundtrip workflows across supported versions with newline sanitization and improved reader alignment handling.


### 0.3.4

- **DWG roundtrip expanded** — Roundtrip workflows now cover supported DWG versions end-to-end, with additional byte-level diagnostics and compatibility fixes in the writer pipeline.

- **DXF output compatibility** — ASCII and Binary DXF roundtrip support tightened across multiple versions, including symbol name sanitization, corrected subclass marker emission, and newline-to-`\P` paragraph marker conversion in Binary DXF strings.

- **ACIS downgrade support** — ACIS SAT/SAB handling now downgrades incompatible record layouts for older consumers, improving 3DSOLID interoperability.

- **AC1021 encoding fix** — Corrected RS encoding behavior for AutoCAD 2007-class DWG files.


### 0.3.2

- **Entity explode** — `EntityType::explode()` decomposes complex entities (polylines, hatches, meshes, dimensions, etc.) into simpler primitives (lines, arcs, faces); `CadDocument::explode_entity()` allocates handles automatically

- **Centralized transform/mirror/translate** — Transformation logic extracted from 38 entity files into `translate.rs`, `transform.rs`, and `mirror.rs` modules; all Entity trait implementations delegate to these centralized functions. Direct `EntityType` dispatch methods added (`entity.translate()`, `entity.apply_transform()`, `entity.mirror_x()`, etc.) alongside the existing trait-based API.

- **DWG parser/writer fixes**

### 0.3.0

- **ACI color support** — Full 256-entry AutoCAD Color Index (ACI) to RGB lookup table, `Color::rgb()` resolves index colors, `Color::approximate_index()` finds nearest ACI match for true colors

- **Hatch edge fix** — Corrected hatch edge reading/writing issues

- **LwPolyline bulge fix** — Fixed bulge value handling in parser and writer

- **Performance optimizations** — Zero-allocation number formatting with `itoa`/`ryu`, buffered I/O, reduced memory allocations throughout DXF read/write pipeline. Parsing/writing speed are dramatically increased.

- **Table entry deduplication** — `add_or_replace` for table entries eliminates handle collisions during read

#### Breaking API change

- **`BlockRecord` entity storage** — `BlockRecord` now stores `entity_handles: Vec<Handle>` instead of owning entities directly. All entities live in flat storage inside `CadDocument` with O(1) handle-based lookup. If you accessed block entities directly, use `doc.get_entity(handle)` instead:
  ```rust
  // Before (0.2.x): iterating block entities directly
  // for entity in &block_record.entities { ... }

  // After (0.3.0): resolve handles through the document
  for &handle in &block_record.entity_handles {
      if let Some(entity) = doc.get_entity(handle) {
          // use entity
      }
  }
  ```
  The `CadDocument` public API (`add_entity()`, `entities()`, `get_entity()`, `get_entity_mut()`) is unchanged.

### 0.2.10

- **Paper space & layout support** — `add_paper_space_entity()`, `add_entity_to_layout()`, `add_layout()` API for creating viewports in multiple paper space layouts
- **Correct DXF paper space structure** — Active layout (`*Paper_Space`) entities in ENTITIES section with code 67; non-active layouts (`*Paper_Space0`, `*Paper_Space1`, …) entities inside BLOCK definitions
- **AutoCAD AUDIT compatibility** — Fixed code 67 paper space flag, MLineStyle angle conversion (radians→degrees), AcDbPlotSettings flag, viewport owner handles
- **DXF reader** — Proper handling of code 67 (paper space flag) in common entity parsing

### 0.2.9

- **ACIS 3DSOLID write support** — SAT text builder (R2000–R2007) and SAB binary (R2013+) with primitives: box, wedge, pyramid, cylinder, cone, sphere, torus
- **`SatDocument` builder API** — `add_plane_surface`, `add_cone_surface`, `add_sphere_surface`, `add_torus_surface`, `add_straight_curve`, `add_ellipse_curve`
- **208/208 DWG roundtrip integrity** — Zero field drift across 26 entity types × 8 versions

### 0.2.8

- **DWG binary read** — Full DWG reader for R13 through R2018
- **DWG binary write** — Full DWG writer for R13 through R2018
- **Handle resolution** — Automatic owner handle assignment after read

### 0.2.7

- **Optional serde support** — `Serialize`/`Deserialize` for all document types with `features = ["serde"]`
- **JSON/YAML round-trip** — Full document serialization and deserialization

### 0.2.6

- **41 entity types** — Added MultiLeader, Table, MLine, Mesh, Underlay, Ole2Frame, Wipeout, Shape, and more
- **Objects** — Dictionaries, Groups, Layouts, MLineStyle, MultiLeaderStyle, TableStyle, PlotSettings, Scale, Materials, VisualStyle, GeoData
- **CLASSES section** — Full read/write support
- **Extended data (XData)** — Full support for application-specific extended data
- **Reactors & extension dictionaries** — Read/write for all entity and object types

### 0.2.0–0.2.5

- ASCII and Binary DXF read/write
- Core entity types (Point, Line, Circle, Arc, Ellipse, Polyline, LwPolyline, Text, MText, Spline, Dimension, Hatch, Solid, Face3D, Insert, Viewport)
- Table system (Layer, LineType, TextStyle, DimStyle, BlockRecord, AppId, View, VPort, UCS)
- Encoding support (~40 code pages)
- Failsafe reading mode
- Unknown entity preservation

---
