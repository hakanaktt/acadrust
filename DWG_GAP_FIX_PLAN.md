# DWG Implementation Gap Fix Plan

## acadrust vs ACadSharp — Phased Remediation

> **Total Phases:** 10  
> **Estimated entity/object implementations:** ~50  
> **Estimated test functions:** ~250+  
> Each phase is self-contained and produces a working, testable increment.

---

## Legend

- ⬜ Not started
- 🔲 In progress
- ✅ Complete

---

## Phase 0: Test Infrastructure & Shared Utilities

> **Goal:** Eliminate test code duplication, create shared helpers, establish roundtrip testing framework.  
> **Why first:** Every subsequent phase depends on solid test infrastructure.

### Tasks

- ✅ **0.1** Create `tests/common/mod.rs` shared test helper module
  - ✅ `fn sample_dwg_path(version: &str) -> PathBuf` — resolve sample file paths
  - ✅ `fn sample_dxf_path(version: &str, format: &str) -> PathBuf`
  - ✅ `fn read_sample_dwg(version: &str) -> CadDocument`
  - ✅ `fn read_sample_dxf(version: &str) -> CadDocument`
  - ✅ `ALL_VERSIONS`, `DWG_SAMPLE_VERSIONS`, `DXF_SAMPLE_VERSIONS` constants
  - ✅ `DXF_WRITABLE_VERSIONS` — all 8 writable versions
  - ✅ `fn read_dwg()`, `fn read_dwg_strict()`, `fn read_dxf()`, `fn read_dxf_strict()`
  - ✅ `fn write_and_read_back_dxf()`, `fn roundtrip_dxf()`
  - ✅ `fn entity_type_name()`, `fn entity_type_histogram()`, `fn entity_type_counts()`
  - ✅ `fn entity_count()`, `fn layer_names()`, `fn linetype_names()`, `fn textstyle_names()`, `fn block_record_names()`

- ✅ **0.2** Create `tests/common/comparison.rs` deep comparison utilities
  - ✅ `fn compare_entity_geometry(a, b) -> Vec<String>` — per-entity-type field comparison
  - ✅ `fn sorted_entities_by_type()`, `fn entity_sort_key()`
  - ✅ `fn assert_f64_eq(a: f64, b: f64, tolerance: f64)`
  - ✅ `fn assert_vec3_eq(a: &Vector3, b: &Vector3, tolerance: f64)`
  - ✅ `fn check_f64()`, `fn check_vec3()`, `fn approx_eq()`

- ✅ **0.3** Create `tests/common/builders.rs` + `tests/dwg_writer_tests.rs`
  - ✅ `fn create_all_entities_document()` — 44-entity comprehensive builder
  - ✅ `fn create_single_entity_doc(name)` — single-entity doc by type name
  - ✅ Master test file with per-phase module scaffold (14 Phase 0 tests)

- ✅ **0.4** Refactor existing tests to use shared helpers (non-breaking)
  - ✅ `dwg_reference_samples.rs` — delegates to `common::read_dwg`
  - ✅ `cad_roundtrip_output.rs` — delegates builder + helpers to common
  - ✅ `comprehensive_entity_test.rs` — delegates `entity_type_counts`, `read_back`
  - ✅ `dwg_vs_dxf_comparison.rs` — delegates all 10 helper functions to common
  - ✅ `dwg_reader_extensive_test.rs` — delegates `read_dwg`, `read_dwg_strict`, `approx_eq`, `entity_type_name`, `entity_histogram`
  - ✅ `all_entities_output_test.rs`, `individual_entity_test.rs` — `mod common` added

### Tests for Phase 0 (14 tests — all passing ✅)
```
test_common_sample_dwg_path_resolution
test_common_sample_dxf_path_resolution
test_common_read_all_sample_dwgs
test_common_read_all_sample_dxfs
test_common_create_test_document_has_defaults
test_common_roundtrip_helper_minimal
test_common_entity_count_helper
test_common_comparison_identical_entities
test_common_comparison_different_entities
test_common_f64_tolerance_pass
test_common_f64_tolerance_fail
test_common_vec3_tolerance_pass
test_common_vec3_tolerance_fail
test_common_entity_sort_key
```

---

## Phase 1: Writer — Core Missing Entities (Polylines & Vertices)

> **Goal:** Add DWG write support for POLYLINE_2D, POLYLINE_3D, POLYFACE_MESH, POLYGON_MESH, and all VERTEX types.  
> **Why:** Polylines are among the most common entities in production drawings. Without vertex/polyline writing, many DWG files lose critical geometry.  
> **Reference:** `ACadSharp-master/src/ACadSharp/IO/DWG/DwgStreamWriters/DwgObjectWriter.Entities.cs` — search for `WritePolyline`, `WriteVertex`

### Tasks

- ✅ **1.1** Implement `write_vertex_2d` in `src/io/dwg/writer/object_writer/write_entities.rs`
  - ✅ Write flags (RC), point (3BD)
  - ✅ Write start_width (BD, negative=same-as-end trick), end_width (conditional BD)
  - ✅ Write bulge (BD), tangent_direction (BD)
  - ✅ Write vertex_id (BL, R2010+ only)
  - ✅ Write owner handle (back to polyline)

- ✅ **1.2** Implement `write_vertex_3d` in `write_entities.rs`
  - ✅ Write flags (RC) + 3D point (3BD)
  - ✅ Separate implementations for `Vertex3D` and `Vertex3DPolyline`

- ✅ **1.3** Implement polyface vertex writers in `write_entities.rs`
  - ✅ `write_pface_vertex` — VertexPface (0x0D): flags + 3BD point
  - ✅ `write_pface_face` — VertexPfaceFace (0x0E): 4 × BS face indices
  - ✅ `write_vertex_mesh` — VertexMesh (0x0C): flags + 3BD point

- ✅ **1.4** Implement `write_polyline_2d` in `write_entities.rs`
  - ✅ Write flags (BS), curve_type (BS), start_width (BD), end_width (BD), thickness (BT), elevation (BD), extrusion (BE)
  - ✅ Write owned_object_count (BL, R2004+) vs first_vertex/last_vertex handles (R13-R2000)
  - ✅ Write child vertex handles + SEQEND handle

- ✅ **1.5** Implement `write_polyline_3d` in `write_entities.rs`
  - ✅ Write curve_flags (RC) + spline_flags (RC)
  - ✅ Write owned handles chain (same R2004+ / pre-R2004 pattern)

