//! Shard planning for [`DwgReader::read_sharded`](crate::DwgReader::read_sharded).
//!
//! A sharded read splits a drawing's model-space entities into groups of
//! bounded size. Each shard is decoded as its own document from the same
//! object section: every shard decodes the non-entity records (header
//! objects, tables, block records, dictionaries and other objects) and the
//! `BLOCK`/`ENDBLK` markers, plus its own group of entity records.

use foldhash::{HashMap, HashMapExt, HashSet, HashSetExt};
use std::sync::Arc;

use crate::document::CadDocument;
use crate::io::dwg::dwg_stream_readers::handle_reader::HandleMap;
use crate::io::dwg::dwg_stream_readers::object_reader::common::{
    is_entity_type, OBJ_BLOCK, OBJ_BLOCK_HEADER, OBJ_ENDBLK,
};
use crate::io::dwg::dwg_stream_readers::object_reader::{tables, DwgObjectReader};
use crate::io::dwg::parallel::map_slice;

/// Owner chains longer than this are treated as broken and not followed.
const MAX_OWNER_DEPTH: usize = 8;

/// The records of one shard, as (handle, offset) pairs.
type ShardRecords = Vec<(u64, i64)>;

/// State kept by a sharded read between shards.
pub(crate) struct ShardState {
    /// Most entity records in one shard.
    pub(crate) max_entities: usize,
    /// The decompressed object section and the file's handle count, kept
    /// once the first shard has read them.
    pub(crate) objects: Option<(Arc<Vec<u8>>, usize)>,
    /// Records (handle → offset) every shard decodes.
    pub(crate) shared: HandleMap,
    /// Each shard's own records, taken as the shard is read.
    pub(crate) plan: Vec<ShardRecords>,
    /// The shard being read.
    pub(crate) index: usize,
}

impl ShardState {
    pub(crate) fn new(max_entities: usize) -> Self {
        Self {
            max_entities,
            objects: None,
            shared: HandleMap::default(),
            plan: Vec::new(),
            index: 0,
        }
    }

    /// The number of shards, once the first one has been read.
    pub(crate) fn count(&self) -> usize {
        self.plan.len()
    }

    /// The object section, handle count and handle map of the current shard,
    /// when an earlier shard has already read the section and planned the
    /// shards.
    pub(crate) fn resume(&mut self) -> Option<(Arc<Vec<u8>>, usize, HandleMap)> {
        let (objects, handle_count) = self.objects.clone()?;
        Some((objects, handle_count, self.handle_map()))
    }

    /// Plan the shards from the first shard's reader, which maps every
    /// handle, and narrow that reader to the first shard's records.
    pub(crate) fn plan_first(
        &mut self,
        reader: &mut DwgObjectReader,
        objects: Arc<Vec<u8>>,
        handle_count: usize,
        document: &CadDocument,
    ) {
        let (shared, plan) = plan_shards(reader, document, self.max_entities);
        self.objects = Some((objects, handle_count));
        self.shared = shared;
        self.plan = plan;
        if self.plan.len() > 1 {
            reader.set_handle_map(self.handle_map());
        }
    }

    /// The shared records and the current shard's own records.
    fn handle_map(&mut self) -> HandleMap {
        let mut handle_map = self.shared.clone();
        if let Some(own) = self.plan.get_mut(self.index) {
            handle_map.extend(std::mem::take(own));
        }
        handle_map
    }
}

/// What planning needs to know about one record.
enum RecordKind {
    /// Decoded by every shard.
    Shared,
    /// A block record at this offset; decoded by every shard.
    BlockHeader(i64),
    /// An entity, with its common data when that reads.
    Entity(Option<EntityPlacement>),
}

/// Where an entity record sits in the drawing.
struct EntityPlacement {
    in_model_space: bool,
    /// The owner when the record names it explicitly (entity mode 0).
    owner: Option<u64>,
}

