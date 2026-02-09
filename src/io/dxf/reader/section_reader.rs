//! DXF section readers

use super::stream_reader::{DxfStreamReader, PointReader};
use crate::document::CadDocument;
use crate::entities::*;
use crate::error::Result;
use crate::objects::*;
use crate::tables::*;
use crate::tables::linetype::LineTypeElement;
use crate::types::*;
use crate::xdata::{ExtendedData, ExtendedDataRecord, XDataValue};

/// States for the mesh reading state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MeshReadState {
    Properties,
    Vertices,
    Faces,
    Edges,
    Creases,
}

/// Section reader for parsing DXF sections
pub struct SectionReader<'a> {
    reader: &'a mut Box<dyn DxfStreamReader>,
}

impl<'a> SectionReader<'a> {
    /// Create a new section reader
    pub fn new(reader: &'a mut Box<dyn DxfStreamReader>) -> Self {
        Self { reader }
    }
    
    /// Read the HEADER section
    pub fn read_header(&mut self, document: &mut CadDocument) -> Result<()> {
        let hdr = &mut document.header;

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 && pair.value_string == "ENDSEC" {
                break;
            }

            if pair.code != 9 {
                continue;
            }

            let var_name = pair.value_string.clone();
            match var_name.as_str() {
                // ── Version / Metadata ──
                "$ACADVER" => {
                    if let Some(p) = self.reader.read_pair()? {
                        document.version = DxfVersion::from_version_string(&p.value_string);
                    }
                }
                "$ACADMAINTVER" => { self.reader.read_pair()?; }
                "$REQUIREDVERSIONS" => {
                    if let Some(p) = self.reader.read_pair()? {
                        if let Some(v) = p.as_int() { hdr.required_versions = v; }
                    }
                }
                "$DWGCODEPAGE" => {
                    if let Some(p) = self.reader.read_pair()? { hdr.code_page = p.value_string.clone(); }
                }
                "$HANDSEED" => {
                    if let Some(p) = self.reader.read_pair()? {
                        if let Ok(h) = u64::from_str_radix(&p.value_string, 16) {
                            hdr.handle_seed = h;
                        }
                    }
                }
                "$LASTSAVEDBY" => {
                    if let Some(p) = self.reader.read_pair()? { hdr.last_saved_by = p.value_string.clone(); }
                }
                "$FINGERPRINTGUID" => {
                    if let Some(p) = self.reader.read_pair()? { hdr.fingerprint_guid = p.value_string.clone(); }
                }
                "$VERSIONGUID" => {
                    if let Some(p) = self.reader.read_pair()? { hdr.version_guid = p.value_string.clone(); }
                }
                "$MENU" => {
                    if let Some(p) = self.reader.read_pair()? { hdr.menu_name = p.value_string.clone(); }
                }
                "$PROJECTNAME" => {
                    if let Some(p) = self.reader.read_pair()? { hdr.project_name = p.value_string.clone(); }
                }
                "$HYPERLINKBASE" => {
                    if let Some(p) = self.reader.read_pair()? { hdr.hyperlink_base = p.value_string.clone(); }
                }
                "$STYLESHEET" => {
                    if let Some(p) = self.reader.read_pair()? { hdr.stylesheet = p.value_string.clone(); }
                }

                // ── Drawing Mode Booleans ──
                "$DIMASO" => { if let Some(p) = self.reader.read_pair()? { hdr.associate_dimensions = p.as_i16() == Some(1); } }
                "$DIMSHO" => { if let Some(p) = self.reader.read_pair()? { hdr.update_dimensions_while_dragging = p.as_i16() == Some(1); } }
                "$ORTHOMODE" => { if let Some(p) = self.reader.read_pair()? { hdr.ortho_mode = p.as_i16() == Some(1); } }
                "$FILLMODE" => { if let Some(p) = self.reader.read_pair()? { hdr.fill_mode = p.as_i16() == Some(1); } }
                "$QTEXTMODE" => { if let Some(p) = self.reader.read_pair()? { hdr.quick_text_mode = p.as_i16() == Some(1); } }
                "$MIRRTEXT" => { if let Some(p) = self.reader.read_pair()? { hdr.mirror_text = p.as_i16() == Some(1); } }
                "$REGENMODE" => { if let Some(p) = self.reader.read_pair()? { hdr.regen_mode = p.as_i16() == Some(1); } }
                "$LIMCHECK" => { if let Some(p) = self.reader.read_pair()? { hdr.limit_check = p.as_i16() == Some(1); } }
                "$PLIMCHECK" => { if let Some(p) = self.reader.read_pair()? { hdr.paper_space_limit_check = p.as_i16() == Some(1); } }
                "$PLINEGEN" => { if let Some(p) = self.reader.read_pair()? { hdr.polyline_linetype_generation = p.as_i16() == Some(1); } }
                "$PSLTSCALE" => { if let Some(p) = self.reader.read_pair()? { hdr.paper_space_linetype_scaling = p.as_i16() == Some(1); } }
                "$TILEMODE" => { if let Some(p) = self.reader.read_pair()? { hdr.show_model_space = p.as_i16() == Some(1); } }
                "$USRTIMER" => { if let Some(p) = self.reader.read_pair()? { hdr.user_timer = p.as_i16() == Some(1); } }
                "$WORLDVIEW" => { if let Some(p) = self.reader.read_pair()? { hdr.world_view = p.as_i16() == Some(1); } }
                "$VISRETAIN" => { if let Some(p) = self.reader.read_pair()? { hdr.retain_xref_visibility = p.as_i16() == Some(1); } }
                "$DISPSILH" => { if let Some(p) = self.reader.read_pair()? { hdr.display_silhouette = p.as_i16() == Some(1); } }
                "$SPLFRAME" => { if let Some(p) = self.reader.read_pair()? { hdr.spline_frame = p.as_i16() == Some(1); } }
                "$DELOBJ" => { if let Some(p) = self.reader.read_pair()? { hdr.delete_objects = p.as_i16() == Some(1); } }
                "$BLIPMODE" => { if let Some(p) = self.reader.read_pair()? { hdr.blip_mode = p.as_i16() == Some(1); } }
                "$ATTREQ" => { if let Some(p) = self.reader.read_pair()? { hdr.attribute_request = p.as_i16() == Some(1); } }
                "$ATTDIA" => { if let Some(p) = self.reader.read_pair()? { hdr.attribute_dialog = p.as_i16() == Some(1); } }

                // ── Drawing Mode Integers ──
                "$DRAGMODE" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.drag_mode = v; } } }

                // ── Units ──
                "$LUNITS" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.linear_unit_format = v; } } }
                "$LUPREC" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.linear_unit_precision = v; } } }
                "$AUNITS" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.angular_unit_format = v; } } }
                "$AUPREC" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.angular_unit_precision = v; } } }
                "$INSUNITS" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.insertion_units = v; } } }
                "$ATTMODE" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.attribute_visibility = v; } } }
                "$PDMODE" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.point_display_mode = v; } } }
                "$USERI1" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.user_int1 = v; } } }
                "$USERI2" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.user_int2 = v; } } }
                "$USERI3" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.user_int3 = v; } } }
                "$USERI4" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.user_int4 = v; } } }
                "$USERI5" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.user_int5 = v; } } }
                "$COORDS" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.coords_mode = v; } } }
                "$OSMODE" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i32() { hdr.object_snap_mode = v; } } }
                "$PICKSTYLE" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.pick_style = v; } } }
                "$SPLINETYPE" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.spline_type = v; } } }
                "$SPLINESEGS" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.spline_segments = v; } } }
                "$SURFU" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.surface_u_density = v; } } }
                "$SURFV" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.surface_v_density = v; } } }
                "$SURFTYPE" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.surface_type = v; } } }
                "$SURFTAB1" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.surface_tab1 = v; } } }
                "$SURFTAB2" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.surface_tab2 = v; } } }
                "$SHADEDGE" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.shade_edge = v; } } }
                "$SHADEDIF" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.shade_diffuse = v; } } }
                "$MAXACTVP" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.max_active_viewports = v; } } }
                "$ISOLINES" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.isolines = v; } } }
                "$CMLJUST" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.multiline_justification = v; } } }
                "$TEXTQLTY" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.text_quality = v; } } }
                "$SORTENTS" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.sort_entities = v; } } }
                "$INDEXCTL" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.index_control = v; } } }
                "$HIDETEXT" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.hide_text = v; } } }
                "$XCLIPFRAME" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.xclip_frame = v; } } }
                "$HALOGAP" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.halo_gap = v; } } }
                "$OBSCOLOR" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.obscured_color = v; } } }
                "$OBSLTYPE" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.obscured_linetype = v; } } }
                "$INTERSECTIONDISPLAY" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.intersection_display = v; } } }
                "$INTERSECTIONCOLOR" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.intersection_color = v; } } }
                "$DIMASSOC" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.dimension_associativity = v; } } }
                "$MEASUREMENT" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.measurement = v; } } }
                "$PROXYGRAPHICS" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.proxy_graphics = v; } } }
                "$TREEDEPTH" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.tree_depth = v; } } }

                // ── Scale / Size Defaults ──
                "$LTSCALE" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.linetype_scale = v; } } }
                "$TEXTSIZE" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.text_height = v; } } }
                "$TRACEWID" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.trace_width = v; } } }
                "$SKETCHINC" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.sketch_increment = v; } } }
                "$THICKNESS" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.thickness = v; } } }
                "$PDSIZE" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.point_display_size = v; } } }
                "$PLINEWID" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.polyline_width = v; } } }
                "$CELTSCALE" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.current_entity_linetype_scale = v; } } }
                "$VIEWTWIST" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.view_twist = v; } } }
                "$FILLETRAD" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.fillet_radius = v; } } }
                "$CHAMFERA" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.chamfer_distance_a = v; } } }
                "$CHAMFERB" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.chamfer_distance_b = v; } } }
                "$CHAMFERC" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.chamfer_length = v; } } }
                "$CHAMFERD" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.chamfer_angle = v; } } }
                "$ANGBASE" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.angle_base = v; } } }
                "$ANGDIR" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.angle_direction = v; } } }
                "$ELEVATION" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.elevation = v; } } }
                "$PELEVATION" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.paper_elevation = v; } } }
                "$FACETRES" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.facet_resolution = v; } } }
                "$CMLSCALE" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.multiline_scale = v; } } }
                "$USERR1" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.user_real1 = v; } } }
                "$USERR2" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.user_real2 = v; } } }
                "$USERR3" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.user_real3 = v; } } }
                "$USERR4" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.user_real4 = v; } } }
                "$USERR5" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.user_real5 = v; } } }
                "$PSVPSCALE" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.viewport_scale_factor = v; } } }
                "$SHADOWPLANELOCATION" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.shadow_plane_location = v; } } }
                "$LOFTANG1" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.loft_angle1 = v; } } }
                "$LOFTANG2" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.loft_angle2 = v; } } }
                "$LOFTMAG1" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.loft_magnitude1 = v; } } }
                "$LOFTMAG2" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.loft_magnitude2 = v; } } }
                "$LOFTPARAM" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.loft_param = v; } } }
                "$LOFTNORMALS" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.loft_normals = v; } } }
                "$LATITUDE" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.latitude = v; } } }
                "$LONGITUDE" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.longitude = v; } } }
                "$NORTHDIRECTION" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.north_direction = v; } } }
                "$TIMEZONE" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i32() { hdr.timezone = v; } } }
                "$STEPSPERSEC" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.steps_per_second = v; } } }
                "$STEPSIZE" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.step_size = v; } } }
                "$LENSLENGTH" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.lens_length = v; } } }
                "$CAMERAHEIGHT" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.camera_height = v; } } }
                "$CAMERADISPLAY" => { if let Some(p) = self.reader.read_pair()? { hdr.camera_display = p.as_i16() == Some(1); } }

                // ── Current Entity Settings ──
                "$CECOLOR" => {
                    if let Some(p) = self.reader.read_pair()? {
                        if let Some(v) = p.as_i16() { hdr.current_entity_color = Color::from_index(v); }
                    }
                }
                "$CELWEIGHT" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.current_line_weight = v; } } }
                "$CEPSNTYPE" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.current_plotstyle_type = v; } } }
                "$ENDCAPS" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.end_caps = v; } } }
                "$JOINSTYLE" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.join_style = v; } } }
                "$LWDISPLAY" => { if let Some(p) = self.reader.read_pair()? { hdr.lineweight_display = p.as_i16() == Some(1); } }
                "$XEDIT" => { if let Some(p) = self.reader.read_pair()? { hdr.xedit = p.as_i16() == Some(1); } }
                "$EXTNAMES" => { if let Some(p) = self.reader.read_pair()? { hdr.extended_names = p.as_i16() == Some(1); } }
                "$PSTYLEMODE" => { if let Some(p) = self.reader.read_pair()? { hdr.plotstyle_mode = p.as_i16() == Some(1); } }
                "$OLESTARTUP" => { if let Some(p) = self.reader.read_pair()? { hdr.ole_startup = p.as_i16() == Some(1); } }

                // ── Dimension Variables ──
                "$DIMSCALE" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.dim_scale = v; } } }
                "$DIMASZ" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.dim_arrow_size = v; } } }
                "$DIMEXO" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.dim_ext_line_offset = v; } } }
                "$DIMDLI" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.dim_line_increment = v; } } }
                "$DIMEXE" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.dim_ext_line_extension = v; } } }
                "$DIMRND" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.dim_rounding = v; } } }
                "$DIMDLE" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.dim_line_extension = v; } } }
                "$DIMTP" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.dim_tolerance_plus = v; } } }
                "$DIMTM" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.dim_tolerance_minus = v; } } }
                "$DIMTXT" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.dim_text_height = v; } } }
                "$DIMCEN" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.dim_center_mark = v; } } }
                "$DIMTSZ" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.dim_tick_size = v; } } }
                "$DIMALTF" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.dim_alt_scale = v; } } }
                "$DIMLFAC" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.dim_linear_scale = v; } } }
                "$DIMTVP" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.dim_text_vertical_pos = v; } } }
                "$DIMTFAC" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.dim_tolerance_scale = v; } } }
                "$DIMGAP" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.dim_line_gap = v; } } }
                "$DIMALTRND" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.dim_alt_rounding = v; } } }
                "$DIMTOL" => { if let Some(p) = self.reader.read_pair()? { hdr.dim_tolerance = p.as_i16() == Some(1); } }
                "$DIMLIM" => { if let Some(p) = self.reader.read_pair()? { hdr.dim_limits = p.as_i16() == Some(1); } }
                "$DIMTIH" => { if let Some(p) = self.reader.read_pair()? { hdr.dim_text_inside_horizontal = p.as_i16() == Some(1); } }
                "$DIMTOH" => { if let Some(p) = self.reader.read_pair()? { hdr.dim_text_outside_horizontal = p.as_i16() == Some(1); } }
                "$DIMSE1" => { if let Some(p) = self.reader.read_pair()? { hdr.dim_suppress_ext1 = p.as_i16() == Some(1); } }
                "$DIMSE2" => { if let Some(p) = self.reader.read_pair()? { hdr.dim_suppress_ext2 = p.as_i16() == Some(1); } }
                "$DIMTAD" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.dim_text_above = v; } } }
                "$DIMZIN" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.dim_zero_suppression = v; } } }
                "$DIMAZIN" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.dim_alt_zero_suppression = v; } } }
                "$DIMALT" => { if let Some(p) = self.reader.read_pair()? { hdr.dim_alternate_units = p.as_i16() == Some(1); } }
                "$DIMALTD" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.dim_alt_decimal_places = v; } } }
                "$DIMTOFL" => { if let Some(p) = self.reader.read_pair()? { hdr.dim_force_line_inside = p.as_i16() == Some(1); } }
                "$DIMSAH" => { if let Some(p) = self.reader.read_pair()? { hdr.dim_separate_arrows = p.as_i16() == Some(1); } }
                "$DIMTIX" => { if let Some(p) = self.reader.read_pair()? { hdr.dim_force_text_inside = p.as_i16() == Some(1); } }
                "$DIMSOXD" => { if let Some(p) = self.reader.read_pair()? { hdr.dim_suppress_outside_ext = p.as_i16() == Some(1); } }
                "$DIMCLRD" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.dim_line_color = Color::from_index(v); } } }
                "$DIMCLRE" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.dim_ext_line_color = Color::from_index(v); } } }
                "$DIMCLRT" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.dim_text_color = Color::from_index(v); } } }
                "$DIMADEC" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.dim_angular_decimal_places = v; } } }
                "$DIMDEC" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.dim_decimal_places = v; } } }
                "$DIMTDEC" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.dim_tolerance_decimal_places = v; } } }
                "$DIMALTU" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.dim_alt_units_format = v; } } }
                "$DIMALTTD" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.dim_alt_tolerance_decimal_places = v; } } }
                "$DIMAUNIT" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.dim_angular_units = v; } } }
                "$DIMFRAC" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.dim_fraction_format = v; } } }
                "$DIMLUNIT" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.dim_linear_unit_format = v; } } }
                "$DIMDSEP" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.dim_decimal_separator = char::from(v as u8); } } }
                "$DIMTMOVE" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.dim_text_movement = v; } } }
                "$DIMJUST" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.dim_horizontal_justification = v; } } }
                "$DIMSD1" => { if let Some(p) = self.reader.read_pair()? { hdr.dim_suppress_line1 = p.as_i16() == Some(1); } }
                "$DIMSD2" => { if let Some(p) = self.reader.read_pair()? { hdr.dim_suppress_line2 = p.as_i16() == Some(1); } }
                "$DIMTOLJ" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.dim_tolerance_justification = v; } } }
                "$DIMTZIN" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.dim_tolerance_zero_suppression = v; } } }
                "$DIMALTZ" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.dim_alt_tolerance_zero_suppression = v; } } }
                "$DIMALTTZ" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.dim_alt_tolerance_zero_tight = v; } } }
                "$DIMATFIT" | "$DIMFIT" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.dim_fit = v; } } }
                "$DIMUPT" => { if let Some(p) = self.reader.read_pair()? { hdr.dim_user_positioned_text = p.as_i16() == Some(1); } }
                "$DIMPOST" => { if let Some(p) = self.reader.read_pair()? { hdr.dim_post = p.value_string.clone(); } }
                "$DIMAPOST" => { if let Some(p) = self.reader.read_pair()? { hdr.dim_alt_post = p.value_string.clone(); } }
                "$DIMBLK" => { if let Some(p) = self.reader.read_pair()? { hdr.dim_arrow_block = p.value_string.clone(); } }
                "$DIMBLK1" => { if let Some(p) = self.reader.read_pair()? { hdr.dim_arrow_block1 = p.value_string.clone(); } }
                "$DIMBLK2" => { if let Some(p) = self.reader.read_pair()? { hdr.dim_arrow_block2 = p.value_string.clone(); } }
                "$DIMLDRBLK" => { if let Some(p) = self.reader.read_pair()? { hdr.dim_leader_arrow_block = p.value_string.clone(); } }
                "$DIMLWD" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.dim_line_weight = v; } } }
                "$DIMLWE" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.dim_ext_line_weight = v; } } }

                // ── Name references ──
                "$CLAYER" => { if let Some(p) = self.reader.read_pair()? { hdr.current_layer_name = p.value_string.clone(); } }
                "$CELTYPE" => { if let Some(p) = self.reader.read_pair()? { hdr.current_linetype_name = p.value_string.clone(); } }
                "$TEXTSTYLE" => { if let Some(p) = self.reader.read_pair()? { hdr.current_text_style_name = p.value_string.clone(); } }
                "$DIMSTYLE" => { if let Some(p) = self.reader.read_pair()? { hdr.current_dimstyle_name = p.value_string.clone(); } }
                "$CMLSTYLE" => { if let Some(p) = self.reader.read_pair()? { hdr.multiline_style = p.value_string.clone(); } }

                // ── Extents / Limits (multi-value XYZ / XY) ──
                "$INSBASE" => { self.read_header_point3(&mut hdr.model_space_insertion_base)?; }
                "$EXTMIN" => { self.read_header_point3(&mut hdr.model_space_extents_min)?; }
                "$EXTMAX" => { self.read_header_point3(&mut hdr.model_space_extents_max)?; }
                "$LIMMIN" => { self.read_header_point2(&mut hdr.model_space_limits_min)?; }
                "$LIMMAX" => { self.read_header_point2(&mut hdr.model_space_limits_max)?; }
                "$PINSBASE" => { self.read_header_point3(&mut hdr.paper_space_insertion_base)?; }
                "$PEXTMIN" => { self.read_header_point3(&mut hdr.paper_space_extents_min)?; }
                "$PEXTMAX" => { self.read_header_point3(&mut hdr.paper_space_extents_max)?; }
                "$PLIMMIN" => { self.read_header_point2(&mut hdr.paper_space_limits_min)?; }
                "$PLIMMAX" => { self.read_header_point2(&mut hdr.paper_space_limits_max)?; }

                // ── UCS ──
                "$UCSBASE" => { if let Some(p) = self.reader.read_pair()? { hdr.ucs_base = p.value_string.clone(); } }
                "$UCSNAME" => { if let Some(p) = self.reader.read_pair()? { hdr.model_space_ucs_name = p.value_string.clone(); } }
                "$PUCSNAME" => { if let Some(p) = self.reader.read_pair()? { hdr.paper_space_ucs_name = p.value_string.clone(); } }
                "$UCSORG" => { self.read_header_point3(&mut hdr.model_space_ucs_origin)?; }
                "$UCSXDIR" => { self.read_header_point3(&mut hdr.model_space_ucs_x_axis)?; }
                "$UCSYDIR" => { self.read_header_point3(&mut hdr.model_space_ucs_y_axis)?; }
                "$PUCSORG" => { self.read_header_point3(&mut hdr.paper_space_ucs_origin)?; }
                "$PUCSXDIR" => { self.read_header_point3(&mut hdr.paper_space_ucs_x_axis)?; }
                "$PUCSYDIR" => { self.read_header_point3(&mut hdr.paper_space_ucs_y_axis)?; }
                "$UCSORTHOVIEW" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.ucs_ortho_view = v; } } }
                "$PUCSORTHOVIEW" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_i16() { hdr.paper_ucs_ortho_view = v; } } }

                // ── Date / Time ──
                "$TDCREATE" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.create_date_julian = v; } } }
                "$TDUCREATE" => { self.reader.read_pair()?; } // UTC variant: skip (no field)
                "$TDUPDATE" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.update_date_julian = v; } } }
                "$TDUUPDATE" => { self.reader.read_pair()?; }
                "$TDINDWG" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.total_editing_time = v; } } }
                "$TDUSRTIMER" => { if let Some(p) = self.reader.read_pair()? { if let Some(v) = p.as_double() { hdr.user_elapsed_time = v; } } }

                _ => {
                    // Skip unknown header variable value(s) – consume until next code 9 or code 0
                    self.skip_header_variable()?;
                }
            }
        }

        Ok(())
    }

    /// Read a 3D point header variable (three successive code/value pairs: 10/20/30)
    fn read_header_point3(&mut self, target: &mut Vector3) -> Result<()> {
        for _ in 0..3 {
            if let Some(p) = self.reader.read_pair()? {
                if let Some(v) = p.as_double() {
                    let base = p.code % 100;
                    if base < 10 || base == 10 { target.x = v; }
                    else if base >= 20 && base < 30 { target.y = v; }
                    else { target.z = v; }
                }
            }
        }
        Ok(())
    }

    /// Read a 2D point header variable (two successive code/value pairs: 10/20)
    fn read_header_point2(&mut self, target: &mut Vector2) -> Result<()> {
        for _ in 0..2 {
            if let Some(p) = self.reader.read_pair()? {
                if let Some(v) = p.as_double() {
                    // First value (code 10) → X, second (code 20) → Y
                    if p.code % 100 < 20 { target.x = v; } else { target.y = v; }
                }
            }
        }
        Ok(())
    }

    /// Skip an unknown header variable — consume value pairs until the next $VAR (code 9) or ENDSEC (code 0)
    fn skip_header_variable(&mut self) -> Result<()> {
        while let Some(p) = self.reader.read_pair()? {
            if p.code == 9 || p.code == 0 {
                self.reader.push_back(p);
                break;
            }
        }
        Ok(())
    }
    
    /// Read the CLASSES section
    pub fn read_classes(&mut self, _document: &mut CadDocument) -> Result<()> {
        // Read classes until ENDSEC
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 && pair.value_string == "ENDSEC" {
                break;
            }
            
            // Classes are defined with code 0 = "CLASS"
            if pair.code == 0 && pair.value_string == "CLASS" {
                // Skip class definition for now
                while let Some(class_pair) = self.reader.read_pair()? {
                    if class_pair.code == 0 {
                        // Next entity - push back and break to outer loop
                        self.reader.push_back(class_pair);
                        break;
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Read the TABLES section
    pub fn read_tables(&mut self, document: &mut CadDocument) -> Result<()> {
        // Read tables until ENDSEC
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 && pair.value_string == "ENDSEC" {
                break;
            }
            
            // Tables start with code 0 = "TABLE"
            if pair.code == 0 && pair.value_string == "TABLE" {
                // Read table name (code 2)
                if let Some(name_pair) = self.reader.read_pair()? {
                    if name_pair.code == 2 {
                        match name_pair.value_string.as_str() {
                            "LAYER" => self.read_layer_table(document)?,
                            "LTYPE" => self.read_linetype_table(document)?,
                            "STYLE" => self.read_textstyle_table(document)?,
                            "BLOCK_RECORD" => self.read_block_record_table(document)?,
                            "DIMSTYLE" => self.read_dimstyle_table(document)?,
                            "APPID" => self.read_appid_table(document)?,
                            "VIEW" => self.read_view_table(document)?,
                            "VPORT" => self.read_vport_table(document)?,
                            "UCS" => self.read_ucs_table(document)?,
                            _ => {
                                // Skip unknown table
                                self.skip_to_endtab()?;
                            }
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Read the BLOCKS section
    pub fn read_blocks(&mut self, document: &mut CadDocument) -> Result<()> {
        // Read blocks until ENDSEC
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 && pair.value_string == "ENDSEC" {
                break;
            }

            // Blocks start with code 0 = "BLOCK"
            if pair.code == 0 && pair.value_string == "BLOCK" {
                self.read_block(document)?;
            }
        }

        Ok(())
    }

    /// Read a single BLOCK...ENDBLK definition
    fn read_block(&mut self, document: &mut CadDocument) -> Result<()> {
        use crate::entities::Block;
        use crate::types::Vector3;

        let mut block_name = String::new();
        let mut base_point = Vector3::new(0.0, 0.0, 0.0);
        let mut description = String::new();
        let mut xref_path = String::new();
        let mut layer = String::from("0");
        let mut handle = Handle::NULL;

        let mut point_reader = PointReader::new();

        // Read BLOCK entity properties
        while let Some(pair) = self.reader.read_pair()? {
            match pair.code {
                0 => {
                    // Start of next entity - put it back and break
                    self.reader.push_back(pair);
                    break;
                }
                2 => {
                    // Block name
                    block_name = pair.value_string.clone();
                }
                3 => {
                    // Block name (alternate)
                    if block_name.is_empty() {
                        block_name = pair.value_string.clone();
                    }
                }
                4 => {
                    // Description
                    description = pair.value_string.clone();
                }
                1 => {
                    // XRef path
                    xref_path = pair.value_string.clone();
                }
                5 => {
                    // Handle
                    if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) {
                        handle = Handle::new(h);
                    }
                }
                8 => {
                    // Layer
                    layer = pair.value_string.clone();
                }
                10 | 20 | 30 => {
                    // Base point coordinates
                    point_reader.add_coordinate(&pair);
                    if let Some(pt) = point_reader.get_point() {
                        base_point = pt;
                    }
                }
                _ => {}
            }
        }

        // Create Block entity
        let mut block = Block::new(block_name.clone(), base_point);
        block.common.handle = handle;
        block.common.layer = layer.clone();
        block.description = description;
        block.xref_path = xref_path;

        // Find the corresponding BlockRecord and add entities to it
        let mut block_entities: Vec<EntityType> = Vec::new();

        // Read entities until ENDBLK
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                match pair.value_string.as_str() {
                    "ENDBLK" => {
                        // Read ENDBLK properties
                        let _block_end = self.read_block_end()?;

                        // Find the BlockRecord and add entities
                        if let Some(block_record) = document.block_records.get_mut(&block_name) {
                            block_record.entities = block_entities;
                        }

                        // Note: Block and BlockEnd are block definition markers, not drawing entities.
                        // They are not added to the document's main entity list.
                        // The block content is stored in the BlockRecord.

                        break;
                    }
                    "POINT" => {
                        if let Some(entity) = self.read_point()? {
                            block_entities.push(EntityType::Point(entity));
                        }
                    }
                    "LINE" => {
                        if let Some(entity) = self.read_line()? {
                            block_entities.push(EntityType::Line(entity));
                        }
                    }
                    "CIRCLE" => {
                        if let Some(entity) = self.read_circle()? {
                            block_entities.push(EntityType::Circle(entity));
                        }
                    }
                    "ARC" => {
                        if let Some(entity) = self.read_arc()? {
                            block_entities.push(EntityType::Arc(entity));
                        }
                    }
                    "ELLIPSE" => {
                        if let Some(entity) = self.read_ellipse()? {
                            block_entities.push(EntityType::Ellipse(entity));
                        }
                    }
                    "POLYLINE" => {
                        if let Some(entity) = self.read_polyline()? {
                            block_entities.push(EntityType::Polyline(entity));
                        }
                    }
                    "LWPOLYLINE" => {
                        if let Some(entity) = self.read_lwpolyline()? {
                            block_entities.push(EntityType::LwPolyline(entity));
                        }
                    }
                    "TEXT" => {
                        if let Some(entity) = self.read_text()? {
                            block_entities.push(EntityType::Text(entity));
                        }
                    }
                    "MTEXT" => {
                        if let Some(entity) = self.read_mtext()? {
                            block_entities.push(EntityType::MText(entity));
                        }
                    }
                    "SPLINE" => {
                        if let Some(entity) = self.read_spline()? {
                            block_entities.push(EntityType::Spline(entity));
                        }
                    }
                    "DIMENSION" => {
                        if let Some(entity) = self.read_dimension()? {
                            block_entities.push(EntityType::Dimension(entity));
                        }
                    }
                    "HATCH" => {
                        if let Some(entity) = self.read_hatch()? {
                            block_entities.push(EntityType::Hatch(entity));
                        }
                    }
                    "SOLID" | "TRACE" => {
                        if let Some(entity) = self.read_solid()? {
                            block_entities.push(EntityType::Solid(entity));
                        }
                    }
                    "3DFACE" => {
                        if let Some(entity) = self.read_face3d()? {
                            block_entities.push(EntityType::Face3D(entity));
                        }
                    }
                    "INSERT" => {
                        if let Some(entity) = self.read_insert()? {
                            block_entities.push(EntityType::Insert(entity));
                        }
                    }
                    "RAY" => {
                        if let Some(entity) = self.read_ray()? {
                            block_entities.push(EntityType::Ray(entity));
                        }
                    }
                    "XLINE" => {
                        if let Some(entity) = self.read_xline()? {
                            block_entities.push(EntityType::XLine(entity));
                        }
                    }
                    "ATTDEF" => {
                        if let Some(entity) = self.read_attdef()? {
                            block_entities.push(EntityType::AttributeDefinition(entity));
                        }
                    }
                    "ATTRIB" => {
                        if let Some(entity) = self.read_attrib()? {
                            block_entities.push(EntityType::AttributeEntity(entity));
                        }
                    }
                    "TOLERANCE" => {
                        if let Some(entity) = self.read_tolerance()? {
                            block_entities.push(EntityType::Tolerance(entity));
                        }
                    }
                    "SHAPE" => {
                        if let Some(entity) = self.read_shape()? {
                            block_entities.push(EntityType::Shape(entity));
                        }
                    }
                    "WIPEOUT" => {
                        if let Some(entity) = self.read_wipeout()? {
                            block_entities.push(EntityType::Wipeout(entity));
                        }
                    }
                    "VIEWPORT" => {
                        if let Some(entity) = self.read_viewport()? {
                            block_entities.push(EntityType::Viewport(entity));
                        }
                    }
                    "LEADER" => {
                        if let Some(entity) = self.read_leader()? {
                            block_entities.push(EntityType::Leader(entity));
                        }
                    }
                    "MULTILEADER" | "MLEADER" => {
                        if let Some(entity) = self.read_multileader()? {
                            block_entities.push(EntityType::MultiLeader(entity));
                        }
                    }
                    "MLINE" => {
                        if let Some(entity) = self.read_mline()? {
                            block_entities.push(EntityType::MLine(entity));
                        }
                    }
                    "MESH" => {
                        if let Some(entity) = self.read_mesh()? {
                            block_entities.push(EntityType::Mesh(entity));
                        }
                    }
                    "IMAGE" => {
                        if let Some(entity) = self.read_raster_image()? {
                            block_entities.push(EntityType::RasterImage(entity));
                        }
                    }
                    "3DSOLID" => {
                        if let Some(entity) = self.read_solid3d()? {
                            block_entities.push(EntityType::Solid3D(entity));
                        }
                    }
                    "REGION" => {
                        if let Some(entity) = self.read_region()? {
                            block_entities.push(EntityType::Region(entity));
                        }
                    }
                    "BODY" => {
                        if let Some(entity) = self.read_body()? {
                            block_entities.push(EntityType::Body(entity));
                        }
                    }
                    "ACAD_TABLE" | "TABLE" => {
                        if let Some(entity) = self.read_table_entity()? {
                            block_entities.push(EntityType::Table(entity));
                        }
                    }
                    "PDFUNDERLAY" | "DWFUNDERLAY" | "DGNUNDERLAY" => {
                        if let Some(entity) = self.read_underlay(&pair.value_string)? {
                            block_entities.push(EntityType::Underlay(entity));
                        }
                    }
                    _ => {
                        // Skip unknown entity type
                    }
                }
            }
        }

        Ok(())
    }

    /// Read ENDBLK entity
    fn read_block_end(&mut self) -> Result<BlockEnd> {
        use crate::entities::BlockEnd;

        let mut block_end = BlockEnd::new();
        let mut layer = String::from("0");
        let mut handle = Handle::NULL;

        while let Some(pair) = self.reader.read_pair()? {
            match pair.code {
                0 => {
                    // Next entity - push back and break
                    self.reader.push_back(pair);
                    break;
                }
                5 => {
                    // Handle
                    if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) {
                        handle = Handle::new(h);
                    }
                }
                8 => {
                    // Layer
                    layer = pair.value_string.clone();
                }
                _ => {}
            }
        }

        block_end.common.handle = handle;
        block_end.common.layer = layer;

        Ok(block_end)
    }
    
    /// Read the ENTITIES section
    pub fn read_entities(&mut self, document: &mut CadDocument) -> Result<()> {
        // Read entities until ENDSEC
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 && pair.value_string == "ENDSEC" {
                break;
            }
            
            // Entities start with code 0
            if pair.code == 0 {
                let entity_type = pair.value_string.clone();
                
                match entity_type.as_str() {
                    "POINT" => {
                        if let Some(entity) = self.read_point()? {
                            let _ = document.add_entity(EntityType::Point(entity));
                        }
                    }
                    "LINE" => {
                        if let Some(entity) = self.read_line()? {
                            let _ = document.add_entity(EntityType::Line(entity));
                        }
                    }
                    "CIRCLE" => {
                        if let Some(entity) = self.read_circle()? {
                            let _ = document.add_entity(EntityType::Circle(entity));
                        }
                    }
                    "ARC" => {
                        if let Some(entity) = self.read_arc()? {
                            let _ = document.add_entity(EntityType::Arc(entity));
                        }
                    }
                    "ELLIPSE" => {
                        if let Some(entity) = self.read_ellipse()? {
                            let _ = document.add_entity(EntityType::Ellipse(entity));
                        }
                    }
                    "POLYLINE" => {
                        if let Some(entity) = self.read_polyline()? {
                            let _ = document.add_entity(EntityType::Polyline(entity));
                        }
                    }
                    "LWPOLYLINE" => {
                        if let Some(entity) = self.read_lwpolyline()? {
                            let _ = document.add_entity(EntityType::LwPolyline(entity));
                        }
                    }
                    "TEXT" => {
                        if let Some(entity) = self.read_text()? {
                            let _ = document.add_entity(EntityType::Text(entity));
                        }
                    }
                    "MTEXT" => {
                        if let Some(entity) = self.read_mtext()? {
                            let _ = document.add_entity(EntityType::MText(entity));
                        }
                    }
                    "SPLINE" => {
                        if let Some(entity) = self.read_spline()? {
                            let _ = document.add_entity(EntityType::Spline(entity));
                        }
                    }
                    "DIMENSION" => {
                        if let Some(entity) = self.read_dimension()? {
                            let _ = document.add_entity(EntityType::Dimension(entity));
                        }
                    }
                    "HATCH" => {
                        if let Some(entity) = self.read_hatch()? {
                            let _ = document.add_entity(EntityType::Hatch(entity));
                        }
                    }
                    "SOLID" | "TRACE" => {
                        if let Some(entity) = self.read_solid()? {
                            let _ = document.add_entity(EntityType::Solid(entity));
                        }
                    }
                    "3DFACE" => {
                        if let Some(entity) = self.read_face3d()? {
                            let _ = document.add_entity(EntityType::Face3D(entity));
                        }
                    }
                    "INSERT" => {
                        if let Some(entity) = self.read_insert()? {
                            let _ = document.add_entity(EntityType::Insert(entity));
                        }
                    }
                    "RAY" => {
                        if let Some(entity) = self.read_ray()? {
                            let _ = document.add_entity(EntityType::Ray(entity));
                        }
                    }
                    "XLINE" => {
                        if let Some(entity) = self.read_xline()? {
                            let _ = document.add_entity(EntityType::XLine(entity));
                        }
                    }
                    "ATTDEF" => {
                        if let Some(entity) = self.read_attdef()? {
                            let _ = document.add_entity(EntityType::AttributeDefinition(entity));
                        }
                    }
                    "TOLERANCE" => {
                        if let Some(entity) = self.read_tolerance()? {
                            let _ = document.add_entity(EntityType::Tolerance(entity));
                        }
                    }
                    "SHAPE" => {
                        if let Some(entity) = self.read_shape()? {
                            let _ = document.add_entity(EntityType::Shape(entity));
                        }
                    }
                    "WIPEOUT" => {
                        if let Some(entity) = self.read_wipeout()? {
                            let _ = document.add_entity(EntityType::Wipeout(entity));
                        }
                    }
                    "VIEWPORT" => {
                        if let Some(entity) = self.read_viewport()? {
                            let _ = document.add_entity(EntityType::Viewport(entity));
                        }
                    }
                    "ATTRIB" => {
                        if let Some(entity) = self.read_attrib()? {
                            let _ = document.add_entity(EntityType::AttributeEntity(entity));
                        }
                    }
                    "LEADER" => {
                        if let Some(entity) = self.read_leader()? {
                            let _ = document.add_entity(EntityType::Leader(entity));
                        }
                    }
                    "MULTILEADER" | "MLEADER" => {
                        if let Some(entity) = self.read_multileader()? {
                            let _ = document.add_entity(EntityType::MultiLeader(entity));
                        }
                    }
                    "MLINE" => {
                        if let Some(entity) = self.read_mline()? {
                            let _ = document.add_entity(EntityType::MLine(entity));
                        }
                    }
                    "MESH" => {
                        if let Some(entity) = self.read_mesh()? {
                            let _ = document.add_entity(EntityType::Mesh(entity));
                        }
                    }
                    "IMAGE" => {
                        if let Some(entity) = self.read_raster_image()? {
                            let _ = document.add_entity(EntityType::RasterImage(entity));
                        }
                    }
                    "3DSOLID" => {
                        if let Some(entity) = self.read_solid3d()? {
                            let _ = document.add_entity(EntityType::Solid3D(entity));
                        }
                    }
                    "REGION" => {
                        if let Some(entity) = self.read_region()? {
                            let _ = document.add_entity(EntityType::Region(entity));
                        }
                    }
                    "BODY" => {
                        if let Some(entity) = self.read_body()? {
                            let _ = document.add_entity(EntityType::Body(entity));
                        }
                    }
                    "ACAD_TABLE" | "TABLE" => {
                        if let Some(entity) = self.read_table_entity()? {
                            let _ = document.add_entity(EntityType::Table(entity));
                        }
                    }
                    "PDFUNDERLAY" | "DWFUNDERLAY" | "DGNUNDERLAY" => {
                        if let Some(entity) = self.read_underlay(&entity_type)? {
                            let _ = document.add_entity(EntityType::Underlay(entity));
                        }
                    }
                    _ => {
                        // Skip unknown entity type
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Read the OBJECTS section
    pub fn read_objects(&mut self, document: &mut CadDocument) -> Result<()> {
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 && pair.value_string == "ENDSEC" {
                break;
            }

            if pair.code == 0 {
                match pair.value_string.as_str() {
                    "DICTIONARY" | "ACDBDICTIONARYWDFLT" => {
                        if let Some(obj) = self.read_dictionary()? {
                            document.objects.insert(obj.handle, ObjectType::Dictionary(obj));
                        }
                    }
                    "LAYOUT" => {
                        if let Some(obj) = self.read_layout()? {
                            document.objects.insert(obj.handle, ObjectType::Layout(obj));
                        }
                    }
                    "XRECORD" => {
                        if let Some(obj) = self.read_xrecord()? {
                            document.objects.insert(obj.handle, ObjectType::XRecord(obj));
                        }
                    }
                    "GROUP" => {
                        if let Some(obj) = self.read_group()? {
                            document.objects.insert(obj.handle, ObjectType::Group(obj));
                        }
                    }
                    "MLINESTYLE" => {
                        if let Some(obj) = self.read_mlinestyle_object()? {
                            document.objects.insert(obj.handle, ObjectType::MLineStyle(obj));
                        }
                    }
                    "IMAGEDEF" => {
                        if let Some(obj) = self.read_image_definition()? {
                            document.objects.insert(obj.handle, ObjectType::ImageDefinition(obj));
                        }
                    }
                    "MLEADERSTYLE" => {
                        if let Some(obj) = self.read_multileader_style()? {
                            document.objects.insert(obj.handle, ObjectType::MultiLeaderStyle(obj));
                        }
                    }
                    "PLOTSETTINGS" => {
                        if let Some(obj) = self.read_plot_settings()? {
                            document.objects.insert(obj.handle, ObjectType::PlotSettings(obj));
                        }
                    }
                    "TABLESTYLE" => {
                        if let Some(obj) = self.read_table_style()? {
                            document.objects.insert(obj.handle, ObjectType::TableStyle(obj));
                        }
                    }
                    "SCALE" => {
                        if let Some(obj) = self.read_scale()? {
                            document.objects.insert(obj.handle, ObjectType::Scale(obj));
                        }
                    }
                    "SORTENTSTABLE" => {
                        if let Some(obj) = self.read_sort_entities_table()? {
                            document.objects.insert(obj.handle, ObjectType::SortEntitiesTable(obj));
                        }
                    }
                    "DICTIONARYVAR" => {
                        if let Some(obj) = self.read_dictionary_variable()? {
                            document.objects.insert(obj.handle, ObjectType::DictionaryVariable(obj));
                        }
                    }
                    _ => {
                        self.skip_unknown_object(&pair.value_string)?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Read a DICTIONARY object
    fn read_dictionary(&mut self) -> Result<Option<Dictionary>> {
        let mut dict = Dictionary::new();
        let mut current_key: Option<String> = None;

        while let Some(pair) = self.reader.read_pair()? {
            match pair.code {
                0 => {
                    // Next object - push back and break
                    self.reader.push_back(pair);
                    break;
                }
                5 => {
                    // Handle
                    if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) {
                        dict.handle = Handle::new(h);
                    }
                }
                330 => {
                    // Owner handle
                    if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) {
                        dict.owner = Handle::new(h);
                    }
                }
                281 => {
                    // Duplicate record cloning flag
                    if let Some(value) = pair.as_i16() {
                        dict.duplicate_cloning = value;
                    }
                }
                280 => {
                    // Hard owner flag
                    if let Some(value) = pair.as_i16() {
                        dict.hard_owner = value != 0;
                    }
                }
                3 => {
                    // Entry key (name)
                    current_key = Some(pair.value_string.clone());
                }
                350 | 360 => {
                    // Entry value (handle) - 350 is soft owner, 360 is hard owner
                    if let Some(key) = current_key.take() {
                        if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) {
                            dict.add_entry(key, Handle::new(h));
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(Some(dict))
    }

    /// Read a LAYOUT object
    fn read_layout(&mut self) -> Result<Option<Layout>> {
        let mut layout = Layout::new("");

        while let Some(pair) = self.reader.read_pair()? {
            match pair.code {
                0 => {
                    // Next object - push back and break
                    self.reader.push_back(pair);
                    break;
                }
                5 => {
                    // Handle
                    if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) {
                        layout.handle = Handle::new(h);
                    }
                }
                330 => {
                    // Owner handle
                    if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) {
                        layout.owner = Handle::new(h);
                    }
                }
                1 => {
                    // Layout name
                    layout.name = pair.value_string.clone();
                }
                70 => {
                    // Layout flags
                    if let Some(value) = pair.as_i16() {
                        layout.flags = value;
                    }
                }
                71 => {
                    // Tab order
                    if let Some(value) = pair.as_i16() {
                        layout.tab_order = value;
                    }
                }
                10 => {
                    // Min limits X
                    if let Some(value) = pair.as_double() {
                        layout.min_limits.0 = value;
                    }
                }
                20 => {
                    // Min limits Y
                    if let Some(value) = pair.as_double() {
                        layout.min_limits.1 = value;
                    }
                }
                11 => {
                    // Max limits X
                    if let Some(value) = pair.as_double() {
                        layout.max_limits.0 = value;
                    }
                }
                21 => {
                    // Max limits Y
                    if let Some(value) = pair.as_double() {
                        layout.max_limits.1 = value;
                    }
                }
                _ => {}
            }
        }

        Ok(Some(layout))
    }

    /// Skip an unknown object type
    fn skip_unknown_object(&mut self, _type_name: &str) -> Result<()> {
        // Read until next code 0
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }
        }
        Ok(())
    }
    
    /// Skip to ENDTAB
    fn skip_to_endtab(&mut self) -> Result<()> {
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 && pair.value_string == "ENDTAB" {
                break;
            }
        }
        Ok(())
    }

    // ===== Table Readers =====

    /// Read LAYER table
    fn read_layer_table(&mut self, document: &mut CadDocument) -> Result<()> {
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 && pair.value_string == "ENDTAB" {
                break;
            }

            if pair.code == 0 && pair.value_string == "LAYER" {
                if let Some(layer) = self.read_layer_entry()? {
                    let _ = document.layers.add(layer);
                }
            }
        }
        Ok(())
    }

    /// Read a single LAYER entry
    fn read_layer_entry(&mut self) -> Result<Option<Layer>> {
        let mut layer = Layer::new("0");

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                // Next entity - push back and break
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                2 => layer.name = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        layer.color = Color::from_index(color_index);
                    }
                }
                6 => layer.line_type = pair.value_string.clone(),
                70 => {
                    if let Some(flags) = pair.as_i16() {
                        layer.flags.frozen = (flags & 1) != 0;
                        layer.flags.locked = (flags & 4) != 0;
                        layer.flags.off = (flags & 2) != 0;
                    }
                }
                290 => {
                    if let Some(plotting) = pair.as_bool() {
                        layer.is_plottable = plotting;
                    }
                }
                370 => {
                    if let Some(lw) = pair.as_i16() {
                        layer.line_weight = LineWeight::from_value(lw);
                    }
                }
                _ => {}
            }
        }

        Ok(Some(layer))
    }

    /// Read LTYPE table
    fn read_linetype_table(&mut self, document: &mut CadDocument) -> Result<()> {
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 && pair.value_string == "ENDTAB" {
                break;
            }

            if pair.code == 0 && pair.value_string == "LTYPE" {
                if let Some(linetype) = self.read_linetype_entry()? {
                    let _ = document.line_types.add(linetype);
                }
            }
        }
        Ok(())
    }

    /// Read a single LTYPE entry
    fn read_linetype_entry(&mut self) -> Result<Option<LineType>> {
        let mut linetype = LineType::new("Continuous");

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                2 => linetype.name = pair.value_string.clone(),
                3 => linetype.description = pair.value_string.clone(),
                73 => {
                    if let Some(count) = pair.as_i16() {
                        linetype.elements.reserve(count as usize);
                    }
                }
                40 => {
                    if let Some(length) = pair.as_double() {
                        linetype.pattern_length = length;
                    }
                }
                49 => {
                    if let Some(dash) = pair.as_double() {
                        linetype.elements.push(LineTypeElement { length: dash });
                    }
                }
                _ => {}
            }
        }

        Ok(Some(linetype))
    }

    /// Read STYLE table
    fn read_textstyle_table(&mut self, document: &mut CadDocument) -> Result<()> {
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 && pair.value_string == "ENDTAB" {
                break;
            }

            if pair.code == 0 && pair.value_string == "STYLE" {
                if let Some(style) = self.read_textstyle_entry()? {
                    let _ = document.text_styles.add(style);
                }
            }
        }
        Ok(())
    }

    /// Read a single STYLE entry
    fn read_textstyle_entry(&mut self) -> Result<Option<TextStyle>> {
        let mut style = TextStyle::new("Standard");

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                2 => style.name = pair.value_string.clone(),
                3 => style.font_file = pair.value_string.clone(),
                4 => style.big_font_file = pair.value_string.clone(),
                40 => {
                    if let Some(height) = pair.as_double() {
                        style.height = height;
                    }
                }
                41 => {
                    if let Some(width) = pair.as_double() {
                        style.width_factor = width;
                    }
                }
                50 => {
                    if let Some(angle) = pair.as_double() {
                        style.oblique_angle = angle;
                    }
                }
                71 => {
                    if let Some(flags) = pair.as_i16() {
                        style.flags.backward = (flags & 2) != 0;
                        style.flags.upside_down = (flags & 4) != 0;
                    }
                }
                _ => {}
            }
        }

        Ok(Some(style))
    }

    /// Read BLOCK_RECORD table
    fn read_block_record_table(&mut self, document: &mut CadDocument) -> Result<()> {
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 && pair.value_string == "ENDTAB" {
                break;
            }

            if pair.code == 0 && pair.value_string == "BLOCK_RECORD" {
                if let Some(block_record) = self.read_block_record_entry()? {
                    let _ = document.block_records.add(block_record);
                }
            }
        }
        Ok(())
    }

    /// Read a single BLOCK_RECORD entry
    fn read_block_record_entry(&mut self) -> Result<Option<BlockRecord>> {
        let mut block_record = BlockRecord::new("*Model_Space");

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                2 => block_record.name = pair.value_string.clone(),
                70 => {
                    if let Some(flags) = pair.as_i16() {
                        block_record.flags.anonymous = (flags & 1) != 0;
                        block_record.flags.has_attributes = (flags & 2) != 0;
                        block_record.flags.is_xref = (flags & 4) != 0;
                        block_record.flags.is_xref_overlay = (flags & 8) != 0;
                    }
                }
                280 => {
                    if let Some(units) = pair.as_i16() {
                        block_record.units = units;
                    }
                }
                _ => {}
            }
        }

        Ok(Some(block_record))
    }

    /// Read DIMSTYLE table
    fn read_dimstyle_table(&mut self, document: &mut CadDocument) -> Result<()> {
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 && pair.value_string == "ENDTAB" {
                break;
            }

            if pair.code == 0 && pair.value_string == "DIMSTYLE" {
                if let Some(dimstyle) = self.read_dimstyle_entry()? {
                    let _ = document.dim_styles.add(dimstyle);
                }
            }
        }
        Ok(())
    }

    /// Read a single DIMSTYLE entry
    fn read_dimstyle_entry(&mut self) -> Result<Option<DimStyle>> {
        let mut ds = DimStyle::new("Standard");

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                5 => { if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { ds.handle = Handle::new(h); } }
                2 => ds.name = pair.value_string.clone(),
                3 => ds.dimpost = pair.value_string.clone(),
                4 => ds.dimapost = pair.value_string.clone(),
                // Scale / lines
                40 => { if let Some(v) = pair.as_double() { ds.dimscale = v; } }
                41 => { if let Some(v) = pair.as_double() { ds.dimasz = v; } }
                42 => { if let Some(v) = pair.as_double() { ds.dimexo = v; } }
                43 => { if let Some(v) = pair.as_double() { ds.dimdli = v; } }
                44 => { if let Some(v) = pair.as_double() { ds.dimexe = v; } }
                45 => { if let Some(v) = pair.as_double() { ds.dimrnd = v; } }
                46 => { if let Some(v) = pair.as_double() { ds.dimdle = v; } }
                47 => { if let Some(v) = pair.as_double() { ds.dimtp = v; } }
                48 => { if let Some(v) = pair.as_double() { ds.dimtm = v; } }
                49 => { if let Some(v) = pair.as_double() { ds.dimfxl = v; } }
                50 => { if let Some(v) = pair.as_double() { ds.dimjogang = v; } }
                140 => { if let Some(v) = pair.as_double() { ds.dimtxt = v; } }
                141 => { if let Some(v) = pair.as_double() { ds.dimcen = v; } }
                142 => { if let Some(v) = pair.as_double() { ds.dimtsz = v; } }
                143 => { if let Some(v) = pair.as_double() { ds.dimaltf = v; } }
                144 => { if let Some(v) = pair.as_double() { ds.dimlfac = v; } }
                145 => { if let Some(v) = pair.as_double() { ds.dimtvp = v; } }
                146 => { if let Some(v) = pair.as_double() { ds.dimtfac = v; } }
                147 => { if let Some(v) = pair.as_double() { ds.dimgap = v; } }
                148 => { if let Some(v) = pair.as_double() { ds.dimaltrnd = v; } }
                // Integer codes
                69 => { if let Some(v) = pair.as_i16() { ds.dimtfill = v; } }
                71 => { if let Some(v) = pair.as_i16() { ds.dimtol = v != 0; } }
                72 => { if let Some(v) = pair.as_i16() { ds.dimlim = v != 0; } }
                73 => { if let Some(v) = pair.as_i16() { ds.dimtih = v != 0; } }
                74 => { if let Some(v) = pair.as_i16() { ds.dimtoh = v != 0; } }
                75 => { if let Some(v) = pair.as_i16() { ds.dimse1 = v != 0; } }
                76 => { if let Some(v) = pair.as_i16() { ds.dimse2 = v != 0; } }
                77 => { if let Some(v) = pair.as_i16() { ds.dimtad = v; } }
                78 => { if let Some(v) = pair.as_i16() { ds.dimzin = v; } }
                79 => { if let Some(v) = pair.as_i16() { ds.dimazin = v; } }
                90 => { if let Some(v) = pair.as_i32() { ds.dimarcsym = v as i16; } }
                170 => { if let Some(v) = pair.as_i16() { ds.dimalt = v != 0; } }
                171 => { if let Some(v) = pair.as_i16() { ds.dimaltd = v; } }
                172 => { if let Some(v) = pair.as_i16() { ds.dimtofl = v != 0; } }
                173 => { if let Some(v) = pair.as_i16() { ds.dimsah = v != 0; } }
                174 => { if let Some(v) = pair.as_i16() { ds.dimtix = v != 0; } }
                175 => { if let Some(v) = pair.as_i16() { ds.dimsoxd = v != 0; } }
                176 => { if let Some(v) = pair.as_i16() { ds.dimclrd = v; } }
                177 => { if let Some(v) = pair.as_i16() { ds.dimclre = v; } }
                178 => { if let Some(v) = pair.as_i16() { ds.dimclrt = v; } }
                179 => { if let Some(v) = pair.as_i16() { ds.dimadec = v; } }
                270 => { if let Some(v) = pair.as_i16() { ds.dimunit = v; } }
                271 => { if let Some(v) = pair.as_i16() { ds.dimdec = v; } }
                272 => { if let Some(v) = pair.as_i16() { ds.dimtdec = v; } }
                273 => { if let Some(v) = pair.as_i16() { ds.dimaltu = v; } }
                274 => { if let Some(v) = pair.as_i16() { ds.dimalttd = v; } }
                275 => { if let Some(v) = pair.as_i16() { ds.dimaunit = v; } }
                276 => { if let Some(v) = pair.as_i16() { ds.dimfrac = v; } }
                277 => { if let Some(v) = pair.as_i16() { ds.dimlunit = v; } }
                278 => { if let Some(v) = pair.as_i16() { ds.dimdsep = v; } }
                279 => { if let Some(v) = pair.as_i16() { ds.dimtmove = v; } }
                280 => { if let Some(v) = pair.as_i16() { ds.dimjust = v; } }
                281 => { if let Some(v) = pair.as_i16() { ds.dimsd1 = v != 0; } }
                282 => { if let Some(v) = pair.as_i16() { ds.dimsd2 = v != 0; } }
                283 => { if let Some(v) = pair.as_i16() { ds.dimtolj = v; } }
                284 => { if let Some(v) = pair.as_i16() { ds.dimtzin = v; } }
                285 => { if let Some(v) = pair.as_i16() { ds.dimaltz = v; } }
                286 => { if let Some(v) = pair.as_i16() { ds.dimalttz = v; } }
                287 => { if let Some(v) = pair.as_i16() { ds.dimfit = v; } }
                288 => { if let Some(v) = pair.as_i16() { ds.dimupt = v != 0; } }
                289 => { if let Some(v) = pair.as_i16() { ds.dimatfit = v; } }
                290 => { if let Some(v) = pair.as_i16() { ds.dimfxlon = v != 0; } }
                295 => { if let Some(v) = pair.as_i16() { ds.dimtxtdirection = v != 0; } }
                // Handle references
                340 => { if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { ds.dimtxsty_handle = Handle::new(h); } }
                341 => { if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { ds.dimldrblk = Handle::new(h); } }
                342 => { if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { ds.dimblk = Handle::new(h); } }
                343 => { if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { ds.dimblk1 = Handle::new(h); } }
                344 => { if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { ds.dimblk2 = Handle::new(h); } }
                345 => { if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { ds.dimltex_handle = Handle::new(h); } }
                346 => { if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { ds.dimltex1_handle = Handle::new(h); } }
                347 => { if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { ds.dimltex2_handle = Handle::new(h); } }
                371 => { if let Some(v) = pair.as_i16() { ds.dimlwd = v; } }
                372 => { if let Some(v) = pair.as_i16() { ds.dimlwe = v; } }
                _ => {}
            }
        }

        Ok(Some(ds))
    }

    /// Read APPID table
    fn read_appid_table(&mut self, document: &mut CadDocument) -> Result<()> {
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 && pair.value_string == "ENDTAB" {
                break;
            }

            if pair.code == 0 && pair.value_string == "APPID" {
                if let Some(appid) = self.read_appid_entry()? {
                    let _ = document.app_ids.add(appid);
                }
            }
        }
        Ok(())
    }

    /// Read a single APPID entry
    fn read_appid_entry(&mut self) -> Result<Option<AppId>> {
        let mut appid = AppId::new("ACAD");

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            if pair.code == 2 {
                appid.name = pair.value_string.clone();
            }
        }

        Ok(Some(appid))
    }

    /// Read VIEW table
    fn read_view_table(&mut self, document: &mut CadDocument) -> Result<()> {
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 && pair.value_string == "ENDTAB" {
                break;
            }

            if pair.code == 0 && pair.value_string == "VIEW" {
                if let Some(view) = self.read_view_entry()? {
                    let _ = document.views.add(view);
                }
            }
        }
        Ok(())
    }

    /// Read a single VIEW entry
    fn read_view_entry(&mut self) -> Result<Option<View>> {
        let mut view = View::new("*Active");
        let mut center = PointReader::new();
        let mut target = PointReader::new();
        let mut direction = PointReader::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                2 => view.name = pair.value_string.clone(),
                10 | 20 | 30 => { center.add_coordinate(&pair); }
                11 | 21 | 31 => { target.add_coordinate(&pair); }
                12 | 22 | 32 => { direction.add_coordinate(&pair); }
                40 => {
                    if let Some(height) = pair.as_double() {
                        view.height = height;
                    }
                }
                41 => {
                    if let Some(width) = pair.as_double() {
                        view.width = width;
                    }
                }
                _ => {}
            }
        }

        if let Some(pt) = center.get_point() {
            view.center = pt;
        }
        if let Some(pt) = target.get_point() {
            view.target = pt;
        }
        if let Some(pt) = direction.get_point() {
            view.direction = pt;
        }

        Ok(Some(view))
    }

    /// Read VPORT table
    fn read_vport_table(&mut self, document: &mut CadDocument) -> Result<()> {
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 && pair.value_string == "ENDTAB" {
                break;
            }

            if pair.code == 0 && pair.value_string == "VPORT" {
                if let Some(vport) = self.read_vport_entry()? {
                    let _ = document.vports.add(vport);
                }
            }
        }
        Ok(())
    }

    /// Read a single VPORT entry
    fn read_vport_entry(&mut self) -> Result<Option<VPort>> {
        let mut vport = VPort::new("*Active");

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            if pair.code == 2 {
                vport.name = pair.value_string.clone();
            }
        }

        Ok(Some(vport))
    }

    /// Read UCS table
    fn read_ucs_table(&mut self, document: &mut CadDocument) -> Result<()> {
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 && pair.value_string == "ENDTAB" {
                break;
            }

            if pair.code == 0 && pair.value_string == "UCS" {
                if let Some(ucs) = self.read_ucs_entry()? {
                    let _ = document.ucss.add(ucs);
                }
            }
        }
        Ok(())
    }

    /// Read a single UCS entry
    fn read_ucs_entry(&mut self) -> Result<Option<Ucs>> {
        let mut ucs = Ucs::new("World");
        let mut origin = PointReader::new();
        let mut x_axis = PointReader::new();
        let mut y_axis = PointReader::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                2 => ucs.name = pair.value_string.clone(),
                10 | 20 | 30 => { origin.add_coordinate(&pair); }
                11 | 21 | 31 => { x_axis.add_coordinate(&pair); }
                12 | 22 | 32 => { y_axis.add_coordinate(&pair); }
                _ => {}
            }
        }

        if let Some(pt) = origin.get_point() {
            ucs.origin = pt;
        }
        if let Some(pt) = x_axis.get_point() {
            ucs.x_axis = pt;
        }
        if let Some(pt) = y_axis.get_point() {
            ucs.y_axis = pt;
        }

        Ok(Some(ucs))
    }

    // ===== Entity Readers =====

    /// Read a POINT entity
    fn read_point(&mut self) -> Result<Option<Point>> {
        let mut point = Point::new();
        let mut location = PointReader::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => point.common.layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        point.common.color = Color::from_index(color_index);
                    }
                }
                370 => {
                    if let Some(lw) = pair.as_i16() {
                        point.common.line_weight = LineWeight::from_value(lw);
                    }
                }
                10 | 20 | 30 => { location.add_coordinate(&pair); }
                39 => {
                    if let Some(thickness) = pair.as_double() {
                        point.thickness = thickness;
                    }
                }
                _ => {}
            }
        }

        if let Some(pt) = location.get_point() {
            point.location = pt;
        }

        Ok(Some(point))
    }

    /// Read a LINE entity
    fn read_line(&mut self) -> Result<Option<Line>> {
        let mut line = Line::new();
        let mut start = PointReader::new();
        let mut end = PointReader::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                // Push back the code 0 pair so it can be read by the caller
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => line.common.layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        line.common.color = Color::from_index(color_index);
                    }
                }
                370 => {
                    if let Some(lw) = pair.as_i16() {
                        line.common.line_weight = LineWeight::from_value(lw);
                    }
                }
                10 | 20 | 30 => { start.add_coordinate(&pair); }
                11 | 21 | 31 => { end.add_coordinate(&pair); }
                39 => {
                    if let Some(thickness) = pair.as_double() {
                        line.thickness = thickness;
                    }
                }
                // Extended data - read and store
                1001 => {
                    // Push back the pair and read XDATA
                    self.reader.push_back(pair);
                    let (extended_data, _next_pair) = self.read_extended_data()?;
                    line.common.extended_data = extended_data;
                }
                _ => {}
            }
        }

        if let Some(pt) = start.get_point() {
            line.start = pt;
        }
        if let Some(pt) = end.get_point() {
            line.end = pt;
        }

        Ok(Some(line))
    }

    /// Read a CIRCLE entity
    fn read_circle(&mut self) -> Result<Option<Circle>> {
        let mut circle = Circle::new();
        let mut center = PointReader::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => circle.common.layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        circle.common.color = Color::from_index(color_index);
                    }
                }
                370 => {
                    if let Some(lw) = pair.as_i16() {
                        circle.common.line_weight = LineWeight::from_value(lw);
                    }
                }
                10 | 20 | 30 => { center.add_coordinate(&pair); }
                40 => {
                    if let Some(radius) = pair.as_double() {
                        circle.radius = radius;
                    }
                }
                39 => {
                    if let Some(thickness) = pair.as_double() {
                        circle.thickness = thickness;
                    }
                }
                _ => {}
            }
        }

        if let Some(pt) = center.get_point() {
            circle.center = pt;
        }

        Ok(Some(circle))
    }

    /// Read an ARC entity
    fn read_arc(&mut self) -> Result<Option<Arc>> {
        let mut arc = Arc::new();
        let mut center = PointReader::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => arc.common.layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        arc.common.color = Color::from_index(color_index);
                    }
                }
                370 => {
                    if let Some(lw) = pair.as_i16() {
                        arc.common.line_weight = LineWeight::from_value(lw);
                    }
                }
                10 | 20 | 30 => { center.add_coordinate(&pair); }
                40 => {
                    if let Some(radius) = pair.as_double() {
                        arc.radius = radius;
                    }
                }
                50 => {
                    if let Some(angle) = pair.as_double() {
                        arc.start_angle = angle;
                    }
                }
                51 => {
                    if let Some(angle) = pair.as_double() {
                        arc.end_angle = angle;
                    }
                }
                39 => {
                    if let Some(thickness) = pair.as_double() {
                        arc.thickness = thickness;
                    }
                }
                _ => {}
            }
        }

        if let Some(pt) = center.get_point() {
            arc.center = pt;
        }

        Ok(Some(arc))
    }

    /// Read an ELLIPSE entity
    fn read_ellipse(&mut self) -> Result<Option<Ellipse>> {
        let mut ellipse = Ellipse::new();
        let mut center = PointReader::new();
        let mut major_axis = PointReader::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => ellipse.common.layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        ellipse.common.color = Color::from_index(color_index);
                    }
                }
                370 => {
                    if let Some(lw) = pair.as_i16() {
                        ellipse.common.line_weight = LineWeight::from_value(lw);
                    }
                }
                10 | 20 | 30 => { center.add_coordinate(&pair); }
                11 | 21 | 31 => { major_axis.add_coordinate(&pair); }
                40 => {
                    if let Some(ratio) = pair.as_double() {
                        ellipse.minor_axis_ratio = ratio;
                    }
                }
                41 => {
                    if let Some(angle) = pair.as_double() {
                        ellipse.start_parameter = angle;
                    }
                }
                42 => {
                    if let Some(angle) = pair.as_double() {
                        ellipse.end_parameter = angle;
                    }
                }
                _ => {}
            }
        }

        if let Some(pt) = center.get_point() {
            ellipse.center = pt;
        }
        if let Some(pt) = major_axis.get_point() {
            ellipse.major_axis = pt;
        }

        Ok(Some(ellipse))
    }

    /// Read a POLYLINE entity
    fn read_polyline(&mut self) -> Result<Option<Polyline>> {
        use crate::entities::polyline::Vertex3D;

        let mut polyline = Polyline::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                // Check if it's a VERTEX or SEQEND
                if pair.value_string == "VERTEX" {
                    // Read vertex
                    let mut vertex_reader = PointReader::new();

                    while let Some(vpair) = self.reader.read_pair()? {
                        if vpair.code == 0 {
                            self.reader.push_back(vpair);
                            break;
                        }
                        match vpair.code {
                            10 | 20 | 30 => { vertex_reader.add_coordinate(&vpair); }
                            _ => {}
                        }
                    }

                    if let Some(pt) = vertex_reader.get_point() {
                        polyline.vertices.push(Vertex3D::new(pt));
                    }
                } else if pair.value_string == "SEQEND" {
                    // End of polyline - skip SEQEND properties
                    while let Some(seqend_pair) = self.reader.read_pair()? {
                        if seqend_pair.code == 0 {
                            self.reader.push_back(seqend_pair);
                            break;
                        }
                    }
                    break;
                } else {
                    // End of polyline, different entity - push back
                    self.reader.push_back(pair);
                    break;
                }
            } else {
                match pair.code {
                    8 => polyline.common.layer = pair.value_string.clone(),
                    62 => {
                        if let Some(color_index) = pair.as_i16() {
                            polyline.common.color = Color::from_index(color_index);
                        }
                    }
                    370 => {
                        if let Some(lw) = pair.as_i16() {
                            polyline.common.line_weight = LineWeight::from_value(lw);
                        }
                    }
                    70 => {
                        if let Some(flags) = pair.as_i16() {
                            if (flags & 1) != 0 {
                                polyline.close();
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(Some(polyline))
    }

    /// Read an LWPOLYLINE entity
    fn read_lwpolyline(&mut self) -> Result<Option<LwPolyline>> {
        use crate::entities::lwpolyline::LwVertex;
        use crate::types::Vector2;

        let mut lwpolyline = LwPolyline::new();
        let mut vertices_x: Vec<f64> = Vec::new();
        let mut vertices_y: Vec<f64> = Vec::new();
        let mut bulges: Vec<f64> = Vec::new();
        let mut widths_start: Vec<f64> = Vec::new();
        let mut widths_end: Vec<f64> = Vec::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => lwpolyline.common.layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        lwpolyline.common.color = Color::from_index(color_index);
                    }
                }
                370 => {
                    if let Some(lw) = pair.as_i16() {
                        lwpolyline.common.line_weight = LineWeight::from_value(lw);
                    }
                }
                70 => {
                    if let Some(flags) = pair.as_i16() {
                        lwpolyline.is_closed = (flags & 1) != 0;
                    }
                }
                38 => {
                    if let Some(elevation) = pair.as_double() {
                        lwpolyline.elevation = elevation;
                    }
                }
                10 => {
                    if let Some(x) = pair.as_double() {
                        vertices_x.push(x);
                    }
                }
                20 => {
                    if let Some(y) = pair.as_double() {
                        vertices_y.push(y);
                    }
                }
                42 => {
                    if let Some(bulge) = pair.as_double() {
                        bulges.push(bulge);
                    }
                }
                40 => {
                    if let Some(width) = pair.as_double() {
                        widths_start.push(width);
                    }
                }
                41 => {
                    if let Some(width) = pair.as_double() {
                        widths_end.push(width);
                    }
                }
                _ => {}
            }
        }

        // Build vertices from collected data
        for i in 0..vertices_x.len().min(vertices_y.len()) {
            let bulge = bulges.get(i).copied().unwrap_or(0.0);
            let start_width = widths_start.get(i).copied().unwrap_or(0.0);
            let end_width = widths_end.get(i).copied().unwrap_or(0.0);

            lwpolyline.vertices.push(LwVertex {
                location: Vector2::new(vertices_x[i], vertices_y[i]),
                bulge,
                start_width,
                end_width,
            });
        }

        Ok(Some(lwpolyline))
    }

    /// Read a TEXT entity
    fn read_text(&mut self) -> Result<Option<Text>> {
        let mut text = Text::new();
        let mut insertion = PointReader::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => text.common.layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        text.common.color = Color::from_index(color_index);
                    }
                }
                370 => {
                    if let Some(lw) = pair.as_i16() {
                        text.common.line_weight = LineWeight::from_value(lw);
                    }
                }
                10 | 20 | 30 => { insertion.add_coordinate(&pair); }
                1 => text.value = pair.value_string.clone(),
                40 => {
                    if let Some(height) = pair.as_double() {
                        text.height = height;
                    }
                }
                50 => {
                    if let Some(rotation) = pair.as_double() {
                        text.rotation = rotation;
                    }
                }
                41 => {
                    if let Some(width_factor) = pair.as_double() {
                        text.width_factor = width_factor;
                    }
                }
                51 => {
                    if let Some(oblique) = pair.as_double() {
                        text.oblique_angle = oblique;
                    }
                }
                7 => text.style = pair.value_string.clone(),
                _ => {}
            }
        }

        if let Some(pt) = insertion.get_point() {
            text.insertion_point = pt;
        }

        Ok(Some(text))
    }

    /// Read an MTEXT entity
    fn read_mtext(&mut self) -> Result<Option<MText>> {
        let mut mtext = MText::new();
        let mut insertion = PointReader::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => mtext.common.layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        mtext.common.color = Color::from_index(color_index);
                    }
                }
                370 => {
                    if let Some(lw) = pair.as_i16() {
                        mtext.common.line_weight = LineWeight::from_value(lw);
                    }
                }
                10 | 20 | 30 => { insertion.add_coordinate(&pair); }
                1 | 3 => {
                    // Text content (can be split across multiple codes)
                    mtext.value.push_str(&pair.value_string);
                }
                40 => {
                    if let Some(height) = pair.as_double() {
                        mtext.height = height;
                    }
                }
                41 => {
                    if let Some(width) = pair.as_double() {
                        mtext.rectangle_width = width;
                    }
                }
                50 => {
                    if let Some(rotation) = pair.as_double() {
                        mtext.rotation = rotation;
                    }
                }
                7 => mtext.style = pair.value_string.clone(),
                _ => {}
            }
        }

        if let Some(pt) = insertion.get_point() {
            mtext.insertion_point = pt;
        }

        Ok(Some(mtext))
    }

    /// Read a SPLINE entity
    fn read_spline(&mut self) -> Result<Option<Spline>> {
        let mut spline = Spline::new();
        let mut current_control_point = PointReader::new();
        let mut current_fit_point = PointReader::new();
        let mut reading_control = false;
        let mut reading_fit = false;

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => spline.common.layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        spline.common.color = Color::from_index(color_index);
                    }
                }
                370 => {
                    if let Some(lw) = pair.as_i16() {
                        spline.common.line_weight = LineWeight::from_value(lw);
                    }
                }
                70 => {
                    if let Some(flags_val) = pair.as_i16() {
                        spline.flags.closed = (flags_val & 1) != 0;
                        spline.flags.periodic = (flags_val & 2) != 0;
                        spline.flags.rational = (flags_val & 4) != 0;
                    }
                }
                71 => {
                    if let Some(degree) = pair.as_i16() {
                        spline.degree = degree as i32;
                    }
                }
                40 => {
                    if let Some(knot) = pair.as_double() {
                        spline.knots.push(knot);
                    }
                }
                10 | 20 | 30 => {
                    // Control point coordinates
                    if pair.code == 10 {
                        // Save previous control point if complete
                        if reading_control {
                            if let Some(pt) = current_control_point.get_point() {
                                spline.control_points.push(pt);
                            }
                        }
                        current_control_point = PointReader::new();
                        reading_control = true;
                    }
                    current_control_point.add_coordinate(&pair);
                }
                11 | 21 | 31 => {
                    // Fit point coordinates
                    if pair.code == 11 {
                        // Save previous fit point if complete
                        if reading_fit {
                            if let Some(pt) = current_fit_point.get_point() {
                                spline.fit_points.push(pt);
                            }
                        }
                        current_fit_point = PointReader::new();
                        reading_fit = true;
                    }
                    current_fit_point.add_coordinate(&pair);
                }
                _ => {}
            }
        }

        // Save last control point if any
        if reading_control {
            if let Some(pt) = current_control_point.get_point() {
                spline.control_points.push(pt);
            }
        }

        // Save last fit point if any
        if reading_fit {
            if let Some(pt) = current_fit_point.get_point() {
                spline.fit_points.push(pt);
            }
        }

        Ok(Some(spline))
    }

    /// Read a DIMENSION entity
    fn read_dimension(&mut self) -> Result<Option<Dimension>> {
        use crate::entities::dimension::*;

        let mut dim_type = DimensionType::Linear;
        let mut definition_point = PointReader::new();
        let mut text_middle_point = PointReader::new();
        let mut insertion_point = PointReader::new();
        let mut first_point = PointReader::new();
        let mut second_point = PointReader::new();
        let mut third_point = PointReader::new();
        let mut fourth_point = PointReader::new();
        let mut text = String::new();
        let mut style_name = String::from("Standard");
        let mut layer = String::from("0");
        let mut color = Color::ByLayer;
        let mut line_weight = LineWeight::ByLayer;
        let mut rotation = 0.0;
        let mut actual_measurement = 0.0;
        let mut leader_length = 0.0;

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        color = Color::from_index(color_index);
                    }
                }
                370 => {
                    if let Some(lw) = pair.as_i16() {
                        line_weight = LineWeight::from_value(lw);
                    }
                }
                70 => {
                    if let Some(type_val) = pair.as_i16() {
                        dim_type = match type_val & 0x0F {
                            0 => DimensionType::Linear,
                            1 => DimensionType::Aligned,
                            2 => DimensionType::Angular,
                            3 => DimensionType::Diameter,
                            4 => DimensionType::Radius,
                            5 => DimensionType::Angular3Point,
                            6 => DimensionType::Ordinate,
                            _ => DimensionType::Linear,
                        };
                    }
                }
                1 => text = pair.value_string.clone(),
                3 => style_name = pair.value_string.clone(),
                10 | 20 | 30 => { definition_point.add_coordinate(&pair); }
                11 | 21 | 31 => { text_middle_point.add_coordinate(&pair); }
                12 | 22 | 32 => { insertion_point.add_coordinate(&pair); }
                13 | 23 | 33 => { first_point.add_coordinate(&pair); }
                14 | 24 | 34 => { second_point.add_coordinate(&pair); }
                15 | 25 | 35 => { third_point.add_coordinate(&pair); }
                16 | 26 | 36 => { fourth_point.add_coordinate(&pair); }
                50 => {
                    if let Some(rot) = pair.as_double() {
                        rotation = rot;
                    }
                }
                42 => {
                    if let Some(measurement) = pair.as_double() {
                        actual_measurement = measurement;
                    }
                }
                40 => {
                    if let Some(length) = pair.as_double() {
                        leader_length = length;
                    }
                }
                _ => {}
            }
        }

        // Build the appropriate dimension type
        let pt1 = first_point.get_point().unwrap_or(Vector3::zero());
        let pt2 = second_point.get_point().unwrap_or(Vector3::zero());
        let pt3 = third_point.get_point().unwrap_or(Vector3::zero());
        let _pt4 = fourth_point.get_point().unwrap_or(Vector3::zero());

        let dimension = match dim_type {
            DimensionType::Aligned => {
                let mut dim = DimensionAligned::new(pt1, pt2);
                dim.base.common.layer = layer;
                dim.base.common.color = color;
                dim.base.common.line_weight = line_weight;
                dim.base.text = text;
                dim.base.style_name = style_name;
                dim.base.actual_measurement = actual_measurement;
                if let Some(def_pt) = definition_point.get_point() {
                    dim.definition_point = def_pt;
                }
                Dimension::Aligned(dim)
            }
            DimensionType::Linear => {
                let mut dim = DimensionLinear::rotated(pt1, pt2, rotation);
                dim.base.common.layer = layer;
                dim.base.common.color = color;
                dim.base.common.line_weight = line_weight;
                dim.base.text = text;
                dim.base.style_name = style_name;
                dim.base.actual_measurement = actual_measurement;
                if let Some(def_pt) = definition_point.get_point() {
                    dim.definition_point = def_pt;
                }
                Dimension::Linear(dim)
            }
            DimensionType::Radius => {
                let center = pt1;
                let chord_point = pt2;
                let mut dim = DimensionRadius::new(center, chord_point);
                dim.base.common.layer = layer;
                dim.base.common.color = color;
                dim.base.common.line_weight = line_weight;
                dim.base.text = text;
                dim.base.style_name = style_name;
                dim.base.actual_measurement = actual_measurement;
                dim.leader_length = leader_length;
                Dimension::Radius(dim)
            }
            DimensionType::Diameter => {
                let center = pt1;
                let point_on_arc = pt2;
                let mut dim = DimensionDiameter::new(center, point_on_arc);
                dim.base.common.layer = layer;
                dim.base.common.color = color;
                dim.base.common.line_weight = line_weight;
                dim.base.text = text;
                dim.base.style_name = style_name;
                dim.base.actual_measurement = actual_measurement;
                Dimension::Diameter(dim)
            }
            DimensionType::Angular => {
                // Angular2Ln: vertex, first_point, second_point
                let mut dim = DimensionAngular2Ln::new(pt1, pt2, pt3);
                dim.base.common.layer = layer;
                dim.base.common.color = color;
                dim.base.common.line_weight = line_weight;
                dim.base.text = text;
                dim.base.style_name = style_name;
                dim.base.actual_measurement = actual_measurement;
                Dimension::Angular2Ln(dim)
            }
            DimensionType::Angular3Point => {
                // Angular3Pt: center, first_point, second_point
                let mut dim = DimensionAngular3Pt::new(pt1, pt2, pt3);
                dim.base.common.layer = layer;
                dim.base.common.color = color;
                dim.base.common.line_weight = line_weight;
                dim.base.text = text;
                dim.base.style_name = style_name;
                dim.base.actual_measurement = actual_measurement;
                Dimension::Angular3Pt(dim)
            }
            DimensionType::Ordinate => {
                // Ordinate: feature_location, leader_endpoint
                // Use x_ordinate by default (could be determined from flags in real implementation)
                let mut dim = DimensionOrdinate::x_ordinate(pt1, pt2);
                dim.base.common.layer = layer;
                dim.base.common.color = color;
                dim.base.common.line_weight = line_weight;
                dim.base.text = text;
                dim.base.style_name = style_name;
                dim.base.actual_measurement = actual_measurement;
                Dimension::Ordinate(dim)
            }
        };

        Ok(Some(dimension))
    }

    /// Read a HATCH entity
    fn read_hatch(&mut self) -> Result<Option<Hatch>> {
        use crate::entities::hatch::*;

        let mut hatch = Hatch::new();
        let mut pattern_name = String::from("SOLID");
        let mut pattern_type = HatchPatternType::Predefined;
        let mut layer = String::from("0");
        let mut color = Color::ByLayer;
        let mut line_weight = LineWeight::ByLayer;
        let mut _num_boundary_paths = 0;
        let mut current_path_edges: Vec<BoundaryEdge> = Vec::new();
        let mut reading_boundary = false;

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        color = Color::from_index(color_index);
                    }
                }
                370 => {
                    if let Some(lw) = pair.as_i16() {
                        line_weight = LineWeight::from_value(lw);
                    }
                }
                2 => pattern_name = pair.value_string.clone(),
                70 => {
                    if let Some(solid_fill) = pair.as_i16() {
                        hatch.is_solid = solid_fill != 0;
                    }
                }
                71 => {
                    if let Some(associative) = pair.as_i16() {
                        hatch.is_associative = associative != 0;
                    }
                }
                75 => {
                    if let Some(style) = pair.as_i16() {
                        hatch.style = match style {
                            0 => HatchStyleType::Normal,
                            1 => HatchStyleType::Outer,
                            2 => HatchStyleType::Ignore,
                            _ => HatchStyleType::Normal,
                        };
                    }
                }
                76 => {
                    if let Some(ptype) = pair.as_i16() {
                        pattern_type = match ptype {
                            0 => HatchPatternType::UserDefined,
                            1 => HatchPatternType::Predefined,
                            2 => HatchPatternType::Custom,
                            _ => HatchPatternType::Predefined,
                        };
                    }
                }
                91 => {
                    if let Some(num_paths) = pair.as_i32() {
                        _num_boundary_paths = num_paths;
                    }
                }
                92 => {
                    // Boundary path type flags - indicates start of a new boundary path
                    if reading_boundary && !current_path_edges.is_empty() {
                        // Save previous path
                        let path = BoundaryPath {
                            flags: BoundaryPathFlags::new(),
                            edges: current_path_edges.clone(),
                            boundary_handles: Vec::new(),
                        };
                        hatch.paths.push(path);
                        current_path_edges.clear();
                    }
                    reading_boundary = true;
                }
                72 => {
                    // Edge type - indicates start of a new edge
                    if let Some(edge_type) = pair.as_i16() {
                        match edge_type {
                            1 => {
                                // Line edge - will be populated by subsequent codes
                                current_path_edges.push(BoundaryEdge::Line(LineEdge {
                                    start: Vector2::new(0.0, 0.0),
                                    end: Vector2::new(0.0, 0.0),
                                }));
                            }
                            2 => {
                                // Circular arc edge
                                current_path_edges.push(BoundaryEdge::CircularArc(CircularArcEdge {
                                    center: Vector2::new(0.0, 0.0),
                                    radius: 0.0,
                                    start_angle: 0.0,
                                    end_angle: 0.0,
                                    counter_clockwise: true,
                                }));
                            }
                            3 => {
                                // Elliptic arc edge
                                current_path_edges.push(BoundaryEdge::EllipticArc(EllipticArcEdge {
                                    center: Vector2::new(0.0, 0.0),
                                    major_axis_endpoint: Vector2::new(1.0, 0.0),
                                    minor_axis_ratio: 1.0,
                                    start_angle: 0.0,
                                    end_angle: 0.0,
                                    counter_clockwise: true,
                                }));
                            }
                            4 => {
                                // Spline edge
                                current_path_edges.push(BoundaryEdge::Spline(SplineEdge {
                                    degree: 3,
                                    rational: false,
                                    periodic: false,
                                    knots: Vec::new(),
                                    control_points: Vec::new(),
                                    fit_points: Vec::new(),
                                    start_tangent: Vector2::new(0.0, 0.0),
                                    end_tangent: Vector2::new(0.0, 0.0),
                                }));
                            }
                            _ => {}
                        }
                    }
                }
                // Note: Full hatch reading would require reading all edge data (codes 10-40, etc.)
                // For now, we create a basic hatch structure
                _ => {}
            }
        }

        // Save last boundary path if any
        if reading_boundary && !current_path_edges.is_empty() {
            let path = BoundaryPath {
                flags: BoundaryPathFlags::new(),
                edges: current_path_edges,
                boundary_handles: Vec::new(),
            };
            hatch.paths.push(path);
        }

        hatch.common.layer = layer;
        hatch.common.color = color;
        hatch.common.line_weight = line_weight;
        hatch.pattern.name = pattern_name;
        hatch.pattern_type = pattern_type;

        Ok(Some(hatch))
    }

    /// Read a SOLID entity
    fn read_solid(&mut self) -> Result<Option<Solid>> {
        let mut corner1 = PointReader::new();
        let mut corner2 = PointReader::new();
        let mut corner3 = PointReader::new();
        let mut corner4 = PointReader::new();
        let mut layer = String::from("0");
        let mut color = Color::ByLayer;
        let mut line_weight = LineWeight::ByLayer;

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        color = Color::from_index(color_index);
                    }
                }
                370 => {
                    if let Some(lw) = pair.as_i16() {
                        line_weight = LineWeight::from_value(lw);
                    }
                }
                10 | 20 | 30 => { corner1.add_coordinate(&pair); }
                11 | 21 | 31 => { corner2.add_coordinate(&pair); }
                12 | 22 | 32 => { corner3.add_coordinate(&pair); }
                13 | 23 | 33 => { corner4.add_coordinate(&pair); }
                _ => {}
            }
        }

        let pt1 = corner1.get_point().unwrap_or(Vector3::zero());
        let pt2 = corner2.get_point().unwrap_or(Vector3::zero());
        let pt3 = corner3.get_point().unwrap_or(Vector3::zero());
        let pt4 = corner4.get_point().unwrap_or(pt3);

        let mut solid = Solid::new(pt1, pt2, pt3, pt4);
        solid.common.layer = layer;
        solid.common.color = color;
        solid.common.line_weight = line_weight;

        Ok(Some(solid))
    }

    /// Read a 3DFACE entity
    fn read_face3d(&mut self) -> Result<Option<Face3D>> {
        let mut corner1 = PointReader::new();
        let mut corner2 = PointReader::new();
        let mut corner3 = PointReader::new();
        let mut corner4 = PointReader::new();
        let mut layer = String::from("0");
        let mut color = Color::ByLayer;
        let mut line_weight = LineWeight::ByLayer;
        let mut invisible_flags = 0i16;

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        color = Color::from_index(color_index);
                    }
                }
                370 => {
                    if let Some(lw) = pair.as_i16() {
                        line_weight = LineWeight::from_value(lw);
                    }
                }
                10 | 20 | 30 => { corner1.add_coordinate(&pair); }
                11 | 21 | 31 => { corner2.add_coordinate(&pair); }
                12 | 22 | 32 => { corner3.add_coordinate(&pair); }
                13 | 23 | 33 => { corner4.add_coordinate(&pair); }
                70 => {
                    if let Some(flags) = pair.as_i16() {
                        invisible_flags = flags;
                    }
                }
                _ => {}
            }
        }

        let pt1 = corner1.get_point().unwrap_or(Vector3::zero());
        let pt2 = corner2.get_point().unwrap_or(Vector3::zero());
        let pt3 = corner3.get_point().unwrap_or(Vector3::zero());
        let pt4 = corner4.get_point().unwrap_or(pt3);

        use crate::entities::face3d::InvisibleEdgeFlags;
        let mut invisible_edges = InvisibleEdgeFlags::new();
        invisible_edges.set_first_invisible((invisible_flags & 1) != 0);
        invisible_edges.set_second_invisible((invisible_flags & 2) != 0);
        invisible_edges.set_third_invisible((invisible_flags & 4) != 0);
        invisible_edges.set_fourth_invisible((invisible_flags & 8) != 0);

        let mut face = Face3D::new(pt1, pt2, pt3, pt4);
        face.common.layer = layer;
        face.common.color = color;
        face.common.line_weight = line_weight;
        face.invisible_edges = invisible_edges;

        Ok(Some(face))
    }

    /// Read an INSERT entity
    fn read_insert(&mut self) -> Result<Option<Insert>> {
        let mut block_name = String::new();
        let mut insertion = PointReader::new();
        let mut x_scale = 1.0;
        let mut y_scale = 1.0;
        let mut z_scale = 1.0;
        let mut rotation = 0.0;
        let mut column_count = 1u16;
        let mut row_count = 1u16;
        let mut column_spacing = 0.0;
        let mut row_spacing = 0.0;
        let mut layer = String::from("0");
        let mut color = Color::ByLayer;
        let mut line_weight = LineWeight::ByLayer;

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        color = Color::from_index(color_index);
                    }
                }
                370 => {
                    if let Some(lw) = pair.as_i16() {
                        line_weight = LineWeight::from_value(lw);
                    }
                }
                2 => block_name = pair.value_string.clone(),
                10 | 20 | 30 => { insertion.add_coordinate(&pair); }
                41 => {
                    if let Some(sx) = pair.as_double() {
                        x_scale = sx;
                    }
                }
                42 => {
                    if let Some(sy) = pair.as_double() {
                        y_scale = sy;
                    }
                }
                43 => {
                    if let Some(sz) = pair.as_double() {
                        z_scale = sz;
                    }
                }
                50 => {
                    if let Some(rot) = pair.as_double() {
                        rotation = rot;
                    }
                }
                70 => {
                    if let Some(col_count) = pair.as_i16() {
                        column_count = col_count.max(1) as u16;
                    }
                }
                71 => {
                    if let Some(r_count) = pair.as_i16() {
                        row_count = r_count.max(1) as u16;
                    }
                }
                44 => {
                    if let Some(col_spacing_val) = pair.as_double() {
                        column_spacing = col_spacing_val;
                    }
                }
                45 => {
                    if let Some(row_spacing_val) = pair.as_double() {
                        row_spacing = row_spacing_val;
                    }
                }
                _ => {}
            }
        }

        let insert_point = insertion.get_point().unwrap_or(Vector3::zero());
        let mut insert = Insert::new(block_name, insert_point);
        insert.common.layer = layer;
        insert.common.color = color;
        insert.common.line_weight = line_weight;
        insert.x_scale = x_scale;
        insert.y_scale = y_scale;
        insert.z_scale = z_scale;
        insert.rotation = rotation;
        insert.column_count = column_count;
        insert.row_count = row_count;
        insert.column_spacing = column_spacing;
        insert.row_spacing = row_spacing;

        Ok(Some(insert))
    }

    /// Read a RAY entity
    fn read_ray(&mut self) -> Result<Option<Ray>> {
        let mut base_point = PointReader::new();
        let mut direction = PointReader::new();
        let mut layer = String::from("0");
        let mut color = Color::ByLayer;

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        color = Color::from_index(color_index);
                    }
                }
                10 | 20 | 30 => { base_point.add_coordinate(&pair); }
                11 | 21 | 31 => { direction.add_coordinate(&pair); }
                _ => {}
            }
        }

        let bp = base_point.get_point().unwrap_or(Vector3::zero());
        let dir = direction.get_point().unwrap_or(Vector3::new(1.0, 0.0, 0.0));
        let mut ray = Ray::new(bp, dir);
        ray.common.layer = layer;
        ray.common.color = color;

        Ok(Some(ray))
    }

    /// Read an XLINE entity
    fn read_xline(&mut self) -> Result<Option<XLine>> {
        let mut base_point = PointReader::new();
        let mut direction = PointReader::new();
        let mut layer = String::from("0");
        let mut color = Color::ByLayer;

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        color = Color::from_index(color_index);
                    }
                }
                10 | 20 | 30 => { base_point.add_coordinate(&pair); }
                11 | 21 | 31 => { direction.add_coordinate(&pair); }
                _ => {}
            }
        }

        let bp = base_point.get_point().unwrap_or(Vector3::zero());
        let dir = direction.get_point().unwrap_or(Vector3::new(1.0, 0.0, 0.0));
        let mut xline = XLine::new(bp, dir);
        xline.common.layer = layer;
        xline.common.color = color;

        Ok(Some(xline))
    }

    /// Read an ATTDEF entity
    fn read_attdef(&mut self) -> Result<Option<AttributeDefinition>> {
        let mut tag = String::new();
        let mut prompt = String::new();
        let mut default_value = String::new();
        let mut insertion_point = PointReader::new();
        let mut height = 0.0;
        let mut rotation = 0.0;
        let mut layer = String::from("0");
        let mut color = Color::ByLayer;

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        color = Color::from_index(color_index);
                    }
                }
                1 => default_value = pair.value_string.clone(),
                2 => tag = pair.value_string.clone(),
                3 => prompt = pair.value_string.clone(),
                10 | 20 | 30 => { insertion_point.add_coordinate(&pair); }
                40 => {
                    if let Some(h) = pair.as_double() {
                        height = h;
                    }
                }
                50 => {
                    if let Some(r) = pair.as_double() {
                        rotation = r;
                    }
                }
                _ => {}
            }
        }

        let mut attdef = AttributeDefinition::new(tag, prompt, default_value);
        attdef.insertion_point = insertion_point.get_point().unwrap_or(Vector3::zero());
        attdef.height = height;
        attdef.rotation = rotation;
        attdef.common.layer = layer;
        attdef.common.color = color;

        Ok(Some(attdef))
    }

    /// Read a TOLERANCE entity
    fn read_tolerance(&mut self) -> Result<Option<Tolerance>> {
        let mut tolerance = Tolerance::new();
        let mut insertion_point = PointReader::new();
        let mut direction = PointReader::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => tolerance.common.layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        tolerance.common.color = Color::from_index(color_index);
                    }
                }
                1 => tolerance.text = pair.value_string.clone(),
                3 => tolerance.dimension_style_name = pair.value_string.clone(),
                10 | 20 | 30 => { insertion_point.add_coordinate(&pair); }
                11 | 21 | 31 => { direction.add_coordinate(&pair); }
                _ => {}
            }
        }

        tolerance.insertion_point = insertion_point.get_point().unwrap_or(Vector3::zero());
        tolerance.direction = direction.get_point().unwrap_or(Vector3::new(1.0, 0.0, 0.0));

        Ok(Some(tolerance))
    }

    /// Read a SHAPE entity
    fn read_shape(&mut self) -> Result<Option<Shape>> {
        let mut shape = Shape::new();
        let mut insertion_point = PointReader::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => shape.common.layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        shape.common.color = Color::from_index(color_index);
                    }
                }
                2 => shape.shape_name = pair.value_string.clone(),
                10 | 20 | 30 => { insertion_point.add_coordinate(&pair); }
                40 => {
                    if let Some(s) = pair.as_double() {
                        shape.size = s;
                    }
                }
                50 => {
                    if let Some(r) = pair.as_double() {
                        shape.rotation = r;
                    }
                }
                _ => {}
            }
        }

        shape.insertion_point = insertion_point.get_point().unwrap_or(Vector3::zero());

        Ok(Some(shape))
    }

    /// Read a WIPEOUT entity
    fn read_wipeout(&mut self) -> Result<Option<Wipeout>> {
        let mut wipeout = Wipeout::new();
        let mut insertion_point = PointReader::new();
        let mut u_vector = PointReader::new();
        let mut v_vector = PointReader::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => wipeout.common.layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        wipeout.common.color = Color::from_index(color_index);
                    }
                }
                10 | 20 | 30 => { insertion_point.add_coordinate(&pair); }
                11 | 21 | 31 => { u_vector.add_coordinate(&pair); }
                12 | 22 | 32 => { v_vector.add_coordinate(&pair); }
                14 => {
                    if let Some(x) = pair.as_double() {
                        wipeout.clip_boundary_vertices.push(Vector2::new(x, 0.0));
                    }
                }
                24 => {
                    if let Some(y) = pair.as_double() {
                        if let Some(last) = wipeout.clip_boundary_vertices.last_mut() {
                            last.y = y;
                        }
                    }
                }
                _ => {}
            }
        }

        wipeout.insertion_point = insertion_point.get_point().unwrap_or(Vector3::zero());
        wipeout.u_vector = u_vector.get_point().unwrap_or(Vector3::new(1.0, 0.0, 0.0));
        wipeout.v_vector = v_vector.get_point().unwrap_or(Vector3::new(0.0, 1.0, 0.0));

        Ok(Some(wipeout))
    }

    /// Read extended data (XDATA) from the current position
    /// Returns the extended data and the last pair read (which is not part of XDATA)
    fn read_extended_data(&mut self) -> Result<(ExtendedData, Option<super::stream_reader::DxfCodePair>)> {
        let mut xdata = ExtendedData::new();
        let mut current_record: Option<ExtendedDataRecord> = None;
        let mut point_reader = PointReader::new();

        while let Some(pair) = self.reader.read_pair()? {
            match pair.code {
                // Application name - start of new record
                1001 => {
                    // Save previous record if exists
                    if let Some(record) = current_record.take() {
                        xdata.add_record(record);
                    }
                    // Start new record
                    current_record = Some(ExtendedDataRecord::new(pair.value_string.clone()));
                }
                // String value
                1000 => {
                    if let Some(ref mut record) = current_record {
                        record.add_value(XDataValue::String(pair.value_string.clone()));
                    }
                }
                // Control string
                1002 => {
                    if let Some(ref mut record) = current_record {
                        record.add_value(XDataValue::ControlString(pair.value_string.clone()));
                    }
                }
                // Layer name
                1003 => {
                    if let Some(ref mut record) = current_record {
                        record.add_value(XDataValue::LayerName(pair.value_string.clone()));
                    }
                }
                // Binary data
                1004 => {
                    if let Some(ref mut record) = current_record {
                        // Parse hex string to bytes
                        let bytes: Vec<u8> = (0..pair.value_string.len())
                            .step_by(2)
                            .filter_map(|i| {
                                let end = (i + 2).min(pair.value_string.len());
                                u8::from_str_radix(&pair.value_string[i..end], 16).ok()
                            })
                            .collect();
                        record.add_value(XDataValue::BinaryData(bytes));
                    }
                }
                // Database handle
                1005 => {
                    if let Some(ref mut record) = current_record {
                        if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) {
                            record.add_value(XDataValue::Handle(Handle::new(h)));
                        }
                    }
                }
                // 3D point (1010, 1020, 1030)
                1010 | 1020 | 1030 => {
                    if let Some(ref mut record) = current_record {
                        point_reader.add_coordinate(&pair);
                        if let Some(pt) = point_reader.get_point() {
                            record.add_value(XDataValue::Point3D(pt));
                        }
                    }
                }
                // 3D position (1011, 1021, 1031)
                1011 | 1021 | 1031 => {
                    if let Some(ref mut record) = current_record {
                        point_reader.add_coordinate(&pair);
                        if let Some(pt) = point_reader.get_point() {
                            record.add_value(XDataValue::Position3D(pt));
                        }
                    }
                }
                // 3D displacement (1012, 1022, 1032)
                1012 | 1022 | 1032 => {
                    if let Some(ref mut record) = current_record {
                        point_reader.add_coordinate(&pair);
                        if let Some(pt) = point_reader.get_point() {
                            record.add_value(XDataValue::Displacement3D(pt));
                        }
                    }
                }
                // 3D direction (1013, 1023, 1033)
                1013 | 1023 | 1033 => {
                    if let Some(ref mut record) = current_record {
                        point_reader.add_coordinate(&pair);
                        if let Some(pt) = point_reader.get_point() {
                            record.add_value(XDataValue::Direction3D(pt));
                        }
                    }
                }
                // Real value
                1040 => {
                    if let Some(ref mut record) = current_record {
                        if let Some(value) = pair.as_double() {
                            record.add_value(XDataValue::Real(value));
                        }
                    }
                }
                // Distance
                1041 => {
                    if let Some(ref mut record) = current_record {
                        if let Some(value) = pair.as_double() {
                            record.add_value(XDataValue::Distance(value));
                        }
                    }
                }
                // Scale factor
                1042 => {
                    if let Some(ref mut record) = current_record {
                        if let Some(value) = pair.as_double() {
                            record.add_value(XDataValue::ScaleFactor(value));
                        }
                    }
                }
                // 16-bit integer
                1070 => {
                    if let Some(ref mut record) = current_record {
                        if let Some(value) = pair.as_i16() {
                            record.add_value(XDataValue::Integer16(value));
                        }
                    }
                }
                // 32-bit integer
                1071 => {
                    if let Some(ref mut record) = current_record {
                        if let Some(value) = pair.as_i32() {
                            record.add_value(XDataValue::Integer32(value));
                        }
                    }
                }
                // Not XDATA - return what we have
                _ => {
                    // Save last record if exists
                    if let Some(record) = current_record.take() {
                        xdata.add_record(record);
                    }
                    return Ok((xdata, Some(pair)));
                }
            }
        }

        // End of file - save last record if exists
        if let Some(record) = current_record.take() {
            xdata.add_record(record);
        }

        Ok((xdata, None))
    }

    // ===== New Entity Readers =====

    /// Read a VIEWPORT entity
    fn read_viewport(&mut self) -> Result<Option<Viewport>> {
        let mut vp = Viewport::new();
        let mut center = PointReader::new();
        let mut view_center = PointReader::new();
        let mut view_direction = PointReader::new();
        let mut view_target = PointReader::new();
        let mut snap_base_x: Option<f64> = None;
        let mut snap_base_y: Option<f64> = None;
        let mut snap_spacing_x: Option<f64> = None;
        let mut snap_spacing_y: Option<f64> = None;
        let mut grid_spacing_x: Option<f64> = None;
        let mut grid_spacing_y: Option<f64> = None;
        let mut ucs_origin = PointReader::new();
        let mut ucs_x_axis = PointReader::new();
        let mut ucs_y_axis = PointReader::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 { self.reader.push_back(pair); break; }
            match pair.code {
                8 => vp.common.layer = pair.value_string.clone(),
                62 => { if let Some(v) = pair.as_i16() { vp.common.color = Color::from_index(v); } }
                10 | 20 | 30 => { center.add_coordinate(&pair); }
                40 => { if let Some(v) = pair.as_double() { vp.width = v; } }
                41 => { if let Some(v) = pair.as_double() { vp.height = v; } }
                90 => { if let Some(v) = pair.as_i32() { vp.status = crate::entities::viewport::ViewportStatusFlags::from_bits(v); } }
                69 => { if let Some(v) = pair.as_i16() { vp.id = v; } }
                12 => { if let Some(v) = pair.as_double() { view_center.add_coordinate(&pair); let _ = v; } }
                22 => { view_center.add_coordinate(&pair); }
                13 => { snap_base_x = pair.as_double(); }
                23 => { snap_base_y = pair.as_double(); }
                14 => { snap_spacing_x = pair.as_double(); }
                24 => { snap_spacing_y = pair.as_double(); }
                15 => { grid_spacing_x = pair.as_double(); }
                25 => { grid_spacing_y = pair.as_double(); }
                16 | 26 | 36 => { view_direction.add_coordinate(&pair); }
                17 | 27 | 37 => { view_target.add_coordinate(&pair); }
                42 => { if let Some(v) = pair.as_double() { vp.lens_length = v; } }
                43 => { if let Some(v) = pair.as_double() { vp.front_clip_z = v; } }
                44 => { if let Some(v) = pair.as_double() { vp.back_clip_z = v; } }
                45 => { if let Some(v) = pair.as_double() { vp.view_height = v; } }
                50 => { if let Some(v) = pair.as_double() { vp.snap_angle = v; } }
                51 => { if let Some(v) = pair.as_double() { vp.twist_angle = v; } }
                72 => { if let Some(v) = pair.as_i16() { vp.circle_sides = v; } }
                331 => {
                    if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) {
                        vp.frozen_layers.push(Handle::new(h));
                    }
                }
                281 => { if let Some(v) = pair.as_i16() { vp.render_mode = crate::entities::viewport::ViewportRenderMode::from_value(v); } }
                71 => { if let Some(v) = pair.as_i16() { vp.ucs_per_viewport = v != 0; } }
                110 | 120 | 130 => { ucs_origin.add_coordinate(&pair); }
                111 | 121 | 131 => { ucs_x_axis.add_coordinate(&pair); }
                112 | 122 | 132 => { ucs_y_axis.add_coordinate(&pair); }
                146 => { if let Some(v) = pair.as_double() { vp.elevation = v; } }
                61 => { if let Some(v) = pair.as_i16() { vp.grid_major = v; } }
                141 => { if let Some(v) = pair.as_double() { vp.brightness = v; } }
                142 => { if let Some(v) = pair.as_double() { vp.contrast = v; } }
                292 => { if let Some(v) = pair.as_bool() { vp.default_lighting = v; } }
                282 => { if let Some(v) = pair.as_i16() { vp.default_lighting_type = v; } }
                _ => {}
            }
        }

        if let Some(pt) = center.get_point() { vp.center = pt; }
        if let Some(pt) = view_direction.get_point() { vp.view_direction = pt; }
        if let Some(pt) = view_target.get_point() { vp.view_target = pt; }
        if let Some(pt) = ucs_origin.get_point() { vp.ucs_origin = pt; }
        if let Some(pt) = ucs_x_axis.get_point() { vp.ucs_x_axis = pt; }
        if let Some(pt) = ucs_y_axis.get_point() { vp.ucs_y_axis = pt; }
        // For 2D points, assemble manually
        if let (Some(x), Some(y)) = (view_center.get_point().map(|p| p.x), view_center.get_point().map(|p| p.y)) {
            vp.view_center = Vector3::new(x, y, 0.0);
        }
        vp.snap_base = Vector3::new(snap_base_x.unwrap_or(0.0), snap_base_y.unwrap_or(0.0), 0.0);
        vp.snap_spacing = Vector3::new(snap_spacing_x.unwrap_or(10.0), snap_spacing_y.unwrap_or(10.0), 0.0);
        vp.grid_spacing = Vector3::new(grid_spacing_x.unwrap_or(10.0), grid_spacing_y.unwrap_or(10.0), 0.0);

        Ok(Some(vp))
    }

    /// Read an ATTRIB entity
    fn read_attrib(&mut self) -> Result<Option<AttributeEntity>> {
        let mut attrib = AttributeEntity::new(String::new(), String::new());
        let mut insertion_point = PointReader::new();
        let mut alignment_point = PointReader::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 { self.reader.push_back(pair); break; }
            match pair.code {
                8 => attrib.common.layer = pair.value_string.clone(),
                62 => { if let Some(v) = pair.as_i16() { attrib.common.color = Color::from_index(v); } }
                370 => { if let Some(v) = pair.as_i16() { attrib.common.line_weight = LineWeight::from_value(v); } }
                1 => attrib.value = pair.value_string.clone(),
                2 => attrib.tag = pair.value_string.clone(),
                7 => attrib.text_style = pair.value_string.clone(),
                10 | 20 | 30 => { insertion_point.add_coordinate(&pair); }
                11 | 21 | 31 => { alignment_point.add_coordinate(&pair); }
                40 => { if let Some(v) = pair.as_double() { attrib.height = v; } }
                41 => { if let Some(v) = pair.as_double() { attrib.width_factor = v; } }
                50 => { if let Some(v) = pair.as_double() { attrib.rotation = v; } }
                51 => { if let Some(v) = pair.as_double() { attrib.oblique_angle = v; } }
                70 => {
                    if let Some(v) = pair.as_i16() {
                        attrib.flags = crate::entities::attribute_definition::AttributeFlags::from_bits(v as i32);
                    }
                }
                71 => { if let Some(v) = pair.as_i16() { attrib.text_generation_flags = v; } }
                72 => {
                    if let Some(v) = pair.as_i16() {
                        attrib.horizontal_alignment = crate::entities::attribute_definition::HorizontalAlignment::from_value(v);
                    }
                }
                74 => {
                    if let Some(v) = pair.as_i16() {
                        attrib.vertical_alignment = crate::entities::attribute_definition::VerticalAlignment::from_value(v);
                    }
                }
                _ => {}
            }
        }

        attrib.insertion_point = insertion_point.get_point().unwrap_or(Vector3::zero());
        attrib.alignment_point = alignment_point.get_point().unwrap_or(Vector3::zero());

        Ok(Some(attrib))
    }

    /// Read a LEADER entity
    fn read_leader(&mut self) -> Result<Option<Leader>> {
        let mut leader = Leader::new();
        let mut normal = PointReader::new();
        let mut horiz_dir = PointReader::new();
        let mut block_offset = PointReader::new();
        let mut annotation_offset = PointReader::new();
        let mut reading_vertex = false;
        let mut current_vertex = PointReader::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 { self.reader.push_back(pair); break; }
            match pair.code {
                8 => leader.common.layer = pair.value_string.clone(),
                62 => { if let Some(v) = pair.as_i16() { leader.common.color = Color::from_index(v); } }
                370 => { if let Some(v) = pair.as_i16() { leader.common.line_weight = LineWeight::from_value(v); } }
                3 => leader.dimension_style = pair.value_string.clone(),
                71 => { if let Some(v) = pair.as_i16() { leader.arrow_enabled = v != 0; } }
                72 => {
                    if let Some(v) = pair.as_i16() {
                        leader.path_type = if v == 1 { crate::entities::leader::LeaderPathType::Spline } else { crate::entities::leader::LeaderPathType::StraightLine };
                    }
                }
                73 => {
                    if let Some(v) = pair.as_i16() {
                        leader.creation_type = match v {
                            0 => crate::entities::leader::LeaderCreationType::WithText,
                            1 => crate::entities::leader::LeaderCreationType::WithTolerance,
                            2 => crate::entities::leader::LeaderCreationType::WithBlock,
                            _ => crate::entities::leader::LeaderCreationType::NoAnnotation,
                        };
                    }
                }
                74 => {
                    if let Some(v) = pair.as_i16() {
                        leader.hookline_direction = if v == 1 { crate::entities::leader::HooklineDirection::Same } else { crate::entities::leader::HooklineDirection::Opposite };
                    }
                }
                75 => { if let Some(v) = pair.as_i16() { leader.hookline_enabled = v != 0; } }
                40 => { if let Some(v) = pair.as_double() { leader.text_height = v; } }
                41 => { if let Some(v) = pair.as_double() { leader.text_width = v; } }
                10 => {
                    // Save previous vertex
                    if reading_vertex {
                        if let Some(pt) = current_vertex.get_point() { leader.vertices.push(pt); }
                    }
                    current_vertex = PointReader::new();
                    current_vertex.add_coordinate(&pair);
                    reading_vertex = true;
                }
                20 | 30 => { current_vertex.add_coordinate(&pair); }
                340 => {
                    if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) {
                        leader.annotation_handle = Handle::new(h);
                    }
                }
                210 | 220 | 230 => { normal.add_coordinate(&pair); }
                211 | 221 | 231 => { horiz_dir.add_coordinate(&pair); }
                212 | 222 | 232 => { block_offset.add_coordinate(&pair); }
                213 | 223 | 233 => { annotation_offset.add_coordinate(&pair); }
                _ => {}
            }
        }

        // Save last vertex
        if reading_vertex {
            if let Some(pt) = current_vertex.get_point() { leader.vertices.push(pt); }
        }
        if let Some(pt) = normal.get_point() { leader.normal = pt; }
        if let Some(pt) = horiz_dir.get_point() { leader.horizontal_direction = pt; }
        if let Some(pt) = block_offset.get_point() { leader.block_offset = pt; }
        if let Some(pt) = annotation_offset.get_point() { leader.annotation_offset = pt; }

        Ok(Some(leader))
    }

    /// Read a MULTILEADER entity (basic property reader)
    fn read_multileader(&mut self) -> Result<Option<MultiLeader>> {
        let mut ml = MultiLeader::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 { self.reader.push_back(pair); break; }
            match pair.code {
                8 => ml.common.layer = pair.value_string.clone(),
                62 => { if let Some(v) = pair.as_i16() { ml.common.color = Color::from_index(v); } }
                370 => { if let Some(v) = pair.as_i16() { ml.common.line_weight = LineWeight::from_value(v); } }
                340 => { if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { ml.style_handle = Some(Handle::new(h)); } }
                170 => {
                    if let Some(v) = pair.as_i16() {
                        ml.content_type = match v {
                            1 => crate::entities::multileader::LeaderContentType::Block,
                            2 => crate::entities::multileader::LeaderContentType::MText,
                            _ => crate::entities::multileader::LeaderContentType::None,
                        };
                    }
                }
                91 => { if let Some(v) = pair.as_i32() { ml.line_color = Color::from_index(v as i16); } }
                40 => { if let Some(v) = pair.as_double() { ml.dogleg_length = v; } }
                41 => { if let Some(v) = pair.as_double() { ml.arrowhead_size = v; } }
                44 => { if let Some(v) = pair.as_double() { ml.text_height = v; } }
                45 => { if let Some(v) = pair.as_double() { ml.scale_factor = v; } }
                174 => { if let Some(v) = pair.as_i16() { ml.text_left_attachment = crate::entities::multileader::TextAttachmentType::from(v); } }
                175 => { if let Some(v) = pair.as_i16() { ml.text_right_attachment = crate::entities::multileader::TextAttachmentType::from(v); } }
                176 => { if let Some(v) = pair.as_i16() { ml.text_angle_type = crate::entities::multileader::TextAngleType::from(v); } }
                291 => { if let Some(v) = pair.as_bool() { ml.enable_dogleg = v; } }
                290 => { if let Some(v) = pair.as_bool() { ml.enable_landing = v; } }
                292 => { if let Some(v) = pair.as_bool() { ml.text_frame = v; } }
                _ => {}
            }
        }

        Ok(Some(ml))
    }

    /// Read an MLINE entity
    fn read_mline(&mut self) -> Result<Option<MLine>> {
        use crate::entities::mline::*;
        let mut mline = MLine::new();
        let mut start_point = PointReader::new();
        let mut normal = PointReader::new();
        let mut current_vertex_pos = PointReader::new();
        let mut current_vertex_dir = PointReader::new();
        let mut current_vertex_miter = PointReader::new();
        let mut vertices: Vec<MLineVertex> = Vec::new();
        let mut reading_vertices = false;
        let mut num_elements = 0usize;
        let mut current_segments: Vec<MLineSegment> = Vec::new();
        let mut current_params: Vec<f64> = Vec::new();
        let mut current_area_fill: Vec<f64> = Vec::new();
        let mut reading_params = false;
        let mut reading_area_fill = false;

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 { self.reader.push_back(pair); break; }
            match pair.code {
                8 => mline.common.layer = pair.value_string.clone(),
                62 => { if let Some(v) = pair.as_i16() { mline.common.color = Color::from_index(v); } }
                370 => { if let Some(v) = pair.as_i16() { mline.common.line_weight = LineWeight::from_value(v); } }
                2 => mline.style_name = pair.value_string.clone(),
                40 => { if let Some(v) = pair.as_double() { mline.scale_factor = v; } }
                70 => { if let Some(v) = pair.as_i16() { mline.justification = MLineJustification::from(v); } }
                71 => { if let Some(v) = pair.as_i16() { mline.flags = MLineFlags::from_bits_truncate(v); } }
                73 => { if let Some(v) = pair.as_i16() { num_elements = v as usize; } }
                10 | 20 | 30 => { start_point.add_coordinate(&pair); }
                210 | 220 | 230 => { normal.add_coordinate(&pair); }
                340 => {
                    if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) {
                        mline.style_handle = Some(Handle::new(h));
                    }
                }
                11 => {
                    // New vertex – save previous if any
                    if reading_vertices {
                        self.finalize_mline_vertex(&mut vertices, &current_vertex_pos, &current_vertex_dir,
                            &current_vertex_miter, &mut current_segments, &mut current_params, &mut current_area_fill,
                            &mut reading_params, &mut reading_area_fill);
                    }
                    current_vertex_pos = PointReader::new();
                    current_vertex_pos.add_coordinate(&pair);
                    current_vertex_dir = PointReader::new();
                    current_vertex_miter = PointReader::new();
                    current_segments = Vec::new();
                    current_params = Vec::new();
                    current_area_fill = Vec::new();
                    reading_params = false;
                    reading_area_fill = false;
                    reading_vertices = true;
                }
                21 | 31 => { current_vertex_pos.add_coordinate(&pair); }
                12 | 22 | 32 => { current_vertex_dir.add_coordinate(&pair); }
                13 | 23 | 33 => { current_vertex_miter.add_coordinate(&pair); }
                74 => {
                    // Number of parameters for this element
                    if reading_params && !current_params.is_empty() {
                        current_segments.push(MLineSegment {
                            parameters: std::mem::take(&mut current_params),
                            area_fill_parameters: std::mem::take(&mut current_area_fill),
                        });
                    }
                    reading_params = true;
                    reading_area_fill = false;
                    current_params = Vec::new();
                    current_area_fill = Vec::new();
                }
                75 => { reading_area_fill = true; }
                41 => {
                    if let Some(v) = pair.as_double() {
                        if reading_area_fill { current_area_fill.push(v); }
                        else if reading_params { current_params.push(v); }
                    }
                }
                42 => { if let Some(v) = pair.as_double() { current_area_fill.push(v); } }
                _ => {}
            }
        }

        // Finalize last vertex
        if reading_vertices {
            self.finalize_mline_vertex(&mut vertices, &current_vertex_pos, &current_vertex_dir,
                &current_vertex_miter, &mut current_segments, &mut current_params, &mut current_area_fill,
                &mut reading_params, &mut reading_area_fill);
        }

        if let Some(pt) = start_point.get_point() { mline.start_point = pt; }
        if let Some(pt) = normal.get_point() { mline.normal = pt; }
        mline.vertices = vertices;
        mline.style_element_count = num_elements;

        Ok(Some(mline))
    }

    fn finalize_mline_vertex(
        &self,
        vertices: &mut Vec<crate::entities::mline::MLineVertex>,
        pos: &PointReader, dir: &PointReader, miter: &PointReader,
        segments: &mut Vec<crate::entities::mline::MLineSegment>,
        params: &mut Vec<f64>, area_fill: &mut Vec<f64>,
        reading_params: &mut bool, _reading_area_fill: &mut bool,
    ) {
        use crate::entities::mline::*;
        if *reading_params && !params.is_empty() {
            segments.push(MLineSegment {
                parameters: std::mem::take(params),
                area_fill_parameters: std::mem::take(area_fill),
            });
        }
        vertices.push(MLineVertex {
            position: pos.get_point().unwrap_or(Vector3::zero()),
            direction: dir.get_point().unwrap_or(Vector3::new(1.0, 0.0, 0.0)),
            miter: miter.get_point().unwrap_or(Vector3::new(0.0, 1.0, 0.0)),
            segments: std::mem::take(segments),
        });
        *reading_params = false;
    }

    /// Read a MESH entity
    fn read_mesh(&mut self) -> Result<Option<Mesh>> {
        use crate::entities::mesh::*;
        let mut mesh = Mesh::new();
        let mut reading_state = MeshReadState::Properties;
        let mut vertex_count = 0usize;
        let mut _face_count = 0usize;
        let mut _edge_count = 0usize;
        let mut _crease_count = 0usize;
        let mut current_vertex = PointReader::new();
        let mut face_indices: Vec<usize> = Vec::new();
        let mut face_subcount: Option<usize> = None;
        let mut edge_buf: Vec<usize> = Vec::new();
        let mut crease_values: Vec<f64> = Vec::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 { self.reader.push_back(pair); break; }
            match pair.code {
                8 => mesh.common.layer = pair.value_string.clone(),
                62 => { if let Some(v) = pair.as_i16() { mesh.common.color = Color::from_index(v); } }
                370 => { if let Some(v) = pair.as_i16() { mesh.common.line_weight = LineWeight::from_value(v); } }
                71 => { if let Some(v) = pair.as_i16() { mesh.version = v; } }
                72 => { if let Some(v) = pair.as_i16() { mesh.blend_crease = v != 0; } }
                91 => {
                    match reading_state {
                        MeshReadState::Properties => {
                            if let Some(v) = pair.as_i32() { mesh.subdivision_level = v; }
                        }
                        _ => {}
                    }
                }
                92 => {
                    if let Some(v) = pair.as_i32() { vertex_count = v as usize; reading_state = MeshReadState::Vertices; }
                }
                93 => {
                    if let Some(v) = pair.as_i32() { _face_count = v as usize; reading_state = MeshReadState::Faces; }
                }
                94 => {
                    if let Some(v) = pair.as_i32() { _edge_count = v as usize; reading_state = MeshReadState::Edges; }
                }
                95 => {
                    if let Some(v) = pair.as_i32() { _crease_count = v as usize; reading_state = MeshReadState::Creases; }
                }
                10 | 20 | 30 => {
                    if reading_state == MeshReadState::Vertices {
                        current_vertex.add_coordinate(&pair);
                        if pair.code == 30 {
                            if let Some(pt) = current_vertex.get_point() {
                                mesh.vertices.push(pt);
                                current_vertex = PointReader::new();
                            }
                        }
                    }
                }
                90 => {
                    if let Some(v) = pair.as_i32() {
                        match reading_state {
                            MeshReadState::Faces => {
                                if face_subcount.is_none() {
                                    face_subcount = Some(v as usize);
                                } else {
                                    face_indices.push(v as usize);
                                    if face_indices.len() == face_subcount.unwrap() {
                                        mesh.faces.push(MeshFace { vertices: std::mem::take(&mut face_indices) });
                                        face_subcount = None;
                                    }
                                }
                            }
                            MeshReadState::Edges => {
                                edge_buf.push(v as usize);
                                if edge_buf.len() == 2 {
                                    mesh.edges.push(MeshEdge { start: edge_buf[0], end: edge_buf[1], crease: Some(0.0) });
                                    edge_buf.clear();
                                }
                            }
                            _ => {}
                        }
                    }
                }
                140 => {
                    if reading_state == MeshReadState::Creases {
                        if let Some(v) = pair.as_double() { crease_values.push(v); }
                    }
                }
                _ => {}
            }
        }

        // Apply crease values to edges
        for (i, crease) in crease_values.into_iter().enumerate() {
            if i < mesh.edges.len() { mesh.edges[i].crease = Some(crease); }
        }

        let _ = vertex_count;

        Ok(Some(mesh))
    }

    /// Read an IMAGE entity
    fn read_raster_image(&mut self) -> Result<Option<RasterImage>> {
        let mut img = RasterImage::new("", Vector3::zero(), 1.0, 1.0);
        let mut insertion_point = PointReader::new();
        let mut u_vector = PointReader::new();
        let mut v_vector = PointReader::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 { self.reader.push_back(pair); break; }
            match pair.code {
                8 => img.common.layer = pair.value_string.clone(),
                62 => { if let Some(v) = pair.as_i16() { img.common.color = Color::from_index(v); } }
                10 | 20 | 30 => { insertion_point.add_coordinate(&pair); }
                11 | 21 | 31 => { u_vector.add_coordinate(&pair); }
                12 | 22 | 32 => { v_vector.add_coordinate(&pair); }
                13 => { if let Some(v) = pair.as_double() { img.size.x = v; } }
                23 => { if let Some(v) = pair.as_double() { img.size.y = v; } }
                340 => { if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { img.definition_handle = Some(Handle::new(h)); } }
                360 => { if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { img.definition_reactor_handle = Some(Handle::new(h)); } }
                70 => { /* display properties flags */ }
                280 => { if let Some(v) = pair.as_i16() { img.clipping_enabled = v != 0; } }
                281 => { if let Some(v) = pair.as_i16() { img.brightness = v as u8; } }
                282 => { if let Some(v) = pair.as_i16() { img.contrast = v as u8; } }
                283 => { if let Some(v) = pair.as_i16() { img.fade = v as u8; } }
                14 => {
                    if let Some(x) = pair.as_double() {
                        img.clip_boundary.vertices.push(Vector2::new(x, 0.0));
                    }
                }
                24 => {
                    if let Some(y) = pair.as_double() {
                        if let Some(last) = img.clip_boundary.vertices.last_mut() { last.y = y; }
                    }
                }
                _ => {}
            }
        }

        img.insertion_point = insertion_point.get_point().unwrap_or(Vector3::zero());
        img.u_vector = u_vector.get_point().unwrap_or(Vector3::new(1.0, 0.0, 0.0));
        img.v_vector = v_vector.get_point().unwrap_or(Vector3::new(0.0, 1.0, 0.0));

        Ok(Some(img))
    }

    /// Read modeler geometry (ACIS) data — shared between 3DSOLID, REGION, BODY
    fn read_modeler_geometry(&mut self) -> Result<(EntityCommon, String, String)> {
        let mut common = EntityCommon::new();
        let mut acis_data = String::new();
        let mut uid = String::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 { self.reader.push_back(pair); break; }
            match pair.code {
                8 => common.layer = pair.value_string.clone(),
                62 => { if let Some(v) = pair.as_i16() { common.color = Color::from_index(v); } }
                370 => { if let Some(v) = pair.as_i16() { common.line_weight = LineWeight::from_value(v); } }
                1 | 3 => {
                    acis_data.push_str(&pair.value_string);
                    acis_data.push('\n');
                }
                2 => uid = pair.value_string.clone(),
                _ => {}
            }
        }

        Ok((common, uid, acis_data))
    }

    /// Read a 3DSOLID entity
    fn read_solid3d(&mut self) -> Result<Option<Solid3D>> {
        let (common, uid, acis_data) = self.read_modeler_geometry()?;
        let mut solid = Solid3D::new();
        solid.common = common;
        solid.uid = uid;
        solid.acis_data.sat_data = acis_data;
        Ok(Some(solid))
    }

    /// Read a REGION entity
    fn read_region(&mut self) -> Result<Option<Region>> {
        let (common, _uid, acis_data) = self.read_modeler_geometry()?;
        let mut region = Region::new();
        region.common = common;
        region.acis_data.sat_data = acis_data;
        Ok(Some(region))
    }

    /// Read a BODY entity
    fn read_body(&mut self) -> Result<Option<Body>> {
        let (common, _uid, acis_data) = self.read_modeler_geometry()?;
        let mut body = Body::new();
        body.common = common;
        body.acis_data.sat_data = acis_data;
        Ok(Some(body))
    }

    /// Read a TABLE entity (basic properties)
    fn read_table_entity(&mut self) -> Result<Option<crate::entities::Table>> {
        let mut insertion_point = PointReader::new();
        let mut table = crate::entities::Table::new(Vector3::zero(), 1, 1);

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 { self.reader.push_back(pair); break; }
            match pair.code {
                8 => table.common.layer = pair.value_string.clone(),
                62 => { if let Some(v) = pair.as_i16() { table.common.color = Color::from_index(v); } }
                370 => { if let Some(v) = pair.as_i16() { table.common.line_weight = LineWeight::from_value(v); } }
                10 | 20 | 30 => { insertion_point.add_coordinate(&pair); }
                342 => {
                    if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { table.table_style_handle = Some(Handle::new(h)); }
                }
                343 => {
                    if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { table.block_record_handle = Some(Handle::new(h)); }
                }
                _ => {}
            }
        }

        table.insertion_point = insertion_point.get_point().unwrap_or(Vector3::zero());
        Ok(Some(table))
    }

    /// Read a PDF/DWF/DGN UNDERLAY entity
    fn read_underlay(&mut self, type_name: &str) -> Result<Option<Underlay>> {
        use crate::entities::underlay::UnderlayType;
        let utype = match type_name {
            "DWFUNDERLAY" => UnderlayType::Dwf,
            "DGNUNDERLAY" => UnderlayType::Dgn,
            _ => UnderlayType::Pdf,
        };
        let mut underlay = Underlay::new(utype);
        let mut insertion_point = PointReader::new();
        let mut normal = PointReader::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 { self.reader.push_back(pair); break; }
            match pair.code {
                8 => underlay.common.layer = pair.value_string.clone(),
                62 => { if let Some(v) = pair.as_i16() { underlay.common.color = Color::from_index(v); } }
                370 => { if let Some(v) = pair.as_i16() { underlay.common.line_weight = LineWeight::from_value(v); } }
                10 | 20 | 30 => { insertion_point.add_coordinate(&pair); }
                210 | 220 | 230 => { normal.add_coordinate(&pair); }
                41 => { if let Some(v) = pair.as_double() { underlay.x_scale = v; } }
                42 => { if let Some(v) = pair.as_double() { underlay.y_scale = v; } }
                43 => { if let Some(v) = pair.as_double() { underlay.z_scale = v; } }
                50 => { if let Some(v) = pair.as_double() { underlay.rotation = v; } }
                281 => { if let Some(v) = pair.as_i16() { underlay.contrast = v as u8; } }
                282 => { if let Some(v) = pair.as_i16() { underlay.fade = v as u8; } }
                340 => {
                    if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { underlay.definition_handle = Handle::new(h); }
                }
                11 => {
                    if let Some(x) = pair.as_double() { underlay.clip_boundary_vertices.push(Vector2::new(x, 0.0)); }
                }
                21 => {
                    if let Some(y) = pair.as_double() {
                        if let Some(last) = underlay.clip_boundary_vertices.last_mut() { last.y = y; }
                    }
                }
                _ => {}
            }
        }

        underlay.insertion_point = insertion_point.get_point().unwrap_or(Vector3::zero());
        underlay.normal = normal.get_point().unwrap_or(Vector3::new(0.0, 0.0, 1.0));
        Ok(Some(underlay))
    }

    // ===== New Object Readers =====

    /// Read an XRECORD object
    fn read_xrecord(&mut self) -> Result<Option<XRecord>> {
        let mut xr = XRecord::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 { self.reader.push_back(pair); break; }
            match pair.code {
                5 => { if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { xr.handle = Handle::new(h); } }
                330 => { if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { xr.owner = Handle::new(h); } }
                280 => {
                    if let Some(v) = pair.as_i16() {
                        xr.cloning_flags = DictionaryCloningFlags::from_value(v);
                    }
                }
                102 => {} // Skip extension dictionaries / reactors groups
                _ => {
                    // All other codes are data entries
                    xr.entries.push(XRecordEntry {
                        code: pair.code,
                        value: XRecordValue::String(pair.value_string.clone()),
                    });
                }
            }
        }

        Ok(Some(xr))
    }

    /// Read a GROUP object
    fn read_group(&mut self) -> Result<Option<Group>> {
        let mut group = Group::new("");

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 { self.reader.push_back(pair); break; }
            match pair.code {
                5 => { if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { group.handle = Handle::new(h); } }
                330 => { if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { group.owner = Handle::new(h); } }
                300 => group.description = pair.value_string.clone(),
                70 => {} // unnamed flag — skip
                71 => { if let Some(v) = pair.as_i16() { group.selectable = v != 0; } }
                340 => {
                    if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) {
                        group.entities.push(Handle::new(h));
                    }
                }
                _ => {}
            }
        }

        Ok(Some(group))
    }

    /// Read an MLINESTYLE object
    fn read_mlinestyle_object(&mut self) -> Result<Option<crate::objects::MLineStyle>> {
        use crate::objects::MLineStyleElement;
        let mut style = crate::objects::MLineStyle::new("");

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 { self.reader.push_back(pair); break; }
            match pair.code {
                5 => { if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { style.handle = Handle::new(h); } }
                330 => { if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { style.owner = Handle::new(h); } }
                2 => style.name = pair.value_string.clone(),
                3 => style.description = pair.value_string.clone(),
                51 => { if let Some(v) = pair.as_double() { style.start_angle = v; } }
                52 => { if let Some(v) = pair.as_double() { style.end_angle = v; } }
                62 => {
                    if let Some(v) = pair.as_i16() {
                        if let Some(last) = style.elements.last_mut() {
                            last.color = Color::from_index(v);
                        } else {
                            style.fill_color = Color::from_index(v);
                        }
                    }
                }
                49 => {
                    // Element offset — start a new element
                    if let Some(v) = pair.as_double() {
                        style.elements.push(MLineStyleElement {
                            offset: v,
                            color: Color::ByLayer,
                            linetype: String::from("BYLAYER"),
                        });
                    }
                }
                6 => {
                    if let Some(last) = style.elements.last_mut() {
                        last.linetype = pair.value_string.clone();
                    }
                }
                _ => {}
            }
        }

        Ok(Some(style))
    }

    /// Read an IMAGEDEF object
    fn read_image_definition(&mut self) -> Result<Option<crate::objects::ImageDefinition>> {
        let mut def = crate::objects::ImageDefinition::new("");

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 { self.reader.push_back(pair); break; }
            match pair.code {
                5 => { if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { def.handle = Handle::new(h); } }
                330 => { if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { def.owner = Handle::new(h); } }
                1 => def.file_name = pair.value_string.clone(),
                10 => { if let Some(v) = pair.as_double() { def.size_in_pixels.0 = v as u32; } }
                20 => { if let Some(v) = pair.as_double() { def.size_in_pixels.1 = v as u32; } }
                11 => { if let Some(v) = pair.as_double() { def.pixel_size.0 = v; } }
                21 => { if let Some(v) = pair.as_double() { def.pixel_size.1 = v; } }
                280 => { if let Some(v) = pair.as_i16() { def.is_loaded = v != 0; } }
                _ => {}
            }
        }

        Ok(Some(def))
    }

    /// Read an MLEADERSTYLE object
    fn read_multileader_style(&mut self) -> Result<Option<MultiLeaderStyle>> {
        let mut style = MultiLeaderStyle::new("Standard");

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 { self.reader.push_back(pair); break; }
            match pair.code {
                5 => { if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { style.handle = Handle::new(h); } }
                330 => { if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { style.owner_handle = Handle::new(h); } }
                3 => style.name = pair.value_string.clone(),
                300 => style.description = pair.value_string.clone(),
                170 => { if let Some(v) = pair.as_i16() { style.content_type = crate::objects::LeaderContentType::from(v); } }
                173 => { if let Some(v) = pair.as_i16() { style.path_type = crate::objects::MultiLeaderPathType::from(v); } }
                91 => { if let Some(v) = pair.as_i32() { style.line_color = Color::from_index(v as i16); } }
                92 => { if let Some(v) = pair.as_i32() { style.line_weight = LineWeight::from_value(v as i16); } }
                290 => { if let Some(v) = pair.as_bool() { style.enable_landing = v; } }
                291 => { if let Some(v) = pair.as_bool() { style.enable_dogleg = v; } }
                43 => { if let Some(v) = pair.as_double() { style.landing_distance = v; } }
                42 => { if let Some(v) = pair.as_double() { style.landing_gap = v; } }
                44 => { if let Some(v) = pair.as_double() { style.arrowhead_size = v; } }
                45 => { if let Some(v) = pair.as_double() { style.text_height = v; } }
                93 => { if let Some(v) = pair.as_i32() { style.text_color = Color::from_index(v as i16); } }
                292 => { if let Some(v) = pair.as_bool() { style.text_frame = v; } }
                174 => { if let Some(v) = pair.as_i16() { style.text_left_attachment = crate::objects::TextAttachmentType::from(v); } }
                178 => { if let Some(v) = pair.as_i16() { style.text_right_attachment = crate::objects::TextAttachmentType::from(v); } }
                175 => { if let Some(v) = pair.as_i16() { style.text_angle_type = crate::objects::TextAngleType::from(v); } }
                176 => { if let Some(v) = pair.as_i16() { style.text_alignment = crate::objects::TextAlignmentType::from(v); } }
                142 => { if let Some(v) = pair.as_double() { style.scale_factor = v; } }
                304 => style.default_text = pair.value_string.clone(),
                340 => { if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { style.line_type_handle = Some(Handle::new(h)); } }
                341 => { if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { style.arrowhead_handle = Some(Handle::new(h)); } }
                342 => { if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { style.text_style_handle = Some(Handle::new(h)); } }
                343 => { if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { style.block_content_handle = Some(Handle::new(h)); } }
                _ => {}
            }
        }

        Ok(Some(style))
    }

    /// Read a PLOTSETTINGS object
    fn read_plot_settings(&mut self) -> Result<Option<PlotSettings>> {
        let mut ps = PlotSettings::new("");

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 { self.reader.push_back(pair); break; }
            match pair.code {
                5 => { if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { ps.handle = Handle::new(h); } }
                330 => { if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { ps.owner = Handle::new(h); } }
                1 => ps.page_name = pair.value_string.clone(),
                2 => ps.printer_name = pair.value_string.clone(),
                4 => ps.paper_size = pair.value_string.clone(),
                6 => ps.plot_view_name = pair.value_string.clone(),
                7 => ps.current_style_sheet = pair.value_string.clone(),
                40 => { if let Some(v) = pair.as_double() { ps.margins.left = v; } }
                41 => { if let Some(v) = pair.as_double() { ps.margins.bottom = v; } }
                42 => { if let Some(v) = pair.as_double() { ps.margins.right = v; } }
                43 => { if let Some(v) = pair.as_double() { ps.margins.top = v; } }
                44 => { if let Some(v) = pair.as_double() { ps.paper_width = v; } }
                45 => { if let Some(v) = pair.as_double() { ps.paper_height = v; } }
                46 => { if let Some(v) = pair.as_double() { ps.origin_x = v; } }
                47 => { if let Some(v) = pair.as_double() { ps.origin_y = v; } }
                142 => { if let Some(v) = pair.as_double() { ps.scale_numerator = v; } }
                143 => { if let Some(v) = pair.as_double() { ps.scale_denominator = v; } }
                _ => {}
            }
        }

        Ok(Some(ps))
    }

    /// Read a TABLESTYLE object
    fn read_table_style(&mut self) -> Result<Option<TableStyle>> {
        let mut ts = TableStyle::new("Standard");

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 { self.reader.push_back(pair); break; }
            match pair.code {
                5 => { if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { ts.handle = Handle::new(h); } }
                330 => { if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { ts.owner_handle = Handle::new(h); } }
                3 => ts.name = pair.value_string.clone(),
                40 => { if let Some(v) = pair.as_double() { ts.horizontal_margin = v; } }
                41 => { if let Some(v) = pair.as_double() { ts.vertical_margin = v; } }
                _ => {}
            }
        }

        Ok(Some(ts))
    }

    /// Read a SCALE object
    fn read_scale(&mut self) -> Result<Option<Scale>> {
        let mut scale = Scale::new("1:1", 1.0, 1.0);

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 { self.reader.push_back(pair); break; }
            match pair.code {
                5 => { if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { scale.handle = Handle::new(h); } }
                330 => { if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { scale.owner_handle = Handle::new(h); } }
                300 => scale.name = pair.value_string.clone(),
                140 => { if let Some(v) = pair.as_double() { scale.paper_units = v; } }
                141 => { if let Some(v) = pair.as_double() { scale.drawing_units = v; } }
                290 => { if let Some(v) = pair.as_bool() { scale.is_unit_scale = v; } }
                _ => {}
            }
        }

        Ok(Some(scale))
    }

    /// Read a SORTENTSTABLE object
    fn read_sort_entities_table(&mut self) -> Result<Option<SortEntitiesTable>> {
        let mut set = SortEntitiesTable::new();
        let mut entity_handle: Option<Handle> = None;

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 { self.reader.push_back(pair); break; }
            match pair.code {
                5 => {
                    if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) {
                        if set.handle.is_null() {
                            set.handle = Handle::new(h);
                        } else if let Some(eh) = entity_handle.take() {
                            set.add_entry(eh, Handle::new(h));
                        }
                    }
                }
                330 => {
                    if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) {
                        set.block_owner_handle = Handle::new(h);
                    }
                }
                331 => {
                    if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) {
                        entity_handle = Some(Handle::new(h));
                    }
                }
                _ => {}
            }
        }

        Ok(Some(set))
    }

    /// Read a DICTIONARYVAR object
    fn read_dictionary_variable(&mut self) -> Result<Option<DictionaryVariable>> {
        let mut dv = DictionaryVariable::new("", "");

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 { self.reader.push_back(pair); break; }
            match pair.code {
                5 => { if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { dv.handle = Handle::new(h); } }
                330 => { if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) { dv.owner_handle = Handle::new(h); } }
                280 => { if let Some(v) = pair.as_i16() { dv.schema_number = v; } }
                1 => dv.value = pair.value_string.clone(),
                _ => {}
            }
        }

        Ok(Some(dv))
    }
}