- ✅ **1.6** Implement `write_polyface_mesh` in `write_entities.rs`
  - ✅ Write vertex_count (BS), face_count (BS)
  - ✅ Write owned_object_count (R2004+) or first/last/seqend handles (R13-R2000)

- ✅ **1.7** Implement `write_polygon_mesh` in `write_entities.rs`
  - ✅ Write flags (BS), surface_type (BS), m_count (BS), n_count (BS), m_density (BS), n_density (BS)
  - ✅ Write owned handles chain

- ✅ **1.8** Register all new types in `write_entity()` match dispatcher

- ✅ **1.9** Handle entity-chain linking for owned vertices in composite writers
  - ✅ Composite writers (`write_*_composite`) write parent + child vertices + SEQEND
  - ✅ Auto-allocate handles for child objects when not assigned
  - ✅ SEQEND written after last vertex

### Tests for Phase 1
```
# Unit tests — per entity, per version
test_write_vertex_2d_r14
test_write_vertex_2d_r2000
test_write_vertex_2d_r2004
test_write_vertex_2d_r2010
test_write_vertex_2d_r2018
test_write_vertex_3d_r14
test_write_vertex_3d_r2000
test_write_vertex_3d_r2010
test_write_pface_vertex_r2000
test_write_pface_vertex_r2010

test_write_polyline_2d_simple_r14
test_write_polyline_2d_simple_r2000
test_write_polyline_2d_simple_r2004
test_write_polyline_2d_simple_r2010
test_write_polyline_2d_simple_r2018
test_write_polyline_2d_with_bulge
test_write_polyline_2d_with_widths
test_write_polyline_2d_closed
test_write_polyline_2d_many_vertices

test_write_polyline_3d_simple_r2000
test_write_polyline_3d_simple_r2010
test_write_polyline_3d_closed
test_write_polyline_3d_spline_fit

test_write_polyface_mesh_r2000
test_write_polyface_mesh_r2010
test_write_polyface_mesh_triangle
test_write_polyface_mesh_quad

test_write_polygon_mesh_r2000
test_write_polygon_mesh_r2010

# Roundtrip tests
test_roundtrip_polyline_2d_all_versions
test_roundtrip_polyline_3d_all_versions
test_roundtrip_polyface_mesh_all_versions
test_roundtrip_polygon_mesh_all_versions

# Geometry preservation
test_polyline_2d_coords_preserved
test_polyline_2d_bulge_preserved
test_polyline_2d_widths_preserved
test_polyline_3d_coords_preserved
test_polyface_mesh_face_indices_preserved

# From sample files
test_read_write_read_polylines_from_sample_ac1015
test_read_write_read_polylines_from_sample_ac1018
test_read_write_read_polylines_from_sample_ac1024
```

---

## Phase 2: Writer — Attributes (ATTRIB & ATTDEF) ✅

> **Goal:** Add DWG write support for ATTRIB and ATTDEF entities.  
> **Why:** Attributes are used in virtually every block-based drawing for title blocks, part numbers, etc. INSERT entities with attributes are broken without these.  
> **Reference:** `DwgObjectWriter.Entities.cs` — `writeAttribute`, `writeAttributeDefinition`

### Tasks

- ✅ **2.1** Implement `write_attribute` in `write_entities.rs`
  - ✅ Write common text data via shared `write_text_data` helper (R13: full fields, R2000+: data-flags optimized)
  - ✅ Write insertion_point, alignment_point, extrusion (version-dependent)
  - ✅ Write thickness (R2000+ BT optimization)
  - ✅ Write rotation, height, oblique_angle, generation_flags
  - ✅ Write horizontal/vertical justification
  - ✅ Write tag string (TV)
  - ✅ Write field_length (BS=0), flags (RC byte), lock_position (B, R2007+)
  - ✅ Write hasMText flag (B, R2010+)
  - ✅ Write style handle (hard pointer)

- ✅ **2.2** Implement `write_attribute_definition` in `write_entities.rs`
  - ✅ Reuses `write_text_data` for text body, writes default_value as text
  - ✅ Extends attribute writing with prompt string (TV) between Flags and LockPosition

- ✅ **2.3** Update INSERT writer to emit ATTRIB children
  - ✅ Write `has_attribs` flag dynamically based on attributes vec
  - ✅ Write owned_object_count (BL, R2004+) for attribute handles
  - ✅ Write attrib handles (R2004+ hard ownership list, R13-R2000 first/last soft pointers)
  - ✅ Write SEQEND handle (hard ownership)
  - ✅ Composite writer: INSERT parent → ATTRIB children → SEQEND

- ✅ **2.4** Register ATTRIB/ATTDEF in `write_entity()` dispatcher
  - ✅ `AttributeEntity` → `write_attribute`
  - ✅ `AttributeDefinition` → `write_attribute_definition`
  - ✅ `Insert` → `write_insert_composite` (auto-detects attributes)

### Additional improvements
- ✅ Fixed TEXT writer data_flags bug: was using inverted logic (bit SET = non-default), now matches reader/C# convention (bit SET = default/skip)
- ✅ Extracted `write_text_data` shared helper used by TEXT, ATTRIB, and ATTDEF writers
- ✅ Added `attributes: Vec<AttributeEntity>` field to `Insert` struct

### Tests for Phase 2
```
test_write_attrib_r14
test_write_attrib_r2000
test_write_attrib_r2004
test_write_attrib_r2010
test_write_attrib_r2018
test_write_attdef_r14
test_write_attdef_r2000
test_write_attdef_r2010
test_write_attdef_r2018

test_write_insert_with_attribs_r2000
test_write_insert_with_attribs_r2010
test_write_insert_with_multiple_attribs
test_write_insert_attrib_chain_r14       # pre-R2004 first/last handle chain
test_write_insert_attrib_chain_r2004     # R2004+ owned_object_count

test_roundtrip_attrib_all_versions
test_roundtrip_attdef_all_versions
test_roundtrip_insert_with_attribs_all_versions

test_attrib_tag_preserved
test_attrib_value_preserved
test_attrib_insertion_point_preserved
test_attdef_prompt_preserved
test_attrib_style_handle_preserved

test_read_write_read_attribs_from_sample_ac1015
test_read_write_read_attribs_from_sample_ac1018
```

---

## Phase 3: Writer — All 7 Dimension Types

> **Goal:** Add DWG write support for DimOrdinate, DimLinear, DimAligned, DimAngular3Pt, DimAngular2Line, DimRadius, DimDiameter.  
> **Why:** Dimensions are fundamental annotation entities. All 7 subtypes share a common dimension base.  
> **Reference:** `DwgObjectWriter.Entities.cs` — `writeDimension` (common) + 7 type-specific methods

