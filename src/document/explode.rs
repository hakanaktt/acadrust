//! Document-aware INSERT explosion.
//!
//! The entity-level operation deliberately has no access to block tables or
//! object dictionaries.  This module supplies the context needed by the DWG
//! writer: block eligibility, base-point normalization, fresh identities, and
//! cloned extension-dictionary graphs.

use std::collections::HashMap;

use super::CadDocument;
use crate::entities::{
    Block, BlockEnd, EntityCommon, EntityType, Insert, Line, MLine, MultiLeader,
};
use crate::tables::BlockRecord;
use crate::types::{Handle, Transform, Vector3};

impl CadDocument {
    /// Expand MULTILEADER block content after resolving the referenced block.
    /// Text and leader geometry are produced by the detached entity path;
    /// block content needs the document's block table and base point.
    pub(crate) fn explode_multileader_with_context(&self, leader: &MultiLeader) -> Vec<EntityType> {
        let mut parts = EntityType::MultiLeader(Box::new(leader.clone())).explode();
        for root in &leader.context.leader_roots {
            for line in &root.lines {
                let arrow_handle = if line
                    .override_flags
                    .contains(crate::entities::LeaderLinePropertyOverrideFlags::ARROWHEAD)
                {
                    line.arrowhead_handle
                } else {
                    leader.arrowhead_handle
                };
                let Some(arrow_handle) = arrow_handle.filter(|handle| !handle.is_null()) else {
                    continue;
                };
                let Some(tip) = line.points.first().copied() else {
                    continue;
                };
                let Some(next) = line.points.get(1).copied() else {
                    continue;
                };
                let Some(record) = self
                    .block_records
                    .iter()
                    .find(|record| record.handle == arrow_handle)
                    .cloned()
                else {
                    continue;
                };
                let base = self
                    .get_entity(record.block_entity_handle)
                    .and_then(|entity| match entity {
                        EntityType::Block(block) => Some(block.base_point),
                        _ => None,
                    })
                    .unwrap_or(record.base_point);
                let source: Vec<EntityType> = record
                    .entity_handles
                    .iter()
                    .filter_map(|handle| self.get_entity(*handle).cloned())
                    .map(|mut entity| {
                        entity
                            .as_entity_mut()
                            .apply_transform(&Transform::from_translation(base * -1.0));
                        entity
                    })
                    .collect();
                let size = if line
                    .override_flags
                    .contains(crate::entities::LeaderLinePropertyOverrideFlags::ARROWHEAD_SIZE)
                {
                    line.arrowhead_size
                } else {
                    leader.context.arrowhead_size
                };
                let direction = next - tip;
                let mut insert = Insert::new(record.name, tip)
                    .with_uniform_scale(size.max(0.0))
                    .with_rotation(direction.y.atan2(direction.x));
                insert.common = leader.common.clone();
                parts.extend(insert.explode_with_transform(&source, &insert.get_transform(), true));
            }
        }
        if leader.content_type != crate::entities::LeaderContentType::Block {
            return parts;
        }
        let Some(block_handle) = leader
            .block_content_handle
            .or(leader.context.block_content_handle)
        else {
            return parts;
        };
        let Some(record) = self
            .block_records
            .iter()
            .find(|record| record.handle == block_handle)
            .cloned()
        else {
            return parts;
        };
        let base = self
            .get_entity(record.block_entity_handle)
            .and_then(|entity| match entity {
                EntityType::Block(block) => Some(block.base_point),
                _ => None,
            })
            .unwrap_or(record.base_point);
        let source: Vec<EntityType> = record
            .entity_handles
            .iter()
            .filter_map(|handle| self.get_entity(*handle).cloned())
            .map(|mut entity| {
                entity
                    .as_entity_mut()
                    .apply_transform(&Transform::from_translation(base * -1.0));
                entity
            })
            .collect();
        let mut insert = Insert::new(record.name, leader.context.block_content_location)
            .with_scale(
                leader.context.block_content_scale.x * leader.scale_factor,
                leader.context.block_content_scale.y * leader.scale_factor,
                leader.context.block_content_scale.z * leader.scale_factor,
            )
            .with_rotation(leader.context.block_rotation + leader.block_rotation);
        insert.normal = leader.context.block_content_normal;
        parts.extend(insert.explode_with_transform(&source, &insert.get_transform(), true));
        parts
    }

