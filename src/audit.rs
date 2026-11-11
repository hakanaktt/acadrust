//! Structural integrity checks and conservative repair for CAD documents.
//!
//! The audit pass is deliberately read-only.  Repair only changes derived
//! indexes and identities when the intended result is unambiguous; duplicate
//! handles and uncertain references remain visible in the report.

use crate::document::CadDocument;
use crate::entities::EntityType;
use crate::objects::ObjectType;
use crate::tables::TableEntry;
use crate::types::Handle;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AuditSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AuditIssueCode {
    NullHandle,
    DuplicateHandle,
    StorageKeyMismatch,
    EntityIndexMismatch,
    ObjectMapMismatch,
    DanglingReference,
    OwnerMismatch,
    OwnerCycle,
    InvalidGeometry,
    InvalidHandleSeed,
    StaleTableKey,
    UnsupportedRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AuditLocation {
    Document,
    TableEntry {
        table: String,
        name: String,
        handle: Handle,
    },
    Entity {
        handle: Handle,
    },
    Object {
        handle: Handle,
    },
    Reference {
        owner: Handle,
        target: Handle,
    },
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AuditIssue {
    pub code: AuditIssueCode,
    pub severity: AuditSeverity,
    pub location: AuditLocation,
    pub handle: Option<Handle>,
    pub message: String,
    pub repairable: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AuditReport {
    /// Number of all table, entity and object records inspected.
    pub scanned_records: usize,
    /// Number of entity records inspected.
    pub scanned_entities: usize,
    /// Number of object records inspected.
    pub scanned_objects: usize,
    /// Number of symbol-table entries inspected.
    pub scanned_table_entries: usize,
    pub issues: Vec<AuditIssue>,
}

impl AuditReport {
    pub fn is_clean(&self) -> bool {
        self.issues.is_empty()
    }
    pub fn error_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|issue| {
                matches!(
                    issue.severity,
                    AuditSeverity::Error | AuditSeverity::Critical
                )
            })
            .count()
    }
    pub fn warning_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|issue| issue.severity == AuditSeverity::Warning)
            .count()
    }
    pub fn count(&self, code: AuditIssueCode) -> usize {
        self.issues
            .iter()
            .filter(|issue| issue.code == code)
            .count()
    }
    pub fn severity_count(&self, severity: AuditSeverity) -> usize {
        self.issues
            .iter()
            .filter(|issue| issue.severity == severity)
            .count()
    }
    fn issue(
        &mut self,
        code: AuditIssueCode,
        severity: AuditSeverity,
        location: AuditLocation,
        handle: Option<Handle>,
        message: impl Into<String>,
        repairable: bool,
    ) {
        self.issues.push(AuditIssue {
            code,
            severity,
            location,
            handle,
            message: message.into(),
            repairable,
        });
    }
}