### Tasks

- ✅ **3.1** Implement `write_dimension_common_data` + `write_dimension_handles` helpers in `write_entities.rs`
  - ✅ Write class version (R2010+)
  - ✅ Write normal (3BD)
  - ✅ Write text_midpoint (2RD)
  - ✅ Write elevation (BD), flags (RC)
  - ✅ Write user_text (TV)
  - ✅ Write text_rotation, horizontal_direction, ins_scale (3BD), ins_rotation
  - ✅ Write attachment_point (BS, R2000+), lspace_style/factor (R2000+)
  - ✅ Write actual_measurement (BD)
  - ✅ Write R2007+ unknown fields (B + B + B)
  - ✅ Write insertion_point (2RD)
  - ✅ Write handles: dimstyle (via dimstyle_handles map), anonymous_block

- ✅ **3.2** Implement `write_dim_ordinate` — definition_point (3BD) + feature_point (3BD) + leader_endpoint (3BD) + flags2 (RC)
- ✅ **3.3** Implement `write_dim_linear` — pt13 (3BD) + pt14 (3BD) + pt10 (3BD) + ext_rotation (BD) + rotation (BD)
- ✅ **3.4** Implement `write_dim_aligned` — pt13 (3BD) + pt14 (3BD) + pt10 (3BD) + ext_rotation (BD)
- ✅ **3.5** Implement `write_dim_angular_3pt` — pt10 (3BD) + pt13 (3BD) + pt14 (3BD) + pt15 (3BD)
- ✅ **3.6** Implement `write_dim_angular_2ln` — pt16 (2RD) + pt13 (3BD) + pt14 (3BD) + pt15 (3BD) + pt10 (3BD)
- ✅ **3.7** Implement `write_dim_radius` — pt10 (3BD) + pt15 (3BD) + leader_length (BD)
- ✅ **3.8** Implement `write_dim_diameter` — pt10 (3BD) + pt15 (3BD) + leader_length (BD)
- ✅ **3.9** Register all 7 types in `write_entity()` dispatcher (ObjectType codes 0x14–0x1A) + added `dimstyle_handles` infrastructure

### Tests for Phase 3
```
# Per dimension type × per version (7 types × 5 writable versions = 35 core tests)
test_write_dim_ordinate_r14
test_write_dim_ordinate_r2000
test_write_dim_ordinate_r2004
test_write_dim_ordinate_r2010
test_write_dim_ordinate_r2018
test_write_dim_linear_r14
test_write_dim_linear_r2000
test_write_dim_linear_r2004
test_write_dim_linear_r2010
test_write_dim_linear_r2018
test_write_dim_aligned_r14
test_write_dim_aligned_r2000
test_write_dim_aligned_r2010
test_write_dim_angular_3pt_r2000
test_write_dim_angular_3pt_r2010
test_write_dim_angular_2line_r2000
test_write_dim_angular_2line_r2010
test_write_dim_radius_r2000
test_write_dim_radius_r2010
test_write_dim_diameter_r2000
test_write_dim_diameter_r2010

# Roundtrip per type
test_roundtrip_dim_ordinate_all_versions
test_roundtrip_dim_linear_all_versions
test_roundtrip_dim_aligned_all_versions
test_roundtrip_dim_angular_3pt_all_versions
test_roundtrip_dim_angular_2line_all_versions
test_roundtrip_dim_radius_all_versions
test_roundtrip_dim_diameter_all_versions

# Geometry preservation
test_dim_linear_points_preserved
test_dim_linear_rotation_preserved
test_dim_radius_leader_length_preserved
test_dim_text_preserved
test_dim_style_handle_preserved
test_dim_anonymous_block_handle_preserved

# From sample files
test_read_write_read_dimensions_from_sample_ac1015
test_read_write_read_dimensions_from_sample_ac1018
test_read_write_read_dimensions_from_sample_ac1024
```

---

## Phase 4: Writer — HATCH Entity

> **Goal:** Add DWG write support for HATCH — the most complex single entity type.  
> **Why:** Hatches are extremely common in architectural, mechanical, and civil drawings.  
> **Reference:** `DwgObjectWriter.Entities.cs` — `writeHatch` (~300 lines in ACadSharp)

### Tasks

- ✅ **4.1** Implement `write_hatch` in `write_entities.rs`
  - ✅ Write gradient data (R2004+): is_gradient, reserved, gradient_angle, shift, single_color, tint, num_colors, color_data, gradient_name
  - ✅ Write is_solid_fill, is_associative, num_paths
  - ✅ Write boundary paths — dispatch by type:
    - ✅ **Polyline path**: has_bulge flag, is_closed, num_vertices, vertex (2RD + optional BD bulge)
    - ✅ **Non-polyline path**: num_edges, each edge by type:
      - ✅ Line edge (start, end — 2RD × 2)
      - ✅ Circular arc edge (center, radius, start_angle, end_angle, is_ccw)
      - ✅ Elliptic arc edge (center, major_endpoint, minor_ratio, start_angle, end_angle, is_ccw)
      - ✅ Spline edge (degree, rational, periodic, num_knots, num_ctrl_pts, [fit_data for R2010+])
    - ✅ Write boundary object handles (num_handles + source_boundary handles)
  - ✅ Write hatch_style (BS), pattern_type (BS)
  - ✅ Write pattern data: angle, scale, is_double, num_def_lines, each line (angle, base, offset, num_dashes, dash_lengths)
  - ✅ Write pixel_size (BD) if non-solid non-MPolygon
  - ✅ Write num_seed_points, seed_points (2RD[])

- ✅ **4.2** Register HATCH in `write_entity()` dispatcher (listed type, DwgObjectType::Hatch)

### Tests for Phase 4
```
test_write_hatch_solid_fill_r2000
test_write_hatch_solid_fill_r2004
test_write_hatch_solid_fill_r2010
test_write_hatch_pattern_fill_r2000
test_write_hatch_pattern_fill_r2010
test_write_hatch_gradient_r2004
test_write_hatch_gradient_r2010

test_write_hatch_polyline_boundary
test_write_hatch_line_edge_boundary
test_write_hatch_arc_edge_boundary
test_write_hatch_ellipse_edge_boundary
test_write_hatch_spline_edge_boundary
test_write_hatch_mixed_edge_boundary
test_write_hatch_multiple_boundaries
test_write_hatch_nested_boundaries

test_write_hatch_with_bulge
test_write_hatch_double_pattern
test_write_hatch_seed_points
test_write_hatch_associative

test_roundtrip_hatch_solid_all_versions
test_roundtrip_hatch_pattern_all_versions
test_roundtrip_hatch_gradient_all_versions

test_hatch_pattern_angle_preserved
test_hatch_pattern_scale_preserved
test_hatch_boundary_coords_preserved
test_hatch_seed_points_preserved

test_read_write_read_hatch_from_sample_ac1015
test_read_write_read_hatch_from_sample_ac1018
test_read_write_read_hatch_from_sample_ac1024
```

