//! Parcel segments (`AECC_PARCEL_SEGMENT`, RX class `AeccDbLotLine`).
//!
//! Measured record layout (Assembly-1b.dwg: 157 records — a closed
//! 15-vertex site boundary plus 156 open two-point lot lines whose
//! endpoints tile that boundary):
//!
//! ```text
//! byte-aligned data blob, then the bit-packed class stream:
//! head BL 80, 0, 80, 80, 0, BL 0x10025F04, 6 bits tag (18 = 0x12),
//! RC 0 keys, BL 0, ... (rest unmapped; 126 bits in the samples)
//! ```
//!
//! The byte-aligned blob holds the segment's bounding-extents rectangle
//! as five `(x, y)` pairs walked around it — `(min x, min y)`,
//! `(max x, min y)`, `(max x, max y)`, `(min x, max y)`, then the first
//! corner again — preceded by a label point offset `(0.1, 0.1)` into the
//! rectangle from its low corner in the samples. For a two-point lot
//! line the rectangle's diagonal is the segment itself; a many-vertex
//! segment (the site boundary) stores only its extents, not its
//! vertices. The segment's name throws on the live API as well
//! ("Retrieve attribute failed"); its geometry is readable there through
//! `BaseCurve` (`Polyline`: vertices, bulges, length).
//!
//! What the samples do not give is the anchor that would make the blob's
//! position structural: it sits at a fixed byte offset from the record
//! start (188 in this drawing, before the bit-packed stream) whose value
//! no exposed reader position states. Mapping it needs that format-side
//! answer, so nothing is decoded yet.
