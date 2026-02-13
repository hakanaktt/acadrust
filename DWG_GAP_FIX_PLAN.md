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

- ⬜ **0.1** Create `tests/common/mod.rs` shared test helper module
  - ⬜ `fn sample_dwg_path(version: &str) -> PathBuf` — resolve sample file paths
  - ⬜ `fn sample_dxf_path(version: &str, format: &str) -> PathBuf`
  - ⬜ `fn read_sample_dwg(version: &str) -> CadDocument`
  - ⬜ `fn read_sample_dxf(version: &str) -> CadDocument`
  - ⬜ `fn all_dwg_versions() -> Vec<&'static str>` — returns `["AC1014","AC1015","AC1018","AC1021","AC1024","AC1027","AC1032"]`
  - ⬜ `fn writable_dwg_versions() -> Vec<&'static str>` — excludes AC1021
  - ⬜ `fn create_test_document() -> CadDocument` — minimal doc with default tables
  - ⬜ `fn assert_roundtrip_dwg(doc: &CadDocument, version: ACadVersion)` — write → read → compare
  - ⬜ `fn assert_entity_roundtrip<F>(version: ACadVersion, build: F)` — create entity, write, read, compare
  - ⬜ `fn count_entities_by_type(doc: &CadDocument) -> HashMap<String, usize>`
  - ⬜ `fn find_entities_of_type<T>(doc: &CadDocument) -> Vec<&T>`

- ⬜ **0.2** Create `tests/common/comparison.rs` deep comparison utilities
  - ⬜ `fn compare_documents(a: &CadDocument, b: &CadDocument) -> ComparisonReport`
  - ⬜ `fn compare_entities(a: &Entity, b: &Entity) -> Vec<FieldDiff>`
  - ⬜ `fn assert_f64_eq(a: f64, b: f64, tolerance: f64)`
  - ⬜ `fn assert_point3d_eq(a: &Vector3, b: &Vector3, tolerance: f64)`

- ⬜ **0.3** Create `tests/dwg_writer_tests.rs` master test file for all writer tests
  - ⬜ Scaffold with per-phase test modules (`mod phase1_core_entities`, etc.)

- ⬜ **0.4** Refactor existing tests to use shared helpers (non-breaking)

### Tests for Phase 0
```
test_common_sample_path_resolution
test_common_read_all_sample_dwgs
test_common_read_all_sample_dxfs
test_common_create_test_document_has_defaults
test_common_roundtrip_helper_minimal
test_common_entity_count_helper
test_common_comparison_identical_docs
test_common_comparison_different_docs
test_common_f64_tolerance_pass
test_common_f64_tolerance_fail
```

---

## Phase 1: Writer — Core Missing Entities (Polylines & Vertices)

> **Goal:** Add DWG write support for POLYLINE_2D, POLYLINE_3D, POLYFACE_MESH, POLYGON_MESH, and all VERTEX types.  
> **Why:** Polylines are among the most common entities in production drawings. Without vertex/polyline writing, many DWG files lose critical geometry.  
> **Reference:** `ACadSharp-master/src/ACadSharp/IO/DWG/DwgStreamWriters/DwgObjectWriter.Entities.cs` — search for `WritePolyline`, `WriteVertex`

### Tasks

- ⬜ **1.1** Implement `write_vertex_2d` in `src/io/dwg/writer/object_writer/entities.rs`
  - ⬜ Write flags, point (with Z-zero-bit optimization for R2000+)
  - ⬜ Write start_width, end_width (with DD compression for R2000+)
  - ⬜ Write bulge, tangent_direction
  - ⬜ Write vertex_id (R2010+)
  - ⬜ Write owner handle (back to polyline)

- ⬜ **1.2** Implement `write_vertex_3d` in `entities.rs`
  - ⬜ Write flags + 3D point (full 3BD)

- ⬜ **1.3** Implement `write_pface_vertex` in `entities.rs`
  - ⬜ Write flags + 3D point
  - ⬜ Write 4 face indices (BS)

- ⬜ **1.4** Implement `write_polyline_2d` in `entities.rs`
  - ⬜ Write flags, curve_type, start_width, end_width, thickness, elevation, extrusion
  - ⬜ Write owned_object_count (R2004+) vs first_vertex/last_vertex/seqend handles (R13-R2000)
  - ⬜ Write child vertex handles + SEQEND handle