---

## Phase 5: Writer — MINSERT, MLINE, OLE2FRAME, 3DSOLID/REGION/BODY ✅

> **Goal:** Add DWG write support for remaining complex entities.  
> **Reference:** `DwgObjectWriter.Entities.cs`

### Tasks

- ✅ **5.1** Implement `write_minsert` in `write_entities.rs`
  - ✅ Reuse INSERT writing logic — modified `write_insert_inner()` to detect `is_array()` and use `DwgObjectType::Minsert` (code 8)
  - ✅ Add column_count, row_count, column_spacing, row_spacing (BS + BS + BD + BD) after has_atts/owned_count

- ✅ **5.2** Implement `write_mline` in `write_entities.rs`
  - ✅ Write scale, justification, base_point, extrusion
  - ✅ Write num_vertices, each vertex (3BD + direction + miter_direction)
  - ✅ Write per-vertex segments (num_elements × [num_params × BD + num_area_fills × BD])
  - ✅ Write mlinestyle handle (hard pointer)

- ✅ **5.3** Implement `write_ole2frame` in `write_entities.rs`
  - ✅ Write version (BS), data_length (BL), binary_data (RC[])

- ✅ **5.4** Implement `write_modeler_geometry` (shared) in `write_entities.rs`
  - ✅ Write acis_version (RC)
  - ✅ R2007+: Write SAB binary chunks (BL len + bytes, terminated by BL 0)
  - ✅ Pre-R2007: Write SAT text lines (TV per line, terminated by empty string)
  - ✅ Pre-R2000: Write wireframe flag (B)
  - ✅ R2007+: Write history handle (H)

- ✅ **5.5** Implement `write_solid3d`, `write_region`, `write_body` — thin wrappers calling `write_modeler_geometry`

- ✅ **5.6** Register all in `write_entity()` dispatcher — MLine, Ole2Frame, Solid3D, Region, Body match arms added

### Bonus fix
- ✅ Fixed `write_acis_data` in DXF writer (`section_writer.rs`) — split SAT data by newlines instead of raw byte chunks to prevent corrupted DXF output

### Tests for Phase 5 (48 total: 14 Phase 0 + 34 Phase 5)
```
test_write_minsert_r2000
test_write_minsert_r2010
test_write_minsert_array_3x4
test_roundtrip_minsert_all_versions
test_minsert_column_row_spacing_preserved
test_minsert_not_minsert_when_1x1
test_minsert_with_attribs

test_write_mline_r2000
test_write_mline_r2010
test_write_mline_simple_2_vertices
test_write_mline_multi_segment
test_roundtrip_mline_all_versions
test_mline_scale_preserved
test_mline_justification_preserved
test_mline_closed

test_write_ole2frame_r2000
test_write_ole2frame_r2010
test_roundtrip_ole2frame_all_versions
test_ole2frame_empty_data
test_ole2frame_large_data

test_write_3dsolid_r2000
test_write_3dsolid_r2010
test_write_3dsolid_empty
test_roundtrip_3dsolid_all_versions
test_3dsolid_sat_data_preserved
test_3dsolid_sab_data

test_write_region_r2000
test_write_region_r2010
test_roundtrip_region_all_versions

test_write_body_r2000
test_write_body_r2010
test_roundtrip_body_all_versions

test_write_all_phase5_entities_together
test_write_phase5_entities_per_version
```

---

## Phase 6: Writer — MULTILEADER & RASTER_IMAGE/WIPEOUT ✅

> **Goal:** Add DWG write support for MULTILEADER (most complex annotation) and image entities.  
> **Reference:** `DwgObjectWriter.Entities.cs` — `writeMultiLeader` (~500 lines), `writeCadImage`
> **Status:** ✅ Complete — 34 tests passing, 0 warnings

### Tasks

- ✅ **6.1** Implement `write_multileader` in `write_entities.rs`
  - ✅ Write class version (BS=2 for R2010+)
  - ✅ Write annotation context: scale_factor, content_base, text_height, text_rotation, etc.
  - ✅ Write leader roots: content_valid, leader_lines count, attachment direction
  - ✅ Write leader lines: vertices, breaks, break_start_index, break_end_index
  - ✅ Write text content: text_string, text_location, text_direction, flow_direction
  - ✅ Write block content: block_handle, block_scale, block_location, transform matrix
  - ✅ Write leader style settings: path_type, line_color, line_weight, landing_flag, etc.
  - ✅ Write arrowhead settings: arrowhead_handle, arrowhead_size
  - ✅ Write all handle references (style, text_style, line_type, arrowhead, block)

- ✅ **6.2** Implement `write_raster_image` in `write_entities.rs`
  - ✅ Write class_version (BL)
  - ✅ Write insertion_point (3BD), u_vector (3BD), v_vector (3BD)
  - ✅ Write image_size (2RD), display_flags (BS), clipping_state (B)
  - ✅ Write brightness, contrast, fade (RC × 3)
  - ✅ Write clip_boundary_type (BS), clip_vertices (rectangular / polygonal)
  - ✅ Write imagedef handle, imagedef_reactor handle
  - ✅ R2010+: clip_mode (B)

- ✅ **6.3** Implement `write_wipeout` — reuses `write_cad_image_inner()` (same binary format, different DXF class name)

- ✅ **6.4** Register in `write_entity()` dispatcher (unlisted types via `write_common_entity_data_unlisted`)
  - ✅ Added `class_numbers: HashMap<String, i16>` to DwgObjectWriter for class number resolution
  - ✅ Added `resolve_class_number()` + `write_common_entity_data_unlisted()` in common.rs
  - ✅ Auto-populate class_numbers from `default_classes()` when doc.classes is empty