    /// Explode an MLINE after resolving its MLINESTYLE object from the
    /// document. The detached entity API cannot access that table.
    pub(crate) fn explode_mline_with_context(&self, mline: &MLine) -> Vec<EntityType> {
        let style = mline
            .style_handle
            .and_then(|handle| self.objects.get(&handle))
            .and_then(|object| match object {
                crate::objects::ObjectType::MLineStyle(style) => Some(style),
                _ => None,
            })
            .or_else(|| {
                self.objects.values().find_map(|object| match object {
                    crate::objects::ObjectType::MLineStyle(style)
                        if style.name.eq_ignore_ascii_case(&mline.style_name) =>
                    {
                        Some(style)
                    }
                    _ => None,
                })
            });
        let fill_on = style.map(|style| style.flags.fill_on).unwrap_or(false);
        let offset_values: Vec<f64> = style
            .map(|style| {
                style
                    .elements
                    .iter()
                    .map(|element| element.offset * mline.scale_factor)
                    .collect()
            })
            .filter(|offsets: &Vec<_>| !offsets.is_empty())
            .unwrap_or_else(|| vec![0.0]);
        let colors: Vec<crate::types::Color> = style
            .map(|style| style.elements.iter().map(|element| element.color).collect())
            .filter(|colors: &Vec<_>| !colors.is_empty())
            .unwrap_or_else(|| vec![mline.common.color]);
        let mut parts = Vec::new();
        let segment_count = if mline.is_closed() && mline.vertices.len() > 2 {
            mline.vertices.len()
        } else {
            mline.vertices.len().saturating_sub(1)
        };
        for (element_index, offset) in offset_values.iter().copied().enumerate() {
            let color = colors
                .get(element_index)
                .copied()
                .unwrap_or(mline.common.color);
            for i in 0..segment_count {
                let a = &mline.vertices[i];
                let b = &mline.vertices[(i + 1) % mline.vertices.len()];
                let mut start = a.position + a.miter.normalize() * offset;
                let mut end = b.position + b.miter.normalize() * offset;
                if !start.x.is_finite() || !start.y.is_finite() || !start.z.is_finite() {
                    start = a.position;
                }
                if !end.x.is_finite() || !end.y.is_finite() || !end.z.is_finite() {
                    end = b.position;
                }
                let mut line = Line::from_points(start, end);
                line.common = crate::entities::modification::explode::inherit_common(&mline.common);
                line.common.color = color;
                parts.push(EntityType::Line(line));
            }
        }
        if fill_on && offset_values.len() >= 2 {
            let low = offset_values.iter().copied().fold(f64::INFINITY, f64::min);
            let high = offset_values
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max);
            for i in 0..segment_count {
                let a = &mline.vertices[i];
                let b = &mline.vertices[(i + 1) % mline.vertices.len()];
                let mut solid = crate::entities::Solid::new(
                    a.position + a.miter.normalize() * low,
                    a.position + a.miter.normalize() * high,
                    b.position + b.miter.normalize() * low,
                    b.position + b.miter.normalize() * high,
                );
                solid.common =
                    crate::entities::modification::explode::inherit_common(&mline.common);
                parts.push(EntityType::Solid(solid));
            }
        }