impl AuditIssue {
    /// Human-readable issue description (an alias for the serialized message).
    pub fn description(&self) -> &str {
        &self.message
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RepairAction {
    ResyncTableKeys { count: usize },
    AssignTableHandles { count: usize },
    AssignEntityHandles { count: usize },
    ReindexEntities { count: usize },
    ReindexObject { old: Handle, new: Handle },
    RepairBlockOwners { count: usize },
    ClearAttributeDefinitionFlags { count: usize },
    SynchronizeHandleAllocator,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RepairReport {
    pub before: AuditReport,
    pub after: AuditReport,
    pub actions: Vec<RepairAction>,
    pub unresolved: Vec<AuditIssue>,
}

impl Default for RepairReport {
    fn default() -> Self {
        Self {
            before: AuditReport::default(),
            after: AuditReport::default(),
            actions: Vec::new(),
            unresolved: Vec::new(),
        }
    }
}

fn table_handles(
    document: &CadDocument,
    seen: &mut HashMap<Handle, Vec<AuditLocation>>,
    report: &mut AuditReport,
) {
    macro_rules! table {
        ($value:expr, $name:literal) => {
            for entry in $value.iter() {
                report.scanned_records += 1;
                report.scanned_table_entries += 1;
                let handle = entry.handle();
                let location = AuditLocation::TableEntry {
                    table: $name.to_string(),
                    name: entry.name().to_string(),
                    handle,
                };
                if handle.is_null() {
                    report.issue(
                        AuditIssueCode::NullHandle,
                        AuditSeverity::Error,
                        location.clone(),
                        Some(handle),
                        format!("{} entry '{}' has a null handle", $name, entry.name()),
                        true,
                    );
                }
                if !handle.is_null() && !seen.entry(handle).or_default().is_empty() {
                    report.issue(
                        AuditIssueCode::DuplicateHandle,
                        AuditSeverity::Error,
                        location.clone(),
                        Some(handle),
                        format!("duplicate handle in {} table", $name),
                        false,
                    );
                }
                seen.entry(handle).or_default().push(location);
            }
        };
    }
    table!(document.layers, "LAYER");
    table!(document.line_types, "LTYPE");
    table!(document.text_styles, "STYLE");
    table!(document.block_records, "BLOCK_RECORD");
    table!(document.dim_styles, "DIMSTYLE");
    table!(document.app_ids, "APPID");
    table!(document.views, "VIEW");
    table!(document.vports, "VPORT");
    table!(document.ucss, "UCS");
    table!(document.vx_table, "VXTABLE");
}

fn add_handle(
    seen: &mut HashMap<Handle, Vec<AuditLocation>>,
    report: &mut AuditReport,
    handle: Handle,
    location: AuditLocation,
) {
    if handle.is_null() {
        report.issue(
            AuditIssueCode::NullHandle,
            AuditSeverity::Error,
            location.clone(),
            Some(handle),
            "record has a null handle",
            true,
        );
    } else if seen.get(&handle).is_some_and(|items| !items.is_empty()) {
        report.issue(
            AuditIssueCode::DuplicateHandle,
            AuditSeverity::Error,
            location.clone(),
            Some(handle),
            "duplicate handle",
            false,
        );
    }
    seen.entry(handle).or_default().push(location);
}

/// Hook for entity-specific geometry checks.  The common invariants are
/// checked here; geometry modules can add stricter checks by appending issues.
pub fn audit_entity(entity: &EntityType, location: AuditLocation, report: &mut AuditReport) {
    let common = entity.common();
    if !common.linetype_scale.is_finite() || common.linetype_scale <= 0.0 {
        report.issue(
            AuditIssueCode::InvalidGeometry,
            AuditSeverity::Error,
            location.clone(),
            Some(common.handle),
            "entity has a non-finite or non-positive linetype scale",
            false,
        );
    }
    if common.layer.trim().is_empty() {
        report.issue(
            AuditIssueCode::InvalidGeometry,
            AuditSeverity::Warning,
            location,
            Some(common.handle),
            "entity has an empty layer name",
            false,
        );
    }
    let invalid = match entity {
        EntityType::Circle(value) => !value.radius.is_finite() || value.radius < 0.0,
        EntityType::Arc(value) => {
            !value.radius.is_finite()
                || value.radius < 0.0
                || !value.start_angle.is_finite()
                || !value.end_angle.is_finite()
        }
        EntityType::Ellipse(value) => {
            !value.minor_axis_ratio.is_finite()
                || value.minor_axis_ratio <= 0.0
                || !value.start_parameter.is_finite()
                || !value.end_parameter.is_finite()
        }
        EntityType::Spline(value) => {
            value.degree < 1
                || value.knots.iter().any(|knot| !knot.is_finite())
                || value
                    .weights
                    .iter()
                    .any(|weight| !weight.is_finite() || *weight <= 0.0)
                || value.knots.windows(2).any(|pair| pair[1] < pair[0])
        }
        EntityType::Mesh(value) => value.faces.iter().any(|face| {
            face.vertices.len() < 3
                || face
                    .vertices
                    .iter()
                    .any(|index| *index >= value.vertices.len())
        }),
        _ => false,
    };
    if invalid {
        report.issue(
            AuditIssueCode::InvalidGeometry,
            AuditSeverity::Error,
            AuditLocation::Entity {
                handle: common.handle,
            },
            Some(common.handle),
            "entity geometry violates a basic finite/range invariant",
            false,
        );
    }
}

fn invalidate_object_passthrough(object: &mut ObjectType) {
    object.clear_storage_data();
}

fn migrate_object_side_data(document: &mut CadDocument, old: Handle, new: Handle) {
    if let Some(value) = document.eed_by_handle.remove(&old) {
        document.eed_by_handle.insert(new, value);
    }
    if let Some(value) = document.object_xdata.remove(&old) {
        document.object_xdata.insert(new, value);
    }
    if let Some(value) = document.xdic_by_handle.remove(&old) {
        document.xdic_by_handle.insert(new, value);
    }
    if let Some(value) = document.reactors_by_handle.remove(&old) {
        document.reactors_by_handle.insert(new, value);
    }
    if let Some(value) = document.block_visibility_params.remove(&old) {
        document.block_visibility_params.insert(new, value);
    }
    if let Some(value) = document.context_scales.remove(&old) {
        document.context_scales.insert(new, value);
    }
    if let Some(value) = document.block_representations.remove(&old) {
        document.block_representations.insert(new, value);
    }
    if let Some(mut value) = document.fields.remove(&old) {
        value.handle = new;
        document.fields.insert(new, value);
    }
    document
        .raw_records
        .retain(|handle, _| *handle != old.value());
    document.original_objects.remove(&old);
}

fn known_reference(known: &HashSet<Handle>, target: Handle) -> bool {
    target.is_null() || known.contains(&target)
}

impl CadDocument {
    /// Run a read-only structural consistency audit.
    pub fn audit(&self) -> AuditReport {
        let mut report = AuditReport::default();
        let mut handles: HashMap<Handle, Vec<AuditLocation>> = HashMap::new();
        table_handles(self, &mut handles, &mut report);

        for entity in &self.entities {
            let handle = entity.common().handle;
            let location = AuditLocation::Entity { handle };
            report.scanned_records += 1;
            report.scanned_entities += 1;
            add_handle(&mut handles, &mut report, handle, location.clone());
            audit_entity(entity, location, &mut report);
            // Attributes and the SEQEND live inside their insert, but other
            // records (extension dictionaries, reactors) still name them.
            if let EntityType::Insert(insert) = entity.as_ref() {
                let nested = insert
                    .attributes
                    .iter()
                    .map(|attribute| attribute.common.handle)
                    .chain(insert.seqend_handle);
                for handle in nested.filter(|handle| !handle.is_null()) {
                    add_handle(
                        &mut handles,
                        &mut report,
                        handle,
                        AuditLocation::Entity { handle },
                    );
                }
            }
        }
        for (key, object) in &self.objects {
            report.scanned_records += 1;
            report.scanned_objects += 1;
            let location = AuditLocation::Object { handle: *key };
            add_handle(&mut handles, &mut report, *key, location.clone());
            let intrinsic = object.intrinsic_handle();
            if intrinsic != *key {
                report.issue(
                    AuditIssueCode::ObjectMapMismatch,
                    AuditSeverity::Error,
                    location.clone(),
                    Some(*key),
                    "object map key differs from intrinsic handle",
                    false,
                );
            }
            if matches!(object, ObjectType::Unknown { .. }) {
                report.issue(
                    AuditIssueCode::UnsupportedRecord,
                    AuditSeverity::Warning,
                    location,
                    Some(*key),
                    "unsupported object retained as raw record",
                    false,
                );
            }
        }

        // Table control objects live in the header, not in a table or the
        // object map; the dictionaries they own point back at them.
        let header = &self.header;
        let controls = [
            header.block_control_handle,
            header.layer_control_handle,
            header.style_control_handle,
            header.linetype_control_handle,
            header.view_control_handle,
            header.ucs_control_handle,
            header.vport_control_handle,
            header.appid_control_handle,
            header.dimstyle_control_handle,
            header.vpent_hdr_control_handle,
        ];
        let known: HashSet<Handle> = handles
            .keys()
            .copied()
            .chain(controls)
            .filter(|handle| !handle.is_null())
            .collect();
        for key in self.objects.keys().copied() {
            if let Some(owner) = self.object_owner(key) {
                if !known_reference(&known, owner) {
                    report.issue(
                        AuditIssueCode::DanglingReference,
                        AuditSeverity::Error,
                        AuditLocation::Reference {
                            owner: key,
                            target: owner,
                        },
                        Some(owner),
                        "object owner handle is dangling",
                        false,
                    );
                }
            }
        }
        for (owner, target) in &self.xdic_by_handle {
            if !known_reference(&known, *target) {
                report.issue(
                    AuditIssueCode::DanglingReference,
                    AuditSeverity::Error,
                    AuditLocation::Reference {
                        owner: *owner,
                        target: *target,
                    },
                    Some(*target),
                    "object extension dictionary is dangling",
                    false,
                );
            }
        }
        for (owner, targets) in &self.reactors_by_handle {
            for target in targets {
                if !known_reference(&known, *target) {
                    report.issue(
                        AuditIssueCode::DanglingReference,
                        AuditSeverity::Warning,
                        AuditLocation::Reference {
                            owner: *owner,
                            target: *target,
                        },
                        Some(*target),
                        "object reactor handle is dangling",
                        false,
                    );
                }
            }
        }
        let block_handles: HashSet<Handle> = self
            .block_records
            .iter()
            .map(|record| record.handle)
            .filter(|handle| !handle.is_null())
            .collect();
        let mut block_membership: HashMap<Handle, Vec<Handle>> = HashMap::new();
        for block in self.block_records.iter() {
            for child in &block.entity_handles {
                block_membership
                    .entry(*child)
                    .or_default()
                    .push(block.handle);
                if !known_reference(&known, *child) {
                    report.issue(
                        AuditIssueCode::DanglingReference,
                        AuditSeverity::Error,
                        AuditLocation::Reference {
                            owner: block.handle,
                            target: *child,
                        },
                        Some(*child),
                        "block membership points at a missing entity",
                        false,
                    );
                }
            }
        }
        // Derived entity index must point back to the exact entity slot.
        for (index_handle, index) in &self.entity_index {
            let valid = self
                .entities
                .get(*index)
                .is_some_and(|entity| entity.common().handle == *index_handle);
            if !valid {
                report.issue(
                    AuditIssueCode::EntityIndexMismatch,
                    AuditSeverity::Error,
                    AuditLocation::Entity {
                        handle: *index_handle,
                    },
                    Some(*index_handle),
                    "entity index points at a different or missing record",
                    true,
                );
            }
        }
        for (index, entity) in self.entities.iter().enumerate() {
            if self.entity_index.get(&entity.common().handle) != Some(&index) {
                report.issue(
                    AuditIssueCode::EntityIndexMismatch,
                    AuditSeverity::Error,
                    AuditLocation::Entity {
                        handle: entity.common().handle,
                    },
                    Some(entity.common().handle),
                    "entity is absent from entity_index or has a stale slot",
                    true,
                );
            }
            let common = entity.common();
            if !common.layer.is_empty() && !self.layers.contains(&common.layer) {
                report.issue(
                    AuditIssueCode::DanglingReference,
                    AuditSeverity::Error,
                    AuditLocation::Reference {
                        owner: common.handle,
                        target: common.layer_handle.unwrap_or(Handle::NULL),
                    },
                    common.layer_handle,
                    "entity layer name does not resolve to a layer table record",
                    false,
                );
            }
            if !common.linetype.is_empty()
                && !common.linetype.eq_ignore_ascii_case("BYLAYER")
                && !common.linetype.eq_ignore_ascii_case("BYBLOCK")
                && !self.line_types.contains(&common.linetype)
            {
                report.issue(
                    AuditIssueCode::DanglingReference,
                    AuditSeverity::Error,
                    AuditLocation::Reference {
                        owner: common.handle,
                        target: common.linetype_handle.unwrap_or(Handle::NULL),
                    },
                    common.linetype_handle,
                    "entity linetype name does not resolve to a linetype table record",
                    false,
                );
            }
            if !known_reference(&known, common.owner_handle) {
                report.issue(
                    AuditIssueCode::DanglingReference,
                    AuditSeverity::Error,
                    AuditLocation::Reference {
                        owner: common.handle,
                        target: common.owner_handle,
                    },
                    Some(common.owner_handle),
                    "entity owner handle is dangling",
                    false,
                );
            }
            if !common.owner_handle.is_null() && !block_handles.contains(&common.owner_handle) {
                report.issue(
                    AuditIssueCode::OwnerMismatch,
                    AuditSeverity::Warning,
                    AuditLocation::Entity {
                        handle: common.handle,
                    },
                    Some(common.owner_handle),
                    "entity owner is not a block record",
                    false,
                );
            }
            if let Some(memberships) = block_membership.get(&common.handle) {
                if memberships.len() > 1 {
                    report.issue(
                        AuditIssueCode::OwnerMismatch,
                        AuditSeverity::Error,
                        AuditLocation::Entity {
                            handle: common.handle,
                        },
                        Some(common.handle),
                        "entity is listed by multiple block records",
                        false,
                    );
                } else if let Some(block) = memberships.first() {
                    if !common.owner_handle.is_null() && common.owner_handle != *block {
                        report.issue(
                            AuditIssueCode::OwnerMismatch,
                            AuditSeverity::Error,
                            AuditLocation::Entity {
                                handle: common.handle,
                            },
                            Some(common.owner_handle),
                            "entity owner differs from its block membership",
                            false,
                        );
                    }
                }
            }
            if let Some(target) = common
                .xdictionary_handle
                .filter(|handle| !known_reference(&known, *handle))
            {
                report.issue(
                    AuditIssueCode::DanglingReference,
                    AuditSeverity::Error,
                    AuditLocation::Reference {
                        owner: common.handle,
                        target,
                    },
                    Some(target),
                    "entity extension dictionary is dangling",
                    false,
                );
            }
            for target in &common.reactors {
                if !known_reference(&known, *target) {
                    report.issue(
                        AuditIssueCode::DanglingReference,
                        AuditSeverity::Warning,
                        AuditLocation::Reference {
                            owner: common.handle,
                            target: *target,
                        },
                        Some(*target),
                        "entity reactor handle is dangling",
                        false,
                    );
                }
            }
        }

        for (key, object) in &self.objects {
            if let ObjectType::Dictionary(dictionary) = object {
                for (_, target) in &dictionary.entries {
                    if !known_reference(&known, *target) {
                        report.issue(
                            AuditIssueCode::DanglingReference,
                            AuditSeverity::Error,
                            AuditLocation::Reference {
                                owner: *key,
                                target: *target,
                            },
                            Some(*target),
                            "dictionary entry target is dangling",
                            false,
                        );
                    }
                }
            } else if let ObjectType::DictionaryWithDefault(dictionary) = object {
                for (_, target) in &dictionary.entries {
                    if !known_reference(&known, *target) {
                        report.issue(
                            AuditIssueCode::DanglingReference,
                            AuditSeverity::Error,
                            AuditLocation::Reference {
                                owner: *key,
                                target: *target,
                            },
                            Some(*target),
                            "dictionary entry target is dangling",
                            false,
                        );
                    }
                }
            }
        }

        // An object owned by a dictionary or a field must be listed by that
        // owner; CAD auditors report such orphans (e.g. AcDbField).
        for key in self.objects.keys() {
            let Some(owner) = self.object_owner(*key) else { continue };
            let listed = match self.objects.get(&owner) {
                Some(ObjectType::Dictionary(d)) => d.entries.iter().any(|(_, t)| t == key),
                Some(ObjectType::DictionaryWithDefault(d)) => {
                    d.entries.iter().any(|(_, t)| t == key)
                }
                Some(ObjectType::Field(f)) => f.child_fields.contains(key),
                _ => true,
            };
            // Reported elsewhere as "AcDbField Owner Id (0) Invalid".
            let null_owner = owner.is_null() && *key != self.header.named_objects_dict_handle;
            if !listed || null_owner {
                report.issue(
                    AuditIssueCode::OwnerMismatch,
                    AuditSeverity::Error,
                    AuditLocation::Reference { owner, target: *key },
                    Some(*key),
                    if null_owner { "object owner handle is null" } else { "object owner does not list the object" },
                    false,
                );
            }
        }

        for block in self.block_records.iter() {
            if block.flags.has_attributes && !self.block_has_attdef(block) {
                report.issue(
                    AuditIssueCode::OwnerMismatch,
                    AuditSeverity::Error,
                    AuditLocation::TableEntry {
                        table: "BLOCK_RECORD".into(),
                        name: block.name.clone(),
                        handle: block.handle,
                    },
                    Some(block.handle),
                    "block record has the attribute-definitions flag but no ATTDEF",
                    true,
                );
            }
        }

        let max_used_handle = known.iter().map(|handle| handle.value()).max().unwrap_or(0);
        if self.header.handle_seed <= max_used_handle || self.next_handle() <= max_used_handle {
            report.issue(
                AuditIssueCode::InvalidHandleSeed,
                AuditSeverity::Warning,
                AuditLocation::Document,
                None,
                "HANDSEED or allocator is not above every stored identity",
                true,
            );
        }
        if self.has_stale_table_keys() {
            report.issue(
                AuditIssueCode::StaleTableKey,
                AuditSeverity::Error,
                AuditLocation::Document,
                None,
                "one or more symbol-table keys no longer match their names",
                true,
            );
        }

        // Report owner cycles for entity and dictionary ownership edges.
        let mut owners: HashMap<Handle, Handle> = HashMap::new();
        for entity in &self.entities {
            let common = entity.common();
            if !common.owner_handle.is_null() {
                owners.insert(common.handle, common.owner_handle);
            }
        }
        for (handle, object) in &self.objects {
            let owner = match object {
                ObjectType::Dictionary(value) => value.owner,
                ObjectType::DictionaryWithDefault(value) => value.owner,
                ObjectType::Unknown { owner, .. } => *owner,
                ObjectType::Arch(value) => value.owner,
                _ => Handle::NULL,
            };
            if !owner.is_null() {
                owners.insert(*handle, owner);
            }
        }
        for start in owners.keys().copied() {
            let mut seen = HashSet::new();
            let mut current = start;
            while let Some(next) = owners.get(&current).copied() {
                if !seen.insert(current) {
                    report.issue(
                        AuditIssueCode::OwnerCycle,
                        AuditSeverity::Error,
                        AuditLocation::Object { handle: start },
                        Some(start),
                        "owner relationship contains a cycle",
                        false,
                    );
                    break;
                }
                current = next;
            }
        }
        report
    }

    fn block_has_attdef(&self, block: &crate::tables::BlockRecord) -> bool {
        block.entity_handles.iter().any(|h| {
            matches!(self.get_entity(*h), Some(EntityType::AttributeDefinition(_)))
        })
    }

    /// Apply only deterministic structural repairs and return before/after
    /// diagnostics.  Ambiguous references and duplicate handles are retained.
    pub fn repair(&mut self) -> RepairReport {
        let before = self.audit();
        let mut actions = Vec::new();
        // Synchronize before allocating replacements. Parsed objects may
        // carry handles above the allocator's cached value.
        self.synchronize_handle_allocator();
        let stale: Vec<Handle> = self
            .block_records
            .iter()
            .filter(|b| b.flags.has_attributes && !self.block_has_attdef(b))
            .map(|b| b.handle)
            .collect();
        for block in self.block_records.iter_mut() {
            if stale.contains(&block.handle) {
                block.flags.has_attributes = false;
            }
        }
        if !stale.is_empty() {
            actions.push(RepairAction::ClearAttributeDefinitionFlags { count: stale.len() });
        }
        let renamed = self.resync_table_keys();
        if renamed > 0 {
            actions.push(RepairAction::ResyncTableKeys { count: renamed });
        }
        let mut null_tables = 0usize;
        macro_rules! count_null {
            ($table:expr) => {
                null_tables += $table
                    .iter()
                    .filter(|entry| entry.handle().is_null())
                    .count();
            };
        }
        count_null!(self.layers);
        count_null!(self.line_types);
        count_null!(self.text_styles);
        count_null!(self.block_records);
        count_null!(self.dim_styles);
        count_null!(self.app_ids);
        count_null!(self.views);
        count_null!(self.vports);
        count_null!(self.ucss);
        count_null!(self.vx_table);
        if null_tables > 0 {
            self.assign_table_entry_handles();
            actions.push(RepairAction::AssignTableHandles { count: null_tables });
        }

        let null_entities: Vec<usize> = self
            .entities
            .iter()
            .enumerate()
            .filter_map(|(index, entity)| entity.common().handle.is_null().then_some(index))
            .collect();
        let needs_reindex =
            !null_entities.is_empty() || before.count(AuditIssueCode::EntityIndexMismatch) > 0;
        if !null_entities.is_empty() {
            for index in null_entities.iter().copied() {
                let handle = self.allocate_handle();
                let (owner, old_member) = {
                    let common = self.entities[index].common();
                    let owner = common.owner_handle;
                    let old_member = self
                        .block_records
                        .iter()
                        .find(|b| b.handle == owner)
                        .and_then(|b| {
                            b.entity_handles.iter().copied().find(|candidate| {
                                !self
                                    .entities
                                    .iter()
                                    .any(|e| e.common().handle == *candidate)
                            })
                        });
                    (owner, old_member)
                };
                let common = std::sync::Arc::make_mut(&mut self.entities[index]).common_mut();
                common.handle = handle;
                common.raw_record = None;
                if let Some(old_member) = old_member {
                    if let Some(block) = self.block_records.iter_mut().find(|b| b.handle == owner) {
                        if let Some(member) =
                            block.entity_handles.iter_mut().find(|m| **m == old_member)
                        {
                            *member = handle;
                        }
                    }
                }
            }
            actions.push(RepairAction::AssignEntityHandles {
                count: null_entities.len(),
            });
        }

        // A non-null object map key is an unambiguous identity for an object
        // whose decoded payload omitted its handle.  Null map keys are moved
        // to fresh handles; duplicate keys cannot exist in HashMap storage.
        let object_keys: Vec<Handle> = self.objects.keys().copied().collect();
        let mut object_actions = Vec::new();
        for old in object_keys {
            let repaired = if let Some(object) = self.objects.get_mut(&old) {
                if object.intrinsic_handle().is_null() && !old.is_null() {
                    object.set_handle(old);
                    invalidate_object_passthrough(object);
                    true
                } else {
                    false
                }
            } else {
                false
            };
            if repaired {
                migrate_object_side_data(self, old, old);
            }
        }
        let null_keys: Vec<Handle> = self
            .objects
            .keys()
            .copied()
            .filter(|handle| handle.is_null())
            .collect();
        for old in null_keys {
            if let Some(mut object) = self.objects.remove(&old) {
                let new = self.allocate_handle();
                object.set_handle(new);
                invalidate_object_passthrough(&mut object);
                migrate_object_side_data(self, old, new);
                self.objects.insert(new, object);
                object_actions.push(RepairAction::ReindexObject { old, new });
            }
        }
        actions.extend(object_actions);

        // Repair a missing owner only where exactly one block claims an entity.
        let mut membership: HashMap<Handle, Vec<Handle>> = HashMap::new();
        for block in self.block_records.iter() {
            for entity in &block.entity_handles {
                membership.entry(*entity).or_default().push(block.handle);
            }
        }
        let mut owner_repairs = 0usize;
        for entity in &mut self.entities {
            let common = std::sync::Arc::make_mut(entity).common_mut();
            if common.owner_handle.is_null() {
                if let Some(blocks) = membership
                    .get(&common.handle)
                    .filter(|blocks| blocks.len() == 1)
                {
                    common.owner_handle = blocks[0];
                    common.raw_record = None;
                    owner_repairs += 1;
                }
            }
        }
        if owner_repairs > 0 {
            actions.push(RepairAction::RepairBlockOwners {
                count: owner_repairs,
            });
        }

        self.resolve_references();

        if needs_reindex {
            self.entity_index.clear();
            for (index, entity) in self.entities.iter().enumerate() {
                if !entity.common().handle.is_null()
                    && !self.entity_index.contains_key(&entity.common().handle)
                {
                    self.entity_index.insert(entity.common().handle, index);
                }
            }
            actions.push(RepairAction::ReindexEntities {
                count: self.entity_index.len(),
            });
        }
        if before.count(AuditIssueCode::InvalidHandleSeed) > 0 || !actions.is_empty() {
            self.synchronize_handle_allocator();
            actions.push(RepairAction::SynchronizeHandleAllocator);
        }
        let after = self.audit();
        let unresolved = after.issues.clone();
        RepairReport {
            before,
            after,
            actions,
            unresolved,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::{EntityType, Line};

    #[test]
    fn new_document_audits_clean() {
        let document = CadDocument::new();
        assert!(
            document.audit().is_clean(),
            "default document issues: {:?}",
            document.audit().issues
        );
        let before = document.clone();
        let _ = document.audit();
        assert_eq!(document, before);
    }

    #[test]
    fn repair_rebuilds_an_entity_index_after_a_null_handle() {
        let mut document = CadDocument::new();
        let handle = document
            .add_entity(EntityType::Line(Line::new()))
            .expect("entity");
        document
            .get_entity_mut(handle)
            .expect("entity lookup")
            .common_mut()
            .handle = Handle::NULL;
        let report = document.repair();
        assert!(report
            .actions
            .iter()
            .any(|action| matches!(action, RepairAction::AssignEntityHandles { count: 1 })));
        assert_eq!(document.entities().count(), 1);
        let repaired_handle = document.entities().next().expect("entity").common().handle;
        assert!(!repaired_handle.is_null());
        assert_eq!(
            document
                .get_entity(repaired_handle)
                .expect("index")
                .common()
                .handle,
            repaired_handle
        );
    }

    #[test]
    fn duplicates_are_reported_without_reassigning_handles() {
        let mut document = CadDocument::new();
        let first = document
            .add_entity(EntityType::Line(Line::new()))
            .expect("first");
        let second = document
            .add_entity(EntityType::Line(Line::new()))
            .expect("second");
        document
            .get_entity_mut(second)
            .expect("second lookup")
            .common_mut()
            .handle = first;
        assert!(document.audit().count(AuditIssueCode::DuplicateHandle) > 0);
        let report = document.repair();
        assert!(report.after.count(AuditIssueCode::DuplicateHandle) > 0);
        assert!(document
            .entities()
            .all(|entity| entity.common().handle == first));
    }

    #[test]
    fn repair_resyncs_table_keys_and_is_idempotent() {
        let mut document = CadDocument::new();
        document.layers.get_mut("0").expect("default layer").name = "Renamed".into();
        assert!(document.audit().count(AuditIssueCode::StaleTableKey) > 0);
        let first = document.repair();
        assert_eq!(first.after.count(AuditIssueCode::StaleTableKey), 0);
        assert!(document.layers.get("Renamed").is_some());
        assert!(document.repair().actions.is_empty());
    }

    #[test]
    fn attdef_flag_without_attdef_is_reported_and_cleared() {
        let mut document = CadDocument::new();
        let block = document.block_records.iter_mut().next().unwrap();
        block.flags.has_attributes = true;
        assert_eq!(document.audit().error_count(), 1);
        let report = document.repair();
        assert!(report.after.is_clean());
        assert!(document.audit().is_clean());
    }

    #[test]
    fn object_with_null_owner_is_reported() {
        let mut document = CadDocument::new();
        let dict = document
            .objects
            .values_mut()
            .find_map(|o| match o {
                ObjectType::Dictionary(d) if !d.owner.is_null() => Some(d),
                _ => None,
            })
            .unwrap();
        dict.owner = Handle::NULL;
        assert!(document.audit().count(AuditIssueCode::OwnerMismatch) >= 1);
    }

    #[test]
    fn dangling_dictionary_entry_is_preserved() {
        let mut document = CadDocument::new();
        let handle = document.allocate_handle();
        let mut dictionary = crate::objects::Dictionary::new();
        dictionary.handle = handle;
        dictionary.add_entry("Missing", Handle::new(0xFFFF));
        document
            .objects
            .insert(handle, ObjectType::Dictionary(dictionary));
        let report = document.repair();
        assert!(report.after.count(AuditIssueCode::DanglingReference) > 0);
        assert!(
            matches!(document.objects.get(&handle), Some(ObjectType::Dictionary(value)) if value.get("Missing") == Some(Handle::new(0xFFFF)))
        );
    }
}
