# Writing civil (`AECC_*`) records

`acadrust::classes::civil::create` writes civil objects into a drawing.
Every record is encoded from named fields; nothing is copied from another
drawing at run time. This note gives the grammar of each record the writer
produces, the objects each kind needs, and what the reference civil
application does when it opens a written drawing. The controlled samples,
their LIST / API / AUDIT dumps and the check scripts are kept outside the
repository (they are saved by the reference application and carry its
template content); the in-repository tests only use drawings written by
acadrust.

Notation: DWG bit notation (`B`, `BS`, `BL`, `BD`, `RL`, `3BD`). A record
body follows the common object (or entity) data. "Constant" means the same
value in every sample named.

## 1. Opening a written drawing: the settings and style tree

The reference application keeps a large tree under the named-objects entry
`Root` (`AECC_TREE_NODE`): settings nodes, label and object styles, class
nodes, display representations (about 3300 objects in a new drawing).

Measured:

| drawing written by acadrust | opened by the reference application |
|---|---|
| empty, no `Root` | opens; the application builds the whole tree |
| `Root` with no entries (`ensure_civil` only) | opens, AUDIT 0 errors; the application builds the whole tree under the written root |
| a point, the point database, the point group node, `_All Points` and its queries, every style, settings and class node reference null | opens without a message; the application builds its tree, but AUDIT reports the point database and `_All Points`, the point has no style and does not draw |
| the same in a drawing the application has opened once (its tree present), defaults looked up as in §3 | opens without a message, AUDIT 0 errors, both points listed with the default styles, in `_All Points`, and drawn where the application draws them; an elevation edit through the API and a save read back by acadrust give the edited value |

A written point must refer to styles that exist when the drawing is read,
but the application completes a partial tree: under every node it keeps
the children written and adds the missing ones (missing root keys, style
collections, settings nodes), so a writer only writes the part a point
refers to. Measured on drawings that copy records of a sample one by one
(kept outside the repository):

