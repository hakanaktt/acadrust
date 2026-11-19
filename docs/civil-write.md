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
  with text ids, created by, modified by, two empty texts per component
```

The collection (style tree key 35) is a point style with id 35 that lists
its styles; its default style has id 30 and no name. Written with the
application's defaults, both records equal the samples' bit for bit apart
from the dates.

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
`_All Points` is 33. A group whose id falls in bucket 0 is stored before
`_All Points`, and the application's audit then reports the group node as
not repaired (R16 with ids `0x80000100` and `0x80010000`); the application
gives no group such an id in the samples, so the writer skips those ids.
Written groups take ids from `0x80010001` up, their query the next id.

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
extents of its points (that pair when it has none). The serial is twice
the group's position in the group node. Numbers are ascending.

### Query statements (`AECC_QUERY_STATEMENT`)

Outer (owner Root, key 75):

```text
BS 0x04BE, BL 75, BL n, n × BL query text id, 9 bits 100100010
handles: n × inner query (soft owner)
strings: "", the n query titles, ""
```

The text ids are stored by hash bucket like the group ids.

Inner (owner the outer), the query `(number = v1 OR number = v2 OR ...)`
(`_All Points`: `(number > 0)`):

```text
BS 0x04BE, BL text id, BL 0, BL 0, 5 bits 01000,
BL k (number of values, byte form), 8 zero bits,
then one record per node in pre-order; each opens with BL 80 (byte form)
and 15 zero bits:
  parentheses: 3 bits (111 when its child is a leaf, 110 when it is an OR),
               byte comparison of a leaf child (1 equal, 3 greater) or 1,
               two zero bytes
  OR (k - 1, left-deep): 3 bits (100, the last 101), bytes 1, 3, 0
  leaf (one per value): 1 bit 1, BL 19 (the point number attribute),
               BL value, a zero byte except after the last leaf
strings: "Point Group Query - <name>", "", the values,
         "AeccQueryNodePARENS", "", (k - 1) × "AeccQueryNodeOR",
         per value "AeccQueryNodeEQ" (or "...GT"), "AeccQueryNodeATTRID",
         "", "", "AeccQueryNodeCONST", "", ""
```

This reproduces the `_All Points` query and the number-list queries of
WP08 (2, 4, 6) and WG01 (5; 7; 2, 4) bit for bit. Range values (`2-6`)
are stored with generated parentheses and an AND of two comparisons and are
not written.

### Edits

`set_cogo_point` and `remove_cogo_point` rewrite the database and
`_All Points` from the points read back; the group keeps its styles and
settings node. Removing a point erases its entity. Every named group whose
record has no overrides and whose query is a number list is rewritten with
its members among the new points (`add_cogo_point` too).

`add_point_group(doc, name, description, numbers)` writes a group with a
number-list query; `set_point_group` renames such a group and gives it a
new list.

## 3a. Alignments

`add_alignment(doc, name, vertices, layer)` writes an alignment of fixed
lines through the vertices (no curves), with the drawing's default
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
BL 0 (byte form), 65 zero bits, BD 2.25, 7 bits 0111010
BL 1 (first element id), BD 0, BD start station (0)
7 bits 0000011, BD 0 (track gauge), B 0, BL 1
BL n, n x {BL 257, BS 2, B 0, BS 0 (fixed), BS id, BS previous id,
           BS next id, 2RD start, 2RD end}
BL 0 x 4 (intervals), BL 0 (equations), BL 1, BL 0 (design speeds)
BD 100 (station increment), BL 0 (no superelevation), BL 0, BL 1
(centerline), BL 2, 2, 2, 0, 3, B 0, BS 0x04BE
handles: soft pointer 0, companion (hard pointer, twice), soft pointer 0,
         style (hard pointer), hard pointer 0
strings: "AeccImpAlignment", "", "", name, five empty texts
```

The key `0x0B001081` holds a value the application derives from the
geometry (the same in two samples with the same line, different for other
lines); written as 0 it is accepted (A4). The rest equals WA01 and WA02
bit for bit (elements are ids 1..n in station order, linked in a chain).

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
       (the ends are nodes 1 and 2, the inner vertices 3...)
edges: BL 2, BL 1 (slots), BL 0, BL 1 (id), BL 48 (flags), BL n, BL 0,
       n x {BL kind (0 line, 2 last), BL flags, BL node, BL face, BL face}
       (flags 2, plus 0x30 when another segment follows; last 10; faces
       1 = outside, 0 on the last vertex), BL 0, BL 0 x 3
faces: BL 2, BL 1, {BL 1, BL 1, BL 7, BL 1, BL 1, BL 0}, BL 0,
       6 x {BL 1, BL 100}, 6 x BL 0
handles: the alignment (soft pointer), three empty soft pointers
strings: name, ""
```

The default feature style gains the companion as a child (its other
fields are kept as the drawing has them). Written for a line and two
lines, the companions equal WA01's and WA02's bit for bit apart from the
text id. An alignment that refers to no companion stops the application
while it opens the drawing (A1); a companion without its topology is
refused (`Errors Found`, A6).

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

A wrong bit count in a query statement makes the application refuse the
whole drawing with a recovery prompt, so every constant run above was
taken from the records and checked bit for bit against them.

## 5. What is left

- Label styles (the label node and its collector and text styles): a
  point written into a new drawing has no label until the user gives it
  one.
- Point groups with other queries (ranges, descriptions, elevations,
  names), override styles per group, more than seven groups (the bucket
  rule is checked up to seven).
- Alignments with curves or spirals, on a site, offset alignments,
  labels; the written alignment style draws no markers (no marker style
  is written).
- Surfaces, profiles, sites, sections, corridors, pipe networks: not
  written.

## 6. Status

| kind | read by fields | write | reference check passed | rebuild flag used | blocked by |
|---|---|---|---|---|---|
| root tree node | yes | yes | yes (empty root: AUDIT 0) | — | — |
| COGO points, database, `_All Points` | yes | yes (a new drawing gets §2a) | yes (§4) | not needed (the application draws points from the database) | — |
| point groups (number lists) | yes | yes | yes (§4) | not needed (the application lists the written members) | other query kinds, override settings |
| point style, its collection, class and settings nodes | yes | yes, in a drawing without the tree (§2a) | yes (§4) | not needed | — |
| label styles | partly | no | — | — | label node settings (1621 bits), collector and text style records |
| alignments (fixed lines, siteless) | yes | yes (a new drawing gets §3b) | yes (§4) | not needed (the application solves the stored elements) | curves, spirals, sites, labels |
| TIN surfaces | yes (with points) | no | — | — | empty-surface layout, point-group definition, styles |
| profiles, views, sites, parcels, feature lines, gradings, sections, corridors, networks | partly | no | — | — | not started |
