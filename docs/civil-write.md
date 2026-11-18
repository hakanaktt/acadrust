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

So the application builds its tree on open, but a written point must refer
to styles that exist when the drawing is read. Until the style records
(point style, label collector style and its components, the points
settings nodes) are encoded by this crate, the point writer only works in
drawings that already hold the tree — a drawing the reference application
has opened once. In a drawing without it, `add_cogo_point` returns an
error and leaves the drawing unchanged.

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
strings: ""
```

### `_All Points` (`AECC_POINT_PG`, owner the group node)

```text
BS 0x04BE, BL 33, BL 0, BL 0, BL 0 written in its one-byte form,
5 bits 00000, 6 bits 100000, BL 0 (serial), BL count, count × RL number,
564 bits (constant in WP01, WP02, WP06, WP07, WP08, WP10)
handles (hard pointers): 0, point style, inner query, database, group node,
         create-points settings node, 0, 0
strings: "", "AeccImpAllPointsPG"
```

Numbers are ascending. The 564-bit tail holds the group's override and
query settings; it is written as the proven constant. Other groups have a
different tail (WP08 group `G`), so `add_point_group` is not written yet.

### Query statements (`AECC_QUERY_STATEMENT`)

Outer (owner Root, key 75):
`BS 0x04BE, BL 75, BL 1, BL text id, 9 bits 100100010 (constant)`;
handle: inner (soft owner); strings `""`, `"Point Group Query - _All
Points"`, `""`.

Inner (owner the outer): `BS 0x04BE, BL text id, BL 0, BL 0`, then the
113-bit query tree for `PointNumber > 0` (constant); strings
`"Point Group Query - _All Points"`, `""`, `"0"`,
`"AeccQueryNodePARENS"`, `""`, `"AeccQueryNodeGT"`,
`"AeccQueryNodeATTRID"`, `""`, `""`, `"AeccQueryNodeCONST"`, `""`, `""`.

### Edits

`set_cogo_point` and `remove_cogo_point` rewrite the database and
`_All Points` from the points read back; the group keeps its styles and
settings node. Removing a point erases its entity.

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

A wrong bit count in a query statement makes the application refuse the
whole drawing with a recovery prompt, so every constant run above was
taken from the records and checked bit for bit against them.

## 5. What is left

- Style records (point style, label collector style and its component
  styles, settings nodes, class nodes): needed to write points, and every
  other kind, into a drawing that has no tree.
- Point groups other than `_All Points` (the 564-bit tail), point styles
  per group, draw order.
- Alignments, surfaces, profiles, sites, sections, corridors, pipe
  networks: not written.

## 6. Status

| kind | read by fields | write | reference check passed | rebuild flag used | blocked by |
|---|---|---|---|---|---|
| root tree node | yes | yes | yes (empty root: AUDIT 0) | — | — |
| COGO points, database, `_All Points` | yes | yes, in a drawing with the tree | yes (§4) | not needed (the application draws points from the database) | style, settings and class node records for drawings without the tree |
| other point groups | numbers only | no | — | — | 564-bit tail |
| point / label styles | partly | no | — | — | style records not decoded |
| alignments | yes | no | — | — | geometry stamp, constant runs, label sets, styles |
| TIN surfaces | yes (with points) | no | — | — | empty-surface layout, point-group definition, styles |
| profiles, views, sites, parcels, feature lines, gradings, sections, corridors, networks | partly | no | — | — | not started |
