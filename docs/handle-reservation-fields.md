# Reservation field manifest

The census classifies each type once. Exhaustive matches and destructures live
with the census visitors, including nested scalar shapes. Types with private
fields are classified by their owning helper or, for private flag bits, by a
single local shape check.

I = assigned identity; P = protected reference, including dangling; S = typed scalar/geometry; X = refuse active backing. Root/private-map protocols are described in [the contract](handle-reservation.md). A newly added field or variant must be classified and tested before positive coverage is extended.

| Struct | Owning module | Declared handle slots |
| --- | --- | --- |
| AppId | `src/tables/appid.rs` | `handle` I |
| Arc | `src/entities/arc.rs` | No direct handle slots; nested payloads are classified separately |
| AssocDimensionAssociation | `src/objects/associative.rs` | `dimension` P |
| AssocDimensionReference | `src/objects/associative.rs` | `xrefs` P, `intersection_objects` P |
| AssociativeObject | `src/objects/associative.rs` | `handle` I, `owner` P, `reactors` P, `xdictionary_handle` P |
| AttributeDefinition | `src/entities/attribute_definition.rs` | No direct handle slots; nested payloads are classified separately |
| AttributeEntity | `src/entities/attribute_entity.rs` | `attdef_handle` P |
| AttributeFlags | `src/entities/attribute_definition.rs` | No direct handle slots; nested payloads are classified separately |
| Block | `src/entities/block.rs` | No direct handle slots; nested payloads are classified separately |
| BlockEnd | `src/entities/block.rs` | No direct handle slots; nested payloads are classified separately |
| BlockFlags | `src/tables/block_record.rs` | No direct handle slots; nested payloads are classified separately |
| BlockRecord | `src/tables/block_record.rs` | `handle` I, `block_entity_handle` I/A, `block_end_handle` I/A, `layout` P, `entity_handles` P, `insert_handles` P |
| BoundaryPath | `src/entities/hatch.rs` | `boundary_handles` P |
| BoundaryPathFlags | `src/entities/hatch.rs` | No direct handle slots; nested payloads are classified separately |
| Circle | `src/entities/circle.rs` | No direct handle slots; nested payloads are classified separately |
| CircularArcEdge | `src/entities/hatch.rs` | No direct handle slots; nested payloads are classified separately |
| Dictionary | `src/objects/mod.rs` | `handle` I, `owner` P, `entries` P, `reactors` P, `xdictionary_handle` P |
| DictionaryVariable | `src/objects/dictionary_variable.rs` | `handle` I, `owner_handle` P |
| DictionaryWithDefault | `src/objects/stub_objects.rs` | `handle` I, `owner` P, `entries` P, `default_handle` P |
| DimContext | `src/objects/object_context_data.rs` | `block` P |
| DimStyle | `src/tables/dimstyle.rs` | `handle` I, `xref_handle` P, `dimblk` P, `dimblk1` P, `dimblk2` P, `dimldrblk` P, `dimtxsty_handle` P, `dimltex_handle` P, `dimltex1_handle` P, `dimltex2_handle` P |
| DimensionAligned | `src/entities/dimension.rs` | No direct handle slots; nested payloads are classified separately |
| DimensionAngular2Ln | `src/entities/dimension.rs` | No direct handle slots; nested payloads are classified separately |
| DimensionAngular3Pt | `src/entities/dimension.rs` | No direct handle slots; nested payloads are classified separately |
| DimensionArc | `src/entities/dimension.rs` | No direct handle slots; nested payloads are classified separately |
| DimensionBase | `src/entities/dimension.rs` | No direct handle slots; nested payloads are classified separately |
| DimensionDiameter | `src/entities/dimension.rs` | No direct handle slots; nested payloads are classified separately |
| DimensionLargeRadial | `src/entities/dimension.rs` | No direct handle slots; nested payloads are classified separately |
| DimensionLinear | `src/entities/dimension.rs` | No direct handle slots; nested payloads are classified separately |
| DimensionOrdinate | `src/entities/dimension.rs` | No direct handle slots; nested payloads are classified separately |
| DimensionRadius | `src/entities/dimension.rs` | No direct handle slots; nested payloads are classified separately |
| Ellipse | `src/entities/ellipse.rs` | No direct handle slots; nested payloads are classified separately |
| EllipticArcEdge | `src/entities/hatch.rs` | No direct handle slots; nested payloads are classified separately |
| EntityCommon | `src/entities/mod.rs` | `handle` I, `layer_handle` P, `linetype_handle` P, `reactors` P, `xdictionary_handle` P, `owner_handle` P, `color_book_handle` P, `full_visual_style_handle` P, `face_visual_style_handle` P, `edge_visual_style_handle` P, `material_handle` P, `plotstyle_handle` P |
| ExtendedData | `src/xdata/mod.rs` | No direct handle slots; nested payloads are classified separately |
| ExtendedDataRecord | `src/xdata/mod.rs` | No direct handle slots; nested payloads are classified separately |
| GradientColorEntry | `src/entities/hatch.rs` | No direct handle slots; nested payloads are classified separately |
| GridFlags | `src/entities/viewport.rs` | No direct handle slots; nested payloads are classified separately |
| Hatch | `src/entities/hatch.rs` | No direct handle slots; nested payloads are classified separately |
| HatchGradientPattern | `src/entities/hatch.rs` | No direct handle slots; nested payloads are classified separately |
| HatchLoopContext | `src/objects/object_context_data.rs` | No direct handle slots; nested payloads are classified separately |
| HatchPattern | `src/entities/hatch.rs` | No direct handle slots; nested payloads are classified separately |
| HatchPatternLine | `src/entities/hatch.rs` | No direct handle slots; nested payloads are classified separately |
| HatchScaleContext | `src/objects/object_context_data.rs` | No direct handle slots; nested payloads are classified separately |
| HatchViewContext | `src/objects/object_context_data.rs` | `view` P |
| Insert | `src/entities/insert.rs` | `view_rep_handle` P, `seqend_handle` P |
| Layer | `src/tables/layer.rs` | `handle` I, `material` P, `plotstyle_handle` P, `xref_block_record_handle` P |
| LayerFlags | `src/tables/layer.rs` | No direct handle slots; nested payloads are classified separately |
| Layout | `src/objects/mod.rs` | `handle` I, `owner` P, `block_record` P, `viewport` P, `viewports` P, `base_ucs` P, `named_ucs` P, `reactors` P, `xdictionary_handle` P, `plot_view_handle` P, `visual_style_handle` P |
| Line | `src/entities/line.rs` | No direct handle slots; nested payloads are classified separately |
| LineEdge | `src/entities/hatch.rs` | No direct handle slots; nested payloads are classified separately |
| LineType | `src/tables/linetype.rs` | `handle` I, `xref_block_record_handle` P |
| LineTypeComplexData | `src/tables/linetype.rs` | `style_handle` P |
| LineTypeElement | `src/tables/linetype.rs` | No direct handle slots; nested payloads are classified separately |
| LwPolyline | `src/entities/lwpolyline.rs` | No direct handle slots; nested payloads are classified separately |
| LwVertex | `src/entities/lwpolyline.rs` | No direct handle slots; nested payloads are classified separately |
| MLine | `src/entities/mline.rs` | `style_handle` P |
| MLineSegment | `src/entities/mline.rs` | No direct handle slots; nested payloads are classified separately |
| MLineStyle | `src/objects/mlinestyle.rs` | `handle` I, `owner` P |
| MLineStyleElement | `src/objects/mlinestyle.rs` | No direct handle slots; nested payloads are classified separately |
| MLineStyleFlags | `src/objects/mlinestyle.rs` | No direct handle slots; nested payloads are classified separately |
| MLineVertex | `src/entities/mline.rs` | No direct handle slots; nested payloads are classified separately |
| MText | `src/entities/mtext.rs` | No direct handle slots; nested payloads are classified separately |
| MTextColumnData | `src/entities/mtext.rs` | No direct handle slots; nested payloads are classified separately |
| MTextColumns | `src/objects/object_context_data.rs` | No direct handle slots; nested payloads are classified separately |
| MTextContext | `src/objects/object_context_data.rs` | No direct handle slots; nested payloads are classified separately |
| MultiLeaderStyle | `src/objects/multileader_style.rs` | `handle` I, `owner_handle` P, `line_type_handle` P, `arrowhead_handle` P, `text_style_handle` P, `block_content_handle` P |
| NamedTableCellStyle | `src/objects/table_style.rs` | No direct handle slots; nested payloads are classified separately |
| ObjectContextData | `src/objects/object_context_data.rs` | `handle` I, `owner_handle` P, `reactors` P, `xdictionary_handle` P, `scale` P |
| PaperMargin | `src/objects/plot_settings.rs` | No direct handle slots; nested payloads are classified separately |
| PlaceHolder | `src/objects/stub_objects.rs` | `handle` I, `owner` P |
| PlotFlags | `src/objects/plot_settings.rs` | No direct handle slots; nested payloads are classified separately |
| PlotSettings | `src/objects/plot_settings.rs` | `handle` I, `owner` P, `reactors` P, `xdictionary_handle` P, `plot_view_handle` P, `visual_style_handle` P |
| PlotWindow | `src/objects/plot_settings.rs` | No direct handle slots; nested payloads are classified separately |
| Point | `src/entities/point.rs` | No direct handle slots; nested payloads are classified separately |
| Polyline | `src/entities/polyline.rs` | No direct handle slots; nested payloads are classified separately |
| Polyline2D | `src/entities/polyline.rs` | No direct handle slots; nested payloads are classified separately |
| Polyline3D | `src/entities/polyline3d.rs` | No direct handle slots; nested payloads are classified separately |
| Polyline3DFlags | `src/entities/polyline3d.rs` | No direct handle slots; nested payloads are classified separately |
| PolylineEdge | `src/entities/hatch.rs` | No direct handle slots; nested payloads are classified separately |
| PolylineFlags | `src/entities/polyline.rs` | No direct handle slots; nested payloads are classified separately |
| ProxyObjectReference | `src/objects/semantic_property.rs` | `handle` I |
| R12EntityMetadata | `src/entities/mod.rs` | No direct handle slots; nested payloads are classified separately |
| RowCellStyle | `src/objects/table_style.rs` | `text_style_handle` P |
| Scale | `src/objects/scale.rs` | `handle` I, `owner_handle` P |
| Seqend | `src/entities/seqend.rs` | No direct handle slots; nested payloads are classified separately |
| Solid | `src/entities/solid.rs` | No direct handle slots; nested payloads are classified separately |
| SortEntitiesTable | `src/objects/sort_entities_table.rs` | `handle` I, `owner_handle` P, `block_owner_handle` P |
| SortEntsEntry | `src/objects/sort_entities_table.rs` | `entity_handle` P, `sort_handle` S (rank) |
| SplineEdge | `src/entities/hatch.rs` | No direct handle slots; nested payloads are classified separately |
| TableCellBorder | `src/objects/table_style.rs` | No direct handle slots; nested payloads are classified separately |
| TableCellStyleData | `src/objects/table_style.rs` | No direct handle slots; nested payloads are classified separately |
| TableContentFormat | `src/objects/table_style.rs` | `text_style` P |
| TableGridFormat | `src/objects/table_style.rs` | `line_type` P |
| TableStyle | `src/objects/table_style.rs` | `handle` I, `owner_handle` P, `modern_cell_style_handle` P |
| Text | `src/entities/text.rs` | No direct handle slots; nested payloads are classified separately |
| TextGenerationFlags | `src/tables/textstyle.rs` | No direct handle slots; nested payloads are classified separately |
| TextStyle | `src/tables/textstyle.rs` | `handle` I, `xref_block_record_handle` P |
| Ucs | `src/tables/ucs.rs` | `handle` I, `named_ucs_handle` P, `base_ucs_handle` P, `xref_handle` P |
| VPort | `src/tables/vport.rs` | `handle` I, `xref_handle` P, `background_handle` P, `visual_style_handle` P, `sun_handle` P, `named_ucs_handle` P, `base_ucs_handle` P |
| Vertex2D | `src/entities/polyline.rs` | No direct handle slots; nested payloads are classified separately |
| Vertex3D | `src/entities/polyline.rs` | No direct handle slots; nested payloads are classified separately |
| Vertex3DPolyline | `src/entities/polyline3d.rs` | `handle` I |
| VertexFlags | `src/entities/polyline.rs` | No direct handle slots; nested payloads are classified separately |
| View | `src/tables/view.rs` | `handle` I, `xref_handle` P, `background_handle` P, `live_section_handle` P, `visual_style_handle` P, `sun_handle` P, `named_ucs_handle` P, `base_ucs_handle` P |
| Viewport | `src/entities/viewport.rs` | `frozen_layers` P, `clip_boundary_handle` P, `ucs_handle` P, `base_ucs_handle` P, `background_handle` P, `shade_plot_handle` P, `visual_style_handle` P, `sun_handle` P |
| ViewportStatusFlags | `src/entities/viewport.rs` | No direct handle slots; nested payloads are classified separately |
| VxTableRecord | `src/tables/vx.rs` | `handle` I, `xref_handle` P, `viewport` P, `previous_entry` P |
| XRecord | `src/objects/xrecord.rs` | `handle` I, `owner` P, `reactors` P, `xdictionary_handle` P |
| XRecordEntry | `src/objects/xrecord.rs` | No direct handle slots; nested payloads are classified separately |

Additional protocol slots: numeric child/owner pairs in raw_record_owners, the first ACDS fingerprint tuple component, table controls/virtual blockmarkers, XRecordValue::Handle and typed XDATA Handle. Private SortEnts entry_map is checked against entries. Raw/private refusals cannot be waived by an independently decoded duplicate.

Tests live in `document::handle_reservation`, `document::handle_usage`, owning-module reservation tests and `tests/handle_reservation.rs`; focused sentinel cases cover header, table, entity-common, object, private-map and nested style/context/association links. Native import and paired MLINE readback are qualified separately against an explicit development pair.

`HeaderVariables.legacy` is typed drafting/view/unit data, with an exhaustive scalar shape guard in `src/document/legacy_header.rs`; it is distinct from the refused `r12_header_raw` backing.

Upstream model audit: signed AttributeDefinition/AttributeEntity.thickness is scalar; Map Object Data remains outside scope through the ClassObject refusal. Seeded private maps retain their complete key/value traversal.
