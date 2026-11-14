# R13 and R14 proxies saved by ODA

`oda_r13.dwg` and `oda_r14.dwg` are one synthetic drawing saved by the Open
Design libraries (QCAD Pro 3.33.1 `dwg2dwg -f -r R13` and `-r R14`). The
drawing was written by acadrust as AC1018 from made-up data; it contains no
data from a user or third-party drawing:

- handle 32: a proxy entity of the application class `SPDSNOTE`
  (`mcsDbObjectNote`) with the 24 data bits `A5 5A C3` and no references;
- handle 33: a proxy object of the application class `SPDSDATACONTAINER`
  (`mcsDbDataContainer`) with the 24 data bits `3C C3 99`, entry `SPDS_PROBE`
  of the named object dictionary;
- handle 34: an RTEXT "probe" at (20, 10), height 2.5, style Standard.

What ODA wrote, as its own DXF export of the same files shows:

| Handle | R13 | R14 |
|---|---|---|
| 32 | ACAD_PROXY_ENTITY, class ACAD_PROXY_ENTITY_WRAPPER (541), data `A5 5A C3` | record of class SPDSNOTE (539), not a proxy record |
| 33 | ACAD_PROXY_OBJECT, class ACAD_PROXY_OBJECT_WRAPPER (542), data `3C C3 99` | the same |
| 34 | ACAD_PROXY_ENTITY, class ACAD_PROXY_ENTITY_WRAPPER (541), 260 data bits, 1 reference | the same |

Each proxy record starts with two BL: 498 (499 for the object), which ODA
exports as DXF 90, and the class number, exported as DXF 91. No data format
and no origin bit follow; the data comes next.