- ⬜ **1.5** Implement `write_polyline_3d` in `entities.rs`
  - ⬜ Write curve_type flags (spline_flags + closed_flags as 2 separate RCs for R2000+)
  - ⬜ Write owned handles chain

- ⬜ **1.6** Implement `write_polyface_mesh` in `entities.rs`
  - ⬜ Write vertex_count, face_count
  - ⬜ Write owned_object_count (R2004+) or first/last/seqend handles (R13-R2000)

- ⬜ **1.7** Implement `write_polygon_mesh` in `entities.rs`
  - ⬜ Write flags, surface_type, m_count, n_count, m_density, n_density
  - ⬜ Write owned handles chain

- ⬜ **1.8** Register all new types in `write_entity()` match dispatcher

- ⬜ **1.9** Handle entity-chain linking for owned vertices in `object_writer/mod.rs`
  - ⬜ Ensure vertices are enqueued after parent polyline
  - ⬜ Ensure SEQEND is written after last vertex

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

## Phase 2: Writer — Attributes (ATTRIB & ATTDEF)

> **Goal:** Add DWG write support for ATTRIB and ATTDEF entities.  
> **Why:** Attributes are used in virtually every block-based drawing for title blocks, part numbers, etc. INSERT entities with attributes are broken without these.  
> **Reference:** `DwgObjectWriter.Entities.cs` — `writeAttribute`, `writeAttributeDefinition`

### Tasks

- ⬜ **2.1** Implement `write_attribute` in `entities.rs`
  - ⬜ Write common text data (R13: full fields, R2000+: data-flags optimized)
  - ⬜ Write insertion_point, alignment_point, extrusion (version-dependent)
  - ⬜ Write thickness (R2000+ BT optimization)
  - ⬜ Write rotation, height, oblique_angle, generation_flags
  - ⬜ Write horizontal/vertical justification
  - ⬜ Write tag string, default value
  - ⬜ Write field_length, flags, lock_position (R2010+)
  - ⬜ Write style handle
  - ⬜ Write attribute version + MText sub-entity (R2018+)

- ⬜ **2.2** Implement `write_attribute_definition` in `entities.rs`
  - ⬜ Extends attribute writing with prompt string
  - ⬜ Write R2018 multi-line support

- ⬜ **2.3** Update INSERT writer to emit ATTRIB children
  - ⬜ Write `has_attribs` flag correctly
  - ⬜ Write owned_object_count (R2004+) for attribute handles
  - ⬜ Write attrib handles + SEQEND handle
  - ⬜ Enqueue ATTRIB entities after INSERT

- ⬜ **2.4** Register ATTRIB/ATTDEF in `write_entity()` dispatcher

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

- ⬜ **3.1** Implement `write_dimension_common` helper in `entities.rs`
  - ⬜ Write class version (R2010+)
  - ⬜ Write extrusion (BE for R2000+)
  - ⬜ Write text_midpoint (2RD)
  - ⬜ Write elevation (BD), flags (RC)
  - ⬜ Write user_text (TV)
  - ⬜ Write text_rotation, horizontal_direction, ins_scale (3BD), ins_rotation
  - ⬜ Write attachment_point (BS, R2000+), lspace_style/factor (R2000+)
  - ⬜ Write actual_measurement (BD)
  - ⬜ Write R2007+ unknown fields (B + B)
  - ⬜ Write flip_arrow1, flip_arrow2 (R2000+)
  - ⬜ Write 12-pt (2RD)
  - ⬜ Write handles: dimstyle, anonymous_block, R2007+ text_style

- ⬜ **3.2** Implement `write_dim_ordinate` — definition_point (3BD) + feature_point + leader_endpoint + flags2 (RC for R2000+)
- ⬜ **3.3** Implement `write_dim_linear` — 4 points (3BD×2 + 2BD + 3BD) + ext_line_rotation + dim_rotation
- ⬜ **3.4** Implement `write_dim_aligned` — 4 points (3BD×2 + 2BD + 3BD) + ext_line_rotation
- ⬜ **3.5** Implement `write_dim_angular_3pt` — definition_point + first_arc + second_arc + 2BD
- ⬜ **3.6** Implement `write_dim_angular_2line` — 4 points (2BD×2 + 3BD×2) + 2BD
- ⬜ **3.7** Implement `write_dim_radius` — definition_point + first_arc + leader_length
- ⬜ **3.8** Implement `write_dim_diameter` — definition_point + first_arc + leader_length
- ⬜ **3.9** Register all 7 types in `write_entity()` dispatcher (ObjectType codes 20–26)

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

