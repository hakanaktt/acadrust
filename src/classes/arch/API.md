# Arch API

This is the calling guide for `acadrust::classes::arch` in 0.6.7.

AEC classes are exposed through `acadrust::classes::arch`. A drawing is a
`CadDocument`; its Arch entities contain
`ExtendedEntityData::Arch(ArchEntityData)`, and its Arch objects
contain `ObjectType::Arch(ArchObject)`. `ArchKind` identifies the known
entity classes. Recognizing a class does not imply support for constructing or
editing every native class field.

## File formats

The DWG reader retains opaque native class bodies, strings, and references. A
same-version DWG save preserves them while writing the entity's current common
properties, including handle, layer, color, and visibility. Cloning an entity
assigns its new identity while preserving the targets of relative references.
Writing an opaque class body into a different DWG version is unsupported.

DXF export requires native DXF codes for every Arch entity and object.
DWG-only AEC payloads produce an error before output is written. This prevents
an apparently successful export that silently loses Arch geometry.
`ArchObject::new` provides class identity only; a native body is required to
write that object to DWG.

## Creation

`wall(start, end, width, height, layer)`, `door(width, height, layer)`, and
`window(width, height, layer)` return a `Result<CadDocument, DxfError>`.
`layer` is `Option<&str>`. Dimensions must be finite and positive. A wall
baseline must be finite, nonzero, and planar.
These constructors target the AC1032 Arch layout.
They build the native entities, default styles, and supporting dictionaries from
parameters without reading or embedding a sample drawing. Doors and windows
are standalone, unanchored entities at the origin. Wall baselines lie in the
world XY plane.

Each entity is placed on the ACAD Arch layer used by the sample
drawings: walls on `A-Wall` (color 113, lineweight 0.50 mm), doors on `A-Door`
(color 31, lineweight 0.25 mm), and windows on `A-Glaz` (color 151, lineweight
0.25 mm). The layer is created when the drawing does not already have it.
Pass `None` to select that standard layer. Pass `Some(name)` to select another
layer. A known Arch layer name is created with its sample color and
lineweight. Any other name is created as a normal layer. An empty name is
rejected.

`cargo run --example architecture_create` writes horizontal, vertical, and
diagonal walls plus a door and a window under `target/architecture-created`.
It requires no input drawings.

## Decoding any class

`ArchEntityData::decode` and `ArchObject::decode` read any Arch
class. They do not need a wall, door, or window schema. Both return a
[`DecodedAec`](crate::classes::arch::DecodedAec):

| Field | Entities | Dictionary objects |
|---|---|---|
| `dxf_name`, `cpp_class_name`, `kind` | Class identity. Unknown names use `ArchKind::Other`. | Same. |
| `schema` | The `i16` in the shared tag `80, 1, 1, schema`. | Same tag. |
| `strings` | Every string in the class text stream, in write order. | Same. |
| `handles` | Class-body handles after the common DWG header. | Same. |
| `placement` | Origin and three axes when the entity tag continues into a frame. | Always absent. |
| `profiles` | Proxy-graphic polylines. | Empty. |

`schema` is `None` when the record does not open with that tag. The strings,
handles, and profiles are still returned. `placement` is set for entities that
continue with the straight-entity frame, including walls, doors, windows,
stairs, spaces, and slabs in the sample drawings. Some entities, including
walls in `INTERIOR_OBJECTS.dwg`, stop after the schema tag; their class body
is not a straight frame, so `placement` stays empty. A rigid placement has
orthonormal axes. Other frames are still reported, with `rigid` set to `false`.

The bytes after the tag are class-specific. Use the typed readers below for
those fields. `decode` does not guess them.

```rust
let decoded = data.decode();
if let Some(placement) = decoded.placement {
    let _origin = placement.origin;
}
let _schema = decoded.schema;
let _labels = decoded.strings;
```

## Geometry and Editing

`profiles` and `preview` describe cached proxy graphics. They are useful for
display but are not a complete native geometry model. `decode_profiles` and
`polyline_from_type6` decode these graphics without modifying an entity.

