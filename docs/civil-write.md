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

## 2c. Section, section view, intersection, site, assembly and corridor styles of a new drawing

A drawing without them gets, when a writer needs them, the application's
default styles, written from templates as the label styles of §2b
(children and dates as fields, the rest constant runs, users empty):

| writer | records | style tree keys |
|---|---|---|
| `add_sections` | section style collection and default (`AECC_SECTION_STYLE`) | 0xC18 |
| `add_section_view` | section view style collection and default (`AECC_SECTION_VIEW_STYLE`, 30896 bits each), section view band set collection and default (`AECC_SECTION_VIEW_BAND_STYLE_SET`) | 0x33, 0xC0E |
| `add_intersection` | roadway marker style collection, its default and twelve marker styles named by GUID (`AECC_ROADWAYMARKER_STYLE`), intersection style collection and default (`AECC_INTERSECTION_STYLE`, each drawing one of the markers) | 0x11B, 0x1031 |
| `add_site` | parcel style collection and default (`AECC_PARCEL_STYLE`), feature line style collection and default (`AECC_FEATURELINE_STYLE`, drawing the default roadway marker; the application's collection also lists three rail styles, which are not written), and the roadway markers | 2, 128, 0x11B |
| `add_assembly` | assembly style collection and default (`AECC_ASSEMBLY_STYLE`, drawing five of the roadway markers), code set style collection and default (`AECC_ROADWAY_STYLE_SET`: no items, every point, link and shape code drawn with the default roadway marker, feature line, link and shape styles), link style collection and default (`AECC_ROADWAYLINK_STYLE`), shape style collection and default (`AECC_ROADWAYSHAPE_STYLE`), and the roadway markers and feature line styles | 0x118, 0xEA0, 0x11C, 0x11D, 0x11B, 128 |
| `add_corridor` | corridor style collection and default (`AECC_CORRIDOR_STYLE`) | 0xEB2 |

Each collection is written when the drawing lacks it; the intersection
and feature line styles refer to the roadway marker styles by their ids
in that collection. The other references are the written records and the
drawing's layer "0" and ByBlock linetype. Written into a new drawing
the records equal WX13's, WI01's and WR01's bit for bit apart from the
dates. A
new drawing with two crossing alignments, their profiles, the
intersection, a TIN surface, a sample line group with two lines (10/10
and 3/33), their sections and two section views, all written by acadrust
(E-DS1), opens with AUDIT 0 errors; the API lists the intersection at
(50, 0) with its quadrants and the views with the style "Standard", the
lines' offsets and the extents of their computed ranges. A new drawing
with a site, a feature line and an alignment on it and an intersection of
two siteless alignments (E-SS1) opens with AUDIT 0 errors; the API lists
the feature line with the written feature line style and LIST shows the
site's parcel style "Standard". A new drawing with an alignment, its
profile, an assembly with two package lanes (a package written by
`write_pkt` next to the drawing; right lane width 4 / depth 0.25 / slope
-0.03, left lane with the defaults) and a corridor on them, all written by
acadrust (E-A1), opens without recovery and with AUDIT 0 errors; the API
lists both subassemblies up to date, the assembly and the corridor with
the written code set style ("Standard", no items, as the application's
own default), and the corridor rebuild builds stations 0-100 every 25 with
the codes Crown, ETW, ETW_Datum and Crown_Datum (at station 0 ETW (4,
-0.12) right and (-3.5, -0.07) left), as in a drawing the application had
opened (E-PK1).

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
BL 0
BL j, j × { BL number, BL text id }   named points
BL j, j × { BL text id, BL number }   the same keyed by name
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
lists the numbers instead (the same members).

Names (`PointQuery::Names`, kept in lower case) and full descriptions
(`PointQuery::FullDescription`; the writer adds no description keys, so a
point's full description is its raw description) take the rows of the
table above. Elevation ranges (`PointQuery::ElevationRange`) and
exclusions (`PointQuery::Excluding`) are written as the application
stores them (WQ04, every query equal bit for bit and in its texts):

```text
statement: (include) AND NOT (exclude), either part optional
constants: the included part's texts, "" when both parts are present,
           the excluded part's texts; a range's text is "low-high"
records:   the included part's parentheses and ORs, the excluded part's,
           then the included part's leaves, the excluded part's
           (AND, NOT and a range's generated parentheses have no record)
parentheses: number bit, 1, single constant, comparison byte (equal for
           a range), byte 0 (2 under the NOT), seven zero bits
range:     three leaves: low (BD), high (BD), and an equal leaf of id 8
           without a constant
the last node record of the included part ends with a constant 52-bit
mark before its seven zero bits when an excluded part follows
texts:     ["AND"], part: ["NOT", ""], "PARENS", "", ORs, leaves; a range
           leaf is "GENERATED_PARENS", "", "AND", then GTE, LTE and EQ
           leaves and the range text
```

Point names live in the point database: after the label style list,
`BL 0`, the names by number `BL j, j x {BL number, BL text id}`, the same
keyed by text `BL j, j x {BL text id, BL number}`, then the descriptions;
the texts list the names twice, then the descriptions; names and
descriptions share the text ids. `set_cogo_point_name` names a point
(names are unique, case ignored). A drawing written with named points and
groups of every kind above (E-Q5) opens with AUDIT 0; the application
lists the queries as its own and the same members.

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
whose one definition adds the points; `add_tin_surface_with(doc, name,
definitions, auto_rebuild, layer)` writes any list of definitions (the
reader's `TinDefinition`, as `tin_definitions()` returns them) and the
triangulation they build. Both use the drawing's default surface style
(the first style of the style tree's surface style collection, key 77).
A drawing without that collection gets it (§3d); a drawing without any
tree first gets the records of §2a.

```text
BL 80, 2 bits 11, BL 0x1554C738, BL 0x087C1120, BL 5 (version)
BL 8 keys: 0x03011E29 RC 1, 0x0D011E25 BL 0, 0x02011E26 BD DBL_MAX,
  0x0B001081 BL 2, BL 0, BL 0 (geometry stamp), 0x03001093 RC 0,
  0x0201230C BD 0, 0x02011E27 BD DBL_MAX, 0x01011E28 BL 8
BL 0 (byte form), 121 zero bits, 3BD origin 0, 9 BD identity, BD 1
RC 1 item, 5 bits 01000
definitions (below)
RL -1, RS 0x04A2, RC 2, RS 0, RS 0x2A3C, "MMS01"
triangulation (tin.rs): BL vertex count, BL 0, 1, 2, 3 (the corners),
  per vertex RL flags (corner 0x8440, point 0x80; the application keeps a
  deleted vertex with bit 0), RL a triangle that uses it, 3 RD; BL
  triangle count, per triangle 3 RL vertices (counter-clockwise), 3 RL
  neighbours (across the edge that starts at the vertex, -1 outside), RL
  flags (1 hidden: uses a corner or a boundary hides it; 4 deleted, with
  vertices -1), RL -1
7 bits 1010100, BS 0x04BE
handles: soft pointer 0, hard owner 0, the definitions' sources, hard
  pointer 0, style
strings: "", "", the definitions' texts, name, "", ""
```

### Definitions

```text
head: 10 bits 8 + 32 n, RC 2 + 8 n (n definitions), RC 4 when one is a
  boundary (else 2), RC 0x81 when one is a contour (else 0x80)
2 bits (00 with a contour, else 10), BL 0
BS 0x800C and B: the number of breakline definitions + 1, its low bit in
  the B, the rest in bits 8 and up of the BS
n x { BL kind, BL flags (9; 4: from drawing objects; 2: proximity),
      BL 16, GUID, BL k (lines: one per object or held line; else 0),
      data by kind }
BL 0 (four-byte form), BL options (0xAC150A00; 0x01000000: rebuild
  automatically), 2 bits 00
```

| kind | definition | data | texts | sources |
|---|---|---|---|---|
| 47 | points | BL n, n x 3BD | — | — |
| 0 | point group | — | the group's name | the group |
| 14, 15, 24 | standard, proximity, non-destructive breaklines | lines | name | the objects |
| 27 | contours | lines, BL 0 | name | the objects |
| 13 | boundary | lines, BL type (1 show, 2 hide, 4 outer, 8 data clip), BL non-destructive | name | the objects |
| 1, 20 | add vertex, vertex elevation | 3BD | — | — |
| 2, 9, 23 | delete vertex, swap edge, delete line (by plan position) | 2BD | — | — |
| 8, 22 | move vertex, add line | 4BD | — | — |
| 17 | raise / lower | BD | — | — |
| 12 | paste surface | — | name | the pasted surface |
| 31 | drawing objects | BL object type (0 points, 1 lines), BL maintain edges, BL n, n x 3BD (copied) | description | — |
| 30 | point file | BL, BL (the file's FILETIME, high half first), 3 option bits | the file | the point file format |
| 33 | imported file | BL, BL (FILETIME), BL | name, the file | — |
| 41 | corridor surface | GUID (the corridor's surface) | name | the corridor |
| 34, 35, 40 | transform, polygon, kind 40 (older drawings) | 16 RD + BL; BL n, n x 2 RD; BL | — | — |

`lines`: `BL objects, BD mid-ordinate distance, BL m, m x { BL n, n x 3BD }
(lines held by the definition, made from points), BL k, k x BL (1 to k),
BD maximum distance (supplementing), BD weeding distance, BD weeding
angle`. A definition made from drawing objects holds no lines (its
objects are the sources, in order after the hard owner); the application
marks such a surface out of date on open (it compares with the objects).

### The written triangulation

Without a flag that makes the application rebuild a surface on open
(none found: `auto_rebuild` only marks the surface to follow its
sources), the writer builds the triangulation the definitions give, in
order (`tin_build.rs`):

- the four corners lie 100 units outside the extents of every position
  the definitions bring in, the lower two at the lowest elevation, the
  upper two at the highest;
- points are inserted Delaunay (Bowyer-Watson, the cavity stopping at
  fixed edges); a point at a vertex's plan position replaces its
  elevation;
- standard breaklines and contours insert their vertices (and
  supplementing points, which leave a vertex they fall on as it is) and
  force each segment in by flipping crossing edges; a segment through
  another vertex is left as the triangulation has it (as the application
  does); proximity breaklines force segments between the vertices
  nearest their points;
- non-destructive breaklines and boundaries split the triangles they
  cross without flipping: from each point to the next, the point and each
  crossing with the edges as they are when the line gets there, at the
  surface's elevation;
- outer boundaries hide triangles outside (non-destructive: by centre;
  destructive: any vertex outside), hide boundaries the ones inside, show
  boundaries show them again, a data clip boundary leaves later data
  outside it out (edits are not data);
- deleting a vertex fills its hole ear by ear, the ear with the highest
  power of the removed point to its circle first, then restores Delaunay
  edges; moving a vertex deletes and inserts it; swapped and added edges
  are fixed; a deleted line hides its triangles; a raise moves every
  vertex present.

Sources are read from the drawing: a point group's points (its numbers
and the COGO points), the vertices of lines, 2D and 3D polylines. Pasted
surfaces, point files, imported files and corridor surfaces bring no
geometry (the application adds it when it rebuilds the surface).

Checked against 67 sample surfaces (each sample's `tin_definitions()`
written again and both triangulations compared triangle by triangle): 62
are identical. The others: a
non-destructive breakline that runs exactly through existing vertices
(the application drops its end point there, TT07, TT34; or the whole
line, TT61), and points far from the rest (TG02, TG03: hull triangles
between the clusters). A written drawing with ten definitions (E-T5:
points, point group, two breaklines, an outer boundary from a polyline,
a hide boundary, add, delete and elevation edits, a raise) opens without
recovery, AUDIT 0 errors, every definition listed in order; the
application's rebuild of it differs in five triangles of the deleted
vertex's hole (not convex there).

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
lists every face:

```text
BL next free face id (faces + 1), BL count (0 without members),
per face: BL id, BL id, BL flags, BL loops, per loop {BL node, BL k}
BL 1, BL 1, BL p + 1, 10 x BL 1, BL p, BL 1 .. p, 5 x BL 0
references after the site's pair: per parcel {parcel (soft), soft 0},
  in face order (face 2 on)
```

A loop entry is a half-edge with the face on its right: its start node
and its index among the node's half-edges sorted by angle
(counterclockwise from +x). Faces and their entries follow how the
application adds a member: the member is split where it meets the
earlier lines (and they where it meets them), then its segments go in one
at a time.

- Node ids: the member's own points first (an open one's ends, then its
  inner points; a closed one's in path order; a point on an earlier node
  takes that node), then the new crossing points in the member's order.
  A point of the member that lies on an earlier segment is inserted into
  that segment; an earlier node on the member is inserted into the member.
- The first segment of a site makes the outside face 1, its entry the
  segment. A segment off every line opens a new loop of the face around
  it (entry: the segment).
- A segment that splits a face: the part holding the face's entry keeps
  the face and the other part becomes the next face, its entry the
  segment's half-edge on that side. An entry the member's crossings split
  is void: a split face then keeps the part on the segment's right (entry
  the segment) and the new face takes the left; a face the member does
  not split takes the half-edge that followed its old entry.
- A segment that closes a loop bounding its own face: the bounded part
  is the new face (entry: the segment's half-edge inside); the outer part
  stays with the old face, whose entry on the loop, if inside, moves to
  the half-edge leaving the same node outside.
- Face flags: 14, or 6 once a later member split the face, closed a loop
  in it, ran a line into it from an earlier line, or added an elevation
  point (outside face).
- Edge flag 0x80 marks an edge another member's crossing added nodes to;
  those points have flags 0x400 (instead of 2). A point on a node an
  earlier member made carries 0x800000 (WP08, WP19); 0x80 (first point)
  only marks an edge that starts off every earlier line.

A parcel is written for every face but the outside one. A parcel keeps
its record and number when a later lot line cuts its face: it takes the
face its label lies in (on the boundary, the face it had); the faces left
without a parcel get new ones, numbered on from the highest number, the
new faces first and then the old faces that lost theirs (WP19, WP20). The
container lists the parcels in face order. Every parcel's label is
placed again: from the middle of its face's extents (arcs included) it
slides along the horizontal or the vertical line through it, whichever
gets farther from the face's boundary (loose lines inside the face
included), to the nearest point as far from the boundary as that line
gets, rightward or upward on a tie. The middle stays where one of the
two lines gets no deeper there (rectangles, WP14's triangle); a loose
line pushes it aside (WP10), a notch up (WP18), a diagonal side to where
the side stops mattering (WP19). A triangle's label is the middle of its
largest angle's bisector, from the vertex to the opposite side, whatever
the triangle's place, turn, vertex order or size (samples WPLA, WPLB,
WPLC: 39 triangles made from one, changing one property at a time),
except where the slide gets nearly as deep without passing it (its
depth at least 0.94 of the bisector middle's: thin triangles, WP14's)
and for a right isosceles triangle with legs along the axes (a square
cut corner to corner: the middle of the vertical run through the
bisector's middle, halfway to the extents' middle). Right triangles near
isosceles (legs 60 and 50 to 65) take a point neither rule gives (x a
quarter of the leg, y between 16 and 25.6): not written. Where another
face's middle is off it the writer takes the middle of the widest run of
the horizontal line through it.
The node records follow the application's hash order: eight
buckets by id mod 8, each in id order (WP15, WP16: 30 and 50 nodes).

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

`add_lot_line(doc, site, vertices, closed, layer)` writes an open or
closed lot line that may cross or touch the site's other lot lines (not
its alignments or feature lines) and returns it with the parcels of the
faces it makes. Samples WP04 (a line across a rectangle), WP05 (and a
disjoint rectangle), WP08 (two open lines closing a loop), WP09 (a T
junction), WP10 (a loose line crossing one side), WP11 (a T and a cross),
WP18 (one line crossing the same side twice), WP19 (a line through a
corner and across a side) and WP20 (one line crossing two sides twice
each): written for the same lines, node ids, coordinates, faces of every
segment, face entries, face flags and the tail equal the samples'; only
the newest member's point flags 0x10 and 0x20 differ (the application's
marks from walking the faces; the writer keeps the rule of members that
meet nothing). The application's marks on the newest member's points
(each point's mark is its segment's; faces left, right):

| sample | segments: faces, marks |
|---|---|
| WP04 | (1,1) 0x30, (2,3) 0x10, (1,1) 0x30 |
| WP09 | (2,3) 0x10 |
| WP10 | (1,1) none, (2,2) 0x30 |
| WP08 | (2,1) 0x30, (2,1) 0x20 |
| WP18 | (1,1) 0x30, (2,3) 0x30, (2,3) 0x30, (2,3) 0x10, (1,1) 0x30 |
| WP19 | (1,1) 0x30, (2,3) 0x10, (1,1) 0x30 |
| WP20 | (1,1) 0x30, (2,3) 0x30, (1,4) 0x30 three times, (5,3) 0x20, (1,1) 0x30 |
| WP11 (third member) | (4,2) 0x10, (5,3) 0x10 |

The segment that closes a face mostly keeps one mark, the others both,
but WP10's first segment has none and WP20's second (which joins two
sides of the rectangle) both; no rule over faces, order or new faces fits
every sample. The application opens the written drawings either way. Checks E-X1 (WP20's lines), E-X2 (WP11's and a disjoint
rectangle), E-X3 (a diagonal through a corner, then a line crossing the
same side twice whose corner lies on the diagonal) and E-X4 (two open
lines closing a loop and a loose line into it), written into a new
drawing, open with AUDIT 0 errors and the API lists every parcel with its
number, area and extents; the application adds a further crossing line
to E-X1 and splits its parcels right.

Lot lines take a bulge per vertex (the segment that starts there; a
closed line's last one closes it). Their arcs are stored as an alignment's
(kind 1, flags 0x010F0000, the bulge and BL 0), and the parcel's label
goes in the middle of the extents the arcs reach: WP21 (a rectangle with
one arc side and an open line with an arc), written for the same lines,
equals the sample bit for bit (container, both segments and the parcel).

Not written: lot lines that meet another member where either has an arc
(the arc would need its bulge split), lot lines meeting alignments or
feature lines, members running along each other. A lot line alone (no closed loop) makes no parcel;
alignments that close a loop make none either (WP01).

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

`add_corridor_with(doc, &CorridorDefinition, layer)` writes a corridor
from the definition the reader returns (`corridor_definition()`): its name
and description, whether it rebuilds automatically, and its baselines,
each along an alignment and one of its profiles, with regions applying an
assembly of the drawing over a station range, each region's frequencies,
station placement flags and targets, and the stations added by hand per
baseline (with their descriptions). `add_corridor(doc, name, baseline,
region, alignment, profile, assembly, layer)` is the one baseline, one
region form over the alignment's stations with the default frequencies.
The corridor is written as the application writes a corridor before its
first rebuild (the samples below: corridors made and changed through the
API without a rebuild): out of date, without stations, applied sections,
feature lines or surfaces. Adding it changes no other record. The
corridor style is the drawing's default (style tree key 0xEB2; a drawing
without it gets the application's default, §2c), the code set style the
first region's assembly's.

```text
BL 80, 0, 80, 80, 0, BL 0x10025F04, BL 0x40008353 (unbuilt), 4 bits 0101
RC n keys, keys: [0x0C04A61C (BL 0) with targets], 0x0B001081 (BL 2,
  two values; written 0), [0x0C04A621, 0x0C04A61E: BL regions, one BD
  per region], [0x0404A627 with targets: its value is the text "[]"],
  [0x0C04A620: one BD per region]
2 bits 01, 128 zero bits, 2 bits 01, RC 82
BL n feature line code settings (0 before a rebuild), BL n surfaces, BD
  maximum triangle side length, BL n boundary lists, BL 0
B rebuild automatically, B 1, RC 32, RC layout (0x48 locked to stations,
  0x28 to the alignment's geometry), RC (baselines << 3 | 2), 1 bit 0,
  6 bits the first baseline's region count, BL 0 (no station)
baseline: [after the first: BL region count, BL 0], regions, BL 0, B 1,
  9 bits 000000101, identifier, BL n, n x BD added station
region: the assembly's identifier, the assembly's group block, BD start,
  BD end, 2 bits 00, BD frequency along tangents, BD along curves, BL
  flags, BD 0, BD along spirals, BD along profile curves, identifier, BL
  target flags, BL 0, BL 0, BL n target entries, n x 10 lists {BL k, k x
  item}, BL 0, 3 bits 110, BL 0
item: BL 0x1015, BL 0, object head, B 0, B 0, BL 0 (a surface), or BL
  0x1017 (0x1117 for the farthest), BL 0, BL k, k x object head, BL 1
  farthest / 0 nearest, B 0, B 0, BL 0
end: 45 bits 101010101010010000001010100001011111000000100
references: soft 0, style, code set style, per baseline {hard 0,
  alignment, profile (hard), per region {assembly (hard), per node
  {subassembly, the subassembly it hangs from or 0} (soft), per target
  entry {subassembly (hard), its target objects (soft)}}}
texts: "AeccImpCorridor", "", name, description, ["[]"], per baseline
  {per region {"", per assembly group {its name, "", "", "" per node},
  the assembly's name, "", per target {its name twice, "", "", "" per
  object}, the region's name}, the baseline's name, one description per
  added station}
```

Region flags: 0x1000 at increment and 0x400 by curvature along curves
(0x1400 both; a region the application never saved after an edit holds
0, read as at increment), 0x8 at horizontal geometry points, 0x80 at
superelevation critical points, 0x20000 at profile geometry points,
0x40000 at profile high and low points. Target flags: 1 at offset target
geometry points, 4 adjacent to offset target start and end, 8 at
increment and 16 by curvature along offset target curves. The keys hold,
per region in corridor order, the frequency along target curves (25
without the key) and the mid-ordinate distances along curves and along
target curves (0.1 without them); the application writes all three once a
region changes one of them. Keys are written in the order above.

A target entry is written for each subassembly of the region's assembly
that has a target, in node order; its list 5 holds surface targets, 8
offset (width) targets and 9 elevation (slope) targets, the same lists
for every stock subassembly tried (nine kinds with targets, list by the target's
type and not by the parameter's position). An offset or elevation target
holds any number of objects (alignments, polylines, feature lines,
profiles, 3D polylines are all object references); a surface target one.

The identifiers are version-4 GUIDs (new ones for every corridor); a
region starts with its assembly's identifier. The group block is the
assembly's own (`BL 0, BL 0, BL n groups, BL 1`, then per group its
nested object heads and attachment nodes), copied from the assembly
record:

```text
BL 80, 0, 80, 80, 0, BL 0x10025F04, BL, 4 bits, RC n keys, keys
120 zero bits, 3BD location, 18 bits, identifier, group block, 36 bits
references: soft 0, hard owner 0, style, per subassembly {subassembly
  (soft), soft 0}, code set style
texts: "AeccImpAssembly", "", name, "", "", "", per group {name, "", "",
  ""}
```

Samples (each one change at a time from a drawing with three alignments
and profiles, a polyline, a 3D polyline, a feature line, a surface and two
assemblies, one with a lane, one with a transition lane and a side slope
ditch): structure (one region, station range, second and third region
with a gap, a region on another assembly, region name, second baseline
without and with a region, a baseline along a feature line, removing a
baseline, renaming, an empty corridor), every frequency and flag, added
stations with and without a description, targets of each kind (an
alignment, a polyline and a feature line as offset target, a profile, a
3D polyline and a feature line as elevation target, a surface, two
objects, the farthest), targets in several regions and baselines, and
the targets of nine stock subassembly kinds. Rewriting each sample's
definition with `add_corridor_with` gives the sample's record field for
field (the references and texts equal; the identifiers, the geometry
stamp and the class word differ by design).

Written into a drawing with two baselines (`BL1` along a curved
alignment: a region on the lane assembly from 0 to 100, every 10 along
tangents, by curvature with a mid-ordinate of 2 along curves, at
horizontal geometry points; a region on the transition lane and ditch
assembly from 150 to 300 every 12 along curves and 5 along profile
curves, at profile high and low points, with the offset alignment and a
polyline as offset target (farthest), a profile as elevation target and
the surface as the ditch's target; added stations 33.3 and 175; `BL2` with
one region at increment and by curvature along curves, by curvature along
target curves every 7, at offset target geometry points), the corridor
opens without recovery, AUDIT 0 errors; the API lists both baselines,
the regions with their assemblies, ranges, every written frequency and
flag, the added stations (33.3 in the first region, 175 in the second),
and the targets with their objects. `CorridorCollection.RebuildAll()`
builds it: stations 0, 10, 20, ..., 33.3, ..., 99.022, 100 in the first
region, 150, 175, 192, 204, ..., 300 in the second, every 25 along `BL2`,
and the codes of both assemblies (the ditch's included, its target
resolved); the rebuilt drawing reads back with its feature lines (seven
codes on `BL1`, four on `BL2`).

`corridor_definition()` reads the definition of any corridor record
(fields of built corridors too; names and references of corridors without
built stations, whose texts and references are not interleaved with the
built ones). It also reads corridor surfaces (build flag, link and feature
line code flags, overhang correction) and their boundaries and masks
(kind, polygon corners, feature line components): the surface head is
`B, B build, B, BL n link codes, n x B breakline, BL 0, BL m feature line
codes, m x B, BL overhang` (older files write the link count in its
one-byte form `01 00000nnn`, read before as an RC 0x20 or 0xA0 and three
bits), and the field after the surfaces is the maximum triangle side
length (a BD). Boundary lists are written per surface that has any, each
introduced by its surface's name in the texts, then each boundary's name
and description; references: per surface {the corridor surface's own TIN
surface (soft, 0 when it is not built), style, 0, render material}, one
per boundary.

### 3i-a. Corridor options

| option | read | write | evidence / blocked by |
|---|---|---|---|
| several baselines along alignments and profiles, their order | yes | yes | §3i |
| baseline along a feature line | yes (feature line, the alignment and profile made for it) | no | the application makes a hidden alignment and profile for it (two more records) |
| offset baselines | no | no | the programming interface lists them but cannot make one; their command is a dialog |
| several regions, gaps, an assembly per region, region names | yes | yes | §3i |
| frequencies along tangents, curves, spirals, profile curves, target curves; mid-ordinates; curve placement; at geometry, superelevation, profile, high/low, offset target points | yes | yes | §3i |
| added stations with descriptions | yes | yes | §3i |
| offset, elevation and surface targets (any objects; nearest or farthest) | yes | yes | §3i |
| rebuild automatically, description | yes | yes | §3i |
| region lock to the alignment geometry | yes (flag) | no | the region holds alignment element identifiers that the alignment gains when the lock is set |
| corridor surfaces, link and feature line codes, overhang correction, build flag; boundaries (extents, polygon, feature lines) and masks; maximum triangle side length | yes | no | each surface has its own TIN surface record and changes the settings tree |
| feature line code settings (connect, draw, pay items) | count only | no | codes exist only once built |
| built results (stations, applied assemblies, feature lines) | yes (feature lines) | no | written out of date; the application builds them |
| corridor without baselines | no | no | its record ends in another run after the 32 (one sample) |

### Code set styles read back

A code set style (`AECC_ROADWAY_STYLE_SET`) lists its entries in its class
data (after the component lists):

```text
6 x { BL kind, BL 0, BL 0 }                 defaults: uncoded markers, links,
                                            shapes, then markers, links,
                                            shapes of unlisted codes
3 x { BL n, n x { BL kind, BL 0, BL 0 } }   listed point, link, area codes
```

`kind` is 1 point, 2 link, 3 area. Each entry has five references
(material, style, label style, feature line style, blank) after the
record's owned children, and each listed code four texts (code, name,
description, blank) at the end of the text stream. `code_set()` and
`code_set_defaults()` read the record so: the default code set a new
drawing gets (no codes) returns its default marker, link and shape
styles; the code sets of the sample drawings with codes return the same
codes and defaults as before, and those whose codes have no name (which
were not read) now read.

### Assemblies and package subassemblies

`add_pkt_subassembly_on(doc, parent, point, ...)` adds a package
subassembly hanging from the point `point` of the subassembly `parent`:
it joins the parent's group as its last node. Its record is that of a
subassembly at the baseline (the host keeps the origin where the
subassembly was inserted, the assembly's location in the samples, and
places it at the parent's point from the group block). Samples CH1-CH5
(a package lane at the baseline; a shoulder on its outer point; a side
slope on the shoulder's outer point; the same on both sides; a shoulder
on the lane's inner point; each made from the same base) are equal bit
for bit to the records written for them, the assembly's identifier
aside. The corridor copies the assembly's node pairs and group texts.

| written | result |
|---|---|
| a new drawing with lane, shoulder and side slope on both sides (three packages written by `write_pkt`) and a corridor (E-CH1) | opens without recovery, AUDIT 0 errors; two groups of three subassemblies (lane at the baseline, shoulder at (23.5, -60.07) and (16.5, -60.07), side slope at (24.5, -60.11) and (15.5, -60.11)), all up to date; the corridor rebuilds with ETW at offset ±3.5, EPS (the shoulder's outer point) at ±4.5 and Daylight at ±7.5 |

`add_assembly(doc, name, location, layer)` writes an empty assembly with
the drawing's default assembly style (style tree key 0x118) and code set
style (0xEA0); a drawing without them gets the application's defaults
(§2c). `add_pkt_subassembly(doc, assembly, name, source, pkt, side,
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
  version-4 GUID), BL 0, BL 0, BL n groups, n x {BL k, k x node},
  BL 0, 36 bits ending with the version word
  node: two attachment records (BS 80, BS 80, BS 0, B 1, BL 240, 5 bits
    00100 each), BL index of the parent's point it hangs from (0 for the
    group's first, at the baseline), BL 0, BL 0
  references: soft 0, hard owner 0, assembly style (hard), per node in
    group order {subassembly, the subassembly it hangs from or 0}
    (soft), code set style (hard)
  texts: "AeccImpAssembly", "", name, "", "", "", per group {"Group -
    (i)", "", then "", "" per node}
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

### OpenCAD Studio's road package

The package OpenCAD Studio builds in code (`OCSRoad`: lane, shoulder and
side slope of one side, with the pavement under lane and shoulder; the
test `tests/civil_ocs_road_package.rs` copies its builder) was written
with `write_pkt` to `%USERPROFILE%\Documents\OpenCAD Studio\Subassemblies\OCSRoad.pkt`
and used by a new drawing written by acadrust (E-OCS1): an alignment, a
profile, an assembly with one `OCSRoad` subassembly per side, named by
that path as OpenCAD Studio names it, and a corridor on them.

| check | result |
|---|---|
| package in the package editor | opens; the preview shows the half section (points P1-P6, links L1-L6, shape S1) |
| drawing | opens without recovery, AUDIT 0 errors |
| subassemblies | listed up to date, generated from the package, with the eight parameters at their defaults (Side 0 right, 1 left); 6 points, 6 links, 1 shape each, at (0, 0), (3.5, -0.07), (4.5, -0.11), (7.5, -1.61), (4.5, -0.41), (0, -0.3) and mirrored |
| `LaneWidth` set to 4 on the right one | the subassembly is rebuilt: (4, -0.08), (5, -0.12), (8, -1.62), (5, -0.42) |
| corridor rebuild | built (stations 0-100 every 25); point codes Crown, ETW, EPS, Daylight, EPS_Datum, Crown_Datum; link codes Top, Pave, Daylight, Datum; shape code Pave; ETW at offset 4 right and -3.5 left |

No codec change was needed. Note for OpenCAD Studio: it writes the
package to the user's Documents known folder (`SHGetKnownFolderPath`)
but names it in the drawing by `%USERPROFILE%\Documents\...`. Where
Documents is redirected (here to a cloud-synced folder), the two differ
and the host does not find the package (it lists the subassemblies with
the status "file not found" and builds nothing, as E-PK2). Naming the
path the package was written to, with `%USERPROFILE%` only when it is a
prefix of that path, avoids this.

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
package's settings. `new_part_name(seed)` makes a part name from a seed
(a version-4 GUID's digits, the same for the same seed, so a package
written again has the same bytes and MD5). The workflow is built with
`point_delta` (DeltaXAndDeltaY), `point_slope` (SlopeAndDeltaX, with the
offset, elevation and superelevation target arguments), `link`, `shape`,
the input parameters `parameter_double`, `parameter_grade`,
`parameter_slope` and `parameter_side`, and `chain`, a flowchart of steps
in order (`__ReferenceID<i>`) and its start node; every activity gets the
editor's settings (shown errors, hint size 200,22.4, the point's display
name `<point>&<link>` when it draws a link). A lane built with them equals
the lane written by hand. `PktPackage::new_package(name, part)` is
`PktPackage::new` with the creator and the workflow's namespace
declarations the package editor writes into its own packages (K01 to
K05), so a package needs nothing from a saved one.

Check (E-PK7): a lane built only with these (`new_package`, part name
from `new_part_name`, the parameters Side, Width 3.5, Depth 0.3, Slope
-0.02, points P1-P4 with links L1-L3, link L4, shape S1) and written by
`write_pkt` opens in the package editor without errors; its preview draws
the lane (P1-P4, L1-L4, S1). Imported by the host (the API's package
import into WR01) it is listed up to date with its four points, links and
one shape. An assembly of it (right lane width 4 / depth 0.25 / slope
-0.03, left lane with the defaults) and a corridor on it, written by
acadrust into WR01, open with AUDIT 0 errors; the corridor rebuild builds
stations 0-100 every 25 with the codes Crown, ETW, ETW_Datum and
Crown_Datum, at station 0 ETW (4, -0.12) right and (-3.5, -0.07) left, as
E-PK1.

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

## 3m. Gravity pipe networks

`add_pipe_network`, `add_structure` and `add_pipe` write a network that
uses the drawing's first parts list (`AECC_NETWORK_PART_LIST`; the
application's templates have one), its null structure and its 12 inch
corrugated metal pipe. A drawing without a parts list gets the
application's default one: the part catalog node (root key 0x136, as N02
holds it: listing the fields of the null structure's and the pipe's part
definitions), the parts list "Standard" and its 33 families with their
size filters, written as N02's records with their references mapped (each
filter's style is the default pipe or structure style; no rules; the
material "ByLayer" or null), and the default pipe style (style tree key
0x116) and structure style (0x117) written from templates as in §2c. The
network rules are not written (the application's records name a rules
module by its installed path).

What the application adds with the first network, structure and pipe
(samples N00-N03, the application's own, over WT01):

| step | records |
|---|---|
| network | the `AECC_NETWORK` entity |
| first structure | its part definition (`AECC_NETWORK_PART_DEF`) in the named-objects dictionary `AECC_NETWORK_PART_DEFS`, keyed by family id and size (`D45435F0...870FSBSH0SBSD0`); an anonymous block with the structure's body (a solid) the definition refers to twice; the name templates root (root key 0x8C) with the structures' node (id 0x125); eight field definitions in the part catalog node |
| first pipe | its part definition (key: family id and size id), the pipes' name template node (id 0x126), ten field definitions in the catalog node |

Records (all written with references and texts of the samples' layout):

```text
network:   NETWORK_EMPTY or NETWORK_WITH_PARTS, BL pipes, BL structures,
           NETWORK_END
           references: 5 x hard 0, parts list, soft 0, hard owner 0,
             pipes, structures
structure: class header, keys, 3 bits 010, 16-byte id (0), 3BD insertion,
           3 x 3BD axes, 2 bits, part record (BL 80, BS 0x04BE, BL 80,
           BL 80, BL 0, BL 0), 3 x BL 0, identifier, cached outline and
           extents, rule results
pipe:      as a structure up to its identifier (identity frame), then
           lengths, slopes and extents, 3BD start, 3BD end, rule results
identifier: 16 bytes, a version-4 GUID in the byte order Windows stores
           one (the version in the high nibble of byte 7, the variant in
           the high bits of byte 8), then the 2 bits 10
           references (both): hard 0, hard 0, soft 0, hard owner 0, part
             definition, style, network, soft 0, soft 0, hard 0, rules
part definition: no version word; fields BL 80, BS 0x04BE, BL 0, BS kind,
           ... (75 bits for a number, 81 for a text, whose value is a
           text)
name templates root: BL 0x8C, BL n, n x BL id (four-byte form), BL 0,
           BL 32, 3 bits 001; node: BL id, constant run; texts "",
           template
```

Connections (`connect_pipe`; samples N04 unconnected, N05 both ends, N06
end only and a chain of three structures and two pipes):

```text
connector (AECC_NETWORK_PART_CONNECTOR, a model space entity on the
           network's layer): CONNECTOR (constant)
           references: soft 0, hard owner 0, the paired connector (soft;
             0 for an open pipe end), its part (hard)
           texts: "AeccImpNetworkPartConnector", "", ""
part record: ..., BL n, n x { BL 0, field, BL 1 }, BL connectors, ...
part references: ..., part definition, style, the connectors (hard),
           network, ...
```

A pipe has no connector until one of its ends is connected, then two
(start, end); a structure has one per pipe it meets, in the order the
pipes were connected. The writer keeps every other bit of the records it
updates. When the application connects a pipe it lowers the structure to
the pipe's end (invert minus half the outer diameter) and gives it a
part definition of its new height; on opening it does neither:

| written | result |
|---|---|
| N04's pipe connected to both structures (E-N50) | AUDIT 0; the pipe lists both structures, each structure one pipe, positions as written |
| N04's pipe connected at its end only (E-N51) | AUDIT 0; start open, end at the second structure |

The structure record caches its plan outline, a circle of radius 0.5
about the insertion point (four quarter arcs from the top, counter
clockwise: full `BD` x, y, bulge `tan(22.5°)` and one zero bit each);
the writer places it at the insertion point (the application's own
structure at (150, 0, -6), N07, differs from the written one only in its
id and in rounding noise of the outline's zero coordinates). A rotated
structure's record is longer (N07, 0.3 rad); its layout is not decoded and
the writer keeps the unrotated outline.

The identifier starts 365 bits into a pipe record (and into a structure
record at the origin; the structure's axes come before it). Nine
identifiers of the application's parts share the bits a version-4 GUID
fixes when read from there, and all are followed by `10`. Written two
bits later, as earlier writers did, the identifier ends in two random
bits where the `10` belongs; then the application regenerates the drawing
endlessly on opening (one core busy in its model space iterator, samples
N01 + a structure, N04 + a structure, N01 + a pipe) or, in a new drawing,
discards the part (`Error 34 (eWrongObjectType)` reading the pipe, then
the recovery prompt), in the quarter of the cases where they happened to
be `10` the drawing opened. That was the cause of every "does not finish
opening" and "Errors Found" result measured before; neither the format
manager, the catalog node's fields nor the part's position matter:

| written | result |
|---|---|
| N02 with the root entries 0x150, 2, 0x80000012, 0x16A, 0x103, 0x4C, 0x16C (each, all), 0x2D or 5 dropped (E-D1 to E-D5) | AUDIT 0 |
| E-D5 without the application's named-objects entries and with only the pipe and structure style collections in the style tree (E-D6, E-D7) | AUDIT 0 |
| E-D7 with the application's network replaced by a written one and a pipe with a random identifier (E-D8, E-D9) | does not finish opening |
| the same pipe with the application's identifier (E-D14) | AUDIT 0 |
| N01 + a structure with identifiers ending `10` (E-X1, E-X3) or not (E-X2, E-X4), at (100, 0, -6) and (150, 0, -6) | AUDIT 0 / does not finish opening, at both places |
| identifier written at 365 then `10`: N01 + structures at (150, 0, -6), (200, 50, -7) and a pipe (E-F1); N00 + a structure (E-F4); WT01 + a network, two structures and a connected pipe (E-F5) | AUDIT 0; the parts listed where written |
| a new drawing with the default parts list, two structures, a pipe connected to both and a second pipe (E-F3) | opens without a message, AUDIT 0; the network lists two structures (one pipe each, extents from the body block) and two pipes, the first from (0, 0, -5) to (100, 0, -6) connected start and end, the second 111.803 long |

The style and rules come from the family's size filter in the parts list
(`AECC_PART_SIZE_FILTER`: references `0, style, rules, ...`). The fields
between the named ones are the samples' constant runs.

Measured (written drawings opened by the application in its own instance,
API dump of the networks, AUDIT without fixing):

| written | result |
|---|---|
| the application's network with the structure's insertion point changed in the record only (E-N2) | AUDIT 0; position, rim, sump and extents recomputed from the insertion point |
| the pipe's ends changed in the record only (E-N3) | AUDIT 0; length, extents as the application's own pipe with those ends |
| no part definitions (E-N4) | 4 AUDIT errors, families unknown |
| structure definition without its body block (E-N5, E-N14, E-N16, E-N17) | AUDIT 0, family listed; the structure has no body (extents of the cached outline) |
| parts list, style and rules references null (E-N6) | AUDIT 0, no parts list, no style |
| network written by acadrust, pipe (E-N10) | AUDIT 0, network and pipe listed, length 90 and extents as the application's |
| structure whose fields the catalog node does not list (E-N9, E-N11, E-N18, E-N20) | the application does not finish opening the drawing (busy) |
| the same with the catalog node of a drawing that has a structure (E-N21) | AUDIT 0, structure listed at its position, rim and sump from it |

The catalog node (`AECC_NETWORK_PART_CATALOG_DEF_NODE`, the parts list's
owner) lists the union of the fields of the drawing's part definitions
and caches data after them (the application's memory addresses, 8-byte
values that differ between two drawings of the same template, not
decoded). The writer leaves it as it is; the application adds the fields
of a new part definition itself.

A structure's part definition refers twice to the block of its body (an
anonymous `*U` block with one solid). For the null structure the
application's body is a sphere of radius 0.5 whose top is the insertion
point; the writer adds the same block when it writes the definition, and
the application takes the structure's extents from it (`(-0.5, -0.5, -1)`
to `(0.5, 0.5, 0)` about the insertion point, as its own structures').

### Other families

`add_structure_of_family` and `add_pipe_of_family` place one size of
every other family of the default parts list (`network_part_families`):
the 21 structure families (cylindrical, eccentric, slab top and two-tier
structures, junction structures, headwalls and end sections) and the
eleven pipe families (the size the
application gives a new part: 12 inch corrugated metal and concrete pipe,
4 inch corrugated HDPE, 2 inch ductile iron, 3 inch HDPE, 3.6 inch PVC,
the egg-shaped, elliptical and arch culverts and the 24 x 24 inch box
culvert). The default parts list holds one size of each family. Sample N11: every family of WT01's parts list placed by the
API, structures at the origin, pipes from (0, 0, -5) to (100, 0, -6).

A pipe record is the same for every family (N11's eleven pipes differ only
in their identifiers); the family is in the part definition, the pipe's
second text (the family name) and its style and rules. A structure
record differs per family after the axes: its part record and the
outline and extents the application caches. The writer writes the
family's runs as the application wrote them at the origin; the
application recomputes the cached data from the insertion point. The
null structure's record does not stand for another family: N11 with the
cylindrical and rectangular structures' records replaced by the null
structure's (E-N60) does not finish opening.

Structure definitions other than the null structure's refer to two
blocks, each with the body (an anonymous `*U` block with one solid). The
application's bodies are the catalog's generated solids; the writer
writes a cylinder (cylindrical families) or a box of the size's extents
below the insertion point in each block.

| written | result |
|---|---|
| WT01 with a network, a structure of each of the six families (the eccentric one turned by 0.5 rad), a pipe of each of the eleven families and a concrete pipe connected to the two cylindrical structures (E-N70); the same in a new drawing (E-N71) | AUDIT 0; every part listed with its family and size, positions as written, extents as the application's own (the turned structure's turned), the connected pipe listing both structures and each structure one pipe |
| the same with every structure family (E-N72, E-N73) | AUDIT 0; the 22 structures and 12 pipes listed with their families and sizes, extents as N11's about the insertion point (the two-tier eccentric structures' box 5e-6 wider on one side, the application's own extents are not centred) |

## 3n. Pressure pipe networks

`add_pressure_network`, `add_pressure_pipe_of_size` (straight or curved),
`add_pressure_part` and `connect_pressure_parts` write a pressure network
that uses the drawing's pressure part list bound to the default catalog
(`611746E2-B19E-4F8E-ACF7-F12491879819`), and parts of that catalog named
by their part description (`pressure_part_descriptions`): the twelve
ductile iron pipes (4 to 36 inch), and every elbow (48), tee (71), cross
(56) and gate valve (7) size of the default part catalog (`add_pressure_pipe`, `add_pressure_arc_pipe`,
`add_pressure_fitting`, `add_pressure_tee`, `add_pressure_cross` and
`add_pressure_appurtenance` place the 4 inch pipe, elbow and tee, the
cross and the 6 inch valve). A drawing without pressure part lists gets
the default one (listing the 4 inch pipe and elbow and the 6 inch valve)
with the default pressure pipe, fitting and appurtenance styles (style
tree keys 0x17E, 0x17F, 0x180; part lists 0x188), written from templates
as in §2c. A part need not be in the part list: its definition carries
the catalog data (E-P31's tee and cross are listed with their sizes).

Samples (the application's own, over WT01): P00 empty network, P01 + a
pipe, P02 + a second pipe, P03 + an elbow, P04 + a gate valve, P05 + a
second elbow, valve and pipe elsewhere, P06 + an elbow and a valve turned
by 0.5 rad, P07 parts connected through the API (pipe to elbow, pipe to
valve, pipe to pipe), P08 a curved pipe, a tee and a cross, P09 every pipe
and valve size and the first elbow and tee sizes; PM01 is WT01 with every
part but the 4 inch pipe, elbow and 6 inch valve removed from its part
list.

| step | records the application adds |
|---|---|
| network | `AECC_PRESSUREPIPES_NETWORK` |
| first pipe | its part definition (`AECC_NETWORK_PART_DEF` in `AECC_NETWORK_PART_DEFS`, keyed by the size's catalog data), the name templates root and the pipes' node (0x17B) |
| first fitting | its definition, which refers to two blocks named after the part family: the 3D body (`<family>`, a solid) and the plan symbol (`<family>_3`, an arc); the fittings' name template node (0x17C) |
| first appurtenance | as a fitting (its symbol is a line); node 0x17D |

```text
network:   NETWORK_EMPTY or NETWORK_WITH_PARTS, BL pipes, BL fittings,
           BL appurtenances, NETWORK_END
           references: 6 x hard 0, soft 0, hard owner 0, pipes,
             fittings, appurtenances, part list (soft)
           texts: "AeccImpPressurePipeNetwork", "", name, ..., name
             templates of appurtenances, fittings, pipes, pipe runs
pipe:      PIPE_HEAD (class header, keys, 3 bits, 16 zero bytes), PIPE_PRE
           (identity frame, BL 2 ports), port 1 (its start), port 2 (its
           end), 18 bits, 3BD start, 3BD end, BD bulge, BS 0x101
           (straight) or 0x102 (curved), 30 bits, BD start elevation, BD
           plan chord, BD end elevation, PIPE_END
fitting,
appurtenance: HEAD (16 zero bytes), 3BD insertion point, 3 x 3BD axes,
           BL ports, 2 bits, the ports (2 for elbows and valves, 3 for
           tees, 4 for crosses), 3BD reference point, the 3 axes again,
           END
port:      3BD position, 3BD direction away from the part, BD engagement
           length, BL connected port (-1 open), BD outside diameter, BD
           nominal diameter, BD wall thickness, BD deflection, BD, 8 bits
           (a part's last port: 2 bits)
references (all parts): network, hard 0, hard 0, part list, soft 0,
           hard owner 0, part definition, style, per port the part it
           connects to (soft, 0 when open), soft 0
```

A connection is recorded on both sides: each port holds the index of the
port it meets and the part it meets is its reference (`8 + port`). The
application moves and turns the parts so the ports meet when it connects
them (P07: the pipe's end trimmed to the elbow's port, the elbow turned
along the pipe); `connect_pressure_parts` connects ports that are at the
same place already and changes nothing else (the record is the
application's but for the connected index, 32 bits shorter than -1).
A curved pipe's port directions are its tangents (P08: bulge -0.4 from
(0, 0) to (100, 0), start direction (-0.7241, -0.6897)); its plan length
field is the chord.

A port's position is the insertion point plus the size's offset turned
by the rotation, its direction the size's direction turned (P06 equals
the formula to the last bit). The reference point is a point of the size
(the elbow's (0, -0.434, 0), the valve's (0, 0, -0.635)) turned and moved
the same way; the body and symbol blocks are drawn there. The
application keeps a part's extents while its reference point is where
it was (a record moved without it keeps its old extents; moved with it,
the extents follow).

The part's style is the style the part list gives parts of its kind
(the list refers to one style per part). The pipe and part records of
the samples hold a key whose value counts the session's new objects;
the writer keeps the sample's value (the application accepts it in every
part). Unlike gravity parts, pressure parts keep no identifier.

The application's body blocks hold the catalog's solids (13 to 44 KB of
binary solid data per size: an elbow is a swept bend with its sockets, a
valve a body with bonnet and wheel); the catalog itself stores sizes as
parameters of a generator the application runs, and the drawing keeps
only the generated solid, so the writer cannot rebuild it from the
dimensions it has. It writes a box of the size's extents (as the
application reports them, about the reference point) and the
application's plan symbols (arcs for elbows, lines for tees, crosses and
valves). Measured:

| written | result |
|---|---|
| P00 + two pipes, two elbows and two valves at P05's places (E-P10) | AUDIT 0; the parts listed with P05's names, ends, positions and extents |
| P00 + a pipe, an elbow and a valve turned by 0.5 rad (E-P12, as P06) | AUDIT 0; positions and extents equal P06's |
| the same without body blocks (definitions refer to none, E-P11) | AUDIT 0; the parts take extents of ±3 about the position |
| a new drawing: default part list and styles, two pipes, a turned elbow and a valve (E-P20) | AUDIT 0; listed with the default part list |
| WT01 with a network, a pipe, an elbow and a valve (E-P21) | AUDIT 0 |
| WT01 with a tee whose run meets a pipe and whose branch meets a pipe that meets a valve, two pipes connected end to start, an elbow between two pipes, two curved pipes (bulge -0.4 flat, 0.3 falling 2) and a cross (E-P30); the same in a new drawing (E-P31) | AUDIT 0; every connection listed by the API on both sides with the written port indices (pipe start/end parts, fitting and valve connections), the curved pipes' bulges and extents as the application's (P08) |
| every size of `pressure_part_descriptions` at P09's places, in WT01 (E-P32) and in a new drawing (E-P33) | AUDIT 0; the 26 parts listed with their descriptions, ends and positions as P09's, the records equal P09's but for the session counter; extents as P09's (where the application's 12, 16 and 20 inch valves reach to +1 in plan, the written box is symmetric) |

Sizes (sample P10: every elbow, tee and cross size of WT01's part list
placed by the API, P09 the valves). Within a kind (elbows, tees, crosses,
valves) every size shares the record's head and end and the definition's
record but for a few bits; what is the size's own is written from a
table of numbers and texts:

- the definition's six texts (size, standard, description, size id,
  family, family id) at fixed places in its text list (elbows 29, 76, 89,
  103, 110, 117; the others 23, 70, 83, 97, 104, 111) and its own bits in
  one or two windows of the record (elbows 196-212 and 655-680, the others
  515-540);
- the definition's key: a prefix of the size (angle or none, kind, a
  digit, size, a digit, pressure class), then the standard, the number of
  ports, the description, size id, family and family id and the catalog's
  id, each with spaces, dashes and slashes as underscores, and the kind's
  domain (`Fitting_DomainElbow`, `Fitting_DomainTee`,
  `Fitting_DomainCross`, `Appurtenance_DomainValve`);
- the ports (offset, direction, engagement length, outside and nominal
  diameter, wall thickness, deflection), the reference point, the body's
  extents and the plan symbol (an arc for elbows, lines for the others).

Written with the table, the 182 records and definitions equal P10's and
P09's but for the session counter.

| written | result |
|---|---|
| WT01 with every fitting and valve size at P10's and P09's places (E-P40); the same in a new drawing (E-P41) | AUDIT 0; the 182 parts listed with their descriptions, positions and extents as the application's own (to the last bit but two rounding) |

Not written: other part kinds of other catalogs (reducers, caps, plugs,
other valves), the catalog's body solids.

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
- Surfaces: the geometry of pasted surfaces, point files, imported files
  and corridor surfaces (the application adds it on rebuild); the exact
  fill of a deleted vertex's hole that is not convex; non-destructive
  lines through existing vertices (§3c).
- Profiles with other curve constraints (fixed or floating curves, curves
  by K or through a point), profiles of alignments the application has
  changed.
- Alignments and feature lines that meet other members on a site, lot
  lines meeting where one has an arc, parcel and feature line styles other than the
  defaults;
  sections; corridor surfaces, feature line and offset baselines (§3i-a); pipe networks: other families and
  sizes.
- Profile views with a vertical scale or style other than the default;
  the graph settings between the named values are constant runs.

## 6. Status

| kind | read by fields | write | reference check passed | rebuild flag used | blocked by |
|---|---|---|---|---|---|
| root tree node | yes | yes | yes (empty root: AUDIT 0) | — | — |
| COGO points (with names), database, `_All Points` | yes | yes (a new drawing gets §2a) | yes (§4) | not needed (the application draws points from the database) | — |
| point groups (number lists, elevation comparisons and ranges, description, name and full description patterns, exclusions) | yes | yes | yes (§4) | not needed (the application lists the written members) | number ranges (listed as numbers), override settings |
| point style, its collection, class and settings nodes | yes | yes, in a drawing without the tree (§2a) | yes (§4) | not needed | — |
| label styles: label node, point, sample line and profile label roots and default styles, profile label set | partly | yes, the defaults in a new drawing (§2b) | yes (§2b) | not needed | other label roots, styles other than the defaults (the settings are constant runs) |
| alignments (fixed lines, free arcs and spiral-curve-spirals, siteless) | yes | yes (a new drawing gets §3b) | yes (§4) | not needed (the application solves the stored elements) | other constraints, sites, labels |
| TIN surfaces: points, point groups, breaklines (standard, proximity, non-destructive), contours, boundaries, vertex and edge edits, raise, drawing objects, pasted surfaces, point and imported files, corridor surfaces | yes (every definition kind of the samples) | yes, with the triangulation the definitions build (a new drawing gets §3d) | yes (§3c) | not found; the written triangulation is built by the writer (§3c) | geometry of pasted, file and corridor sources; the cases of §3c |
| layout profiles (fixed tangents; free symmetric and asymmetric parabolas, free circular curves) | yes | yes (a new drawing gets §3f) | yes (§4) | not needed (the application solves the stored entities) | fixed and floating curves |
| sites, alignments and feature lines (arcs, elevation points) on a site | yes | yes (a new drawing gets the node of sites and the default parcel and feature line styles, §2c) | yes (§4) | not needed (the application rewrites the site's cache) | alignments or feature lines meeting other members; containers the application edited after inserting elevation points |
| parcels of closed and open, crossing and touching lot lines, with arcs | yes (number, face, boundary) | yes (§3g) | yes (§3g) | not needed | arcs where lines meet; lot lines meeting alignments or feature lines; the label of a nearly isosceles right triangle; the newest member's point marks 0x10, 0x20 where it meets another |
| profile views (automatic or user ranges) and their profile label groups | yes (frame) | yes (a new drawing gets the default view style, §3h) | yes (§3h) | not needed (the application recomputes the view from its style) | other styles and vertical scales |
| assemblies and package subassemblies | partly (corridor's group block) | yes (a new drawing gets the default assembly, code set, link and shape styles, §2c) | yes (§3i, §2c) | not needed | — |
| corridors: baselines along alignments, regions, frequencies, placement flags, added stations, targets (unbuilt) | yes (fields of built ones; names and references of unbuilt ones) | yes, with assemblies of the drawing (a new drawing gets the default corridor style, §2c) | yes (§3i, §2c) | the corridor is written out of date and built by the application's rebuild | see §3i-a |
| sample line groups and two-vertex sample lines | yes | yes, with the drawing's sample line label style (§3j) | yes (§3j) | not needed | lines with more vertices |
| sections of a TIN surface (unsampled) | yes (geometry once sampled) | yes (§3j) | yes (§3j) | the application samples the source | corridor sources, lines added after sampling |
| section views of a line (default style, unsampled section) | yes (frame) | yes, with the drawing's section view style or the default one (§3j, §2c) | yes (§3j, §2c) | not needed | sampled sections, several sections |
| intersections of two crossing roads (the object only) | yes (stations, angles) | yes, with the drawing's intersection style or the default one (§3k, §2c) | yes (§3k, §2c) | not needed | offset alignments, curb returns, corridor regions and locking actions; tee junctions |
| gravity pipe networks: every structure and pipe family of the default parts list (its one size each), connections | partly (part positions, pipe ends, connector counts, sizes of older part definitions) | yes (a new drawing gets the default parts list and styles, §3m) | yes (§3m) | not needed (the application recomputes outlines, lengths and extents) | sizes the default parts list does not hold, the catalog's body solids |
| pressure networks: every pipe, elbow, tee, cross and gate valve size of the default catalog (straight and curved pipes), connections | yes (ports: positions, directions, joint sizes, connections; pipe ends and bulge, part positions and rotations) | yes (a new drawing gets the default pressure part list and styles, §3n) | yes (§3n) | not needed | part kinds of other catalogs, the catalog's body solids |
| gradings, mass haul | partly (grading criteria, mass haul line and view frame) | no | — | — | no controlled samples (§6a) |

### 6a. Gradings and mass haul

No sample of the application's own making could be produced for either:

- The programming interfaces create neither: the .NET classes for gradings
  and grading groups have no create methods, the COM interfaces only read
  styles and settings, and the mass haul diagram is made by a wizard.
- Creating a grading group (its command and dialog, with or without a
  name) ends the application when it runs from a script. Creating a
  grading on a feature line from a command call with every prompt
  answered, in a drawing that has a grading group already (a sample from
  the application's tutorials), opens "Select Grading Group" first; its
  OK ends the application the same way.
- A grading record (770 to 998 bits in that tutorial drawing) holds no
  geometry: it refers to its site and label style and to the site's
  topology by identifiers. Its faces, daylight and target lines are in the
  site's topology, computed by the application from the target surface and
  the criteria, so a grading cannot be written by changing values in a
  copied record; the writer would have to build that geometry itself.