- ⬜ **4.1** Implement `write_hatch` in `entities.rs`
  - ⬜ Write gradient data (R2004+): is_gradient, reserved, gradient_angle, shift, single_color, tint, num_colors, color_data, gradient_name
  - ⬜ Write is_solid_fill, is_associative, num_paths
  - ⬜ Write boundary paths — dispatch by type:
    - ⬜ **Polyline path**: has_bulge flag, is_closed, num_vertices, vertex (2RD + optional BD bulge)
    - ⬜ **Non-polyline path**: num_edges, each edge by type:
      - ⬜ Line edge (start, end — 2RD × 2)
      - ⬜ Circular arc edge (center, radius, start_angle, end_angle, is_ccw)
      - ⬜ Elliptic arc edge (center, major_endpoint, minor_ratio, start_angle, end_angle, is_ccw)
      - ⬜ Spline edge (degree, rational, periodic, num_knots, num_ctrl_pts, [fit_data for R2010+])
    - ⬜ Write boundary object handles (num_handles + source_boundary handles)
  - ⬜ Write hatch_style (BS), pattern_type (BS)
  - ⬜ Write pattern data: angle, scale, is_double, num_def_lines, each line (angle, base, offset, num_dashes, dash_lengths)
  - ⬜ Write pixel_size (BD) if non-solid non-MPolygon
  - ⬜ Write num_seed_points, seed_points (2RD[])

- ⬜ **4.2** Register HATCH in `write_entity()` dispatcher (unlisted type, match by DXF name)

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

## Phase 5: Writer — MINSERT, MLINE, OLE2FRAME, 3DSOLID/REGION/BODY

> **Goal:** Add DWG write support for remaining complex entities.  
> **Reference:** `DwgObjectWriter.Entities.cs`

### Tasks

- ⬜ **5.1** Implement `write_minsert` in `entities.rs`
  - ⬜ Reuse INSERT writing logic
  - ⬜ Add column_count, row_count, column_spacing, row_spacing (BS + BS + BD + BD)

- ⬜ **5.2** Implement `write_mline` in `entities.rs`
  - ⬜ Write scale, justification, base_point, extrusion
  - ⬜ Write num_vertices, each vertex (3BD + direction + miter_direction)
  - ⬜ Write per-vertex segments (num_elements × [num_params × BD + num_area_fills × BD])
  - ⬜ Write mlinestyle handle

- ⬜ **5.3** Implement `write_ole2frame` in `entities.rs`
  - ⬜ Write binary data blob (BS flags + BS mode + RL data_size + RC[] data)

- ⬜ **5.4** Implement `write_3dsolid` in `entities.rs`
  - ⬜ Write ACIS SAT data: version (RC), num_sat_records, total_size
  - ⬜ Write SAT text records (each: BS + RC + RL + RC[])
  - ⬜ Write end markers, isolines, has_history (R2007+), has_wireframe

- ⬜ **5.5** Implement `write_region` and `write_body` — reuse 3DSOLID logic (shared ACIS format)

- ⬜ **5.6** Register all in `write_entity()` dispatcher

### Tests for Phase 5
```
test_write_minsert_r2000
test_write_minsert_r2010
test_write_minsert_array_3x4
test_roundtrip_minsert_all_versions
test_minsert_column_row_spacing_preserved

test_write_mline_r2000
test_write_mline_r2010
test_write_mline_simple_2_vertices
test_write_mline_multi_segment
test_roundtrip_mline_all_versions
test_mline_scale_preserved
test_mline_justification_preserved

test_write_ole2frame_r2000
test_write_ole2frame_r2010
test_roundtrip_ole2frame_all_versions

test_write_3dsolid_r2000
test_write_3dsolid_r2010
test_write_region_r2000
test_write_body_r2000
test_roundtrip_3dsolid_all_versions
test_roundtrip_region_all_versions
test_3dsolid_sat_data_preserved

test_read_write_read_mlines_from_sample
test_read_write_read_solids_from_sample
```

---

## Phase 6: Writer — MULTILEADER & RASTER_IMAGE/WIPEOUT