Native parameter readers return `None` for unsupported or inconsistent record
layouts. The corresponding setters return [`ArchEditError`](crate::classes::arch::ArchEditError)
and leave the entity and its graphics unchanged. Supported edits update the
native parameters and cached graphics together. NaN, infinity, nonpositive
sizes, and unsupported transforms are rejected.

| Reader | Setter | Native parameters |
|---|---|---|
| `straight_wall_geometry` | `set_wall` | Baseline, full thickness, base height |
| `door_size` | `set_door` | Leaf width and height |
| `window_size` | `set_window` | Opening width and height |
| `opening_size` | `set_opening` | Opening width and height |
| `stair_size` | Unsupported | Read run, width, floor-to-floor height |
| `member_size` | `set_member` | Read length, breadth, depth; edit length along the native axis |

The native layouts currently supported by the readers and setters are AC1032
layouts. A straight wall can be horizontal, vertical, or at any other angle in
the XY plane. A compact 0 or 1 in its direction grows to a full number when the
new direction needs one. Other compact fields are still rejected when a change
would require resizing an opaque bit stream.
Stair edits and structural-member cross-section changes return
[`ArchEditError`](crate::classes::arch::ArchEditError) because their dependent
native parameters and styles have not been fully decoded.

```rust
use acadrust::classes::arch::{door_size, set_door};

// `data` is the Arch payload of an ExtendedEntity.
// `graphics` is `&mut entity.common.graphic_data`.
if let Some(size) = door_size(data) {
    set_door(data, graphics, 900.0, size.height).expect("door");
}
```

`translate`, `transform_points`, and `reshape` currently accept planar
translations of supported straight walls. Other transforms require a typed
native setter. `set_profile_bounds` supports axis-aligned straight walls;
`reshape_rectangle` updates a wall baseline and width. Bounds setters for other
opaque native classes return [`ArchEditError::UnsupportedLayout`](crate::classes::arch::ArchEditError::UnsupportedLayout).

`continuous_walls` groups connected straight wall segments.
`continuous_wall_outline` builds their mitered display ribbon.
`set_continuous_wall` requires the same segment count and applies the entire
chain transactionally.

`decode_fields` refreshes the named fields after setting a raw native body. The
named fields come from supported native layouts; they are not arbitrary byte
matches or a complete schema for an unknown class.

## Display geometry

AEC proxy graphics usually store only a bounding box, so the renderer uses
measured display geometry when the class layout is understood:

| Reader | Class | Result |
|---|---|---|
| `opening_geometry` | door, window, opening | Jamb origin, wall directions, width, height, depth |
| `stair_geometry` | stair | Origin, run direction, run, width, height |
| `railing_geometry` | railing | Origin, run, railing height, post spacing |
| `curtain_wall_geometry` | curtain wall | Origin, length, height, thickness |
| `grid_geometry` | column and ceiling grid | Origin, extents, spacing |
| `member_geometry` | structural member | Origin, axis, cross-section axes and sizes |

Each reader returns `None` when the native record is not one of the measured
AC1032 layouts. The renderer then falls back to the cached proxy profiles.
Multi-view and mask block references are resolved through their
`AEC_MVBLOCK_DEF` / `AEC_MASKBLOCK*` objects to the referenced block records;
the plan representation is drawn in place of the reference bounding box.
`render` is an optional feature, so these readers are also useful to callers
that provide their own display pipeline.

## Hosted Entities

`entities_anchored_to` finds entities linked to a wall by native opening
anchors. `remove_entity(wall)` also removes its anchored openings and their
owned anchor and associative objects. Removing a door leaves its wall.

## Validation

The default Arch regression tests do not require sample drawings or an
ACAD installation. Local drawing and manually saved ACAD fixture checks
are explicitly ignored; selecting them requires their documented fixtures and
missing files fail the test.

`cargo run --example architecture` creates example drawings. The explicit
`samples` mode reads local drawings under `AEC SAMPLE DRAWINGS`. Those optional
fixtures are not part of the library's runtime contract.