        // MLINESTYLE square caps are represented by a segment spanning the
        // outermost style elements at each open end.  Round and inner-arc
        // caps require true plane-aware ARC geometry and remain deferred to a
        // modeler-specific path; emitting the square cap here preserves the
        // native LINE result for the common custom-style case.
        let style_flags = style.map(|style| style.flags);
        let cap_line = |at_start: bool| {
            let Some(flags) = style_flags else {
                return None;
            };
            let square = if at_start {
                flags.start_square_cap
            } else {
                flags.end_square_cap
            };
            if !square {
                return None;
            }
            if at_start
                && mline
                    .flags
                    .contains(crate::entities::MLineFlags::NO_START_CAPS)
            {
                return None;
            }
            if !at_start
                && mline
                    .flags
                    .contains(crate::entities::MLineFlags::NO_END_CAPS)
            {
                return None;
            }
            let vertex = if at_start {
                mline.vertices.first()?
            } else {
                mline.vertices.last()?
            };
            let miter = vertex.miter.normalize();
            if miter.length_squared() <= 1e-20 {
                return None;
            }
            let low = offset_values.iter().copied().fold(f64::INFINITY, f64::min);
            let high = offset_values
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max);
            let mut line = Line::from_points(
                vertex.position + miter * low,
                vertex.position + miter * high,
            );
            line.common = crate::entities::modification::explode::inherit_common(&mline.common);
            Some(EntityType::Line(line))
        };
        let cap_arc = |at_start: bool| {
            let Some(flags) = style_flags else {
                return None;
            };
            let round = if at_start {
                flags.start_round_cap
            } else {
                flags.end_round_cap
            };
            if !round
                || (at_start
                    && mline
                        .flags
                        .contains(crate::entities::MLineFlags::NO_START_CAPS))
                || (!at_start
                    && mline
                        .flags
                        .contains(crate::entities::MLineFlags::NO_END_CAPS))
                || mline.normal.cross(&Vector3::UNIT_Z).length_squared() > 1e-12
            {
                return None;
            }
            let vertex = if at_start {
                mline.vertices.first()?
            } else {
                mline.vertices.last()?
            };
            let miter = vertex.miter.normalize();
            let radius = offset_values
                .iter()
                .copied()
                .map(f64::abs)
                .fold(0.0, f64::max);
            if miter.length_squared() <= 1e-20 || radius <= 1e-12 {
                return None;
            }
            let start_angle =
                miter.y.atan2(miter.x) + if at_start { 0.0 } else { std::f64::consts::PI };
            let end_angle = start_angle + std::f64::consts::PI;
            let mut arc = crate::entities::Arc {
                common: crate::entities::modification::explode::inherit_common(&mline.common),
                center: vertex.position,
                radius,
                start_angle,
                end_angle,
                thickness: 0.0,
                normal: mline.normal,
            };
            arc.common.color = mline.common.color;
            Some(EntityType::Arc(arc))
        };
        if let Some(cap) = cap_line(true) {
            parts.push(cap);
        }
        if !mline.is_closed() {
            if let Some(cap) = cap_line(false) {
                parts.push(cap);
            }
        }
        if let Some(cap) = cap_arc(true) {
            parts.push(cap);
        }
        if !mline.is_closed() {
            if let Some(cap) = cap_arc(false) {
                parts.push(cap);
            }
        }
        parts
    }

    /// Explode an INSERT using this document's block table and object graph.
    ///
    /// A MINSERT, xref, xref-dependent, or non-explodable definition is
    /// refused just as ACAD's EXPLODE command is.  The returned entities
    /// are detached from the source graph and have fresh handles suitable for
    /// insertion into this document.  The source INSERT and block contents
    /// are never modified.
    pub(crate) fn explode_insert_with_context(&mut self, insert: &Insert) -> Vec<EntityType> {
        if insert.is_minsert() {
            return Vec::new();
        }
        let Some(record) = self.block_records.get(&insert.block_name).cloned() else {
            return Vec::new();
        };
        if !record.explodable
            || record.flags.is_xref
            || record.flags.is_xref_overlay
            || record.flags.is_external
            || !record.xref_path.is_empty()
        {
            return Vec::new();
        }

        let base = self
            .get_entity(record.block_entity_handle)
            .and_then(|entity| match entity {
                EntityType::Block(block) => Some(block.base_point),
                _ => None,
            })
            .unwrap_or(record.base_point);
        let to_base = Transform::from_translation(base * -1.0);
        let source: Vec<EntityType> = record
            .entity_handles
            .iter()
            .filter_map(|handle| self.get_entity(*handle).cloned())
            .map(|mut entity| {
                entity.as_entity_mut().apply_transform(&to_base);
                entity
            })
            .collect();

        let transform = insert.get_transform();
        let mut parts = Vec::new();
        let unsupported: Vec<EntityType> = if !insert.has_uniform_scale() {
            source
                .iter()
                .filter(|entity| Self::is_nonuniform_modeler(entity))
                .cloned()
                .collect()
        } else {
            Vec::new()
        };
        if !unsupported.is_empty() {
            let drawable_count = source
                .iter()
                .filter(|entity| {
                    !matches!(
                        entity,
                        EntityType::Block(_) | EntityType::BlockEnd(_) | EntityType::Seqend(_)
                    )
                })
                .count();
            if unsupported.len() == drawable_count {
                // ACAD leaves an INSERT containing only unsupported
                // modeler members untouched under a non-uniform transform.
                return Vec::new();
            }
            let geometry = unsupported
                .into_iter()
                .map(|mut entity| {
                    entity.as_entity_mut().apply_transform(&transform);
                    insert.resolve_exploded_properties(entity.common_mut());
                    entity
                })
                .collect();
            if let Some(fallback) = self.make_anonymous_fallback(geometry, &insert.common) {
                parts.push(fallback);
            }
        }
        for entity in &source {
            if Self::is_nonuniform_modeler(entity) && !insert.has_uniform_scale() {
                continue;
            }
            if let EntityType::Insert(nested) = entity {
                let composite =
                    Transform::from_matrix(transform.matrix * nested.get_transform().matrix);
                if Insert::has_unrepresentable_shear(&composite) {
                    if let Some(fallback) = self.make_shear_fallback(insert, nested, &transform) {
                        parts.push(fallback);
                        continue;
                    }
                }
            }
            parts.extend(insert.explode_with_transform(
                std::slice::from_ref(entity),
                &transform,
                false,
            ));
        }
        self.detach_exploded_parts(&mut parts, insert.common.owner_handle);
        parts
    }

    fn is_nonuniform_modeler(entity: &EntityType) -> bool {
        matches!(
            entity,
            EntityType::Solid3D(_) | EntityType::Region(_) | EntityType::Body(_)
        )
    }

    /// Preserve an exact nested INSERT whose parent transform introduces
    /// shear.  The INSERT record cannot encode that affine map, so bake the
    /// nested block's transformed primitives into an anonymous `*E` block and
    /// return an identity reference to it, matching ACAD's fallback.
    fn make_shear_fallback(
        &mut self,
        parent: &Insert,
        nested: &Insert,
        parent_transform: &Transform,
    ) -> Option<EntityType> {
        let record = self.block_records.get(&nested.block_name).cloned()?;
        if !record.explodable
            || record.flags.is_xref
            || record.flags.is_xref_overlay
            || record.flags.is_external
            || !record.xref_path.is_empty()
        {
            return None;
        }
        let base = self
            .get_entity(record.block_entity_handle)
            .and_then(|entity| match entity {
                EntityType::Block(block) => Some(block.base_point),
                _ => None,
            })
            .unwrap_or(record.base_point);
        let to_base = Transform::from_translation(base * -1.0);
        let source: Vec<EntityType> = record
            .entity_handles
            .iter()
            .filter_map(|handle| self.get_entity(*handle).cloned())
            .map(|mut entity| {
                entity.as_entity_mut().apply_transform(&to_base);
                entity
            })
            .collect();
        let composite =
            Transform::from_matrix(parent_transform.matrix * nested.get_transform().matrix);
        let mut geometry = nested.explode_with_transform(&source, &composite, true);
        if geometry.is_empty() {
            return None;
        }
        for entity in &mut geometry {
            parent.resolve_exploded_properties(entity.common_mut());
        }

        let mut common = nested.common.clone();
        if common.layer == "0" {
            common.layer = parent.common.layer.clone();
            common.layer_handle = parent.common.layer_handle;
        }
        if common.color == crate::types::Color::ByBlock {
            common.color = parent.common.color;
            common.color_name = parent.common.color_name.clone();
            common.color_book_handle = parent.common.color_book_handle;
        }
        self.make_anonymous_fallback(geometry, &common)
    }

    fn make_anonymous_fallback(
        &mut self,
        geometry: Vec<EntityType>,
        common: &EntityCommon,
    ) -> Option<EntityType> {
        if geometry.is_empty() {
            return None;
        }
        let mut ordinal = 0u64;
        let name = loop {
            let candidate = format!("*E{}", ordinal);
            if !self.block_records.contains(&candidate) {
                break candidate;
            }
            ordinal += 1;
        };
        let record_handle = self.allocate_handle();
        let marker_handle = self.allocate_handle();
        let end_handle = self.allocate_handle();
        let mut fallback_record = BlockRecord::new(&name);
        fallback_record.handle = record_handle;
        fallback_record.block_entity_handle = marker_handle;
        fallback_record.block_end_handle = end_handle;
        fallback_record.flags.anonymous = true;
        self.block_records.add(fallback_record).ok()?;

        let mut marker = Block::new(&name, Vector3::ZERO);
        marker.common.handle = marker_handle;
        marker.common.owner_handle = record_handle;
        self.add_entity(EntityType::Block(marker)).ok()?;
        for mut entity in geometry {
            let old = entity.common().handle;
            let fresh = self.allocate_handle();
            let cloned_dictionary = !old.is_null() && self.clone_extension_graph(old, fresh);
            entity.common_mut().handle = fresh;
            entity.common_mut().raw_record = None;
            entity.common_mut().reactors.clear();
            if cloned_dictionary {
                entity.common_mut().xdictionary_handle = self.extension_dictionary_handle(fresh);
            } else {
                entity.common_mut().xdictionary_handle = None;
            }
            entity.common_mut().owner_handle = record_handle;
            self.add_entity(entity).ok()?;
        }
        let mut end = BlockEnd::new();
        end.common.handle = end_handle;
        end.common.owner_handle = record_handle;
        self.add_entity(EntityType::BlockEnd(end)).ok()?;

        let mut fallback = Insert::new(name, Vector3::ZERO);
        fallback.common = common.clone();
        Some(EntityType::Insert(fallback))
    }

    /// Assign fresh entity and compound-child identities and clone each
    /// entity's extension dictionary.  A source extension dictionary is a
    /// hard-owned graph; retaining its handle under a copied entity makes the
    /// original and exploded entity race for the same owner at DWG save time.
    /// A modeler copy's solid history is hard-owned too; the copy drops it.
    fn detach_exploded_parts(&mut self, parts: &mut [EntityType], owner: Handle) {
        for part in parts {
            let old = part.common().handle;
            let fresh = self.allocate_handle();
            let cloned_dictionary = !old.is_null() && self.clone_extension_graph(old, fresh);
            if cloned_dictionary {
                part.common_mut().xdictionary_handle = self.extension_dictionary_handle(fresh);
            }
            part.common_mut().handle = fresh;
            part.common_mut().owner_handle = owner;
            part.common_mut().raw_record = None;
            part.common_mut().reactors.clear();
            if !cloned_dictionary {
                part.common_mut().xdictionary_handle = None;
            }
            Insert::detach_exploded_history(part);

            if let EntityType::Insert(nested) = part {
                for attribute in &mut nested.attributes {
                    let old_attribute = attribute.common.handle;
                    let fresh_attribute = self.allocate_handle();
                    let cloned_dictionary = !old_attribute.is_null()
                        && self.clone_extension_graph(old_attribute, fresh_attribute);
                    if cloned_dictionary {
                        attribute.common.xdictionary_handle =
                            self.extension_dictionary_handle(fresh_attribute);
                    }
                    attribute.common.handle = fresh_attribute;
                    attribute.common.owner_handle = fresh;
                    attribute.common.raw_record = None;
                    attribute.common.reactors.clear();
                    if !cloned_dictionary {
                        attribute.common.xdictionary_handle = None;
                    }
                }
                nested.view_rep_handle = None;
                nested.seqend_handle = nested.seqend_handle.map(|_| self.allocate_handle());
            }
        }
    }

    /// Clone an entity's extension dictionary and the dictionary/XRECORD graph
    /// it owns.  The graph is intentionally limited to object kinds whose
    /// handle relationships are represented by the public object model; an
    /// opaque object remains attached to its source and is dropped from the
    /// detached entity rather than producing a dangling reference.
    fn clone_extension_graph(&mut self, source_owner: Handle, target_owner: Handle) -> bool {
        // The block graph remapper already handles every typed object family,
        // ownership chain, XDATA handle and storage provenance.  Reuse it for
        // an entity extension dictionary as well; a source INSERT's xdict is
        // the same hard-owned graph shape as a block record's xdict.
        if let Some(graph) = self.export_block_graph(source_owner) {
            self.import_block_graph(&graph, target_owner, &HashMap::new());
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::{Block, Line, MLine};
    use crate::objects::{MLineStyle, MLineStyleElement, MLineStyleFlags, ObjectType};
    use crate::tables::BlockRecord;
    use crate::types::Vector3;

    fn document_with_line() -> (CadDocument, Insert, Handle, Handle) {
        let mut document = CadDocument::new();
        let mut record = BlockRecord::new("B");
        record.handle = document.allocate_handle();
        let owner = record.handle;
        document.block_records.add(record).unwrap();

        let mut block = Block::new("B", Vector3::ZERO);
        block.common.owner_handle = owner;
        let block_handle = document.add_entity(EntityType::Block(block)).unwrap();
        document
            .block_records
            .get_mut("B")
            .unwrap()
            .block_entity_handle = block_handle;

        let mut line = Line::from_points(Vector3::ZERO, Vector3::new(1.0, 0.0, 0.0));
        line.common.owner_handle = owner;
        let line_handle = document.add_entity(EntityType::Line(line)).unwrap();

        let mut insert = Insert::new("B", Vector3::new(10.0, 20.0, 0.0));
        insert.common.owner_handle = document.header.model_space_block_handle;
        (document, insert, owner, line_handle)
    }

    #[test]
    fn context_explode_refuses_xrefs_and_minsert() {
        let (mut document, mut insert, _, _) = document_with_line();
        document.block_records.get_mut("B").unwrap().flags.is_xref = true;
        assert!(document
            .explode_entity(&EntityType::Insert(insert.clone()))
            .is_empty());
        document.block_records.get_mut("B").unwrap().flags.is_xref = false;
        insert = insert.with_array(2, 1, 5.0, 0.0);
        assert!(document
            .explode_entity(&EntityType::Insert(insert))
            .is_empty());
    }

    #[test]
    fn context_explode_mline_emits_square_style_caps() {
        let mut document = CadDocument::new();
        let style_handle = document.allocate_handle();
        let mut style = MLineStyle::new("CAPS");
        style.handle = style_handle;
        style.flags = MLineStyleFlags::simple();
        style.elements = vec![MLineStyleElement::new(1.0), MLineStyleElement::new(-1.0)];
        document
            .objects
            .insert(style_handle, ObjectType::MLineStyle(style));

        let mut mline =
            MLine::from_points(&[Vector3::new(0.0, 0.0, 0.0), Vector3::new(10.0, 0.0, 0.0)]);
        mline.style_name = "CAPS".to_string();
        mline.style_handle = Some(style_handle);
        let parts = document.explode_entity(&EntityType::MLine(mline));
        let lines: Vec<_> = parts
            .iter()
            .filter(|part| matches!(part, EntityType::Line(_)))
            .collect();
        assert_eq!(lines.len(), 4);
        assert!(lines.iter().any(|part| match part {
            EntityType::Line(line) => line.start.x.abs() < 1e-9 && line.end.x.abs() < 1e-9,
            _ => false,
        }));
        assert!(lines.iter().any(|part| match part {
            EntityType::Line(line) =>
                (line.start.x - 10.0).abs() < 1e-9 && (line.end.x - 10.0).abs() < 1e-9,
            _ => false,
        }));
    }

    #[test]
    fn context_explode_mline_emits_xy_round_style_caps() {
        let mut document = CadDocument::new();
        let style_handle = document.allocate_handle();
        let mut style = MLineStyle::new("ROUND");
        style.handle = style_handle;
        style.flags = MLineStyleFlags::round_caps();
        style.elements = vec![MLineStyleElement::new(1.0), MLineStyleElement::new(-1.0)];
        document
            .objects
            .insert(style_handle, ObjectType::MLineStyle(style));

        let mut mline =
            MLine::from_points(&[Vector3::new(0.0, 0.0, 0.0), Vector3::new(10.0, 0.0, 0.0)]);
        mline.style_name = "ROUND".to_string();
        mline.style_handle = Some(style_handle);
        let parts = document.explode_entity(&EntityType::MLine(mline));
        assert_eq!(
            parts
                .iter()
                .filter(|part| matches!(part, EntityType::Line(_)))
                .count(),
            2
        );
        assert_eq!(
            parts
                .iter()
                .filter(|part| matches!(part, EntityType::Arc(_)))
                .count(),
            2
        );
    }

    #[test]
    fn context_explode_assigns_fresh_owner_and_handle() {
        let (mut document, insert, _owner, source_handle) = document_with_line();
        let parts = document.explode_entity(&EntityType::Insert(insert));
        assert_eq!(parts.len(), 1);
        let line = match &parts[0] {
            EntityType::Line(value) => value,
            _ => panic!("expected line"),
        };
        assert!(line.common.handle.is_valid());
        assert_ne!(line.common.handle, source_handle);
        assert_eq!(
            line.common.owner_handle,
            document.header.model_space_block_handle
        );
        assert_eq!(line.start, Vector3::new(10.0, 20.0, 0.0));
    }

    #[test]
    fn context_explode_clones_entity_extension_dictionary() {
        let (mut document, insert, _, source_handle) = document_with_line();
        let source_dict = document.ensure_extension_dictionary(source_handle);
        let source_record = document.ensure_xrecord(source_handle, "EXPLODE_TEST");
        assert_eq!(
            document.extension_dictionary_handle(source_handle),
            Some(source_dict)
        );
        assert!(document.objects.contains_key(&source_record));

        let parts = document.explode_entity(&EntityType::Insert(insert));
        let line = match &parts[0] {
            EntityType::Line(value) => value,
            _ => panic!("expected line"),
        };
        let cloned_dict = line.common.xdictionary_handle.expect("cloned dictionary");
        assert_ne!(cloned_dict, source_dict);
        assert!(matches!(
            document.objects.get(&cloned_dict),
            Some(ObjectType::Dictionary(_))
        ));
        assert_eq!(
            document.extension_dictionary_handle(line.common.handle),
            Some(cloned_dict)
        );
    }

    #[test]
    fn context_explode_remaps_nested_insert_children() {
        let (mut document, mut insert, _, _) = document_with_line();
        let owner = document.block_records.get("B").unwrap().handle;
        let mut nested = Insert::new("CHILD", Vector3::new(1.0, 2.0, 0.0));
        nested.common.handle = Handle::new(0x1200);
        nested.common.owner_handle = owner;
        nested.seqend_handle = Some(Handle::new(0x1202));
        let mut attribute = crate::entities::AttributeEntity::new("TAG".into(), "value".into());
        attribute.common.handle = Handle::new(0x1201);
        attribute.common.owner_handle = nested.common.handle;
        nested.attributes.push(attribute);
        document.add_entity(EntityType::Insert(nested)).unwrap();

        // The helper's block member list now contains the line and nested
        // INSERT.  The source nested record remains untouched after EXPLODE.
        insert.common.owner_handle = document.header.model_space_block_handle;
        let parts = document.explode_entity(&EntityType::Insert(insert));
        let output = parts
            .iter()
            .find_map(|part| match part {
                EntityType::Insert(value) => Some(value),
                _ => None,
            })
            .expect("nested insert output");
        assert!(output.common.handle.is_valid());
        assert_ne!(output.common.handle, Handle::new(0x1200));
        assert_eq!(output.attributes.len(), 1);
        assert!(output.attributes[0].common.handle.is_valid());
        assert_ne!(output.attributes[0].common.handle, Handle::new(0x1201));
        assert_eq!(
            output.attributes[0].common.owner_handle,
            output.common.handle
        );
        assert_ne!(output.seqend_handle, Some(Handle::new(0x1202)));
    }

    #[test]
    fn context_explode_bakes_nested_shear_into_anonymous_block() {
        let mut document = CadDocument::new();
        let child_owner = {
            let mut record = BlockRecord::new("CHILD");
            record.handle = document.allocate_handle();
            let owner = record.handle;
            document.block_records.add(record).unwrap();
            let mut marker = Block::new("CHILD", Vector3::ZERO);
            marker.common.owner_handle = owner;
            let marker_handle = document.add_entity(EntityType::Block(marker)).unwrap();
            document
                .block_records
                .get_mut("CHILD")
                .unwrap()
                .block_entity_handle = marker_handle;
            let mut line = Line::from_points(Vector3::ZERO, Vector3::new(1.0, 0.0, 0.0));
            line.common.owner_handle = owner;
            document.add_entity(EntityType::Line(line)).unwrap();
            owner
        };
        let parent_owner = {
            let mut record = BlockRecord::new("PARENT");
            record.handle = document.allocate_handle();
            let owner = record.handle;
            document.block_records.add(record).unwrap();
            let mut marker = Block::new("PARENT", Vector3::ZERO);
            marker.common.owner_handle = owner;
            let marker_handle = document.add_entity(EntityType::Block(marker)).unwrap();
            document
                .block_records
                .get_mut("PARENT")
                .unwrap()
                .block_entity_handle = marker_handle;
            owner
        };
        let mut nested =
            Insert::new("CHILD", Vector3::ZERO).with_rotation(std::f64::consts::FRAC_PI_4);
        nested.common.owner_handle = parent_owner;
        document.add_entity(EntityType::Insert(nested)).unwrap();
        let mut outer = Insert::new("PARENT", Vector3::ZERO).with_scale(2.0, 1.0, 1.0);
        outer.common.owner_handle = document.header.model_space_block_handle;
        let parts = document.explode_entity(&EntityType::Insert(outer));
        let fallback = parts
            .iter()
            .find_map(|part| match part {
                EntityType::Insert(insert) if insert.block_name.starts_with("*E") => Some(insert),
                _ => None,
            })
            .expect("nested shear should use an anonymous fallback block");
        assert!(document.block_records.contains(&fallback.block_name));
        assert!(document
            .entities_in_block(&fallback.block_name)
            .any(|entity| { matches!(entity, EntityType::Line(_)) }));
        assert_ne!(child_owner, parent_owner);
    }

    #[test]
    fn context_explode_keeps_nonuniform_modeler_member_in_anonymous_block() {
        let mut document = CadDocument::new();
        let mut record = BlockRecord::new("MODEL");
        record.handle = document.allocate_handle();
        let owner = record.handle;
        document.block_records.add(record).unwrap();
        let mut marker = Block::new("MODEL", Vector3::ZERO);
        marker.common.owner_handle = owner;
        let marker_handle = document.add_entity(EntityType::Block(marker)).unwrap();
        document
            .block_records
            .get_mut("MODEL")
            .unwrap()
            .block_entity_handle = marker_handle;
        let mut solid = crate::entities::Solid3D::new();
        solid.common.owner_handle = owner;
        document.add_entity(EntityType::Solid3D(solid)).unwrap();
        let mut line = Line::from_points(Vector3::ZERO, Vector3::new(1.0, 0.0, 0.0));
        line.common.owner_handle = owner;
        document.add_entity(EntityType::Line(line)).unwrap();

        let mut insert = Insert::new("MODEL", Vector3::ZERO).with_scale(2.0, 1.0, 1.0);
        insert.common.owner_handle = document.header.model_space_block_handle;
        let parts = document.explode_entity(&EntityType::Insert(insert));
        assert!(parts.iter().any(|part| matches!(part, EntityType::Line(_))));
        let fallback = parts
            .iter()
            .find_map(|part| match part {
                EntityType::Insert(value) if value.block_name.starts_with("*E") => Some(value),
                _ => None,
            })
            .expect("modeler member fallback INSERT");
        assert!(document
            .entities_in_block(&fallback.block_name)
            .any(|entity| { matches!(entity, EntityType::Solid3D(_)) }));
    }
}