### Tests for Phase 6 (34 tests)
```
test_write_multileader_text_r2000          test_write_multileader_text_r2010
test_write_multileader_text_r2018          test_write_multileader_block_r2000
test_write_multileader_block_r2010         test_roundtrip_multileader_all_versions
test_multileader_text_preserved            test_multileader_leader_points_preserved
test_multileader_multiple_leaders          test_multileader_default_values
test_multileader_override_flags            test_multileader_with_block_attributes
test_multileader_annotation_context_fields test_leader_line_point_manipulation

test_write_raster_image_r2000              test_write_raster_image_r2010
test_write_raster_image_r2018              test_raster_image_clipped_rectangular
test_raster_image_clipped_polygonal        test_roundtrip_raster_image_all_versions
test_raster_image_insertion_point_preserved test_raster_image_size_preserved
test_raster_image_brightness_contrast_fade test_raster_image_with_world_size
test_raster_image_display_flags

test_write_wipeout_r2000                   test_write_wipeout_r2010
test_write_wipeout_r2018                   test_roundtrip_wipeout_all_versions
test_wipeout_rectangular                   test_wipeout_from_corners
test_wipeout_default_values

test_phase6_all_entities_combined          test_phase6_per_version
```

---

## Phase 7: Writer — Critical Non-Graphical Objects ✅

> **Goal:** Add DWG write support for LAYOUT, PLOTSETTINGS, XRECORD, GROUP, DICTIONARYWDFLT, DICTIONARYVAR — required for AutoCAD-compatible output.  
> **Why:** Without LAYOUT and PLOTSETTINGS, DWG files may not open correctly in AutoCAD.  
> **Reference:** `DwgObjectWriter.Objects.cs`  
> **Status:** ✅ Complete — 39 tests pass, 155 total integration tests, 733 lib tests, 0 warnings

### Tasks

- ✅ **7.1** Implement `write_dictionary_with_default` in `write_objects.rs`
  - ✅ Extend dictionary writing with default_entry handle (hard pointer)
  - ✅ Uses `write_common_non_entity_data_unlisted("ACDBDICTIONARYWDFLT", ...)`

- ✅ **7.2** Implement `write_dictionary_variable` in `write_objects.rs`
  - ✅ Write schema_number (RC) + value (TV)
  - ✅ Uses `write_common_non_entity_data_unlisted("DICTIONARYVAR", ...)`

- ✅ **7.3** Implement `write_xrecord` in `write_objects.rs`
  - ✅ Write cloning_flags (BS, R2000+)
  - ✅ Serialize entries to raw LE byte buffer (RS group_code + typed value)
  - ✅ Write BL data_length + raw bytes
  - ✅ Supports all XRecordValue types: String, Double, Point3D, Int16/32/64, Byte, Bool, Handle, Chunk

- ✅ **7.4** Implement `write_plot_settings_obj` + `write_plot_settings_data` in `write_objects.rs`
  - ✅ Write page_name, printer_name, paper_size (TV)
  - ✅ Write margins (4×BD), paper size (2×BD), origin (2×BD)
  - ✅ Write paper_units, rotation, plot_type (BS)
  - ✅ Write plot_window (4×BD), scale_num/den (BD), stylesheet (TV)
  - ✅ Write scale_type, std_scale, image_origin
  - ✅ R2004+: shade_plot_mode, resolution, DPI, plot_view handle
  - ✅ R2007+: visual_style handle

- ✅ **7.5** Implement `write_layout` in `write_objects.rs`
  - ✅ Call `write_plot_settings_data` for inherited PlotSettings portion
  - ✅ Write layout_name (TV), tab_order (BL), flags (BS)
  - ✅ Write UCS origin/axes/elevation/ortho_type
  - ✅ Write limits (2RD×2), insertion_base (3BD), extents (3BD×2)
  - ✅ R2004+: viewport_count (BL) + viewport handles
  - ✅ Write handle references: block_record, viewport, base_ucs, named_ucs

- ✅ **7.6** Implement `write_group` in `write_objects.rs`
  - ✅ Write description (TV), unnamed (BS), selectable (BS)
  - ✅ Write num_handles (BL) + entity handles (hard pointer)

- ✅ **7.7** Implement `write_mline_style` in `write_objects.rs`
  - ✅ Write name, description, flags (BS), fill_color (CMC)
  - ✅ Write start/end angles (BD), num_elements (RC)
  - ✅ Per element: offset (BD), color (CMC), R2018+ linetype handle / pre-R2018 linetype index

- ✅ **7.8** Register all in object writer dispatcher (`write_nongraphical_objects`)
  - ✅ Dictionary, DictionaryWithDefault, DictionaryVariable, XRecord, PlotSettings, Layout, Group, MLineStyle

- ✅ **7.9** Infrastructure changes
  - ✅ Added `write_common_non_entity_data_unlisted` helper for unlisted types (class_number lookup)
  - ✅ Extended Layout struct with UCS fields (origin, axes, elevation, ortho_type)
  - ✅ Changed Layout.tab_order from i16 to i32 (matches DWG BL format)
  - ✅ Added Layout.plot_settings, base_ucs, named_ucs, viewport_handles fields
  - ✅ Added Layout::model() constructor
  - ✅ Fixed DXF reader/writer for tab_order type change

### Tests for Phase 7 (39 tests)
```
test_dictionary_roundtrip
test_dictionary_with_default_construction
test_dictionary_with_default_roundtrip
test_dictionary_variable_construction
test_dictionary_variable_roundtrip
test_xrecord_construction
test_xrecord_roundtrip
test_xrecord_with_all_value_types
test_xrecord_cloning_flags_roundtrip
test_xrecord_large_chunk
test_plot_settings_construction
test_plot_settings_roundtrip
test_plot_settings_flags
test_plot_settings_enums
test_plot_window_normalization
test_layout_model_construction
test_layout_paper_construction
test_layout_roundtrip
test_layout_plot_settings_integration
test_layout_defaults
test_group_construction
test_group_unnamed
test_group_roundtrip
test_group_with_many_entities
test_mlinestyle_standard
test_mlinestyle_custom
test_mlinestyle_roundtrip
test_mlinestyle_flags
test_mlinestyle_empty_elements
test_dwg_write_all_objects_smoke
test_dwg_write_objects_all_versions
test_dictionary_with_many_entries
test_empty_dictionary
test_empty_group
test_empty_xrecord
test_reference_sample_objects_present
test_reference_sample_layouts_present
test_dxf_roundtrip_preserves_mlinestyle
test_dwg_roundtrip_dictionary
```

---

## Phase 8: Writer — Remaining Non-Graphical Objects ✅