> **Goal:** Add DWG write support for MULTILEADER (most complex annotation) and image entities.  
> **Reference:** `DwgObjectWriter.Entities.cs` — `writeMultiLeader` (~500 lines), `writeCadImage`

### Tasks

- ⬜ **6.1** Implement `write_multileader` in `entities.rs`
  - ⬜ Write class version (RS)
  - ⬜ Write annotation context: scale_factor, content_base, text_height, text_rotation, etc.
  - ⬜ Write leader roots: is_valid, leader_lines count, attachment direction
  - ⬜ Write leader lines: vertices, breaks, break_start_index, break_end_index
  - ⬜ Write text content: default_text, text_location, text_direction, flow_direction
  - ⬜ Write block content: block_reference, block_scale, block_position
  - ⬜ Write leader style settings: leader_type, line_color, line_weight, landing_flag, etc.
  - ⬜ Write arrowhead settings: arrowhead_id, arrowhead_size
  - ⬜ Write all handle references

- ⬜ **6.2** Implement `write_raster_image` in `entities.rs`
  - ⬜ Write class_version (BL)
  - ⬜ Write insertion_point (3BD), u_vector (3BD), v_vector (3BD)
  - ⬜ Write image_size (2RD), display_flags (BS), clipping_state (B)
  - ⬜ Write brightness, contrast, fade (RC × 3)
  - ⬜ Write clip_boundary_type (BS), clip_vertices
  - ⬜ Write imagedef handle, imagedef_reactor handle

- ⬜ **6.3** Implement `write_wipeout` — reuse raster_image logic (same binary format, different class)

- ⬜ **6.4** Register in `write_entity()` dispatcher (unlisted types, match by DXF name)

### Tests for Phase 6
```
test_write_multileader_text_r2010
test_write_multileader_text_r2013
test_write_multileader_text_r2018
test_write_multileader_block_r2010
test_write_multileader_multiple_leaders
test_write_multileader_no_landing
test_roundtrip_multileader_all_versions
test_multileader_text_preserved
test_multileader_leader_points_preserved

test_write_raster_image_r2000
test_write_raster_image_r2010
test_write_raster_image_clipped
test_write_raster_image_rectangular_clip
test_roundtrip_raster_image_all_versions

test_write_wipeout_r2000
test_write_wipeout_r2010
test_roundtrip_wipeout_all_versions

test_image_insertion_point_preserved
test_image_size_preserved
test_image_clip_boundary_preserved
```

---

## Phase 7: Writer — Critical Non-Graphical Objects

> **Goal:** Add DWG write support for LAYOUT, PLOTSETTINGS, XRECORD, GROUP, DICTIONARYWDFLT, DICTIONARYVAR — required for AutoCAD-compatible output.  
> **Why:** Without LAYOUT and PLOTSETTINGS, DWG files may not open correctly in AutoCAD.  
> **Reference:** `DwgObjectWriter.Objects.cs`

### Tasks

- ⬜ **7.1** Implement `write_dictionary_with_default` in `objects.rs`
  - ⬜ Extend dictionary writing with default_entry handle (hard ownership)

- ⬜ **7.2** Implement `write_dictionary_var` in `objects.rs`
  - ⬜ Write int_value (RC) + string_value (TV)

- ⬜ **7.3** Implement `write_xrecord` in `objects.rs`
  - ⬜ Write cloning_flags (BS, R2000+)
  - ⬜ Write data entries — dispatch by group code type:
    - ⬜ String entries (TV)
    - ⬜ Double/Point entries (RD, 3RD)
    - ⬜ Integer entries (RC, RS, RL, RLL)
    - ⬜ Handle entries (H)
    - ⬜ Binary chunk entries (RC[])
  - ⬜ Write num_obj_id_handles + object_id handles

- ⬜ **7.4** Implement `write_plotsettings` in `objects.rs`
  - ⬜ Write page_setup_name, printer_name (TV)
  - ⬜ Write paper_size, plot_origin, margins (4 × BD)
  - ⬜ Write paper_width, paper_height (BD × 2)
  - ⬜ Write plot_type, plot_rotation (BS × 2)
  - ⬜ Write plot_window_area (4 × BD)
  - ⬜ Write scale_numerator, scale_denominator (BD × 2)
  - ⬜ Write style_sheet (TV)
  - ⬜ Write shade_plot_mode (BS), shade_plot_res_level (BS), shade_plot_custom_dpi (BS) — R2004+
  - ⬜ Write plot_view_name (TV)

