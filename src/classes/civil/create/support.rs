//! Classes, the root tree node, and the record writer every civil writer
//! shares.

use super::super::bits::Bits;
use crate::classes::arch::{ArchEntityData, ArchObject};
use crate::classes::{DxfClass, ProxyFlags};
use crate::entities::{EntityCommon, EntityType, ExtendedEntity, ExtendedEntityData};
use crate::error::DxfError;
use crate::io::dwg::dwg_reference_type::DwgReferenceType;
use crate::io::dwg::dwg_stream_writers::{object_writer::DwgObjectWriter, DwgMergedWriter};
use crate::objects::ObjectType;
use crate::types::{DxfVersion, Handle};
use crate::CadDocument;

/// Version word that opens every civil record body.
pub(super) const VERSION: i16 = 0x04BE;

/// Key of a node in the root tree node.
pub(super) const KEY_POINT_GROUPS: i32 = 32;
pub(super) const KEY_POINTS: i32 = 34;
pub(super) const KEY_ALL_POINTS_QUERY: i32 = 75;

/// Name of the root tree node in the named objects dictionary.
const ROOT: &str = "Root";
/// The root's texts. The second names a collection the reference
/// application keeps under the root; it is the same in every sample.
const ROOT_STRINGS: [&str; 2] = ["", "Pipe Network Interference Set Collection"];

struct Class {
    dxf: &'static str,
    cpp: &'static str,
    module: &'static str,
    proxy_flags: u16,
    entity: bool,
}