> **Goal:** Add DWG write support for all remaining non-graphical objects.  
> **Reference:** `DwgObjectWriter.Objects.cs`  
> **Status:** ✅ Complete — 43 tests pass, 198 total integration tests, 733 lib tests, 0 warnings  
> **Note:** VisualStyle, Material, TableStyle, and PdfDefinition writers are intentionally skipped — the C# reference also skips them (returns `true` from `isNotWritable`) due to incomplete/undocumented DWG binary format. PdfDefinition has no ObjectType variant.

### Tasks

- ✅ **8.1** Implement `write_image_definition` in `write_objects.rs`
  - ✅ Write class_version (BL), image_size (2RD), file_path (TV), is_loaded (B)
  - ✅ Write resolution_units (RC), pixel_size (2RD)
  - ✅ Unlisted type ("IMAGEDEF")

- ✅ **8.2** Implement `write_image_definition_reactor` in `write_objects.rs`
  - ✅ Write class_version (BL = 2) only
  - ✅ Unlisted type ("IMAGEDEF_REACTOR")

- ✅ **8.3** Implement `write_mleader_style` in `write_objects.rs`
  - ✅ R2010+ version header (BS = 2)
  - ✅ Write all 40+ style properties: content_type, draw_order, path_type, line_color, line_weight, landing/dogleg, arrowhead, text properties, block properties, scale/constraints
  - ✅ Write 4 handle references (line_type, arrowhead, text_style, block_content — all HardPointer)
  - ✅ R2010+: attachment_direction, text_bottom/top_attachment
  - ✅ R2013+: unknown flag
  - ✅ Unlisted type ("MLEADERSTYLE")

- ✅ **8.4** Implement `write_scale` in `write_objects.rs`
  - ✅ Write unknown (BS=0), name (TV), paper_units (BD), drawing_units (BD), is_unit_scale (B)
  - ✅ Unlisted type ("SCALE")

- ✅ **8.5** Implement `write_sort_entities_table` in `write_objects.rs`
  - ✅ Write block_owner_handle (SoftPointer) first
  - ✅ Write num_entries (BL), then per entry: sort_handle (raw handle in main stream) + entity_handle (SoftPointer in handle stream)
  - ✅ Unlisted type ("SORTENTSTABLE")

- ✅ **8.6** Implement `write_raster_variables` in `write_objects.rs`
  - ✅ Write class_version (BL), display_frame (BS), quality (BS), units (BS)
  - ✅ Unlisted type ("RASTERVARIABLES")

- ✅ **8.7** Implement `write_book_color` in `write_objects.rs`
  - ✅ Write color_index (BS=0 always)
  - ✅ R2004+: true_color (BL), flags (RC), color_name (TV if present), book_name (TV if present)
  - ✅ Unlisted type ("DBCOLOR")

- ⬜ **8.8** Implement `write_pdf_definition` — **Skipped** (no ObjectType variant for UnderlayDefinition)