- ⬜ **7.5** Implement `write_layout` in `objects.rs`
  - ⬜ Write plotsettings base data (call write_plotsettings internals)
  - ⬜ Write layout_name (TV)
  - ⬜ Write tab_order (BL), control_flags (BS)
  - ⬜ Write insertion_base (3BD), extents min/max (3BD × 2)
  - ⬜ Write ucs_origin (3BD), ucs_x_axis (3BD), ucs_y_axis (3BD)
  - ⬜ Write elevation (BD), ucs_ortho_type (BS)
  - ⬜ Write limits_min, limits_max (2RD × 2) — R2004+
  - ⬜ Write handle references: block_record, active_vport, ucs, named_ucs, viewport list

- ⬜ **7.6** Implement `write_group` in `objects.rs`
  - ⬜ Write description (TV), unnamed (BS), selectable (BS)
  - ⬜ Write num_handles + entity handles (hard pointer)

- ⬜ **7.7** Implement `write_mlinestyle` in `objects.rs`
  - ⬜ Write name, description, flags, fill_color, start_angle, end_angle
  - ⬜ Write num_elements, each element (offset, color, linetype handle)

- ⬜ **7.8** Register all in object writer dispatcher

- ⬜ **7.9** Update document writing pipeline to emit layouts
  - ⬜ Ensure Model_Space layout and at least one Paper_Space layout are written
  - ⬜ Link layout → block_record handle correctly

### Tests for Phase 7
```
test_write_dictionary_with_default_r2000
test_write_dictionary_with_default_r2010
test_write_dictionary_var_r2000
test_write_dictionary_var_r2010

test_write_xrecord_string_entries
test_write_xrecord_numeric_entries
test_write_xrecord_handle_entries
test_write_xrecord_binary_chunk
test_write_xrecord_mixed_entries
test_roundtrip_xrecord_all_versions

test_write_plotsettings_r2000
test_write_plotsettings_r2004
test_write_plotsettings_r2010
test_plotsettings_margins_preserved
test_plotsettings_scale_preserved

test_write_layout_model_space_r2000
test_write_layout_model_space_r2010
test_write_layout_paper_space_r2000
test_write_layout_paper_space_r2010
test_write_layout_with_ucs
test_roundtrip_layout_all_versions
test_layout_tab_order_preserved
test_layout_extents_preserved
test_layout_block_record_link_valid

test_write_group_r2000
test_write_group_r2010
test_write_group_multiple_entities
test_roundtrip_group_all_versions

test_write_mlinestyle_r2000
test_write_mlinestyle_r2010
test_write_mlinestyle_multi_element
test_roundtrip_mlinestyle_all_versions

test_document_has_model_layout_after_write
test_document_has_paper_layout_after_write
test_document_layouts_link_to_block_records
```

---

## Phase 8: Writer — Remaining Non-Graphical Objects

> **Goal:** Add DWG write support for all remaining non-graphical objects.  
> **Reference:** `DwgObjectWriter.Objects.cs`

### Tasks

- ⬜ **8.1** Implement `write_imagedef` in `objects.rs`
  - ⬜ Write class_version (BL), file_path (TV), is_loaded (B)
  - ⬜ Write resolution_units (RC), pixel_width (RD), pixel_height (RD)

- ⬜ **8.2** Implement `write_imagedef_reactor` in `objects.rs`
  - ⬜ Write class_version (BL) only

- ⬜ **8.3** Implement `write_mleaderstyle` in `objects.rs`
  - ⬜ Write all style properties: content_type, draw_order, leader_type, line_color, etc.
  - ⬜ Write text style, block content, scale, attachments
  - ⬜ Write handle references

- ⬜ **8.4** Implement `write_scale` in `objects.rs`
  - ⬜ Write name (TV), paper_units (BD), drawing_units (BD), is_unit_scale (B)

- ⬜ **8.5** Implement `write_sortentstable` in `objects.rs`
  - ⬜ Write parent_handle, num_entries, sort_handle/entity_handle pairs

- ⬜ **8.6** Implement `write_raster_variables` in `objects.rs`
  - ⬜ Write class_version, display_frame, quality, units