/// Civil classes a writer registers, with the class table values the
/// reference application gives them. The application name is only the
/// module name; the application accepts it (see docs/civil-write.md).
const CLASSES: &[Class] = &[
    Class { dxf: "AECC_TREE_NODE", cpp: "AeccDbTreeNode", module: "AeccvBase130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_POINT_NODE", cpp: "AeccDbPointsNode", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_POINT_GROUP_NODE", cpp: "AeccDbPointGroupsNode", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_POINT_PG", cpp: "AeccDbPG", module: "AeccLand130", proxy_flags: 0x37d, entity: false },
    Class { dxf: "AECC_QUERY_STATEMENT", cpp: "AeccDbQueryStatement", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_SETTINGS_NODE", cpp: "AeccDbSettingsNode", module: "AeccvBase130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_CLASS_NODE", cpp: "AeccDbClassNode", module: "AeccvBase130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_ROOT_SETTINGS_NODE", cpp: "AeccDbRootSettingsNode", module: "AeccvBase130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_USER_DEFINED_ATTRIBUTE_CLASSIFICATION", cpp: "AeccDbUserDefinedAttributeClassification", module: "AeccvBase130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_USER_DEFINED_ATTRIBUTE", cpp: "AeccDbUserDefinedAttribute", module: "AeccvBase130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_POINT_STYLE", cpp: "AeccDbPointStyle", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_ALIGNMENT_STYLE", cpp: "AeccDbAlignmentStyle", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_ALIGNMENT_PARCEL_NODE", cpp: "AeccDbAlignmentParcelNode", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_FEATURE_STYLE", cpp: "AeccDbFeatureStyle", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_PROFILE_STYLE", cpp: "AeccDbVAlignmentStyle", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_VALIGNMENT_STYLE_EXTENSION", cpp: "AeccDbVAlignmentStyleExtension", module: "AeccLand130", proxy_flags: 0x77c, entity: false },
    Class { dxf: "AECC_PARCEL_NODE", cpp: "AeccDbParcelNode", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_PARCEL_STYLE", cpp: "AeccDbParcelStyle", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_FEATURELINE_STYLE", cpp: "AeccDbFeatureLineStyle", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_FEATURE_LINE", cpp: "AeccDbFeatureLine", module: "AeccLand130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_PARCEL", cpp: "AeccDbFace", module: "AeccLand130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_PARCEL_SEGMENT", cpp: "AeccDbLotLine", module: "AeccLand130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_SURFACE_STYLE", cpp: "AeccDbSurfaceStyle", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_TIN_SURFACE", cpp: "AeccDbSurfaceTin", module: "AeccLand130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_ALIGNMENT", cpp: "AeccDbAlignment", module: "AeccLand130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_NETWORK", cpp: "AeccDbNetwork", module: "AeccNetwork130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_STRUCTURE", cpp: "AeccDbStructure", module: "AeccNetwork130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_PIPE", cpp: "AeccDbPipe", module: "AeccNetwork130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_NETWORK_PART_CONNECTOR", cpp: "AeccDbNetworkPartConnector", module: "AeccNetwork130", proxy_flags: 0x81, entity: true },
    Class { dxf: "AECC_NETWORK_PART_DEF", cpp: "AeccDbNetworkPartDef", module: "AeccNetwork130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_PIPE_STYLE", cpp: "AeccDbPipeStyle", module: "AeccNetwork130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_STUCTURE_STYLE", cpp: "AeccDbStructureStyle", module: "AeccNetwork130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_NETWORK_PART_CATALOG_DEF_NODE", cpp: "AeccDbNetworkCatalogDef", module: "AeccNetwork130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_NETWORK_PART_LIST", cpp: "AeccDbPartList", module: "AeccNetwork130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_NETWORK_PART_FAMILY_ITEM", cpp: "AeccDbPartFamilyItem", module: "AeccNetwork130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_PART_SIZE_FILTER", cpp: "AeccDbPartSizeFilter", module: "AeccNetwork130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_PRESSUREPIPES_NETWORK", cpp: "AeccDbPressurePipeNetwork", module: "AeccPressurePipes130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_PRESSUREPIPE", cpp: "AeccDbPressurePipe", module: "AeccPressurePipes130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_FITTING", cpp: "AeccDbFitting", module: "AeccPressurePipes130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_APPURTENANCE", cpp: "AeccDbAppurtenance", module: "AeccPressurePipes130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_PRESSURE_PART_LIST", cpp: "AeccDbPressurePartList", module: "AeccPressurePipes130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_PRESSURE_PIPE_STYLE", cpp: "AeccDbPressurePipeStyle", module: "AeccPressurePipes130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_FITTING_STYLE", cpp: "AeccDbFittingStyle", module: "AeccPressurePipes130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_APPURTENANCE_STYLE", cpp: "AeccDbAppurtenanceStyle", module: "AeccPressurePipes130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_PROFILE", cpp: "AeccDbVAlignment", module: "AeccLand130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_NOTIFICATIONINFOS", cpp: "AeccDbVAlignNotificationInfos", module: "AeccLand130", proxy_flags: 0x77c, entity: false },
    Class { dxf: "AECC_ENTITYEXTENDPARAM", cpp: "AeccDbVAlignFreeParabolaPassPtInfos", module: "AeccLand130", proxy_flags: 0x77c, entity: false },
    Class { dxf: "AECC_ENTITYCIRCLEINFOS", cpp: "AeccDbVAlignFreeCircleInfos", module: "AeccLand130", proxy_flags: 0x77c, entity: false },
    Class { dxf: "AECC_DB_FIXED_PARABOLA_HLPT", cpp: "AeccDbVAlignFixedParabolaHLPtInfos", module: "AeccLand130", proxy_flags: 0x77c, entity: false },
    Class { dxf: "AECC_DB_FIXED_PARABOLA_HLPTPATSPEAK", cpp: "AeccDbVAlignFixedParabolaHLPtPatsPeakInfos", module: "AeccLand130", proxy_flags: 0x77c, entity: false },
    Class { dxf: "AECC_DB_FIXED_ARC_HLPT", cpp: "AeccDbVAlignFixedArcHLPtInfos", module: "AeccLand130", proxy_flags: 0x77c, entity: false },
    Class { dxf: "AECC_DB_FLOAT_TANGENT_GRADELEN", cpp: "AeccDbVAlignFloatTangentGradeLenInfos", module: "AeccLand130", proxy_flags: 0x77c, entity: false },
    Class { dxf: "AECC_PROFILE_VIEW_STYLE", cpp: "AeccDbGraphStyleProfile", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_PROFILE_VIEW", cpp: "AeccDbGraphProfile", module: "AeccLand130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_VALIGNMENT_LINE_LABEL_GROUP", cpp: "AeccDbVAlignmentLineLabeling", module: "AeccLand130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_VALIGNMENT_PVI_LABEL_GROUP", cpp: "AeccDbVAlignmentPVILabeling", module: "AeccLand130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_VALIGNMENT_SAGCURVE_LABEL_GROUP", cpp: "AeccDbVAlignmentSagCurveLabeling", module: "AeccLand130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_VALIGNMENT_CRESTCURVE_LABEL_GROUP", cpp: "AeccDbVAlignmentCrestCurveLabeling", module: "AeccLand130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_ALIGNMENT_STATION_LABEL_GROUP", cpp: "AeccDbAlignmentStationLabeling", module: "AeccLand130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_ALIGNMENT_MINOR_STATION_LABEL_GROUP", cpp: "AeccDbAlignmentMinorStationLabeling", module: "AeccLand130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_ALIGNMENT_GEOMPOINT_LABEL_GROUP", cpp: "AeccDbAlignmentGeomPointLabeling", module: "AeccLand130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_ASSEMBLY", cpp: "AeccDbAssembly", module: "AeccRoadway130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_SUBASSEMBLY", cpp: "AeccDbSubassembly", module: "AeccRoadway130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_SUBASSEMBLY_STYLE", cpp: "AeccDbSubAssemblyStyle", module: "AeccRoadway130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_ASSEMBLY_STYLE", cpp: "AeccDbAssemblyStyle", module: "AeccRoadway130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_ROADWAY_STYLE_SET", cpp: "AeccDbRoadwayStyleSet", module: "AeccRoadway130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_ROADWAYLINK_STYLE", cpp: "AeccDbRoadwayLinkStyle", module: "AeccRoadway130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_ROADWAYSHAPE_STYLE", cpp: "AeccDbRoadwayShapeStyle", module: "AeccRoadway130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_CORRIDOR_STYLE", cpp: "AeccDbCorridorStyle", module: "AeccRoadway130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_NAME_TEMPLATES_ROOT", cpp: "AeccDbNameTemplatesRoot", module: "AeccvBase130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_NAME_TEMPLATES_NODE", cpp: "AeccDbNameTemplatesNode", module: "AeccvBase130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_CORRIDOR", cpp: "AeccDbCorridor", module: "AeccRoadway130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_SAMPLE_LINE_STYLE", cpp: "AeccDbSampleLineStyle", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_SAMPLE_LINE_GROUP", cpp: "AeccDbSampleLineGroup", module: "AeccLand130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_SAMPLE_LINE", cpp: "AeccDbSampleLine", module: "AeccLand130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_SECTION", cpp: "AeccDbSection", module: "AeccLand130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_GRAPH_SECTION_VIEW", cpp: "AeccDbGraphCrossSection", module: "AeccLand130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_INTERSECTION", cpp: "AeccDbIntersection", module: "AeccRoadway130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_SECTION_STYLE", cpp: "AeccDbSectionStyle", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_SAMPLE_LINE_GROUP_STYLE", cpp: "AeccDbSampleLineGroupStyle", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_MASSHAULLINE", cpp: "AeccDbMassHaulLine", module: "AeccLand130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_MASS_HAUL_LINE_STYLE", cpp: "AeccDbMassHaulLineStyle", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_SECTION_VIEW_STYLE", cpp: "AeccDbGraphStyleCrossSection", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_SECTION_VIEW_BAND_STYLE_SET", cpp: "AeccDbGraphSectionBandStyleSet", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_SECTIONDATABAND_LABELGROUP", cpp: "AeccDbSectionDataBandLabeling", module: "AeccLand130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_INTERSECTION_STYLE", cpp: "AeccDbIntersectionStyle", module: "AeccRoadway130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_ROADWAYMARKER_STYLE", cpp: "AeccDbRoadwayMarkerStyle", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_LABEL_NODE", cpp: "AeccDbLabelNode", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_LABEL_COLLECTOR_STYLE", cpp: "AeccDbLabelStyleCollector", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_LABEL_TEXT_STYLE", cpp: "AeccDbLabelStyleText", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_LABEL_TEXT_ITERATOR_STYLE", cpp: "AeccDbLabelStyleTextIterator", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_LABEL_RADIAL_LINE_STYLE", cpp: "AeccDbLabelStyleRadialLine", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_LABEL_BLOCK_STYLE", cpp: "AeccDbLabelStyleBlock", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_LABEL_TICK_STYLE", cpp: "AeccDbLabelStyleTick", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_LABEL_VECTOR_ARROW_STYLE", cpp: "AeccDbLabelStyleVectorArrow", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_LABEL_TEXT_ITERATOR_CURVE_OR_SPIRAL_STYLE", cpp: "AeccDbLabelStyleTextIteratorCurveOrSpiral", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_PROFILE_VIEW_DATA_BAND_STYLE", cpp: "AeccDbGraphStyleProfileDataBands", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_PROFILE_VIEW_VERTICAL_GEOMETRY_BAND_STYLE", cpp: "AeccDbGraphStyleVertGeometryBands", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_PROFILE_VIEW_HORIZONTAL_GEOMETRY_BAND_STYLE", cpp: "AeccDbGraphStyleHorGeometryBands", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_PROFILEDATABAND_LABELGROUP", cpp: "AeccDbProfileDataBandLabeling", module: "AeccLand130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_VERTICALGEOMETRYBAND_LABELGROUP", cpp: "AeccDbVerticalGeometryBandLabeling", module: "AeccLand130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_HORIZONTALGEOMETRYBAND_LABELGROUP", cpp: "AeccDbHorizontalGeometryBandLabeling", module: "AeccLand130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_PROFILE_VIEW_SUPERELEVATION_DIAGRAM_BAND_STYLE", cpp: "AeccDbGraphStyleSuperElevationBands", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_PROFILE_VIEW_SECTIONAL_DATA_BAND_STYLE", cpp: "AeccDbGraphStyleSectionalDataBands", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_PROFILE_VIEW_PIPE_NETWORK_BAND_STYLE", cpp: "AeccDbGraphProfileStylePipeNetworkBand", module: "AeccNetwork130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_SUPERELEVATIONBAND_LABELGROUP", cpp: "AeccDbSuperElevationBandLabeling", module: "AeccLand130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_SECTIONALDTABAND_LABELGROUP", cpp: "AeccDbSectionalDataBandLabeling", module: "AeccLand130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_GRAPH_PROFILE_NETWORK_PART", cpp: "AeccDbGraphProfileNetworkPart", module: "AeccNetwork130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_PIPENETWORKBAND_LABELGROUP", cpp: "AeccDbPipeNetworkBandLabeling", module: "AeccLand130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_PROFILE_LABEL_SET", cpp: "AeccDbVAlignStnLabelsDlgTemplate", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_ASSOC_ACTION", cpp: "AeccDbAssocAction", module: "AeccvBase130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_ASSOC_OFFSET_VALIGNMENT_ACTION_BODY", cpp: "AeccDbAssocOffsetVAlignment", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_ASSOC_OFFSET_ALIGNMENT_ACTION_BODY", cpp: "AeccDbAssocOffsetAlignment", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_ASSOC_PVILOCKING_MULTIALIGNINTER_ACTION_BODY", cpp: "AeccDbAssocPviLockingMultiAlignInter", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_ASSOC_INTERSECTION_ACTION_BODY", cpp: "AeccDbAssocIntersection", module: "AeccRoadway130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_ASSOC_CURBRETURN_ALIGNMENT_ACTION_BODY", cpp: "AeccDbAssocCurbReturnAlignment", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_ASSOC_CURBRETURN_VALIGNMENT_ACTION_BODY", cpp: "AeccDbAssocCurbReturnVAlignment", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_INTERSECTION_LOCATION_LABEL", cpp: "AeccDbIntersectionLocationLabel", module: "AeccRoadway130", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_ASSOC_DEPENDENCY", cpp: "AeccDbAssocDependency", module: "AeccvBase130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_ASSOC_DEPENDENCY_BODY", cpp: "AeccDbAssocDependencyBody", module: "AeccvBase130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_COGO_POINT", cpp: "AeccDbCogoPoint", module: "AeccLand130", proxy_flags: 0x33c, entity: true },
    Class { dxf: "AECC_POND", cpp: "AeccDbPond", module: "AeccStormWaterControl130|Product Desc: Autodesk Civil 3D |Company: Autodesk,Inc. |WEB Address: www.autodesk.com", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_UNDERGROUNDSTORAGE", cpp: "AeccDbUndergroundStorage", module: "AeccStormWaterControl130|Product Desc: Autodesk Civil 3D |Company: Autodesk,Inc. |WEB Address: www.autodesk.com", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_CHANNEL", cpp: "AeccDbChannel", module: "AeccDrainageChannel130|Product Desc: Autodesk Civil 3D |Company: Autodesk,Inc. |WEB Address: www.autodesk.com", proxy_flags: 0x37d, entity: true },
    Class { dxf: "AECC_GRADING_GROUP", cpp: "AeccDbGradingGroup", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_GRADING", cpp: "AeccDbGrading", module: "AeccLand130|Product Desc: Autodesk Civil 3D |Company: Autodesk,Inc. |WEB Address: www.autodesk.com", proxy_flags: 0x37d, entity: true },
];

/// The class entry's DWG schema version: the stormwater control classes
/// (pond, underground storage, channel) are recorded with 26 in the
/// reference application's class table; every other written class keeps
/// the established 25 the application accepts.
fn class_dwg_version(dxf: &str) -> i32 {
    if matches!(dxf, "AECC_POND" | "AECC_UNDERGROUNDSTORAGE" | "AECC_CHANNEL") { 26 } else { 25 }
}

pub(super) fn invalid(message: &str) -> DxfError {
    DxfError::Parse(message.into())
}

/// Registers `dxf` (once) and returns its DWG type code.
pub(super) fn register(doc: &mut CadDocument, dxf: &str) -> Result<i16, DxfError> {
    if let Some(class) = doc.classes.get_by_name(dxf) {
        return Ok(class.class_number);
    }
    let class = CLASSES.iter().find(|c| c.dxf == dxf).ok_or_else(|| invalid("unknown civil class"))?;
    let mut entry = if class.entity { DxfClass::new_entity(class.dxf, class.cpp) } else { DxfClass::new(class.dxf, class.cpp) };
    entry.application_name = class.module.into();
    entry.proxy_flags = ProxyFlags(class.proxy_flags);
    entry.dwg_version = class_dwg_version(dxf);
    doc.classes.add_or_update(entry);
    doc.classes.get_by_name(dxf).map(|c| c.class_number).ok_or_else(|| invalid("civil class was not registered"))
}

/// One reference of a record body and its kind.
#[derive(Clone, Copy)]
pub(super) struct Ref(pub DwgReferenceType, pub u64);

pub(super) fn soft_pointer(h: u64) -> Ref {
    Ref(DwgReferenceType::SoftPointer, h)
}
pub(super) fn hard_pointer(h: u64) -> Ref {
    Ref(DwgReferenceType::HardPointer, h)
}
pub(super) fn soft_owner(h: u64) -> Ref {
    Ref(DwgReferenceType::SoftOwnership, h)
}

/// A record body: fields go to `w`, references and texts are listed here
/// and written after the fields, in order.
pub(super) struct Body {
    pub refs: Vec<Ref>,
    pub strings: Vec<String>,
}

/// Writes the constant bit run `bits` ("0"/"1" characters).
pub(super) fn bits(w: &mut DwgMergedWriter, bits: &str) {
    for b in bits.bytes() {
        w.write_bit(b == b'1');
    }
}

/// BL in its one-byte form even for 0 (some fields are always written so).
pub(super) fn bl_byte(w: &mut DwgMergedWriter, value: u8) {
    w.write_2bits(1);
    w.write_byte(value);
}

fn finish(w: &mut DwgMergedWriter, body: &Body) {
    for r in &body.refs {
        w.write_handle(r.0, r.1);
    }
    for s in &body.strings {
        w.write_variable_text(s);
    }
}

/// Encodes and stores (or replaces) a civil dictionary object.
pub(super) fn put_object(
    doc: &mut CadDocument,
    dxf: &str,
    handle: Handle,
    owner: Handle,
    reactors: Vec<Handle>,
    fields: impl FnOnce(&mut DwgMergedWriter),
    body: Body,
) -> Result<(), DxfError> {
    put_record(doc, dxf, handle, owner, reactors, true, fields, body)
}

/// [`put_object`] for a record that does not open with the version word
/// (part definitions).
pub(super) fn put_plain_object(
    doc: &mut CadDocument,
    dxf: &str,
    handle: Handle,
    owner: Handle,
    reactors: Vec<Handle>,
    fields: impl FnOnce(&mut DwgMergedWriter),
    body: Body,
) -> Result<(), DxfError> {
    put_record(doc, dxf, handle, owner, reactors, false, fields, body)
}

#[allow(clippy::too_many_arguments)]
fn put_record(
    doc: &mut CadDocument,
    dxf: &str,
    handle: Handle,
    owner: Handle,
    reactors: Vec<Handle>,
    versioned: bool,
    fields: impl FnOnce(&mut DwgMergedWriter),
    body: Body,
) -> Result<(), DxfError> {
    let type_code = register(doc, dxf)?;
    let mut writer = DwgObjectWriter::new(doc)?;
    let (raw, handle_bits) = writer.encode_civil_object(type_code, handle, owner, &reactors, |w| {
        if versioned {
            w.write_bit_short(VERSION);
        }
        fields(w);
        finish(w, &body);
    });
    let mut object = ArchObject::new(dxf, handle, owner);
    object.cpp_class_name = CLASSES.iter().find(|c| c.dxf == dxf).map(|c| c.cpp).unwrap_or("").into();
    object.reactors = reactors;
    object.raw_dwg_data = Some(raw);
    object.raw_dwg_handle_bits = handle_bits;
    object.raw_dwg_version = Some(DxfVersion::AC1032);
    doc.objects.insert(handle, ObjectType::Arch(object));
    Ok(())
}

/// Re-encodes the civil object at `handle` with new fields, keeping its
/// class, owner, reactors and extension dictionary.
pub(super) fn rewrite_object(doc: &mut CadDocument, handle: Handle, fields: impl FnOnce(&mut DwgMergedWriter), body: Body) -> Result<(), DxfError> {
    let old = object(doc, handle).ok_or_else(|| invalid("no civil object to rewrite"))?.clone();
    let type_code = register(doc, &old.dxf_name)?;
    let mut writer = DwgObjectWriter::new(doc)?;
    let (raw, handle_bits) = writer.encode_civil_object_with(type_code, handle, old.owner, &old.reactors, &old.xdictionary_handle, |w| {
        w.write_bit_short(VERSION);
        fields(w);
        finish(w, &body);
    });
    let mut object = old;
    object.raw_dwg_data = Some(raw);
    object.raw_dwg_handle_bits = handle_bits;
    object.raw_dwg_version = Some(DxfVersion::AC1032);
    doc.objects.insert(handle, ObjectType::Arch(object));
    Ok(())
}

/// The references of a civil object as written: kind and handle, in
/// stream order.
pub(super) fn refs_of(object: &ArchObject) -> Option<Vec<Ref>> {
    object
        .typed_handles()
        .into_iter()
        .map(|(h, kind)| {
            let kind = match kind {
                2 => DwgReferenceType::SoftOwnership,
                3 => DwgReferenceType::HardOwnership,
                4 => DwgReferenceType::SoftPointer,
                5 => DwgReferenceType::HardPointer,
                _ => return None,
            };
            Some(Ref(kind, h))
        })
        .collect()
}

/// Encodes a civil entity and stores it (replacing one with the same handle).
pub(super) fn put_entity(
    doc: &mut CadDocument,
    dxf: &str,
    common: EntityCommon,
    fields: impl FnOnce(&mut DwgMergedWriter),
    body: Body,
) -> Result<Handle, DxfError> {
    let type_code = register(doc, dxf)?;
    let mut writer = DwgObjectWriter::new(doc)?;
    let (raw, handle_bits) = writer.encode_architecture_entity(type_code, &common, |w| {
        fields(w);
        finish(w, &body);
    });
    let cpp = CLASSES.iter().find(|c| c.dxf == dxf).map(|c| c.cpp).unwrap_or("");
    let mut data = ArchEntityData::from_parts(dxf, cpp, common.graphic_data.as_deref());
    data.dwg_type_code = type_code;
    data.dwg_handle_bits = handle_bits;
    data.dwg_source_version = Some(DxfVersion::AC1032);
    data.raw_dwg_data = Some(raw);
    let handle = common.handle;
    let entity = EntityType::Extended(Box::new(ExtendedEntity { common, data: ExtendedEntityData::Arch(data) }));
    if let Some(slot) = doc.get_entity_mut(handle) {
        *slot = entity;
        return Ok(handle);
    }
    doc.add_entity(entity).map_err(|_| invalid("civil entity was not added"))
}

/// The civil object at `handle`.
pub(super) fn object<'a>(doc: &'a CadDocument, handle: Handle) -> Option<&'a ArchObject> {
    match doc.objects.get(&handle) {
        Some(ObjectType::Arch(a)) => Some(a),
        _ => None,
    }
}

