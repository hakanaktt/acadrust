//! One current, read-only inventory of protected numeric CAD identities.
use super::{CadDocument, HandleReservationError};
use crate::{entities::EntityType, types::Handle, AuditLocation};
use std::collections::HashMap;
mod entities;
mod objects;
mod tables;
type Result<T> = std::result::Result<T, HandleReservationError>;
pub(crate) struct HandleUsage {
    max: u64,
    identities: HashMap<Handle, (AuditLocation, String)>,
    markers: HashMap<Handle, (Handle, bool)>,
    location: AuditLocation,
    prefix: String,
}
impl HandleUsage {
    fn new() -> Self {
        Self {
            max: 0,
            identities: HashMap::new(),
            markers: HashMap::new(),
            location: AuditLocation::Document,
            prefix: String::new(),
        }
    }
    pub(crate) fn protect(&mut self, h: Handle) {
        self.max = self.max.max(h.value());
    }
    pub(crate) fn optional(&mut self, h: Option<Handle>) {
        if let Some(h) = h {
            self.protect(h);
        }
    }
    pub(crate) fn refs(&mut self, hs: &[Handle]) {
        for h in hs {
            self.protect(*h);
        }
    }
    pub(crate) fn bad(&self, path: &str, h: Option<Handle>) -> HandleReservationError {
        HandleReservationError::InconsistentDocument {
            location: self.location.clone(),
            field_path: format!("{}{}", self.prefix, path),
            handle: h,
            conflicting_location: h
                .and_then(|h| self.identities.get(&h).map(|p| Box::new(p.0.clone()))),
        }
    }
    pub(crate) fn unsupported(&self, path: &str) -> HandleReservationError {
        HandleReservationError::UninspectableData {
            location: self.location.clone(),
            field_path: format!("{}{}", self.prefix, path),
        }
    }
    pub(crate) fn primary(&mut self, h: Handle, path: &str) -> Result<()> {
        if h.is_null() || self.identities.contains_key(&h) {
            return Err(self.bad(path, Some(h)));
        }
        self.identities
            .insert(h, (self.location.clone(), path.into()));
        self.protect(h);
        Ok(())
    }
    pub(crate) fn text(&self, value: &str, path: &str) -> Result<()> {
        if value.contains("%<") {
            Err(self.unsupported(path))
        } else {
            Ok(())
        }
    }
}
pub(super) fn census(d: &CadDocument) -> Result<u64> {
    let CadDocument {
        version: _,
        maintenance_version: _,
        header: _,
        layers: _,
        line_types: _,
        text_styles: _,
        block_records: _,
        dim_styles: _,
        app_ids: _,
        views: _,
        vports: _,
        ucss: _,
        vx_table: _,
        vx_control_entries: _,
        classes: _,
        notifications: _,
        entities: _,
        entity_index: _,
        objects: _,
        block_visibility_params: _,
        context_scales: _,
        block_representations: _,
        fields: _,
        summary_info: _,
        source_path: _,
        dgn_ls_definitions: _,
        dgn_ls_components: _,
        eed_by_handle: _,
        object_xdata: _,
        xdic_by_handle: _,
        reactors_by_handle: _,
        block_entity_handles: _,
        dwg_source_version: _,
        preview: _,
        acis_sab_handles: _,
        raw_acds_data: _,
        r12_header_raw: _,
        r12_table_usage: _,
        r12_block_tags: _,
        raw_records: _,
        original_objects: _,
        raw_record_owners: _,
        raw_acds_fingerprint: _,
        dwg_data_store_handles: _,
        section_view_style: _,
        view_rep_refs: _,
        section_view_reps: _,
        next_handle: _,
    } = d;

    let mut u = HandleUsage::new();
    if !d.eed_by_handle.is_empty() {
        return Err(u.unsupported("eed_by_handle"));
    }
    if !d.raw_records.is_empty() {
        return Err(u.unsupported("raw_records"));
    }
    if !d.original_objects.is_empty() {
        return Err(u.unsupported("original_objects"));
    }
    if !d.block_visibility_params.is_empty() {
        return Err(u.unsupported("block_visibility_params"));
    }
    if !d.dgn_ls_definitions.is_empty() {
        return Err(u.unsupported("dgn_ls_definitions"));
    }
    if !d.dgn_ls_components.is_empty() {
        return Err(u.unsupported("dgn_ls_components"));
    }
    if !d.fields.is_empty() {
        return Err(u.unsupported("fields"));
    }
    if d.raw_acds_data.is_some() {
        return Err(u.unsupported("raw_acds_data"));
    }
    if d.r12_header_raw.is_some() {
        return Err(u.unsupported("r12_header_raw"));
    }
    if d.section_view_style.is_some() {
        return Err(u.unsupported("section_view_style"));
    }
    tables::visit(d, &mut u)?;
    u.refs(&d.vx_control_entries);
    u.refs(&d.section_view_reps);
    u.refs(&d.acis_sab_handles);
    for h in &d.dwg_data_store_handles {
        u.protect(*h);
    }
    for (k, v) in &d.xdic_by_handle {
        u.protect(*k);
        u.protect(*v);
    }
    for (k, v) in &d.context_scales {
        u.protect(*k);
        u.protect(*v);
    }
    for (k, v) in &d.block_representations {
        u.protect(*k);
        u.protect(*v);
    }
    for (k, v) in &d.reactors_by_handle {
        u.protect(*k);
        u.refs(v);
    }
    for (k, v) in &d.block_entity_handles {
        u.protect(*k);
        u.refs(v);
    }
    for (k, v) in &d.view_rep_refs {
        u.protect(*k);
        u.refs(v);
    }
    for k in d.r12_table_usage.keys() {
        u.protect(*k);
    }
    for k in d.r12_block_tags.keys() {
        u.protect(*k);
    }
    for (k, v) in &d.raw_record_owners {
        u.protect(Handle::new(*k));
        u.protect(Handle::new(*v));
    }
    for (h, _, _) in &d.raw_acds_fingerprint {
        u.protect(Handle::new(*h));
    }
    for (h, v) in &d.object_xdata {
        u.prefix = format!("object_xdata[{h}].");
        u.protect(*h);
        entities::xdata(v, &mut u)?;
    }
    u.prefix.clear();
    if d.entity_index.len() != d.entities.len() {
        return Err(u.bad("entity_index.length", None));
    }
    for (i, e) in d.entities.iter().enumerate() {
        let c = e.common();
        u.location = AuditLocation::Entity { handle: c.handle };
        u.prefix = format!("entities[{i}].");
        if d.entity_index.get(&c.handle) != Some(&i) {
            return Err(u.bad("entity_index", Some(c.handle)));
        }
        if let Some((owner, end)) = u.markers.get(&c.handle) {
            let expected = if *end {
                matches!(e.as_ref(), EntityType::BlockEnd(_))
            } else {
                matches!(e.as_ref(), EntityType::Block(_))
            };
            if !expected || *owner != c.owner_handle {
                return Err(u.bad("block_marker.alias", Some(c.handle)));
            }
        } else {
            u.primary(c.handle, "common.handle")?;
        }
        entities::visit(e, &mut u)?;
    }
    for (h, o) in &d.objects {
        u.location = AuditLocation::Object { handle: *h };
        u.prefix = format!("objects[{h}].");
        objects::visit(*h, o, &mut u)?;
    }
    Ok(u.max)
}
#[cfg(test)]
mod tests;
