# Open qualification: owned MLeader tolerance content

The public content-type enum includes Tolerance (3). This differs from a
classic LEADER pointing at a separate TOLERANCE/AcDbFcf entity.

The bounded `experimental_tolerance_h1_preserves_tag_and_text_only` regression
authors type 3 with `has_text_contents` and an FCF-control string, then uses the
production AC1032 DWG, ASCII DXF and binary DXF writers/readers. This checks the
codec's own consistency only. It exposed and guards a DXF normalization defect:
type 3 must not become None. Unknown source codes are diagnosed before legacy
enum defaulting; the public authoring `From<i16>` APIs remain compatible.

H1 does **not** qualify the physical tolerance layout, style binding, attachment
semantics or acceptance by an independent CAD application. Document conversion
therefore does not infer a DIMSTYLE from this string or pretend it is MText.

The next issue should obtain one independently authored, editable tolerance
MLeader with recognizable notation, saved as DWG and ASCII DXF. Record the CAD
version, creation method, content type, input hashes and resulting object/context
fields. Reopen and edit one tolerance value and one attachment in that CAD
application, save again, and compare those fields. Verify the codec output in
that application as well. Update the bounded regression only after that evidence
establishes the payload and relationships. One internally generated DWG is not
independent evidence.

Primary references: [Autodesk ContentType](https://help.autodesk.com/cloudhelp/2026/ENU/OARX-ManagedRefGuide/files/OARX-ManagedRefGuide-Autodesk_AutoCAD_DatabaseServices_ContentType.html),
[DraftSight IMLeader creation example](https://help.solidworks.com/2025/english/api/draftsightapi/Interop.dsAutomation~Interop.dsAutomation.IMLeader.html),
[DraftSight tolerance attachment](https://help.solidworks.com/2025/english/api/draftsightapi/Interop.dsAutomation~Interop.dsAutomation.IMLeader~ToleranceAttachment.html),
and the [ODA DWG specification](https://www.opendesign.com/files/guestdownloads/OpenDesign_Specification_for_.dwg_files.pdf).
The latter's text/else-block context layout alone does not resolve this question.