/// Main stream (data, first body bit after the version word, end) of a
/// civil object.
pub(super) fn fields_of(object: &ArchObject) -> Option<(&[u8], usize, usize)> {
    let (data, start, end) = object.main_stream()?;
    let mut b = Bits::new(data, start, end);
    (b.bit_short()? == VERSION).then_some(())?;
    Some((data, b.pos, end))
}

/// The root tree node, `None` when the drawing has none.
pub(super) fn root(doc: &CadDocument) -> Option<Handle> {
    doc.named_objects_dictionary()?.get(ROOT)
}

/// Keys and nodes of the root tree node, in stored order.
pub(super) fn root_entries(doc: &CadDocument) -> Option<Vec<(i32, Handle)>> {
    let node = object(doc, root(doc)?)?;
    let (data, start, end) = fields_of(node)?;
    let mut b = Bits::new(data, start, end);
    (b.bit_long()? == 1).then_some(())?;
    let n = usize::try_from(b.bit_long()?).ok()?;
    let keys = (0..n).map(|_| b.bit_long()).collect::<Option<Vec<_>>>()?;
    let handles = node.decode().handles;
    (handles.len() >= n).then_some(())?;
    Some(keys.into_iter().zip(handles).map(|(k, h)| (k, Handle::new(h))).collect())
}

