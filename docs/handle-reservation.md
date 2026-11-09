# Checked handle reservation

`CadDocument::try_reserve_handles(count)` reserves a contiguous tail range after a
fresh, read-only inventory of current record identities and protected CAD handle
references. Missing targets are protected too: an unrelated new record must not
accidentally satisfy an existing dangling reference.

```rust
use acadrust::CadDocument;
let mut document = CadDocument::new();
// Budget all new records, including shared dependencies and required scaffold.
let reserved = document.try_reserve_handles(3)?;
assert_eq!(reserved.len(), 3);
let first = reserved.get(0).unwrap();
assert!(!first.is_null());
# Ok::<(), acadrust::HandleReservationError>(())
```

The range is owned, must-use and lazy (`len`, `is_empty`, `get`, owned
`IntoIterator` with `ExactSizeIterator`). There is no vector sized by count and
no storage/Serde contract. Dropping a reservation does not release its numbers.
The caller still has to insert coherent records with those explicit identities.
Arbitrary later public mutations are not certified by an earlier reservation.

A zero count is a total no-op, without scanning or certifying the document.
For positive counts, all arithmetic is checked:

```text
floor = max(1, private next_handle, header.handle_seed, protected_max + 1)
end = floor + checked_u64(count)
reserved = [floor, end)
```

NULL and u64::MAX are never issued. End equal to MAX is representable: the last
issued identity is MAX-1, and the next positive checked request fails. Existing
floors never decrease. There is no recycling of lower holes. On success only the
private allocator and HANDSEED change, both to end. Every error leaves the entire
document, raw storage, notifications and active entity history untouched.

Errors distinguish `InconsistentDocument` (identity/index/alias incoherence),
`UninspectableData` (unqualified carrier), and `Exhausted`. Structural/coverage
errors carry an audit location and field path; identity errors also identify the
handle and conflicting location when available. Error order is unspecified when
multiple independent defects exist; successful ranges are map-order independent.

## Coverage

The initial scope is documents assembled in memory from the supported typed
records, such as constructor defaults and native IFCCAD conversion output.
File readers often retain raw records, EED, or replay payloads that cause
`UninspectableData`. Expect many documents loaded directly from DWG or DXF to
be refused. Eligibility depends on the contents of the document at the time of
reservation. See the [field manifest](handle-reservation-fields.md).

Entities: Point, Line, Circle, Arc, Ellipse, Polyline, Polyline2D, Polyline3D,
LwPolyline, Text, MText, all nine Dimensions, Hatch, Solid, Insert, Block,
BlockEnd, Viewport, single-line AttributeDefinition/AttributeEntity, MLine and
Seqend. Objects: Dictionary, Layout, XRecord, MLineStyle, PlotSettings,
MultiLeaderStyle, TableStyle, Scale, ObjectContextData, SortEntitiesTable,
DictionaryVariable, PlaceHolder, DictionaryWithDefault and only the
DimensionAssociation associative payload. Contexts: AnnotScale, BlkRef, Text,
MText, Dim, HatchScale and HatchView. Other variants are explicitly refused.

All ten tables and all typed header handles are visited. Identities are globally
unique, assigned primary IDs are non-NULL, object keys match intrinsic IDs,
entity indices agree in both directions, and normalized table keys agree.
Header/control aliases and matching BlockRecord/flat BLOCK/ENDBLK aliases are
checked as identities of the same record. Assigned inline attributes, vertices
and SEQEND IDs are primary identities; their unassigned constructor NULLs are
allowed. Repeated references and undo memberships are protected, not extra records.

Private side maps are scanned independently, including orphan owners and dormant
references: xdictionary/reactor maps, block children, VX order, annotation scales,
block representations, view graphs, R12 keys, raw-owner links and ACDS fingerprint
handles. Sort ranks, subentity indices, hashes, class numbers and ordinary literal
strings/numbers are not database identities. Names remain D3 binding concerns.

Typed XDATA Handle values and both XRecord reference representations are visited.
No scan of arbitrary strings, application-defined binary values or every integer
is claimed. Recognized field expressions (`%<`) in text/attribute/dimension text
and MultiLeaderStyle.default_text are conservatively refused, including literal
uses of that marker. Embedded/multiline attributes are outside this version.

Raw entity/graphics/R12 backings, DS flags, raw EED (even with a decoded duplicate),
verbatim/original record stores, raw ACDS/R12 header, dynamic fields/DGN/visibility
side stores and section-view styles are refused without being discarded. Typed
XRecords require complete entries, empty raw_data and absent raw_dwg_data.
Raw Layout plot codes, TableStyle replay and SortEnts replay are likewise refused.

## Boundaries

This method does not repair indices, resolve references, validate geometry or
prove semantic binding compatibility. It follows storage, not graph edges, so
cycles do not trigger recursive graph traversal. Public allocators, insertion,
readers, writers, audit and repair keep their existing behavior.

The old infallible allocator can still panic at exhaustion. A current host must
reject a candidate whose next floor is MAX until it supports fallible allocation.
Reserve the complete reconstruction closure/scaffold explicitly. Implicit future
writer/editor allocations are not certified by this operation; qualify actual
production readback separately. Resource names can also have dangling references:
a free record name alone is not proof that creation cannot rebind existing content.

No IFCCAD format change, DocumentSelection storage/reconstruction API or OCS
blocked-content retention implementation is provided by this numeric primitive.