- ⬜ **8.7** Implement `write_dbcolor` in `objects.rs`
  - ⬜ Write RGB + book name + color name (R2004+)

- ⬜ **8.8** Implement `write_pdf_definition` in `objects.rs`
  - ⬜ Write file_path (TV) + page_number (TV)

- ⬜ **8.9** Implement `write_visual_style` in `objects.rs`
  - ⬜ Write all visual style properties (flags, colors, edge settings)

- ⬜ **8.10** Implement `write_tablestyle` in `objects.rs`
  - ⬜ Write all table style cell settings

- ⬜ **8.11** Implement `write_material` in `objects.rs`
  - ⬜ Write material properties

- ⬜ **8.12** Implement `write_placeholder` in `objects.rs`
  - ⬜ Write empty body (no data beyond common)

- ⬜ **8.13** Implement `write_wipeout_variables` in `objects.rs`
  - ⬜ Write display_frame (BS)

- ⬜ **8.14** Register all in object writer dispatcher

### Tests for Phase 8
```
test_write_imagedef_r2000
test_write_imagedef_r2010
test_write_imagedef_reactor_r2000
test_roundtrip_imagedef_all_versions
test_imagedef_file_path_preserved
test_imagedef_resolution_preserved

test_write_mleaderstyle_r2010
test_write_mleaderstyle_r2018
test_roundtrip_mleaderstyle_all_versions

test_write_scale_r2010
test_roundtrip_scale_all_versions
test_scale_units_preserved

test_write_sortentstable_r2000
test_write_sortentstable_r2010
test_roundtrip_sortentstable_all_versions

test_write_raster_variables_r2000
test_write_dbcolor_r2004
test_write_dbcolor_r2010
test_write_pdf_definition_r2010
test_write_visual_style_r2010
test_write_tablestyle_r2010
test_write_material_r2010
test_write_placeholder_r2000
test_write_wipeout_variables_r2000

test_roundtrip_dbcolor_all_versions
test_roundtrip_pdf_definition_all_versions
test_roundtrip_visual_style_all_versions
```

---

## Phase 9: Writer — Missing Table Entries + Minor Sections

> **Goal:** Add UCS table writer, VPortEntityHeader writer, and minor DWG sections (ObjFreeSpace, Template, FileDepList, RevHistory).  
> **Reference:** `DwgObjectWriter.Tables.cs`, `DwgWriter.cs`

### Tasks

- ⬜ **9.1** Implement `write_ucs` in `tables.rs`
  - ⬜ Write origin (3BD), x_direction (3BD), y_direction (3BD)
  - ⬜ Write elevation (BD) — R2000+
  - ⬜ Write orthographic_type (BS) — R2000+
  - ⬜ Write orthographic_origin per type (6 × [BS + 3BD]) — R2000+

- ⬜ **9.2** Implement `write_vport_entity_header` in `tables.rs`
  - ⬜ Write flags, entity handles

- ⬜ **9.3** Register UCS and VPortEntityHeader in table writer dispatcher

- ⬜ **9.4** Implement `write_obj_free_space` section in writer pipeline
  - ⬜ Write object count, date timestamps, offsets

- ⬜ **9.5** Implement `write_template` section
  - ⬜ Write template description + MEASUREMENT variable

- ⬜ **9.6** Implement `write_file_dep_list` section
  - ⬜ Write file dependency records (XRefs, images, fonts)

- ⬜ **9.7** Implement `write_rev_history` section
  - ⬜ Write revision history (3 × BL zeros)

- ⬜ **9.8** Add these sections to the writer pipeline in `dwg_writer.rs`
  - ⬜ Emit in correct order between objects and handles sections

### Tests for Phase 9
```
test_write_ucs_r2000
test_write_ucs_r2010
test_write_ucs_with_elevation
test_write_ucs_orthographic_types
test_roundtrip_ucs_all_versions
test_ucs_origin_preserved
test_ucs_axes_preserved

test_write_vport_entity_header_r2000
test_roundtrip_vport_entity_header

test_write_obj_free_space_section_r2004
test_write_obj_free_space_section_r2010
test_write_template_section_r2004
test_write_file_dep_list_section_r2004
test_write_rev_history_section_r2004

test_all_sections_present_in_written_dwg_r2004
test_all_sections_present_in_written_dwg_r2010
test_all_sections_present_in_written_dwg_r2018
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