/// Node the root tree node keeps under `key`.
pub(super) fn root_node(doc: &CadDocument, key: i32) -> Option<Handle> {
    root_entries(doc)?.into_iter().find(|(k, _)| *k == key).map(|(_, h)| h)
}

fn write_root(doc: &mut CadDocument, handle: Handle, entries: &[(i32, Handle)], strings: Vec<String>) -> Result<(), DxfError> {
    let nod = doc.header.named_objects_dict_handle;
    let keys: Vec<i32> = entries.iter().map(|e| e.0).collect();
    put_object(
        doc,
        "AECC_TREE_NODE",
        handle,
        nod,
        vec![nod],
        |w| {
            w.write_bit_long(1);
            w.write_bit_long(keys.len() as i32);
            for k in &keys {
                w.write_bit_long(*k);
            }
            w.write_bit_long(0);
            bits(w, "010");
        },
        Body { refs: entries.iter().map(|e| soft_owner(e.1.value())).collect(), strings },
    )
}

/// Adds (or repoints) the root entry `key`.
pub(super) fn set_root_entry(doc: &mut CadDocument, key: i32, node: Handle) -> Result<(), DxfError> {
    let root = ensure_civil(doc)?;
    let mut entries = root_entries(doc).ok_or_else(|| invalid("root tree node cannot be read"))?;
    match entries.iter_mut().find(|e| e.0 == key) {
        Some(e) => e.1 = node,
        // The application stores the entries by hash bucket (key mod 8),
        // a new key last in its bucket (WP00 -> WP01 adds 32, 34, 75 so).
        None => {
            let bucket = |k: i32| k as u32 % 8;
            let at = entries.iter().position(|e| bucket(e.0) > bucket(key)).unwrap_or(entries.len());
            entries.insert(at, (key, node));
        }
    }
    let strings = object(doc, root).map(|o| o.decode().strings).unwrap_or_default();
    write_root(doc, root, &entries, strings)
}