- ⬜ **8.9** Implement `write_visual_style` — **Skipped** (C# reference also skips — undocumented DWG binary format)

- ⬜ **8.10** Implement `write_tablestyle` — **Skipped** (C# reference also skips — complex sub-structures)

- ⬜ **8.11** Implement `write_material` — **Skipped** (C# reference also skips — ~200+ fields, partial reader only)

- ✅ **8.12** Implement `write_placeholder` in `write_objects.rs`
  - ✅ Empty body (no data beyond common non-entity data)
  - ✅ Listed type (DwgObjectType::AcDbPlaceholder = 0x50)

- ✅ **8.13** Implement `write_wipeout_variables` in `write_objects.rs`
  - ✅ Write display_frame (BS)
  - ✅ Unlisted type ("WIPEOUTVARIABLES")

- ✅ **8.14** Register all 9 implementable types in object writer dispatcher
  - ✅ ImageDefinition, ImageDefinitionReactor, MultiLeaderStyle, Scale, SortEntitiesTable, RasterVariables, BookColor, PlaceHolder, WipeoutVariables

### Tests for Phase 8 (43 tests)
```
test_write_imagedef_r2000
test_write_imagedef_r2010
test_roundtrip_imagedef_all_versions
test_imagedef_file_path_preserved
test_imagedef_resolution_preserved
test_imagedef_dimensions
test_write_imagedef_reactor_r2000
test_imagedef_reactor_construction
test_write_mleaderstyle_r2010
test_write_mleaderstyle_r2018
test_roundtrip_mleaderstyle_all_versions
test_mleaderstyle_default_values
test_mleaderstyle_block_content
test_mleaderstyle_text_attachments
test_write_scale_r2010
test_roundtrip_scale_all_versions
test_scale_units_preserved
test_scale_unit_scale
test_write_sortentstable_r2000
test_write_sortentstable_r2010
test_roundtrip_sortentstable_all_versions
test_sortentstable_entries
test_sortentstable_draw_order
test_write_raster_variables_r2000
test_raster_variables_defaults
test_write_dbcolor_r2004
test_write_dbcolor_r2010
test_roundtrip_dbcolor_all_versions
test_dbcolor_empty_names
test_write_placeholder_r2000
test_placeholder_construction
test_write_wipeout_variables_r2000
test_wipeout_variables_defaults
test_dwg_write_all_phase8_objects_smoke
test_dwg_write_phase8_all_versions
test_empty_sortentstable
test_imagedef_zero_pixels
test_scale_zero_drawing_units
test_sortentstable_update_existing
test_bookcolor_with_names
test_mleaderstyle_draw_order_enums
test_resolution_unit_roundtrip
test_reference_sample_phase8_objects_present
```

---

## Phase 9: Writer — Missing Table Entries + Minor Sections ✅

> **Goal:** Add UCS table writer, VPortEntityHeader writer, fix VPort writer, and minor DWG sections (ObjFreeSpace, Template, FileDepList, RevHistory).  
> **Reference:** `DwgObjectWriter.Tables.cs`, `DwgWriter.cs`  
> **Status:** ✅ Complete — 39 tests, 237 total integration tests, 733 lib tests

### Tasks

- ✅ **9.1** Implement `write_ucs` in `tables.rs`
  - ✅ Write name (TV), xref_dependant_bit, origin (3BD), x_axis (3BD), y_axis (3BD)
  - ✅ Write elevation (BD=0) — R2000+
  - ✅ Write orthoViewType (BS=0), orthoType (BS=0) — R2000+
  - ✅ Write handles: owner (SoftPointer), baseUCS (HP=0), namedUCS (HP=0) — R2000+

- ✅ **9.2** Implement `write_vport_entity_header` in `tables.rs`
  - ✅ Write name, xref_dependant_bit, 1-flag, ctrl handle, block handle
  - ✅ Marked `#[allow(dead_code)]` — not dispatched (matches C# reference)

- ✅ **9.3** Register UCS in table writer dispatcher (`write_table_entries` in mod.rs)
  - ✅ VPortEntityHeader deliberately not dispatched (C# reference also skips it)

- ✅ **9.4** Implement `write_obj_free_space` section in writer pipeline
  - ✅ Write 53 bytes: Int32(0), UInt32(handle_count), Julian(0,0), UInt32(0), UInt8(4), 8×UInt32 magic values

- ✅ **9.5** Implement `write_template` section
  - ✅ Write 4 bytes: Int16(0) desc_length, UInt16(1) MEASUREMENT

- ✅ **9.6** Implement `write_file_dep_list` section
  - ✅ Write 8 bytes: UInt32(0) features, UInt32(0) files — R2004+ only, uncompressed, align 0x80

- ✅ **9.7** Implement `write_rev_history` section
  - ✅ Write 12 bytes: 3×UInt32(0) — R2004+ only, compressed

- ✅ **9.8** Add these sections to the writer pipeline in `dwg_writer.rs`
  - ✅ FileDepList + RevHistory in R2004+ block
  - ✅ ObjFreeSpace + Template for all versions after handles section

- ✅ **9.9** Fix `write_vport` to match C# reference (10+ corrections)
  - ✅ View mode: 4×B individual bits (was BL)
  - ✅ Aspect ratio: multiplied by view_height
  - ✅ Added fast_zoom(B=true) and UCSICON display(2×B)
  - ✅ Render mode moved to R2000+ section
  - ✅ R2007+ ambient color uses CMC, handles use SoftPointer
  - ✅ R2000+ UCS: added unknown(B=false) before UCS_per_viewport(B=true)
  - ✅ Grid/snap field order corrected

- ✅ **9.10** Add `write_xref_dependant_bit` helper
  - ✅ R2007+: writes BS(0), pre-R2007: writes B(false)+BS(0)+B(false)

### Tests for Phase 9 (39 tests)
```
# UCS writer tests (11)
test_write_ucs_r2000
test_write_ucs_r2010
test_write_ucs_r2018
test_roundtrip_ucs_all_versions
test_ucs_origin_preserved
test_ucs_axes_preserved
test_ucs_custom_axes
test_ucs_construction
test_ucs_with_elevation
test_write_multiple_ucs_entries
test_dxf_roundtrip_preserves_ucs

# VPort writer tests (7)
test_write_vport_r2000
test_write_vport_r2010
test_write_vport_r2018
test_vport_custom_settings
test_vport_active_construction
test_vport_grid_snap_settings
test_roundtrip_vport_all_versions

# Minor section tests (11)
test_obj_free_space_section_size
test_obj_free_space_handle_count
test_obj_free_space_magic_values
test_template_section_size
test_template_measurement
test_file_dep_list_section_size
test_file_dep_list_empty
test_rev_history_section_size
test_rev_history_all_zeros
test_obj_free_space_zero_handles
test_obj_free_space_large_handle_count

# Sections present tests (3)
test_dwg_write_r2004_has_minor_sections
test_dwg_write_r2010_has_minor_sections
test_dwg_write_r2018_has_minor_sections

# Combined smoke tests (2)
test_dwg_write_phase9_all_tables_smoke
test_dwg_write_phase9_all_versions

# Edge cases (4)
test_empty_ucs_table
test_ucs_zero_origin
test_vport_zero_height
test_vport_large_aspect_ratio

# Reference sample test (1)
test_reference_sample_tables_present
```

---

## Phase 10: Reader Gaps + AC1021 Compression + Final Parity

> **Goal:** Close remaining reader gaps (TABLE entity, PDF Underlay, MESH, GeoData, dynamic block params) and implement LZ77 AC21 compressor + Reed-Solomon encoder for R2007 write support.  
> **Reference:** `DwgObjectReader.cs` (unlisted types), `DwgLZ77AC21Compressor.cs` (NotImplemented in ACadSharp too)

### Tasks

#### 10A — Reader: Missing Entity Types

- ⬜ **10.1** Add `Table` entity struct to `src/entities/table.rs`
  - ⬜ Define struct fields: version, insertion_point, horizontal_direction, table_value, overrides
  - ⬜ Define cell struct: type, flags, merged_value, text_string, text_height, rotation, color, etc.
  - ⬜ Register in `EntityType` enum

- ⬜ **10.2** Implement `read_table` in object_reader entities
  - ⬜ Read class version, insertion_point, data_flags, horizontal_direction
  - ⬜ Read table_value (BL), num_rows, num_cols
  - ⬜ Read cell data per row/col

- ⬜ **10.3** Add `PdfUnderlay` / `DgnUnderlay` / `DwfUnderlay` entity structs
  - ⬜ Define struct: normal, insertion_point, scale, rotation, flags, clip_boundary
  - ⬜ Register in `EntityType` enum

- ⬜ **10.4** Implement `read_underlay` in object_reader entities
  - ⬜ Read class version, insertion_point, x/y/z_scale, rotation, flags
  - ⬜ Read contrast, fade, clip_boundary points
  - ⬜ Read definition_handle

- ⬜ **10.5** Add `Mesh` entity struct to `src/entities/mesh.rs`
  - ⬜ Define struct: version, subdivision_level, vertices, faces, edges, creases
  - ⬜ Register in `EntityType` enum

- ⬜ **10.6** Implement `read_mesh` as unlisted type reader
  - ⬜ Read version, subdivision_level, num_vertices, vertices (3BD[])
  - ⬜ Read num_faces, face_data (BL[]), num_edges, edge_data, num_creases, crease_data

- ⬜ **10.7** Add `GeoData` object struct to `src/objects/geodata.rs`
  - ⬜ Define struct: version, coordinate_type, design_point, reference_point, unit_scale, etc.

- ⬜ **10.8** Implement `read_geodata` as unlisted type reader
  - ⬜ Read version, coordinate_type, design_point (3BD), reference_point (3BD)
  - ⬜ Read horizontal_units, vertical_units, scale_estimation, sea_level

- ⬜ **10.9** Add dynamic block parameter stubs (read-only, store raw data)
  - ⬜ `EvaluationGraph` — read num_nodes, node data, has_edges, edges
  - ⬜ `BlockRotationParameter` — read fields
  - ⬜ `BlockVisibilityParameter` — read fields
  - ⬜ `BlockFlipParameter` — read fields

#### 10B — AC1021 (R2007) Write Support

- ⬜ **10.10** Implement LZ77 AC21 compressor in `src/io/dwg/compression/lz77_ac21.rs`
  - ⬜ Implement hash-table-based compression matching AC21 opcode table
  - ⬜ Handle 4 opcode cases: case 0 (long match), case 1 (short match), case 2 (2-byte offset), default (nibble)
  - ⬜ Ensure decompressor(compressor(data)) == data

- ⬜ **10.11** Implement Reed-Solomon encoder in `src/io/dwg/reed_solomon.rs`
  - ⬜ Implement byte interleaving (reverse of existing de-interleaving)
  - ⬜ Support factor=3, block_size=239 for file header
  - ⬜ Support dynamic factor/block_size for section pages

- ⬜ **10.12** Add AC21 file header writer in `src/io/dwg/writer/file_header/`
  - ⬜ Write 34 compressed metadata fields
  - ⬜ Apply Reed-Solomon encoding
  - ⬜ Write section page map with AC21 layout

- ⬜ **10.13** Enable R2007 in the writer pipeline
  - ⬜ Remove AC1021 write restriction
  - ⬜ Route to AC21 file header writer

#### 10C — Final Parity Verification

- ⬜ **10.14** Create comprehensive parity test suite
  - ⬜ Read every sample DWG → write → read → deep compare for every entity/object/table type
  - ⬜ Cross-version: read AC1018 → write as AC1024 → verify

- ⬜ **10.15** Create AutoCAD compatibility verification scripts
  - ⬜ Document manual verification steps with AutoCAD/BricsCAD

### Tests for Phase 10
```
# Reader — new entity types
test_read_table_entity_from_sample
test_read_pdf_underlay_from_sample
test_read_mesh_from_sample
test_read_geodata_from_sample
test_read_evaluation_graph_from_sample
test_read_block_rotation_param
test_read_block_visibility_param

# AC21 compression
test_lz77_ac21_compress_empty
test_lz77_ac21_compress_small_literal
test_lz77_ac21_compress_repeated_pattern
test_lz77_ac21_compress_large_random
test_lz77_ac21_roundtrip_identity        # decompress(compress(x)) == x
test_lz77_ac21_compress_matches_known_output
test_lz77_ac21_compress_all_zeros
test_lz77_ac21_compress_all_ones

# Reed-Solomon encoding
test_reed_solomon_encode_decode_identity  # decode(encode(x)) == x
test_reed_solomon_encode_factor_3_block_239
test_reed_solomon_encode_dynamic_factor

# AC21 file header writing
test_ac21_file_header_write_read_roundtrip
test_ac21_file_header_compressed_metadata

# R2007 full roundtrip
test_write_r2007_simple_document
test_write_r2007_with_entities
test_roundtrip_r2007_all_entity_types
test_read_sample_ac1021_write_ac1021_read_back

# Final parity — every version
test_full_parity_ac1014
test_full_parity_ac1015
test_full_parity_ac1018
test_full_parity_ac1021
test_full_parity_ac1024
test_full_parity_ac1027
test_full_parity_ac1032

# Cross-version
test_cross_version_ac1015_to_ac1024
test_cross_version_ac1018_to_ac1032
test_cross_version_ac1024_to_ac1015

# Entity coverage verification
test_all_entity_types_have_writer
test_all_entity_types_have_reader
test_all_object_types_have_writer
test_all_object_types_have_reader
test_all_table_types_have_writer
test_no_unknown_entities_in_roundtrip
```

---

## Summary Matrix

| Phase | Focus | New Writers | New Readers | Est. Tests | Priority |
|-------|-------|-------------|-------------|------------|----------|
| **0** | Test infrastructure | 0 | 0 | ~10 | 🔴 First |
| **1** | Polylines & vertices | 7 entities | 0 | ~45 | 🔴 Critical |
| **2** | Attributes | 2 entities | 0 | ~25 | 🔴 Critical |
| **3** | All dimensions | 7 entities | 0 | ~40 | 🔴 High |
| **4** | Hatch | 1 entity | 0 | ~30 | 🔴 High |
| **5** | MInsert/MLine/OLE/3DSolid | 5 entities | 0 | ~25 | 🟡 Medium |
| **6** | MultiLeader & Images | 3 entities | 0 | ~25 | 🟡 Medium |
| **7** | Critical objects | 7 objects | 0 | ~35 | 🔴 Critical |
| **8** | Remaining objects | 13 objects | 0 | ~25 | 🟡 Medium |
| **9** | Tables & sections | 2 tables + 4 sections | 0 | ~15 | 🟡 Medium |
| **10** | Reader gaps + AC21 write | 3 entities (write) | 6+ entities/objects | ~40 | 🟢 Final |
| **Total** | | **~50 writers** | **~6 readers** | **~315 tests** | |

### Recommended Execution Order

```
Phase 0 → Phase 7 → Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6 → Phase 8 → Phase 9 → Phase 10
```

Phase 7 (LAYOUT/PLOTSETTINGS) should come right after Phase 0 because without layouts, written DWG files may not even open in AutoCAD — making all subsequent entity testing harder.

---

## File Touchpoints Per Phase

| Phase | Files Modified | Files Created |
|-------|---------------|--------------|
| 0 | existing tests | `tests/common/mod.rs`, `tests/common/comparison.rs`, `tests/dwg_writer_tests.rs` |
| 1 | `src/io/dwg/writer/object_writer/entities.rs`, `object_writer/mod.rs` | — |
| 2 | `src/io/dwg/writer/object_writer/entities.rs`, `object_writer/mod.rs` | — |
| 3 | `src/io/dwg/writer/object_writer/entities.rs` | — |
| 4 | `src/io/dwg/writer/object_writer/entities.rs` | — |
| 5 | `src/io/dwg/writer/object_writer/entities.rs` | — |
| 6 | `src/io/dwg/writer/object_writer/entities.rs` | — |
| 7 | `src/io/dwg/writer/object_writer/objects.rs`, `object_writer/mod.rs`, `dwg_writer.rs` | — |
| 8 | `src/io/dwg/writer/object_writer/objects.rs` | — |
| 9 | `src/io/dwg/writer/object_writer/tables.rs`, `dwg_writer.rs` | — |
| 10 | reader + `compression/lz77_ac21.rs` + `reed_solomon.rs` | `src/entities/table.rs`, `src/entities/mesh.rs`, `src/objects/geodata.rs`, writer file_header |