| written besides the points | opened by the reference application |
|---|---|
| point and label styles owned by the root, not in a style tree | opens; the application adds its own style tree; AUDIT reports `_All Points` (its settings node is not the application's) |
| style tree (root key 31) with only the point style collection (key 35) | the application lists the written style as the drawing's only point style and adds every other collection |
| label styles under a label node (style tree key 3) | listed as the drawing's point label styles |
| no label style at all (database, group node and points settings refer to none) | opens, AUDIT 0 errors; points have no label, the application adds its label styles |
| root settings node (root key 45) with only the points settings, which list only the create-points settings | AUDIT 0 errors; the application adds the other 88 settings nodes and the 56 other points settings |
| settings nodes written without settings (`BL 0, B 0, BL 0` after the child keys) | AUDIT 0 errors; the application fills in its defaults (it saves the full root settings record, 35648 bits) |
| root settings ending in the tree nodes' 3 bits, or points settings ending `BL 0, B 0` without the item count | refused (`Errors Found`) |
| the label node without its settings | opens with 64 AUDIT errors (its label styles take their properties from it) |

So a point needs, in a drawing without the tree, the records of §2a:
the style tree with the point style collection, the class node of COGO
points, and the root settings node with the points and create-points
settings. `add_cogo_point` writes them when the drawing has none of root
keys 31, 5 and 45; a drawing that has one of them but no point styles is
refused unchanged.

The class table entries the writer adds give only the module name as the
application name (`AeccLand130`, `AeccvBase130`); the application accepts
them.

## 2. Root tree node (`AECC_TREE_NODE`, named-objects entry `Root`)

```text
BS 0x04BE, BL 1, BL n, n × BL key, BL 0, 3 bits 010
handles (soft owner): n × node, in key order
strings: "", "Pipe Network Interference Set Collection"
owner and reactor: the named objects dictionary
```

Keys used by the writer: 32 point group node, 34 point database, 75 outer
query of `_All Points`. Other keys the application adds: 31 style tree,
45 root settings node, 5 class nodes, 362 tag manager. New entries are
appended; existing ones keep their order.

## 2a. The style, class and settings nodes of a new drawing

Every node (`AECC_TREE_NODE`, `AECC_CLASS_NODE`, `AECC_SETTINGS_NODE`,
`AECC_ROOT_SETTINGS_NODE`, `AECC_USER_DEFINED_ATTRIBUTE_CLASSIFICATION`)
has the root's layout:

```text
BS 0x04BE, BL key, BL n, n × BL child key, then
  tree and class nodes:  BL 0, 3 bits 010
  settings nodes:        BL 0, B has settings, [settings], BL items, items
handles: n × child (soft owner), then the node's own references
strings: own name, "" when the own key is a text id, the names of the
         children whose keys are text ids, then the node's own texts
```

A key with the high bit set is a text id (`0x80000000 | n`, numbered per
session; the writer uses `0x80020001` to `0x80020003`); its name is the
text. Written (`B 0`: the application fills in the settings):

| record | key | children | own references | texts |
|---|---|---|---|---|
| style tree (root 31) | 31 | 35 → point styles | — | `""` |
| class nodes (root 5) | 5 | text id → `AeccDbCogoPoint` | hard owner 0 | `""`, `AeccDbCogoPoint` |
| `AeccDbCogoPoint` | text id | `0xEA6` → classification | hard owner 0 | name, `""`, name |
| classification | `0xEA6` | — | — | `""` |
| root settings (root 45) | 45 | text id → `AeccPoints` | soft pointer 0 | `""`, `AeccPoints` |
| `AeccPoints` | text id | text id → `AeccCreatePoints` | soft pointers: default label style (none), default point style | name, `""`, `AeccCreatePoints` |
| `AeccCreatePoints` | text id | — | — | name, `""` |

The root settings record of a drawing the application built holds 89
settings children, a settings block and 715 style items of 34 bits each
(`BL 715` before them, one soft pointer each); written without settings
its body is 18 bits after the key.

### Point styles (`AECC_POINT_STYLE`)

```text
BS 0x04BE, BL id, BL n, n × BL child id, BL 0,
BL 0, B 1, RC 4, keys 0x04001008, 0x04001010 (no value),
                 0x1600101F, 0x1600101E (FILETIME: created, modified)
BL 32, 3 bits 001
8 component lists: plan (marker, label), model (marker, label),
  profile (1), section (1), four empty hatch lists. A component:
  BL index, BS 80, BS 80, BS 0, BS 80, B visible, CMC color (BS 0,
  BL 0xC1000000 ByBlock, RC 0), BS lineweight (-2 plan and model, -1
  the others), BD linetype scale 1, BL 1, BL 0, BL 0x01000000, BL 0
marker: BS 3 (cross), 2 bits 10, BD 0.1/12 (size), RC 0 (plotted units),
  44 bits the same in every point style of the samples
handles: n × child (soft owner), soft pointer 0, per component (layer
  "0", linetype ByBlock (plan, model) or ByLayer, plot style 0) as hard
  pointers, hard pointer 0
strings: own name, "" when the id is a text id, the names of the children
  with text ids, the texts of the type-4 keys (0x04001008, 0x04001010:
  the users who made and changed the style, written empty), two empty
  texts per component
```

The collection (style tree key 35) is a point style with id 35 that lists
its styles; its default style has id 30 and no name. Written with the
application's defaults, both records equal the samples' bit for bit apart
from the dates.

## 2b. Label styles of a new drawing

A new drawing gets the default label styles with its style tree: the
label node (`AECC_LABEL_NODE`, style tree key 3) with three label roots
(`AECC_LABEL_COLLECTOR_STYLE`): points (key 0x113 → collection 0x1000B40C
→ default 30 "Standard", class name `AeccDbCogoPoint`, three text
components: point number, elevation, description), sample lines (0x112 →
0x1002A00F → 30, `AeccDbSampleLine`, one text component "Sample Name")
and profiles (0x49 with its six collections, their default styles and
their text and radial line components), and the profile label set
collection (`AECC_PROFILE_LABEL_SET`, style tree key 96) with its default
set, which refers to the line, PVI, sag curve and crest curve label
styles. The points settings name the default point label style, so the
points written into the drawing take it, and sample line groups and
profile views find their label styles.

The 38 records follow the style layout of §2a (id, children, keys,
component lists, class data). They are the application's records with
the users empty and the dates the time of writing; the children and
dates are written as fields, everything between them as constant runs
(the styles' settings, not split into fields). Their references are the
other records of the tree and the drawing's layer "0", the ByBlock
linetype and the text style "Standard". Written into a new drawing the
records equal WL13's bit for bit apart from the dates (36 records walked
in parallel from the label node). A new drawing with a point, an
alignment, a profile with a parabola, a profile view and a sample line
group with two lines, all written by acadrust (E-LB2), opens with AUDIT 0
errors; the API lists the point with the point label style "Standard", the
group's lines and the view's four
label groups (E-L2 without label styles had AUDIT 2 errors).

## 2c. Section, section view and intersection styles of a new drawing

A drawing without them gets, when a writer needs them, the application's
default styles, written from templates as the label styles of §2b
(children and dates as fields, the rest constant runs, users empty):

| writer | records | style tree keys |
|---|---|---|
| `add_sections` | section style collection and default (`AECC_SECTION_STYLE`) | 0xC18 |
| `add_section_view` | section view style collection and default (`AECC_SECTION_VIEW_STYLE`, 30896 bits each), section view band set collection and default (`AECC_SECTION_VIEW_BAND_STYLE_SET`) | 0x33, 0xC0E |
| `add_intersection` | roadway marker style collection, its default and twelve marker styles named by GUID (`AECC_ROADWAYMARKER_STYLE`), intersection style collection and default (`AECC_INTERSECTION_STYLE`, each drawing one of the markers) | 0x11B, 0x1031 |
| `add_site` | parcel style collection and default (`AECC_PARCEL_STYLE`), feature line style collection and default (`AECC_FEATURELINE_STYLE`, drawing the default roadway marker; the application's collection also lists three rail styles, which are not written), and the roadway markers | 2, 128, 0x11B |

Each collection is written when the drawing lacks it; the intersection
and feature line styles refer to the roadway marker styles by their ids
in that collection. The other references are the written records and the
drawing's layer "0" and ByBlock linetype. Written into a new drawing
the records equal WX13's and WI01's bit for bit apart from the dates. A
new drawing with two crossing alignments, their profiles, the
intersection, a TIN surface, a sample line group with two lines (10/10
and 3/33), their sections and two section views, all written by acadrust
(E-DS1), opens with AUDIT 0 errors; the API lists the intersection at
(50, 0) with its quadrants and the views with the style "Standard", the
lines' offsets and the extents of their computed ranges. A new drawing
with a site, a feature line and an alignment on it and an intersection of
two siteless alignments (E-SS1) opens with AUDIT 0 errors; the API lists
the feature line with the written feature line style and LIST shows the
site's parcel style "Standard".

## 3. COGO points

### Defaults looked up in the tree

- points settings node: the `AECC_SETTINGS_NODE` whose first text is
  `AeccPoints`; its last two references are the default label style and
  the default point style;
- create-points settings node: the settings node named `AeccCreatePoints`
  (a point group refers to it);
- class node: the `AECC_CLASS_NODE` naming `AeccDbCogoPoint` (the point
  database's last reference).

A point added without a style gets the default point style and label
style as its own, as the application's add does.

### Point entity (`AECC_COGO_POINT`)

```text
common entity data (model space, layer "0" unless given, no proxy graphics)
BS 0x04BE, BL number, 6 bits 001010 (constant)
```

The application draws a point from the database and its styles; a point
written without proxy graphics opens and draws (WP01x experiments).

### Point database (`AECC_POINT_NODE`, owner Root, key 34)

```text
BS 0x04BE, BL 34, BL 0, BL 0, 3 bits 010
BL count, count × { BL 1, 3BD position, BL number, BS 0x0420 }
BL n, n × BL number          points with their own point style
BL m, m × BL number          points with their own label style
BL 0, BL 0, BL 0
BL k, k × { BL number, BL text id }
13 × BL 0
handles: n × point style (hard pointer), m × label style (hard pointer),
         class node (soft pointer)
strings: "", then the raw description of the k points in order
```

Text ids are `0x80000000 | n`; the application numbers texts per session
and accepts ids a writer numbers itself. The writer gives
`0x80000001` to the `_All Points` query and numbers distinct
descriptions from `0x80000002` (equal texts share an id).

### Point group node (`AECC_POINT_GROUP_NODE`, Root key 32)

```text
BS 0x04BE, BL 32, BL n, n × BL group id, BL 0, 3 bits 010
handles: n × group (soft owner), default label style, default point style
         (hard pointers)
strings: one text (empty in every sample), then the names of the groups
         other than `_All Points`, in stored order
```

The ids are stored by hash bucket (id mod 8), a new id last in its bucket;
`_All Points` is 33. A group in bucket 0 is stored before `_All Points`
(WQ03: 13 groups, two of them in bucket 0). Written groups take ids from
`0x80010001` up, their query the next id.

### Point groups (`AECC_POINT_PG`, owner the group node)

```text
BS 0x04BE, BL id, BL 0, BL 0 (no override lists, no keys),
BL 0 written in its one-byte form, 5 bits 00000, 6 bits 100000,
BL serial, BL count, count × RL number,
2 bits 00, BD 0, 3BD extents min, 3BD extents max,
164 bits (the same in `_All Points` and every named group of WP01, WP08,
WP10 and WG01: the group's override settings, not mapped field by field)
handles (hard pointers): label style, point style, inner query, database,
         group node, create-points settings node, 0, 0
strings: `_All Points`: "", "AeccImpAllPointsPG"
         named group: name, description, "AeccImpPG"
```

`_All Points` has id 33, serial 0, no label style, and always the extents
pair (1e20, 1e20, 1e20) / (-1e20, -1e20, -1e20); a named group stores the
extents of its points (that pair when it has none). The serial counts
groups in creation order: `_All Points` 0, the first named group 2, the
next ones 3, 4, ... (WQ03); a rewritten group keeps its serial. Two
groups with the same serial make the application's audit report the group
node as not repaired (a group in bucket 0 written with serial 0, or ten
groups numbered by their position). Numbers are ascending.

### Query statements (`AECC_QUERY_STATEMENT`)

Outer (owner Root, key 75):

```text
BS 0x04BE, BL 75, BL n, n × BL query text id, 9 bits 100100010
handles: n × inner query (soft owner)
strings: "", the n query titles, ""
```

The text ids are stored by hash bucket like the group ids.

Inner (owner the outer), the query `(attribute <comparison> c1 OR
attribute <comparison> c2 OR ...)` (`_All Points`: `(number > 0)`):

```text
BS 0x04BE, BL text id, BL 0, BL 0, 5 bits 01000,
BL k (number of constants, byte form), 7 zero bits,
then one record per node in pre-order, each opening with the attribute's
header: 3 bits value type, flags byte, 12 zero bits, 3 bits class
  parentheses: 3 bits (number attribute, 1, child is a leaf), byte
               comparison of a leaf child (1 under an OR), 15 zero bits
  OR (k - 1, left-deep): 3 bits (number attribute, 0, last), bytes 1, 3,
               7 zero bits
  leaf (one per constant): 1 bit number attribute, BL attribute id, the
               constant (point number: BL; elevation: BD; description:
               none), 7 zero bits except after the last leaf
strings: "Point Group Query - <name>", "", the constants as texts,
         "AeccQueryNodePARENS", "", (k - 1) x "AeccQueryNodeOR",
         per constant the comparison node ("AeccQueryNodeEQ", "...GT",
         "...LT", "...GTE", "...LTE"), "AeccQueryNodeATTRID", "", "",
         "AeccQueryNodeCONST", "", "", and for a description its pattern
```

| attribute | value type | flags | class | attribute id |
|---|---|---|---|---|
| point number | 1 | 0x50 | 000 | 19 |
| elevation | 2 | 0x50 | 001 | 5 |
| raw description | 3 | 0x50 | 101 | 8 |
| name | 3 | 0x10 | 101 | 8 |
| full description | 4 | 0x50 | 010 | 8 |

Comparison codes: 1 equal, 3 greater, 4 less, 5 greater or equal, 6 less
or equal. A list of description patterns (`TEST,ABC`) is an OR of equal
leaves. This reproduces the `_All Points` query, the number-list queries of
WP08 (2, 4, 6) and WG01 (5; 7; 2, 4) and the elevation (`>25`, `<12`,
`>=26`, `<=11`, `20`) and description (`TEST`, `te*`, `T?ST`, `TEST,ABC`)
queries of WQ03 bit for bit. The application matches descriptions over
the whole text, case ignored, `*` any run and `?` one character (WQ03:
`TEST` selects TEST and test, not TESTS); it stores a name query in lower
case. Number ranges (`2-6`) are stored with generated parentheses, an AND
of two comparisons and an equal leaf holding the range text; the writer
lists the numbers instead (the same members). Names and full descriptions
(which depend on point names and description keys) and exclusions are not
written.

### Edits

`set_cogo_point` and `remove_cogo_point` rewrite the database and
`_All Points` from the points read back; the group keeps its styles and
settings node. Removing a point erases its entity. Every named group whose
record has no overrides and whose query is one the writer writes is
rewritten with its members among the new points (`add_cogo_point` too).

`add_point_group_query(doc, name, description, query)` writes a group
whose query is a number list, an elevation comparison or a list of raw
description patterns (`PointQuery`); `set_point_group_query` renames such
a group and gives it a new query. `add_point_group` and `set_point_group`
take a number list.

## 3a. Alignments

`add_alignment(doc, name, vertices, layer)` writes an alignment of fixed
lines through the vertices; `add_alignment_with_curves(doc, name,
vertices, curves, layer)` adds at inner vertices a free arc of a given
radius or a free spiral-curve-spiral of given spiral lengths and radius
(`VertexCurve`), tangent to the lines before and after the vertex.
Both use the drawing's default
alignment style (the first style of the style tree's alignment style
collection, key 93). A drawing that lacks that collection or the node of
siteless alignments (root key 336) gets them (§3b); a drawing without any
tree first gets the records of §2a.

### 3b. Alignment style and siteless node of a new drawing

Written only when missing (the reference application completes the rest
of the tree as for points):

| record | layout |
|---|---|
| alignment style collection (style tree key 93) | `AECC_ALIGNMENT_STYLE` id 93 listing the default style (id 30); its own reference is the default style |
| default alignment style | `AECC_ALIGNMENT_STYLE` id 30, the style layout of §2a with 29 settings in the application's order (values 4096, 1.0, byte 0, 1.0, 0.18, 256, 1.0, 0.001, the dates; 17 keys of type 0x10 whose references, marker styles, are written empty), components 0–5, 11 (hidden) and 20 in plan with ByLayer color, the same hidden in model, 9 in section with ByBlock color; linetype ByBlock throughout (the application's style has its own dashed linetype on the hidden component 11), no class data |
| siteless node (root key 336) | `AECC_ALIGNMENT_PARCEL_NODE`, tree node layout, child key 30 → the default feature style |
| default feature style | `AECC_FEATURE_STYLE` key 30, no children, `BL 0, BL 32, BL 210`, 21 bits `000000110010000000110`, `BL 0x80000001`, an empty topology: nodes `BL 1, BL 0, BL 0`, edges `BL 1, BL 0`, faces `BL 2, BL 0, BL 0, 3 × {BL 1, BL 100}, 3 × {BL 1, BL 1}, 6 × BL 0` |

The two styles equal the samples' bit for bit apart from the dates.

### Alignment entity (`AECC_ALIGNMENT`)

```text
BL 80, BL 0, BL 80, BL 80, BL 0, BL 0x10025F04, 6 bits 010101
RC 1, key 0x0B001081: BL 2, BL 0, BL 0
BL 0 (byte form), 115 zero bits, 8 bits 00000010, RC 0x40 | edge >> 2,
2 bits edge & 3, 5 bits 11010 (edge: the alignment's edge in its
companion, 1, or in its site's container, its member number; read before
as BD 2.25 and 7 bits 0111010 for edge 1)
BL 1 (first element id), BD 0, BD start station (0)
7 bits 0000011, BD 0 (track gauge), B 0, BL 1
BL n, the lines then the arcs:
  line: BL 257, BS 2, B 0, BS 0 (fixed), BS id, BS previous id,
        BS next id, 2RD start, 2RD end (the vertices)
  arc:  BL 258, BS 2, B 0, B 0, BD -1 (no design speed), BD -1, BS 1,
        BD -1, BL 29 (fitted between two lines), B 0, BS id, BS previous
        id, BS next id, BD radius
  spiral-curve-spiral: BL 260, the arc's head with BL 0 (fitted between
        two lines) and no B, BS id, BS previous id, BS next id, BL 256
        (spiral definition), BD spiral in, BD radius, BD spiral out
BL 0 x 4 (intervals), BL 0 (equations), BL 1, BL 0 (design speeds)
BD 100 (station increment), BL 0 (no superelevation), BL 0, BL 1
(centerline), BL 2, 2, 2, 0, 3, B 0, BS 0x04BE
handles: soft pointer 0, companion (hard pointer, twice), soft pointer 0,
         style (hard pointer), hard pointer 0
strings: "AeccImpAlignment", "", "", name, five empty texts, three more
         per curve element
```

The key `0x0B001081` holds a value the application derives from the
geometry (the same in two samples with the same line, different for other
lines); written as 0 it is accepted (A4). The lines are ids 1..n in
station order, each running from vertex to vertex (the application solves
where an arc cuts them); the arcs take the ids after the lines in station
order, and the links follow the path (WA03: lines 1 and 2, arc 3 between
them, 1 -> 3 -> 2; WA04 the same with a spiral-curve-spiral). The rest
equals WA01 and WA02, and with their polylines and curves WA03 and WA04,
bit for bit.

### Companion (`AECC_FEATURE_STYLE`)

Every siteless alignment has a companion under the default feature style
of the siteless node: the alignment's site topology. It is a node record
keyed by a text id (written from `0x80030001`) whose name is 32
hexadecimal digits:

```text
BS 0x04BE, BL text id, BL 0 (no children), BL 0, BL 32, BL 210,
21 bits 000000110010000000110, BL 0x80000001,
nodes: BL n + 1 (next free id), BL n, BL 1 (first id),
       n x {3BD point (z 0), 3 bits 010, BL next id (0 last)}
       (the ends are nodes 1 and 2, the inner points 3...)
edges: BL 2, BL 1 (slots), BL 0, BL 1 (id), BL 48 (flags), BL n, BL 0,
       n x {BL kind (0 line, 1 arc, 2 last), BL flags, BL node, BL face,
            BL face, an arc: BD bulge (tan of a quarter of its signed
            angle), BL 0}
       (flags 2, plus 0x30 when another segment follows and 0x010F0000
       for an arc; last 10; faces 1 = outside, 0 on the last point),
       BL 0, BL 0 x 3
faces: BL 2, BL 1, {BL 1, BL 1, BL 7, BL 1, BL 1, BL 0}, BL 0,
       6 x {BL 1, BL 100}, 6 x BL 0
handles: the alignment (soft pointer), three empty soft pointers
strings: name, ""
```

The points are the solved path (the crate's own solver): the ends, then
the start of every line, arc and spiral chord; an arc edge carries its
bulge, a spiral is written as chords between its solved points (the
application's WA04 companion splits each spiral into ten arcs, edge flags
16; the written form opens and audits clean). The default feature style gains the
companion as a child (its other fields are kept as the drawing has them).
Written for a line and two lines, the companions equal WA01's and WA02's
bit for bit apart from the text id; for WA03 the tangent points and the
bulge differ from the application's in the last bits. An alignment that refers to no companion stops the application
while it opens the drawing (A1); a companion without its topology is
refused (`Errors Found`, A6).

## 3c. TIN surfaces

`add_tin_surface(doc, name, points, layer)` writes an `AECC_TIN_SURFACE`
whose one definition adds the points, with its triangulation, and the
drawing's default surface style (the first style of the style tree's
surface style collection, key 77). A drawing without that collection
gets it (§3d); a drawing without any tree first gets the records of §2a.

```text
BL 80, 2 bits 11, BL 0x1554C738, BL 0x087C1120, BL 5 (version)
BL 8 keys: 0x03011E29 RC 1, 0x0D011E25 BL 0, 0x02011E26 BD DBL_MAX,
  0x0B001081 BL 2, BL 0, BL 0 (geometry stamp), 0x03001093 RC 0,
  0x0201230C BD 0, 0x02011E27 BD DBL_MAX, 0x01011E28 BL 8
BL 0 (byte form), 121 zero bits, 3BD origin 0, 9 BD identity, BD 1
RC 1 item, 5 bits 01000
definitions: 2 bits 00, RL 0x80020A28 (bits 11-15: one definition),
  BL 0, BL 0, BL 0xE8A5800C, BL 40, BL 0, 3 bits 000, GUID, BL 0,
  added vertices: BL n, n x 3BD
  BL 0 (four-byte form), BL 0xAC150A00, 2 bits 00
RL -1, RS 0x04A2, RC 2, RS 0, RS 0x2A3C, "MMS01"
triangulation (tin.rs): BL vertex count, BL 0, 1, 2, 3 (the corners),
  per vertex RL flags (corner 0x8440, point 0x80), RL a triangle that
  uses it, 3 RD; BL triangle count, per triangle 3 RL vertices
  (counter-clockwise), 3 RL neighbours (across the edge that starts at
  the vertex, -1 outside), RL flags (1: uses a corner, not drawn), RL -1
7 bits 1010100, BS 0x04BE
handles: soft pointer 0, hard owner 0, hard pointer 0, style
strings: "", "", name, "", ""
```

The four corners lie 100 units outside the points' extents (WS02, WS03),
with the nearest point's elevation; the writer triangulates the points
inside them (Delaunay). Written with WS03's ten points, the drawn
triangles are the eleven of the application's own surface, and apart
from the geometry stamp (written as 0) and the triangle order the record
equals WS02's. The corners' flags of the samples carry 16 more bits
(`0xBBCC`) that are no field; the writer leaves them out.

## 3d. Surface style of a new drawing

The surface style collection (style tree key 77, `AECC_SURFACE_STYLE` id
77 listing the default style, its own reference the default style) and
the default style (id 30) use the style layout of §2a with the
application's 216 settings in stored order: 46 doubles, 44 bytes, 67
longs, 17 colors (`BS 0, BL method and value, RC 0`), 23 references
(type 0x0F, soft pointers: the eight legend table and label styles
written empty, the others the linetype "Continuous" and the text style
"Standard"), 17 texts (type 4; hatch descriptions such as
`1;0;0;100.0;0.0;2.0;ANSI31`, " " and "+") and the dates. Components:
twelve in plan (the border, index 2, drawn) and model (the triangles,
index 1, drawn) with the default colors, one in section; class data
`RC 0`. Both records equal WS02's bit for bit apart from the dates.

A key of type 4 has its value in the text stream: a style's texts are
its name, then the texts of its type-4 keys in key order (the point and
alignment styles' two are the names of the users who made and changed
the style, written empty), then two empty texts per component. A
surface style written with only the dates opens but draws nothing; one
whose texts miss the keys' texts is refused (`Errors Found`).

## 3e. Profiles

`add_profile(doc, alignment, name, pvis, curves, layer)` writes a layout
profile of fixed tangents through the PVIs (station, elevation) with a
free symmetric parabola of the given length at inner PVIs (0 for none;
crest where the grade falls); `add_profile_with_curves` takes per inner
PVI a `VerticalCurve`: none, a free symmetric parabola of a length, a free
asymmetric parabola of two lengths (before and after the PVI) or a free
circular curve of a radius. Both use the drawing's default profile style
(the first style of the style tree's profile style collection, key 74).
The alignment is written again with the profile in its list; only an
alignment whose record this crate reproduces bit for bit is accepted (its
class keys are kept as stored). A drawing without the profile style
collection gets it (§3f).

A profile adds the `AECC_PROFILE` entity and seven helper records it owns
(`AECC_DB_FLOAT_TANGENT_GRADELEN`, `AECC_DB_FIXED_PARABOLA_HLPT`,
`AECC_DB_FIXED_PARABOLA_HLPTPATSPEAK`, `AECC_ENTITYCIRCLEINFOS`,
`AECC_ENTITYEXTENDPARAM`, `AECC_DB_FIXED_ARC_HLPT`,
`AECC_NOTIFICATIONINFOS`: the version word and `BL 0`, the last with three
more bits `010`).

```text
BL 80, BL 0, BL 80, BL 80, BL 0, BL 0x10025F04, 6 bits 010101
RC 24 class keys: 14 of type 4 (texts "A", "HP", "Overall LP", "EVC",
  "BVC", "K", "EVCE", "Overall HP", "BVCE", "BREAK", "LP", "EVCS", "BVCS",
  "PVI"), 7 of type 0x12 (the helper records, in reference order), bytes
  0x0300FB05 = 0 and 0x0300FF00 = 1, the geometry stamp 0x0B001081
BL 0 (one-byte form), 63 zero bits, BD 2, 9 bits 011011010
BL 0, BD 0, BD 0, BD 30, 5 x BS 0, 10 bits 0000001000
BL n entity records, the tangents then the curves; each: 11 bits
  (2 tangent, 4 circular, 6 parabola, 8 asymmetric parabola), 23 bits
  (0x08 << 13), BL constraint, BL id, BL previous id, BL next id, then
  tangent (0):     2RD start, 2RD end, 4 zero bits
  parabola (0):    BL 1, BD length, BL 0x1001 crest / 0x1000 sag, BL 0,
                   3 bits 010, 3 x BD -1 (no minimum K)
  circular (9):    BD radius
  asymmetric (10): BD length before, BD length after the PVI
2 bits 10, the identifier (a version-4 GUID), 107 constant bits ending
  with the version word
references: the seven helpers (hard ownership), soft 0, hard 0, hard 0,
  soft 0, style (hard), alignment (hard), soft 0, hard 0, soft 0, soft 0
texts: "AeccImpVAlignment", "", "", the type-4 keys' texts, name, four
  empty texts and one more per symmetric parabola
```

The parabola's first `BL` is 1 in current records (WR02) where older
records hold 0, read before as `BD 0`; the reader now accepts both. The
records written with the PVIs and curves of WR01, WR02 and WR10 to WR13
(circular curves of radius 50 and 80, asymmetric parabolas of 8/12 and
10/6 at PVI (50, 20) between (0, 10) and (100, 15)) equal the samples' bit
for bit apart from the stamp and the identifier. A circular or asymmetric
curve's record ends with its values; the reader, which looked for a
parabola's tail after them, now takes the next record there.

The alignment lists its profiles (the oldest first in its references, soft
pointers; the newest first in its record) after its elements:

```text
BL n, BL 0, BL 0, BL n
n x {the profile's identifier, B 1, BL 32763, BL 0 x 3, BD end station
     of the alignment's first element, BL 1, BL 0, BL element count (0 for
     one element), 6 RD (0, 0, that end station, 0, 0, 0)}
then BL 0 (equations), BL 1, BL 0 (speeds) as before
texts: two more empty texts per profile
```

The identifier must be a version-4 GUID: with random bytes in its place
the application stops while it opens the drawing (E-P1, E-P3), with the
application's identifier the same records open (E-P9). A profile written
without the alignment's entry opens but is not listed under the
alignment, and is left out when the drawing is saved (E-R1). The entry's
last value is 2.8e-14 instead of 0 in the application's record for a
curved alignment (WR06); the writer writes 0.

## 3f. Profile style of a new drawing

The profile style collection (style tree key 74, `AECC_PROFILE_STYLE` id
74 listing the default style, its own reference the default style) and the
default style (id 30) use the style layout of §2a with the application's
30 settings in stored order (values 4097, byte 1, 5.0, 1.0, 1.0, 1.0, 0.2,
5.0, 258, byte 1, 5.0, 1.0, 5.0, the dates; 16 keys of type 0x10, marker
styles, written empty; key `0x12010131` of type 0x12, the style's
extension record, a hard ownership). After the keys the style has the only
pointer table of the samples' styles:

```text
BL 0, B 1, RC 1, BL 0x10010101, BL 1, B 1
references: one hard pointer (empty) after the keys' references
texts: "class AeccAttributeTable<int,class AcDbHardPointerId>" after the
       keys' texts
```

Components: 9 in model, 0-6 and 11 in profile (ByLayer, linetype scale
2), 10 in section (ByBlock); no class data. Each of the two styles owns an
extension record (`AECC_VALIGNMENT_STYLE_EXTENSION`): the style layout
with id 0, no children, the users and the dates, one profile component 12
(ByLayer), no class data. The four records equal W001's bit for bit apart
from the dates.

## 3g. Sites and alignments on a site

`add_site(doc, name, description)` writes an empty site;
`add_alignment_on_site(doc, site, name, vertices, curves, layer)` writes an
alignment like `add_alignment_with_curves` whose geometry joins the site's
container instead of a companion; `add_feature_line(doc, site, name,
points, layer)` writes a feature line of straight segments through 3D
points on a site, and `add_feature_line_vertices(doc, site, name,
vertices, layer)` one whose vertices may start arcs (a bulge each) and
be elevation points. A site is an `AECC_PARCEL` entity named
`AeccImpFace` that belongs to no other site; it owns its container (an
`AECC_FEATURE_STYLE` record) and is listed by the node of the sites (root
key 2, `AECC_PARCEL_NODE`).

```text
site:
BL 80, BL 0, BL 80, BL 80, BL 0, BL 0x10025F04, B 0, BL site text id,
3 bits 101
RC 6 keys: 0x10008018, 0x10008017 (layers), 0x0D00800A BL 0,
  0x10008016 (layer), 0x02008022 BD 0.1, 0x0200800B BD 5.0
BL 0 (one-byte form), 121 zero bits
6 x BD 0, BL 0, BL 99, BL 0, BL 0, BL 1, BL 1, BL 1, B 0, BS 0x04BE
references: the three layers (hard), soft pointer 0, the node of the sites
  (hard), the container (hard ownership), the parcel style (hard), hard
  pointer 0, the parcel style twice (hard)
texts: "AeccImpFace", "", name, description

node of the sites:
BS 0x04BE, BL 2, BL 0, BL n, n x BL site text id, BL 32, 3 bits 001
references: n x site (soft pointer), soft pointer 0, the node's own (the
  default parcel area and segment label styles, hard)
texts: "", the site names

container:
BS 0x04BE, BL 0, BL 0, BL 0, BL 32, 7 bits 0111010,
elevation points: BL next free key, BL k, k x {BL key, BL 21, BD the
  point's index among its feature line's points, 17 bits
  10000000000101010, BL key, BL 0, BL 0, BD elevation, B 0, BL 0},
  BL 1, BL 0
BL 0x80000001,
nodes: BL next free id, BL n, BL first id, n x {3BD (z the elevation),
  3 bits 010, BL next id} (ids per member: its ends, then its inner points;
  records in hash order, seven buckets by (id - 1) mod 7, the newest first)
edges: BL next free id, BL m, per member BL 0, BL id, BL flags (0x30 an
  alignment, 0x10030 a feature line), BL count,
  BL 0, count x {BL kind (0 line, 1 arc, 2 last), BL flags, BL node,
  BL 1, BL 1 (0, 0 on the last point), an arc: BD bulge, BL 0}, BL j,
  the keys of the edge's j elevation points in edge order, BL 0 x 3
faces: BL 2, BL 1 (0 without members), the outside face {BL 1, BL 1,
  BL 14 (6 once a member has an elevation point), BL m, per member {BL 1 + the points of the members before it,
  BL 0}}, BL 1, 6 x {BL 1, BL 1},
  6 x BL 0
references: hard pointer 0 per elevation point, per member its record
  (soft) and soft pointer 0, then
  (with members) the site (soft) and soft pointer 0, then the default
  feature line style (hard)
texts: ""
```

The points of a container run as one chain through its members in edge
order: a point's flags are 2 (0x400400 for an elevation point), plus 0x30
when another segment follows it
in its edge or a later one, plus 0x80 on the first point of an edge after
the first, plus 0x010F0000 for an arc; an edge's last point's are 10.

The site takes the default parcel style and feature line style: the child
with id 30 of the style tree's parcel style collection (key 2) and feature
line style collection (key 128; that collection also holds named styles).
The site text ids are written from `0x80040001`. Written for the samples'
names, the site, its container and the node of sites equal WT01's bit for
bit apart from the text id; the containers written for WT07's and WT10 to
WT13's alignments (one to four members, one to three segments each, ten
nodes) and WF01 to WF07's feature lines and alignments (in every order)
equal the samples'. The run after the site's keys is a cache the
application rewrites: once an alignment joins a site it saves other values
there (six BDs of the members' extents among them), but a site written
with the empty-site values opens with any container (E-T1).

An alignment on a site is the siteless record with the container in place
of the companion and its member number as its edge id (the samples'
records are the same bit for bit, WA09 and WA01, WT13); no companion is
written and the siteless node is not changed. With the edge id left at 1
the drawing opens, but the application no longer lists a feature line that
comes before the alignment in the container (E-F6).

A feature line is a short record; its points are its edge in the
container:

```text
BL 80, BL 0, BL 80, BL 80, BL 0, BL 0x10025F04, 6 bits 010010,
128 zero bits, BL 0, BL edge id, 7 bits 1101000, BS 0x04BE
references: soft pointer 0, the container twice (hard), soft pointer 0,
  the feature line style (hard; the one the container refers to)
texts: "AeccImpFeatureLine", "", "", name
```

Written for WF01 and WF02's points, the records equal the samples' bit for
bit.

An elevation point is a node of its feature line's edge like a PI, with
its own flags and an entry in the container's table of elevation points
(the empty table is the run earlier read as constant). The writer keys
the elevation points 1..k through the members in edge order; the
application keeps its keys and node ids when points are inserted later,
so a container it edited (WF22, WF25, WF26) is read but not rewritten. An
elevation point on an arc splits it: both parts are arc segments with
their own bulge. Written for WF20, WF21, WF24 (straight, one elevation
point at 25 with elevation 11 and 13) and WF29 (an elevation point at the
middle of an arc of bulge 0.5), the containers and feature lines equal
the samples' bit for bit. WT01 plus that feature line (E-FE1) and a new
drawing with a site and two feature lines, one with an arc of bulge 0.5,
one of -0.3 and an elevation point, the other with two elevation points
(E-FE2), open with AUDIT 0 errors; the API lists the points, the
elevation points and the bulges as written and the 2D length 164.983 of
the first. Without the hard pointers of the table the drawing is refused
(`Errors Found`).

A new
member must not cross or touch the site's members: the application splits
crossing members at their intersections and makes parcels of the closed
faces (WT09), which is not written. A site is accepted only when writing
its container from what is read gives back its record.

A drawing without the node of sites gets it with no label styles, and a
drawing without parcel or feature line styles gives the site and its
container none: the application opens such a site (E-S12, E-S13), AUDIT 0
errors, lists its alignments and keeps the site without a style.

### Parcels

`add_parcel(doc, site, vertices, layer)` writes a closed lot line (a parcel
segment) through `vertices` on a site and the parcel inside it. The lot
line is a member of the site's container like an alignment or a feature
line; its inside becomes a new face of the container's topology, and the
parcel (an `AECC_PARCEL` named `AeccImpFace` with the site as its second
reference) points at that face.

```text
lot line (AECC_PARCEL_SEGMENT):
BL 80, BL 0, BL 80, BL 80, BL 0, BL 0x10025F04, 6 bits 010010,
128 zero bits, BL 0, BL edge id, 10 bits 1101010100, BS 0x04BE
references: soft pointer 0, the container twice (hard), soft pointer 0,
  hard pointer 0
texts: "AeccImpLotLine", "", ""

parcel:
BL 80, BL 0, BL 80, BL 80, BL 0, BL 0x10025F04, B 0, BL 0, 3 bits 101,
RC 3 keys: 0x0D00800A BL 0, 0x02008022 BD 0.1, 0x0200800B BD 1/24,
BL 0, 121 zero bits,
BL number, BL 0, BL 0, BD 0, 3BD label location, BL 80, BL 1,
3BD label location, BL 2, BL 1, BL face, BL 0, B 0, BS 0x04BE
references: soft pointer 0, the site (hard), hard ownership 0, the
  parcel style (hard; the site's), the area label style (hard; the label
  node's key 2 → 0x0F00C032 → 30)
texts: "AeccImpFace", "", "" (the application names the parcel from its
  style and number, "Standard : 1")
```

In the container a closed edge has flags 0x31 and no last point: its last
point starts the segment back to the first (flags 0x22 on the last
member, 0x32 otherwise); its nodes take ids in path order. The face section
lists the outside face and one face per parcel:

```text
BL next free face id, BL count (0 without members),
outside face: BL 1, BL 1, BL flags (6 with two or more lot lines, else
  14), BL m, per member {BL its first node, BL 0}
per parcel: BL face, BL face, BL 14, BL 1, BL entry node, BL 1
BL 1, BL 1, BL p + 1, 10 x BL 1, BL p, BL 1 .. p, 5 x BL 0
references after the site's pair: per parcel {parcel (soft), soft 0}
```

The faces are numbered from 2 in member order; a lot line's segments have
its face on the inside and 1 outside (left/right as the line runs). A
counterclockwise line's entry node is its first, a clockwise one's its last.
The node records follow the application's hash order: eight buckets by id
mod 8, each in id order (WP15, WP16: 30 and 50 nodes; the earlier rule,
seven buckets newest first, gives the same order up to 14 nodes and is
replaced). The area label goes in the middle of the parcel's extents; the
application takes the parcel's face from that point, so where it falls on
the boundary (a right triangle) the writer takes the middle of the widest
run of the horizontal line through it inside the parcel (the application
itself takes another inner point there).

Samples WP03 (one counterclockwise rectangle), WP06 (clockwise), WP12 to
WP14 (two to four disjoint lot lines, one clockwise, one a triangle) and
WP17 (a clockwise right triangle) have lot lines added through the COM
interface; written for the same lines, the segments,
parcels and containers equal the samples' bit for bit, apart from the
container's edge flag 0x800, which the application sets on an edge read
from a file when another member joins. Checks: WT01 plus the four parcels
of WP14 (E-P1), a new drawing with a site and two parcels (E-P2), and WT01
with an alignment, a parcel, a feature line and a second parcel (E-P3)
open with AUDIT 0 errors; the API lists every parcel with its number, area
and extents, and LIST its boundary.

Not written: lot lines that cross or touch other members (the application
splits both at the intersection, inserting nodes flagged 0x400, and
re-faces the site: WP04, WP05, WP09 to WP11), open lot lines, lot lines
with arcs, and the order of the remaining faces after a split. A lot line
alone (no closed loop) makes no parcel; alignments that close a loop make
none either (WP01).

## 3h. Profile views

`add_profile_view(doc, alignment, name, origin, layer)` writes a profile
view of an alignment and all its profiles, its lower left corner at
`origin`, with the drawing's default profile view style (style tree key
53) and the range a new view gets: stations from 200 before the
alignment's start to 200 past its end, elevations from the profiles'
lowest to their highest rounded out to 10, with 20 more below and above
(WV01: alignment 0-100, profile 10-20, view -200-300 and -10-40; WV03:
0-300 and 2-24.5, view -200-500 and -20-50).
`add_profile_view_with_range(doc, alignment, name, origin, stations,
elevations, layer)` writes a view with a user-specified station range,
elevation range or both; with only the stations given, the automatic
elevations come from the profiles within them. The view stores a range
word twice (`0x80000420`, plus 0x1000 for user stations and 0x2000 for
user elevations); everything else follows from the range. The alignment is written
again with the view: its record's second count after the elements (`BL
n, BL v, BL 0, BL n`) is the number of views, and the views follow the
profiles in its references (soft pointers, oldest first).

The view record (`AECC_PROFILE_VIEW`, an entity in model space) is the
reader's frame (`view_frame_fields`) followed by 4841 bits of graph
settings: the graph's title and frame, its four axes (left and right
titled "Elevation", bottom and top "Station"), the range again and four
values 10. Within them only 35 doubles depend on the view, all from the
range: the width and height, their halves, the station and elevation
limits; the rest (the default style's settings, with fixed values 1/6,
1/12, -1/12, 1/48 and a title angle of 270 degrees) is the same in WV01
to WV04, so it is written as constant runs between the named values. The
references: soft 0, hard owner 0, the style, the alignment (hard), per
profile its record (soft), its style (hard) and a hard 0, then hard 0 x
4, soft 0, hard 0; texts "", "", name, "", name, "". The records written
for WV01 to WV04's origins, ranges and profiles equal theirs bit for bit,
and so do WV10 to WV13's (WR01 with a view whose stations are 10-90,
elevations 0-30, both -50-150 and 5-45, both 0-100 and -7.5-33.25, set
through the API). WR01 with the third of them written by acadrust
(E-WV12) and a new drawing with three views of one alignment (stations
50-250; elevations 0-40; stations -20-320 and elevations -5.5-30.25,
E-PR1) open with AUDIT 0 errors; the API lists the written ranges (and,
for the first, the automatic elevations of the profile between stations
50 and 250).

The application recomputes the view from its style when it opens the
drawing: a view written with an empty style (E-V2: no settings, no
components) opens with the alignment's own range and a vertical scale of
1. A drawing without the profile view style collection therefore gets the
default style: the collection (`AECC_PROFILE_VIEW_STYLE` id 53, listing
id 30) and the default style, both the style layout of §2a with the
application's 168 settings in stored order (16 references to the text
style "Standard", 16 label format codes of type 0x18, the users and the
dates) and 36 plan components (ByBlock lineweight and linetype, the
application's colors); no class data. Both equal WV01's records apart
from the dates. A view written into a new drawing with them (E-V3) opens
with AUDIT 0 errors and the API values of the application's own view:
station -200 to 300, elevations 10-20, extents (0, -100) to (500, 400).

When the drawing has a default profile label set (style tree key 96, id
30), each profile gets four label groups in the view, entities in model
space: lines (a label per tangent), PVIs (two labels: the first and the
last tangent), sag curves and crest curves (a label per curve; a curve is
a crest where the grade falls across it). Their styles are the label
set's references 1 to 4 (line, PVI, sag, crest).

```text
293-bit run (the same in the four kinds), BL n, n labels, the kind's end
label: the kind's template around the labelled entity's id (BL):
  line 186 + 126 bits, curve 86 + 126 bits, PVI at the start 44 + 238
  bits, PVI at the end 122 + 224 bits
end: line 346, PVI 410, curve 346 bits (sag and crest the same)
references: soft 0, soft 0, profile (soft), label style (hard), view
  (soft), per label {profile (hard), view (soft)}
texts: "AeccImpVAlignmentLineLabeling" (PVI..., SagCurve..., CrestCurve...),
  four empty texts
```

The groups of WV01, WV02 and WV03 (two and four line labels, none to two
curve labels) are reproduced bit for bit. With the view written into WR01
(E-V1: view, groups and the alignment equal WV01's bit for bit) the
application opens the drawing with AUDIT 0 errors and lists the view with
the same API values as its own. A new drawing gets the default label
styles with the profile label set (§2b), so its views get the four label
groups.

## 3i. Corridors

`add_corridor(doc, name, baseline, region, alignment, profile, assembly,
layer)` writes a corridor with one baseline along the alignment and its
profile and one region over the alignment's stations that applies an
assembly of the drawing every 25 units. It is written as the application
writes a corridor before its first rebuild (WC13, WC24 to WC27: the
corridor added through the API without a rebuild): out of date, without
stations, applied sections, feature lines or surfaces. Adding it changes
no other record (the alignment, the profile and the assembly stay as they
are). The corridor style is the drawing's default (style tree key 0xEB2),
the code set style the assembly's.

```text
BL 80, 0, 80, 80, 0, BL 0x10025F04, BL 0x40008353 (unbuilt), 4 bits 0101
RC 1, key 0x0B001081 (BL 2, two values; written 0)
2 bits 01, 128 zero bits, 2 bits 01, RC 82
BL 0 (items), BL 0 (surfaces), BL 0, BL 0 (outlines), BL 0
2 bits 01, RC 32, RC 0x48, RC (baselines << 3 | 2), 1 bit 0,
6 bits region count, BL 0 (stations)
region: identifier, the assembly's group block, BD start, BD end,
  2 bits 00, BD frequency along tangents (25), BD along curves (25),
  BL 0, BD 0, BD 25, BD 10, identifier, BL 0 x 3, BL 0 (targets),
  BL 0 (stations), 3 bits 110, BL 0
baseline: BL 0, B 1, 9 bits 000000101, identifier, 47 bits ending with
  the version word
references: soft 0, style, code set style, hard 0, alignment, profile,
  assembly (hard), per subassembly {subassembly (soft), soft 0}
texts: "AeccImpCorridor", "", name, "", "", per assembly group {its name,
  "", "", ""}, the assembly's name, "", the region's name, the
  baseline's name
```

The identifiers are version-4 GUIDs (new ones for every corridor). The
group block is the assembly's own (`BL 0, BL 0, BL n groups, BL 1`, then
per group its nested object heads and attachment nodes), copied from the
assembly record:

```text
BL 80, 0, 80, 80, 0, BL 0x10025F04, BL, 4 bits, RC n keys, keys
120 zero bits, 3BD location, 18 bits, identifier, group block, 36 bits
references: soft 0, hard owner 0, style, per subassembly {subassembly
  (soft), soft 0}, code set style
texts: "AeccImpAssembly", "", name, "", "", "", per group {name, "", "",
  ""}
```

Written with the samples' identifiers, stations, frequencies and
assemblies (one and two groups), the record equals theirs bit for bit
(the station range 10-90 and a tangent frequency of 10 included).

Written into WC23 (an assembly of two lanes, E-C1) and WC20 (one lane,
E-C2), the corridor opens with AUDIT 0 errors and the API lists its
baseline (alignment, profile, stations 0-100) and region (assembly,
stations 0-100) as for the application's own unbuilt corridor; the API
reports it out of date. The API rebuild run after reopening a drawing in
the check harness changes no corridor, the application's own included (a
built corridor keeps its stations after a frequency edit and a rebuild),
so the built result of a written corridor is not checked.

### Assemblies and package subassemblies

`add_assembly(doc, name, location, layer)` writes an empty assembly with
the drawing's default assembly style (style tree key 0x118) and code set
style (0xEA0); the drawing must have both (a drawing the application has
opened). `add_pkt_subassembly(doc, assembly, name, source, pkt, side,
values, layer)` adds a subassembly built from a package (§3l) as a group
of its own, attached at the assembly's location, with the package's
layout on `side` with `values`, and writes the assembly again with the
group. `source` gives the two texts that say where the package lies:
the library list (a JSON array; the host writes a path under the user's
folder as `%USERPROFILE%\\...`) and the resolved path. The package file
must hold the bytes `write_pkt` writes for `pkt`: the record keeps their
MD5 as the script's id.

```text
assembly: BL 80, 0, 80, 80, 0, BL 0x10025F04, BL 0x40000353, 4 bits 0010,
  RC 0, 120 zero bits, 3BD location, 18 bits, 16-byte identifier (a
  version-4 GUID), BL 0, BL 0, BL n groups, n x {122-bit group run, BL 0},
  BL 0, 36 bits ending with the version word
  references: soft 0, hard owner 0, assembly style (hard), per
    subassembly {subassembly, soft 0} (soft), code set style (hard)
  texts: "AeccImpAssembly", "", name, "", "", "", per group {"Group -
    (i)", "", "", ""}
package subassembly: BL 80, 0, 80, 80, 0, BL 0x10025F04, 010101, RC 4
  keys of type 4 (0x0404D01E, 0x0404D020, 0x0404D01D, 0x0404D01F; a
  stock subassembly lacks 0x0404D01D), BL 0 in its one-byte form, 121
  zero bits, 3BD origin (the assembly's location), 9 BD rotation
  (identity), BD 1, 62 bits,
  BL n points, n x {BL code count, B 1, B 0, BL 0, BD offset, BD elevation},
  BL n links, n x {BL code count, B 1, B 0, BL n, n x RL point},
  BL n shapes, n x {BL code count, B 1, B 0, BL n, n x RL link, BL 0},
  parameter set (§3i corridors): integers then doubles, each sorted by
    name; integer {0x1012, 0, key, B 0, B 1, the choices' keys, index},
    double {0x1013, 16 | 9 grade | 10 slope, BD value, B 0, B 0, index},
    index = the input's place among the package's inputs; no texts
  341 bits (the same in every package and stock subassembly seen)
  references: soft 0, hard owner 0, the assembly (soft), subassembly
    style (hard), the subassembly itself (soft), code set style (hard)
  texts: "AeccImpSubassembly", "", "", library list, "{"docs_permanent_id":
    ""}", the package version, the MD5 of the package file (upper-case
    hex), name, "", resolved path, "Subassembly.<package name>", 5 x "",
    the point, link and shape codes, per parameter {name, display name,
    "", ""} and its choices {name, display name}, ""
```

The record stores the layout only: auxiliary points and links are not
in it, targets and descriptions are not either (the parameter texts keep
the display name), and the stock record's resource module and `Version`
parameter have no counterpart. A drawing without the subassembly style
collection (style tree key 0x11A) gets the application's default,
written from templates as the styles of §2c, and one without the name
templates (root key 0x8C, the root and its node naming the groups
`Group - (<n>)`) gets them.

Samples W-K01 to W-K05: WR01 with an assembly at (0, -50) and K01 to K05
(§3l) imported and added by the application, K01 also on the left and
with width 3, depth 0.5, slope -0.05 (W-K01L, W-K01b). Written by
`add_pkt_subassembly` from the same packages, sides and values, every
subassembly record equals the sample's bit for bit and in its texts
apart from the MD5 (the samples point at the editor's file); the
assembly equals apart from its identifier.

Checks with packages written by `write_pkt` next to the drawing (a lane
built in code, right lane width 4 / depth 0.25 / slope -0.03, left lane
with the defaults, a corridor on the assembly), every drawing written by
acadrust from WR01:

| drawing | library list / path | opens | AUDIT | subassemblies | corridor rebuild |
|---|---|---|---|---|---|
| E-PK1 | `%USERPROFILE%\...` / absolute | without recovery | 0 errors | listed, up to date, parameters as written, 4 points, 4 links, 1 shape each | built from the package: stations 0-100 every 25, codes Crown, ETW, ETW_Datum, Crown_Datum, at station 0 ETW (4, -0.12) right and (-3.5, -0.07) left |
| E-PK2 | as E-PK1, package moved away | without recovery | 0 errors | listed with status "file not found", stored geometry and parameters kept | runs, builds nothing (no codes, no points) |
| E-PK3 | `E-PK3-K01W.pkt` / same (relative) | without recovery | 0 errors | "file not found" | builds nothing |
| E-PK4 | `.\E-PK4-K01W.pkt` / same | without recovery | — | "file not found" | builds nothing |
| E-PK5 | relative / absolute | without recovery | — | up to date | built as E-PK1 |
| E-PK6 | absolute (no variable) / relative | without recovery | — | up to date | built as E-PK1 |

The host resolves the package from either text when it is absolute (an
environment variable at its start expanded); a relative path is not
resolved against the drawing's folder.

## 3j. Sample lines

`add_sample_line_group(doc, alignment, name, layer)` writes an empty
sample line group of an alignment; `add_sample_line(doc, group, name,
station, left, right, layer)` adds a two-vertex sample line at a station,
reaching `left` to the left and `right` to the right of the alignment. The
alignment is written again with the group: the third count after its
elements (`BL n, BL v, BL g, BL n`) is the number of groups, and the
groups follow the profiles and views in its references. Adding a line
changes only the group (its line count and references).

```text
group (AECC_SAMPLE_LINE_GROUP, an entity in model space):
  BL 80, 0, 80, 80, 0, BL 0x10025F04, BL 0x40000353, 4 bits 0101,
  RC 1, key 0x0B001081 (BL 2, two values; written 0), a 155-bit run,
  BL line count, a 395-bit run
  references: soft 0, hard owner 0, alignment, hard 0, the lines (soft,
    by station), soft 0 x 3, sample line style, label style (hard)
  texts: "AeccImpSampleLineGroup", "", name, "", "", "0", "0", "", "", ""
line (AECC_SAMPLE_LINE):
  BL 80, 0, 80, 80, 0, BL 0x10025F04, BL 0x40008353, 4 bits 0010, RC 0,
  120 zero bits, 3BD 0, 9 BD identity, BD station,
  two vertices {BL 1, BD 0, BD offset (right positive), BD 0},
  a 100-bit run ending with the version word
  references: soft 0, hard owner 0, hard 0, soft 0, soft 0, style, group
    (hard)
  texts: "AeccImpSampleLine", "", name, ""
```

The class word of a line is 0x40008353 or 0x40000354 in the samples (the
first line of a session has the first); the writer writes the first. The
groups (no to three lines) and lines of WL10 to WL13 are reproduced bit
for bit apart from the stamp and that word, and the alignment bit for bit.
With the group and three lines written into WR00 (E-L1) the application
opens the drawing with AUDIT 0 errors and lists the group, its lines in
station order, their stations and the left, center and right vertices of
each.

The group's label style is the default sample line label style (style
tree key 3, then 0x112, 0x1002A00F, id 30). Written with no label style
(E-L2, a new drawing) the group opens but AUDIT reports its null label
style, so a drawing without the label node gets the default label
styles (§2b); a drawing without the sample line style collection (style tree key 0xCD5) gets it and its
default style (the users and dates, plan components 0 and 1, model
components 0 and 1 hidden), which the application accepts (E-L2).

### Sections

`add_sections(doc, group, surface)` makes a TIN surface a sampled section
source of a group and adds a section of it to each of the group's lines,
named "<group> - <line> - <surface>(1)", with the default section style
(style tree key 0xC18). The records are the application's before it
samples the surface (WX12: the source made sampled through the API in the
generator's session, which computes no section points); the section holds
no points until the application updates it.

```text
group: after its line count, BL k sources, k x {BL 1, a 134-bit run},
  then the rest; per source its section style and surface (soft) after
  the lines, and texts "0", "", "" after the fourth text
line: BL sections, BL section views before its closing run; the sections
  and views (soft) after its own references; its class word is kept
section (AECC_SECTION, an entity in model space):
  BL 80, 0, 80, 80, 0, BL 0x10025F04, BL 0x40008354, 4 bits 0010, RC 0,
  120 zero bits, 3BD 0, 9 BD identity, BL 0, BL 0, BL 1, BL 0, BL 0,
  a 154-bit run ending with the version word
  references: soft 0, hard owner 0, section style, sample line (hard),
    source (soft)
  texts: "AeccImpSection", "", name, ""
```

Written into WX11 (a TIN surface, a group with one line, E-X1) the
section, the line and the group equal WX12's bit for bit apart from the
group's stamp; the application opens the drawing with AUDIT 0 errors and
its API values of the section equal those of its own. Lines are not added
to a group that samples a source (the sections a new line would need are
not written).

### Section views

`add_section_view(doc, line, name, origin, layer)` writes a section view
of a sample line with the drawing's default section view style (style
tree key 0x33), its point of offset 0 at the lowest elevation at
`origin`. The range is the one the application gives a new view: the
line's offsets and the section's elevations padded by 20 and rounded out
to 10 (offsets -10..10 give -30..30, -15..25 give -40..50, -3..33 give
-30..60). A section as written holds no points, so its elevations are 0
and the view spans -20 to 20. The line is written again with the view: its
view count and the view (soft) after its sections.

```text
view (AECC_GRAPH_SECTION_VIEW, an entity in model space):
  BL 80, 2 bits 11, BL 357877593, BL 142348576, BL 5, BL 2 keys
    (0x0301D029 byte 1, 0x0D01D024 BL 0), BL 0 (byte form), 121 zero bits
  3BD origin, 9 BD identity, BL n sections, n x 5 bits 10000, BL 0, B 1,
  3BD origin, BL 4, BL 0x80000440,
  twice: 3BD (offset min, elevation min, 0), (0, elevation min, 0),
    (offset max, elevation max, 0)
  BL 528, the title (BL byte count, UTF-16: the field code
    "<[33673217(2097|...|2817)]>"), BL 0,
  7 BD (0, height, 0, 0, 1/48, 0, 1/120), BL 0, 0, 8, 1, 4, 0x80000440,
  3BD origin, BD 0, 0, 0, 1, BD 1 (vertical scale),
  graph settings: the title again, then constant runs around the offset
    minimum (6 times), maximum (6), width (2) and center (2)
  references: soft 0, hard owner 0, view style, sample line (hard), per
    section {section (soft), hard 0, hard 0}, hard 0 x 4, soft 0, hard 0
  texts: "", "", name, "", name, ""
```

The elevation values of the graph settings (-20, 20, 40) are part of the
constant runs: the samples (views at (200, 0) and (300, 100), lines with
and without a section, offsets -10..10, -15..25 and -3..33; WX13 to WX18)
all show unsampled sections. Each sample's view is reproduced bit for bit.
Written into WX12 and WX11 (E-SV-WX13, E-SV-WX17) and into WX10 with a
group, two lines (3/33 and 10/10), their sections and three views written
by acadrust (E-SV2), the drawings open with AUDIT 0 errors and the API
lists the views with the written locations, the lines' offsets as their
offset range and the extents of the computed range ((170, 0) to (260, 40)
for -30..60 at (200, 0)). Views of sampled sections, of lines with several
sections are not written. A drawing without the section or section view
styles gets the defaults (§2c).

## 3k. Intersections

`add_intersection(doc, name, main, main_profile, other, other_profile,
assembly_sets, layer)` writes the intersection object of two alignments
that cross once, on line elements and away from their ends, with the
drawing's default intersection style (style tree key 0x1031). Only the
intersection is written: the offset alignments, curb returns, corridor
regions and the profile locking actions the application's command makes
with it are not, and the record's references to them are null. The two
assembly set files the record names (the file and its form under the
content folder) are the caller's; empty texts are accepted.

```text
intersection (AECC_INTERSECTION, an entity in model space):
  BL 80, 0, 80, 80, 0, BL 0x10025F04, BL 0x4000836B, 2 bits 00, 16-byte
    id (0x80, 15 zero bytes), 2 bits 00, BL 1, BL 1, BL 2 roads, BL 1
  per road (main first): BD station of the crossing, BL 1, 1, 1, 1,
    BL arm (2, 4)
  per region (NE, SE, SW, NW): BL index 1-4, BL arm (2, 4, 3, 1), BD
    angle, B in arm forward, B out arm forward
  a 236-bit run: the secondary road grade rules (applied, maximum grade
    0.05, maximum change 0.04, distance rule 1, distance 50) and the
    version word
  references: soft 0, style (hard), per road {alignment (hard), profile,
    left offset alignment and profile, right offset alignment and
    profile (soft)}, soft 0, per region {soft 0, in alignment, out
    alignment, soft 0}
  texts: "AeccImpIntersection", "", name, "", "NE - Quadrant",
    "SE - Quadrant", "SW - Quadrant", "NW - Quadrant", the two assembly
    set files
```

The regions run counterclockwise from the main road's forward arm: NE
from it to the other road's arm on its left, SE from the other road's
arm on its right to it, SW and NW likewise from its backward arm. A
region goes from its in arm to its out arm; its angle is the difference
of their headings, each taken in [0, 2π) (`atan2` of the arm's direction:
the line element's direction, reversed for an arm against the
stationing), and its two bits say whether each arm runs along rising
stations. The record keeps no location: the application places the
intersection at the main road's station.

Samples WI01 (roads (0, 0)-(200, 0) and (100, -100)-(100, 100)), WI11
(the second road from (50, -100) to (150, 100)), WI21 ((60, -50)-(60,
150)) and WI51 ((130, 100)-(70, -100), heading the other way), made with
the application's command (corridors, curb return alignments and
profiles turned off in its settings), are reproduced bit for bit apart
from the class word. Written into those drawings without the command's
other objects (E-I1, E-I2 with empty assembly set texts, E-I-WI51) and
into W001 with two alignments, their profiles and the intersection all
written by acadrust, the main road curved before the crossing (E-I3),
the drawings open with AUDIT 0 errors; the API lists the roads with their
alignments and profiles and null offsets, the four quadrants with their
in and out alignments and the written angles, and the location (175, 75)
of E-I3 follows from the written station through the curve.

## 3l. Subassembly packages (`pkt`)

A package is a ZIP archive of deflated parts; `read_pkt` and `write_pkt`
read and write it from named fields (`PktPackage`). The samples K00 (a new
package) and K01 to K05 (a lane; with a decision and an auxiliary point;
with an offset target and a variable; with a switch on the side, a
sequence and an enumeration; with a preview image, a description and a
help file) are packages the package editor saved; each reads and writes
back to equal parts (XML parts equal as documents, other parts equal
bytes).

```text
[Content_Types].xml  BOM, Types: Default Extension per part extension
  (atc, cfg, xaml, image, help, pvd, emd, cdmd), ContentType ""
<id>.atc   the tool catalog: catalog id, the tool (name, id, image
  64 x 64 [src], [Description], ToolTip "<description>
Version: <v>" or
  "Version: <v>", Help file/command/data, StockToolRef, Data:
  GeometryGenerateMode UseDotNet, DotNetClass Assembly="<id>.xaml"
  Subassembly.<name>, Version, Params, Units m)
  Params, per input argument that is not a target, in declaration order:
  <name DataType TypeInfo DisplayName Description>value</name>
  double 16, grade 9 ([new Grade(g)]), slope 10 ([new Slope(s)]), text
  16, enumeration long 16 with <Enum><item DisplayName>key</item>...</Enum>
  (Side: Left 1, Right 0; other groups from the emd part); a Side of None
  (-1, a new package) is not listed; DisplayName and Description fall
  back to the name and the display name
<id>.cfg   CreatedWith: ProductName, Version, VersionNumber
<id>.xaml  the workflow (below)
<id>.pvd   preview cross slopes (segment, slope, defined) and cant values
<id>.emd   enumeration groups (name; items name, display name, key from
  301 up) and the names of the defined variables
<id>.cdmd  CodeData with no items (every package seen)
```

The workflow:

```text
Activity mc:Ignorable x:Class="Subassembly", this:Subassembly.<arg>=
  default per argument that has one, namespace declarations
  x:Members: Geometry, SubassemblyErrorCenter, SubassemblyRunMode
    (InOutArgument), then the package's arguments in order: x:Property
    Name Type="InArgument|OutArgument(type)", x:Property.Attributes
    (EnabledFlag2Attribute, DisplayName2Attribute, Description2Attribute)
  defaults: numbers as text, [new Grade(g)], [new Slope(s)],
    [new EnumType(key, "name", "group")], [new PreviewOffsetTarget("name",
    valid, offset)]
  Flowchart: Variables (the 17 built-in read-only enumeration values, the
    enumeration items as values, the defined variables), view state,
    StartNode (nested along the links, a node written before is an
    x:Reference), x:Reference per node in the flowchart's order
nodes: FlowStep (action, Next), FlowDecision Condition (True, False),
  FlowSwitch x:TypeArguments="x:String" Expression (Default, cases with
  x:Key)
activities: CreatePoint / CreateAuxPoint (PointNumber, Positioning,
  FromPoint, AutoLink, AutoLinkGeometryName, ApplyAOR, Arguments keyed
  <name><positioning number> (DeltaX1, DeltaY1; Slope2, DeltaX2,
  OffsetTarget2, ElevationTarget2, Superelevation2; ...), PointCodes,
  AutoLinkCodes), CreateLink / CreateAuxLink (LinkNumber, StartPoint,
  EndPoint, IsEnabled, LinkCodes), CreateShape (ShapeNumber, Links =
  ComponentNames joined by commas, ShapeCodes), Sequence (activities),
  InternalVariableDefine (VariableName, VariableType, DefaultValue,
  Variable), SetVariableValue, SetOutputParameterValue
every activity: ActivityId, DisplayName, ShowErrors, [HintSize], [view
  state]; unset references are written as {x:Null} first
code lists: scg:List InArgument(x:String) Capacity 4, doubled as needed
```

Other activities (curves, intersections, surface links, ...) are refused
by `read_pkt`. A package needs the 32-digit part name; the creator and the
workflow's namespace declarations come from the caller (a package the
editor saved has them), `PktPackage::new` gives the rest of a new
package's settings.

### Layout

`pkt::layout(pkg, side, values)` runs the flowchart in layout mode from
its start node: a step runs its activity, a decision takes the branch of
its condition, a switch the case named by its expression's text (else
the default). Parameters not given take their defaults, a target its
preview default; `Side` is the side given.

```text
DeltaXAndDeltaY  from + (s dx, dy)
SlopeAndDeltaX   from + (s dx, slope |dx|)  (the slope falls over |dx|
                 whichever way the point goes)
from = the point's FromPoint, the attachment point when unset;
s = +1 on the right, -1 on the left
AutoLink         a link from the FromPoint to the point, with its codes
```

Auxiliary points and links are evaluated (later expressions may use
them) but are not part of the geometry. Points, links and shapes come in
the order created; the codes in the order points, links, shapes.
Expressions are the editor's Visual Basic form in brackets; the subset
read: numbers, text, `True`/`False`, `+ - * / ^ &`, comparisons,
`And`/`AndAlso`/`Or`/`OrElse`/`Not`, `IF(c, a, b)`, `Math.Abs/Min/Max/
Sqrt/Sin/Cos/Tan/Atan/Round/Floor/Ceiling`, names of parameters,
variables, enumeration values and points, `.Value` of an enumeration,
`.IsValid`/`.Offset`/`.Elevation` of a target, `.X`/`.Y` of a point,
`SA.IsLayout`. Anything else (other positionings, `.ToString()`, curves)
is refused with an error.

The layouts of K01 (right; left; width 3, depth 0.5, slope -0.05) and of
K02 to K05 (width 4, depth 0.25, slope -0.03) equal the points, links,
shapes and codes the host application stored for them (W-K01, W-K01L,
W-K01b, W-K02 to W-K05) and the editor's previews.

## 4. Reference check

For every written drawing the check opens it in its own reference instance
(nothing else running), records the open dialogs, dumps the API values of
every point and group and the style collections, runs LIST and AUDIT
(no fix), saves, changes point 1's elevation through the API, saves again,
and reads both saves back with acadrust.

Points (a drawing the application had opened once, plus points 1 at
(100, 200, 10) "TEST" and 2 at (150, 250, 12) written by acadrust): no
recovery prompt, AUDIT 0 errors in 3300+ objects, the API lists both points
in the points collection and `_All Points` with numbers 1, 2, the default
point and label styles, LIST shows the primary point group and the styles,
the drawn extents equal those of a point the application makes; after the
API sets point 1's elevation to 11 and saves, acadrust reads point 1 at
(100, 200, 11) with its description and styles, point 2 unchanged.

Point groups (the same drawing with points 1 to 6, group `G` listing 2,
4, 6 and group `Q` listing 1, 3, 7): opens without a message, AUDIT 0
errors, the API lists `G` with numbers 2, 4, 6 and the query string
`(PointNumber=2 OR PointNumber=4 OR PointNumber=6)`, `Q` with 1, 3. Then
acadrust removes point 3, adds point 7 and renames `G` to `H` listing 1, 5:
AUDIT 0 errors, `H` holds 1, 5, `Q` 1, 7, `_All Points` 1, 2, 4, 5, 6, 7.

Query kinds (a new drawing with points 1 to 6 at elevations 10, 11, 20,
26, 30, 12 described TEST, test, TEXT, ABC, TE, TESTS, and ten groups
written by acadrust: descriptions `TEST`, `te*`, `T?ST`, `TEST,ABC`,
elevations `>25`, `<12`, `>=26`, `<=11`, `=20` and the numbers 2, 3, 4):
opens without a message, AUDIT 0 errors, the API gives each group the
written query string (`(RawDescription='te*')`, `(PointElevation>=26)`,
...) and the members the application gives the same queries (1, 2; 1, 2,
3, 5, 6; 1, 2; 1, 2, 4; 4, 5; 1, 2; 4, 5; 1, 2; 3; 2, 3, 4).

A new drawing (`CadDocument::new()`, never opened by the application)
with points 1 at (100, 200, 10) "TEST" and 2 at (150, 250, 12) and group
`G` listing 2, written by acadrust with the §2a records: opens without a
message, AUDIT 0 errors, the API lists the written point style as the
drawing's only point style and both points with it (no label style),
`_All Points` 1, 2 and `G` 2; both points draw a cross of the size the
application gives its own points; after the API sets point 1's elevation
to 11 and saves, acadrust reads point 1 at (100, 200, 11).

A new drawing with alignment `AL1` through (0, 0), (100, 0), (200, 50)
and point 1 written by acadrust (§2a, §3b): opens without a message, AUDIT
0 errors, the API lists the written alignment style as the drawing's only
alignment style and `AL1` with it (stations 0 to 211.803, two lines);
after the API sets the reference station to 1000 and saves, acadrust
reads the name, start station 1000 and both lines.

Alignments (WA00a and WA00b, drawings the application has opened, plus
`AL1` written by acadrust through (0, 0), (100, 0) and through (0, 0),
(100, 0), (200, 50)): AUDIT 0 errors, the API lists `AL1` as a centerline
alignment with the default style, stations 0 to 100 (0 to 211.803), one
or two line elements with the written points; after the API sets the
reference station to 1000 and saves, acadrust reads the name, start
station 1000 and both fixed lines.

Surfaces (W001, a drawing the application has opened, plus surface `S`
written by acadrust from WS02's three points and from WS03's ten): AUDIT
0 errors; the API reports 3 points and one triangle (0, 0, 0), (10, 0,
1), (0, 10, 2), and for the ten points the same general and triangle
values as WS03 (mean elevation 2.7708, 11 triangles, edge lengths 7.07 to
20, areas 25 to 100). After the API adds ten points to the three-point
surface and saves, acadrust reads the rebuilt triangulation (14
vertices, 11 drawn triangles).

Curves (new drawings with `AL1` written by acadrust through (0, 0),
(100, 0), (200, 50) with radius 169.443; through (0, 0), (100, 0),
(200, 50), (250, 150) with radii 50 and 30; and through (0, 0), (100, 0),
(200, -50), (300, -50), (350, 50) with radii 50 (a right turn), none and
40): AUDIT 0 errors; the API lists the lines and arcs with their links,
the arcs with the written radii, the tangent points the writer computes
(to 1e-13), centers on the inside of the turn (clockwise for the right
turn) and stations 0 to 210.365, 322.487 and 418.026; the first equals
WA03's alignment (stations and arc length 78.562). Spiral-curve-spirals
(`AL1` through (0, 0), (100, 0), (200, 50) with 20/50/20, as WA04; and
through (0, 0), (100, 0), (200, 50), (250, 150), (400, 150) with 20/50/20,
an arc of 30 and 15/40/25): AUDIT 0 errors, the API lists each group as
clothoid, arc, clothoid with the written lengths and radii, the first
from station 78.131 to 121.314 and ending at 211.248 as WA04.

Profiles (WR00, a drawing the application has opened with alignment AL1
of WA01, plus profile P1 written by acadrust through (0, 10), (50, 20),
(100, 15), and the same with a parabola of length 20; and W001 plus a
curved alignment and two profiles, one through (0, 10), (50, 20),
(100, 12), (150, 18) with a crest of 20 and a sag of 30, one through
(10, 5), (200, 9), all written by acadrust): AUDIT 0 errors; the API lists
the profiles under the alignment with their tangents and parabolas at the
stations the application gives them (tangents 0-40, 60-100 and the
parabola 40-60 for the first pair; 0-40, 60-85, 115-150 and the curves
40-60, 85-115 for the second) and the PVIs with their curve kinds. A new
drawing with alignment `AL1` (0, 0)-(300, 0), profile `P1` through (0,
10), (50, 20), (100, 15), (150, 25), (250, 18) with a circular curve of
radius 50, an asymmetric parabola of 8/12 and a symmetric parabola of 20,
and `P2` through (0, 5), (120, 9), (300, 2) with a circular curve of
radius 800: AUDIT 0 errors; the API lists the circular curve from station
42.664 to 57.445, the asymmetric parabola from 92 to 112, the parabola
from 140 to 160 and `P2`'s circular curve from 91.127 to 148.867, the PVIs
with their curve kinds.

A new drawing with alignment `AL1` through (0, 0), (100, 0) and profile
`P1` through (0, 10), (50, 20), (100, 15), all written by acadrust (§2a,
§3b, §3f): opens without a message, AUDIT 0 errors, the API lists `P1`
under `AL1` with the written profile style and its two tangents and PVIs.

Sites (WA00 and W001, drawings the application has opened, and new
drawings): WA00 plus sites `Site 1` and `Second` written by acadrust, an
alignment on `Site 1` through (0, 0), (100, 0), (200, 50) with an arc of
radius 80 and one through (0, 50), (100, 80); W001 plus a site with four
alignments of two lines each (twelve nodes); a new drawing with two sites,
the same two alignments on the first and a profile with a parabola on the
curved one; WT01 (the application's site) plus four alignments written on
it: no message, AUDIT 0 errors; the API lists the sites with their names
and their alignments in the written order, the alignments with their
lines and arcs (the arc of radius 80 from station 81.115 to 118.206), and
the profile under its alignment with its tangents and parabola. Feature
lines (WT01 plus FL1 through (0, 0, 10), (50, 0, 12), (100, 30, 11) and FL2
through (0, 50, 5), (100, 50, 6); the same with an alignment of three lines
between them; a new drawing with a site, FL1, an alignment of two lines and
FL2): AUDIT 0 errors; the API lists both feature lines on the site with
their points and elevations and their 2D lengths 108.310 and 100.

A new drawing with surface `S` from WS03's ten points, alignment `AL1`
through (0, 0), (20, 20) and point 1 at (5, 15, 2), all written by
acadrust (§2a, §3b, §3d): opens without a message, AUDIT 0 errors, the
API lists the three written styles as the drawing's only point,
alignment and surface styles, the surface with WS03's eleven triangles,
and the drawing shows the surface border, the alignment and the point.

A wrong bit count in a query statement makes the application refuse the
whole drawing with a recovery prompt, so every constant run above was
taken from the records and checked bit for bit against them.

## 5. What is left

- Label styles other than the defaults of §2b, and label roots other
  than points, sample lines and profiles in a new drawing.
- Point groups by name or full description, with exclusions, AND of
  several criteria, elevation ranges, override styles per group.
- Alignments with lone spirals or other curve constraints, on a site,
  offset alignments, labels, design speeds; the written alignment style draws no markers (no marker style
  is written).
- Surfaces from point groups, breaklines, boundaries or edits.
- Profiles with other curve constraints (fixed or floating curves, curves
  by K or through a point), profiles of alignments the application has
  changed.
- Crossing members on a site (their splits and faces), open lot lines and
  lot lines with arcs, parcel and feature line styles other than the
  defaults;
  sections, corridors, pipe networks: not written.
- Profile views with a vertical scale or style other than the default;
  the graph settings between the named values are constant runs.

## 6. Status

| kind | read by fields | write | reference check passed | rebuild flag used | blocked by |
|---|---|---|---|---|---|
| root tree node | yes | yes | yes (empty root: AUDIT 0) | — | — |
| COGO points, database, `_All Points` | yes | yes (a new drawing gets §2a) | yes (§4) | not needed (the application draws points from the database) | — |
| point groups (number lists, elevation comparisons, description patterns) | yes | yes | yes (§4) | not needed (the application lists the written members) | names, full descriptions, exclusions, override settings |
| point style, its collection, class and settings nodes | yes | yes, in a drawing without the tree (§2a) | yes (§4) | not needed | — |
| label styles: label node, point, sample line and profile label roots and default styles, profile label set | partly | yes, the defaults in a new drawing (§2b) | yes (§2b) | not needed | other label roots, styles other than the defaults (the settings are constant runs) |
| alignments (fixed lines, free arcs and spiral-curve-spirals, siteless) | yes | yes (a new drawing gets §3b) | yes (§4) | not needed (the application solves the stored elements) | other constraints, sites, labels |
| TIN surfaces (added points) | yes | yes (a new drawing gets §3d) | yes (§4) | not needed (the stored triangulation is drawn; the application rebuilds it after an edit) | other definitions |
| layout profiles (fixed tangents; free symmetric and asymmetric parabolas, free circular curves) | yes | yes (a new drawing gets §3f) | yes (§4) | not needed (the application solves the stored entities) | fixed and floating curves |
| sites, alignments and feature lines (arcs, elevation points) on a site | yes | yes (a new drawing gets the node of sites and the default parcel and feature line styles, §2c) | yes (§4) | not needed (the application rewrites the site's cache) | crossing members; containers the application edited after inserting elevation points |
| parcels inside closed lot lines | yes (number, face, boundary) | yes (§3g) | yes (§3g) | not needed | crossing or touching members, open lot lines, arcs |
| profile views (automatic or user ranges) and their profile label groups | yes (frame) | yes (a new drawing gets the default view style, §3h) | yes (§3h) | not needed (the application recomputes the view from its style) | other styles and vertical scales |
| assemblies and package subassemblies | partly (corridor's group block) | yes, in a drawing with assembly and code set styles (§3i) | yes (§3i) | not needed | the code set and assembly styles of a new drawing |
| corridors (one baseline, one region, unbuilt) | yes | yes, with an assembly of the drawing (§3i) | yes (§3i) | the corridor is written out of date and built by the application's rebuild | assemblies and subassemblies, built results, surfaces, several regions or baselines |
| sample line groups and two-vertex sample lines | yes | yes, with the drawing's sample line label style (§3j) | yes (§3j) | not needed | lines with more vertices |
| sections of a TIN surface (unsampled) | yes (geometry once sampled) | yes (§3j) | yes (§3j) | the application samples the source | corridor sources, lines added after sampling |
| section views of a line (default style, unsampled section) | yes (frame) | yes, with the drawing's section view style or the default one (§3j, §2c) | yes (§3j, §2c) | not needed | sampled sections, several sections |
| intersections of two crossing roads (the object only) | yes (stations, angles) | yes, with the drawing's intersection style or the default one (§3k, §2c) | yes (§3k, §2c) | not needed | offset alignments, curb returns, corridor regions and locking actions; tee junctions |
| gradings, networks | partly | no | — | — | not started |