/// Makes the drawing a civil drawing: adds the root tree node (and its
/// class) when the drawing has none. Returns the root.
///
/// The reference application builds its settings and style tree itself
/// when it opens a drawing whose root lacks them (docs/civil-write.md), so
/// nothing else is written here.
pub fn ensure_civil(doc: &mut CadDocument) -> Result<Handle, DxfError> {
    if let Some(root) = root(doc) {
        return Ok(root);
    }
    let handle = doc.allocate_handle();
    write_root(doc, handle, &[], ROOT_STRINGS.iter().map(|s| s.to_string()).collect())?;
    doc.named_objects_dictionary_mut().ok_or_else(|| invalid("drawing has no named objects dictionary"))?.add_entry(ROOT, handle);
    Ok(handle)
}


/// A node's fields after its child keys, its references after its
/// children, and its texts after its children's names.
pub(super) struct NodeTail {
    pub key: i32,
    pub children: Vec<(i32, u64)>,
    /// `(text id, name)` of the children whose keys are text ids.
    pub names: Vec<String>,
    pub bits: String,
    pub refs: Vec<Ref>,
    pub own: Vec<String>,
    pub texts: Vec<String>,
}

/// Reads a node record (root, tree, class, settings or feature style
/// node): key, children and everything after them, kept as they are.
pub(super) fn read_node(doc: &CadDocument, node: Handle) -> Option<NodeTail> {
    let o = object(doc, node)?;
    let (data, start, end) = fields_of(o)?;
    let mut b = Bits::new(data, start, end);
    let key = b.bit_long()?;
    let n = usize::try_from(b.bit_long()?).ok()?;
    let keys = (0..n).map(|_| b.bit_long()).collect::<Option<Vec<_>>>()?;
    let bits: String = (b.pos..end).map(|i| if data[i / 8] >> (7 - i % 8) & 1 == 1 { '1' } else { '0' }).collect();
    let typed = o.typed_handles();
    (typed.len() >= n).then_some(())?;
    let kind = |k: u8| match k {
        2 => DwgReferenceType::SoftOwnership,
        3 => DwgReferenceType::HardOwnership,
        4 => DwgReferenceType::SoftPointer,
        _ => DwgReferenceType::HardPointer,
    };
    let text = |k: i32| k as u32 & 0x8000_0000 != 0;
    let strings = o.decode().strings;
    let own_len = if text(key) { 2 } else { 1 };
    let named = keys.iter().filter(|k| text(**k)).count();
    (strings.len() >= own_len + named).then_some(())?;
    Some(NodeTail {
        key,
        children: keys.into_iter().zip(typed.iter().map(|t| t.0)).collect(),
        names: strings[own_len..own_len + named].to_vec(),
        bits,
        refs: typed[n..].iter().map(|(v, k)| Ref(kind(*k), *v)).collect(),
        own: strings[..own_len].to_vec(),
        texts: strings[own_len + named..].to_vec(),
    })
}

