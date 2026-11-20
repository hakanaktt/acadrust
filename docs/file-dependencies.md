# DWG file dependencies

`CadDocument.file_dependencies` exposes the physical `AcDb:FileDepList` as
typed `FileDependency` records. It is independent of the drawing header GUID.
An XREF uses the `Acad:XRef` feature and stored filename as its lookup key.
The found path is retained file metadata; the writer never derives a local
machine path. Other feature records, version GUID, timestamp, filesize,
graphics flag and reference count remain available for roundtrip.

The AC1018+ reader bounds counts, byte lengths and feature indexes by the
decompressed section. R2007+ text uses UTF-16 byte counts. Feature names also
accept the independently observed 32-bit-wide UTF-16 representation. Strings
are written using the existing legacy codepage machinery before R2007.
The all-zero eight-byte AC18 dependency list can be stored as a sparse section
with no pages and is recognized as an empty list. Malformed present records
fail strict reading; failsafe reading records a decode diagnostic.

The DWG writer emits the list in AC1018 and newer. Older DWG versions have
no FileDepList section, so a drawing saved to one is written without it. DXF has no
new private dependency-list extension in this contribution. The current
semantic inventory V1 does not enumerate the physical dependency list;
regression tests compare these typed records separately.

The physical record layout follows section 17 of the
[ODA DWG specification](https://static.opendesign.com/files/guestdownloads/OpenDesign_Specification_for_.dwg_files.pdf)
and was cross-checked against the independent
[LibreDWG FileDepList declaration](https://github.com/LibreDWG/libredwg/blob/master/src/filedeplist.spec).
Local writer/reader tests cover AC1018, AC1021 and AC1032, Unicode modern
filenames, serde projection, empty sparse sections and malformed records.
Independent external XREF fixtures remain a separate qualification item.

A bounded independent check uses LibreDWG 0.14.9137 for source headers, XREF
definition base/unit, INSERT scale/rotation and geometry. Its FileDepList reader
has two relevant limitations: its Unicode `FIELD_T32` route calls `TU32`, which
consumes a further word for an empty value; the legacy feature-vector macro uses
`TV` instead of String32. The same modern record with four nonempty strings
reads back all GUID and scalar fields correctly. Empty fields and the legacy
feature list therefore also receive a literal section-layout check against ODA
section 17. This is not an AutoCAD compatibility certification. The writer keeps
the specified unterminated, length-prefixed strings rather than padding fields
to accommodate the independent parser.