/// Split the records of the file into shards.
///
/// Only model space is split. Its entities go in the order of the model
/// space block record's entity list, at most `max_entities` records a
/// shard, each together with its child records (attributes, vertices, faces
/// and sequence ends). An entity whose child records alone exceed the limit
/// gets a shard of its own. Every non-entity record and every `BLOCK` and
/// `ENDBLK` is in every shard; the remaining entities (block contents, paper
/// space, unreadable records) are in the first shard only, so the first
/// shard carries every block definition and layout.
///
/// Returns the records every shard decodes and each shard's own records.
/// There is one shard holding every record when model space fits in one, or
/// when the file has no block entity lists (before R2004).
fn plan_shards(
    reader: &DwgObjectReader,
    document: &CadDocument,
    max_entities: usize,
) -> (HandleMap, Vec<ShardRecords>) {
    let mut handles = reader.handles();
    handles.sort_unstable();
    let with_offsets = |handles: &[u64]| -> ShardRecords {
        handles
            .iter()
            .filter_map(|&handle| reader.offset_for(handle).map(|offset| (handle, offset)))
            .collect()
    };
    let whole = |handles: &[u64]| (HandleMap::default(), vec![with_offsets(handles)]);
    if !reader.version().r2004_plus() {
        return whole(&handles);
    }

    let class_is_entity: HashMap<i16, bool> = document
        .classes
        .iter()
        .filter(|class| class.class_number >= 500)
        .map(|class| (class.class_number, class.is_an_entity))
        .collect();
    let kinds = map_slice(&handles, |&handle| {
        classify(reader, &class_is_entity, handle)
    });

    let mut shared = Vec::new();
    let mut first = Vec::new();
    let mut model = HashSet::new();
    let mut entities = HashSet::new();
    let mut owned = Vec::new();
    let mut model_list = None;
    for (&handle, kind) in handles.iter().zip(kinds) {
        match kind {
            RecordKind::Shared => shared.push(handle),
            RecordKind::BlockHeader(offset) => {
                if model_list.is_none() {
                    model_list = model_space_entity_list(reader, offset);
                }
                shared.push(handle);
            }
            RecordKind::Entity(placement) => {
                entities.insert(handle);
                match placement {
                    Some(EntityPlacement {
                        in_model_space: true,
                        ..
                    }) => {
                        model.insert(handle);
                    }
                    Some(EntityPlacement {
                        owner: Some(owner), ..
                    }) => owned.push((handle, owner)),
                    _ => first.push(handle),
                }
            }
        }
    }
    let Some(model_list) = model_list else {
        return whole(&handles);
    };

    // A child record goes with the model-space entity at the top of its
    // owner chain; anything else owned (block contents) stays in the first
    // shard.
    let owner_of: HashMap<u64, u64> = owned.iter().copied().collect();
    let mut children: HashMap<u64, Vec<u64>> = HashMap::new();
    for (handle, owner) in owned {
        let mut top = owner;
        for _ in 0..MAX_OWNER_DEPTH {
            match owner_of.get(&top) {
                Some(&up) => top = up,
                None => break,
            }
        }
        if entities.contains(&owner) && model.contains(&top) {
            children.entry(top).or_default().push(handle);
        } else {
            first.push(handle);
        }
    }

    let mut order = Vec::with_capacity(model.len());
    let mut placed = HashSet::with_capacity(model.len());
    for handle in model_list {
        if model.contains(&handle) && placed.insert(handle) {
            order.push(handle);
        }
    }
    let mut unlisted: Vec<u64> = model
        .iter()
        .copied()
        .filter(|handle| !placed.contains(handle))
        .collect();
    unlisted.sort_unstable();
    order.extend(unlisted);

    let max_entities = max_entities.max(1);
    let mut groups: Vec<Vec<u64>> = vec![Vec::new()];
    let mut group_size = 0;
    for handle in order {
        let own = children.remove(&handle).unwrap_or_default();
        if group_size > 0 && group_size + 1 + own.len() > max_entities {
            groups.push(Vec::new());
            group_size = 0;
        }
        group_size += 1 + own.len();
        if let Some(group) = groups.last_mut() {
            group.push(handle);
            group.extend(own);
        }
    }
    if groups.len() == 1 {
        return whole(&handles);
    }

    let shared = with_offsets(&shared).into_iter().collect();
    let plan = groups
        .into_iter()
        .enumerate()
        .map(|(index, mut group)| {
            if index == 0 {
                group.extend_from_slice(&first);
            }
            with_offsets(&group)
        })
        .collect();
    (shared, plan)
}

fn classify(
    reader: &DwgObjectReader,
    class_is_entity: &HashMap<i16, bool>,
    handle: u64,
) -> RecordKind {
    let Some(offset) = reader.offset_for(handle).filter(|&offset| offset >= 0) else {
        return RecordKind::Shared;
    };
    let Ok(type_code) = reader.type_code_at(offset as usize) else {
        return RecordKind::Shared;
    };
    if type_code == OBJ_BLOCK_HEADER {
        return RecordKind::BlockHeader(offset);
    }
    let is_entity = if type_code >= 500 {
        class_is_entity.get(&type_code).copied().unwrap_or(false)
    } else {
        is_entity_type(type_code)
    };
    if !is_entity || type_code == OBJ_BLOCK || type_code == OBJ_ENDBLK {
        return RecordKind::Shared;
    }
    // Corrupt records are tolerated here as they are by the document
    // builder: the record is kept, and the builder reports it.
    let placement = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let (type_code, mut record) = reader.read_record_at(offset as usize).ok()?;
        let common = reader.read_common_entity_data(&mut record, type_code);
        Some(EntityPlacement {
            in_model_space: common.entity_mode == 2,
            owner: (common.entity_mode == 0 && common.owner_handle != 0)
                .then_some(common.owner_handle),
        })
    }));
    RecordKind::Entity(placement.ok().flatten())
}

/// The entity list of the block record at `offset`, if it is model space's.
fn model_space_entity_list(reader: &DwgObjectReader, offset: i64) -> Option<Vec<u64>> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let (type_code, mut record) = reader.read_record_at(offset as usize).ok()?;
        reader.read_common_non_entity_data(&mut record, type_code);
        let block = tables::read_block_header(&mut record, reader.version());
        block
            .name
            .eq_ignore_ascii_case("*Model_Space")
            .then_some(block.entity_handles)
    }))
    .ok()
    .flatten()
}