/// Adds child `(key, record)` named `name` (when `key` is a text id) to
/// a node, keeping everything else the node holds. Children are stored by
/// hash bucket (key mod 8), a new one last in its bucket.
pub(super) fn add_node_child(doc: &mut CadDocument, node: Handle, key: i32, child: Handle, name: &str) -> Result<(), DxfError> {
    let mut n = read_node(doc, node).ok_or_else(|| invalid("node record cannot be read"))?;
    let o = object(doc, node).ok_or_else(|| invalid("node record cannot be read"))?;
    let (dxf, owner, reactors) = (o.dxf_name.clone(), o.owner, o.reactors.clone());
    let text = |k: i32| k as u32 & 0x8000_0000 != 0;
    // Pair each text-keyed child with its name to keep them together.
    let mut names = n.names.clone().into_iter();
    let mut list: Vec<(i32, u64, Option<String>)> = n.children.iter().map(|c| (c.0, c.1, text(c.0).then(|| names.next().unwrap_or_default()))).collect();
    let bucket = |k: i32| k as u32 % 8;
    let at = list.iter().position(|c| bucket(c.0) > bucket(key)).unwrap_or(list.len());
    list.insert(at, (key, child.value(), text(key).then(|| name.to_string())));
    n.children = list.iter().map(|c| (c.0, c.1)).collect();
    let mut refs: Vec<Ref> = list.iter().map(|c| soft_owner(c.1)).collect();
    refs.extend(n.refs);
    let mut strings = n.own;
    strings.extend(list.iter().filter_map(|c| c.2.clone()));
    strings.extend(n.texts);
    put_object(
        doc,
        &dxf,
        node,
        owner,
        reactors,
        |w| {
            w.write_bit_long(n.key);
            w.write_bit_long(n.children.len() as i32);
            for c in &n.children {
                w.write_bit_long(c.0);
            }
            bits(w, &n.bits);
        },
        Body { refs, strings },
    )
}
