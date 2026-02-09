//! CAD document structure

use crate::classes::DxfClassCollection;
use crate::entities::{EntityCommon, EntityType};
use crate::objects::ObjectType;
use crate::tables::*;
use crate::types::{DxfVersion, Color, Handle, Vector2, Vector3};
use crate::Result;
use std::collections::HashMap;

/// DWG header variables containing drawing settings
#[derive(Debug, Clone)]
pub struct HeaderVariables {
    // ==================== Version-specific Flags ====================
    /// REQUIREDVERSIONS (R2013+) - Bit coded required versions
    pub required_versions: i64,
    
    // ==================== Drawing Mode Flags ====================
    /// DIMASO - Associates dimensions with geometry
    pub associate_dimensions: bool,
    /// DIMSHO - Updates dimensions while dragging
    pub update_dimensions_while_dragging: bool,
    /// ORTHOMODE - Orthogonal mode on/off
    pub ortho_mode: bool,
    /// FILLMODE - Fill mode for solids/hatches
    pub fill_mode: bool,
    /// QTEXTMODE - Quick text mode (boxes instead of text)
    pub quick_text_mode: bool,
    /// MIRRTEXT - Mirror text on/off
    pub mirror_text: bool,
    /// REGENMODE - Auto regeneration mode
    pub regen_mode: bool,
    /// LIMCHECK - Limits checking on/off
    pub limit_check: bool,
    /// PLIMCHECK - Paper space limits checking
    pub paper_space_limit_check: bool,
    /// PLINEGEN - Line type pattern generation for polylines
    pub polyline_linetype_generation: bool,
    /// PSLTSCALE - Paper space line type scaling (0=viewport, 1=normal)
    pub paper_space_linetype_scaling: bool,
    /// TILEMODE - Show model space (tile mode)
    pub show_model_space: bool,
    /// USRTIMER - User timer on/off
    pub user_timer: bool,
    /// WORLDVIEW - World view on/off
    pub world_view: bool,
    /// VISRETAIN - Retain xref visibility settings
    pub retain_xref_visibility: bool,
    /// DISPSILH - Silhouette display for 3D objects
    pub display_silhouette: bool,
    /// SPLFRAME - Display spline control polygon
    pub spline_frame: bool,
    /// DELOBJ - Delete source objects for regions/solids
    pub delete_objects: bool,
    /// DRAGMODE - Drag mode (0=off, 1=on request, 2=auto)
    pub drag_mode: i16,
    /// BLIPMODE - Blip mode on/off
    pub blip_mode: bool,
    /// ATTREQ - Attribute entry dialogs
    pub attribute_request: bool,
    /// ATTDIA - Attribute dialog mode
    pub attribute_dialog: bool,
    
    // ==================== Unit Settings ====================
    /// LUNITS - Linear units format (0=Scientific, 1=Decimal, 2=Engineering, 3=Architectural, 4=Fractional)
    pub linear_unit_format: i16,
    /// LUPREC - Linear unit precision (0-8)
    pub linear_unit_precision: i16,
    /// AUNITS - Angular units format (0=Decimal degrees, 1=DMS, 2=Gradians, 3=Radians, 4=Surveyor)
    pub angular_unit_format: i16,
    /// AUPREC - Angular unit precision (0-8)
    pub angular_unit_precision: i16,
    /// INSUNITS - Insertion units (0=Unitless, 1=Inches, 2=Feet, etc.)
    pub insertion_units: i16,
    /// ATTMODE - Attribute display mode (0=off, 1=normal, 2=all)
    pub attribute_visibility: i16,
    /// PDMODE - Point display mode
    pub point_display_mode: i16,
    /// USERI1-5 - User integer variables
    pub user_int1: i16,
    pub user_int2: i16,
    pub user_int3: i16,
    pub user_int4: i16,
    pub user_int5: i16,
    /// COORDS - Coordinate display mode
    pub coords_mode: i16,
    /// OSMODE - Object snap mode bits
    pub object_snap_mode: i32,
    /// PICKSTYLE - Pick style
    pub pick_style: i16,
    /// SPLINETYPE - Spline type (5=quadratic, 6=cubic)
    pub spline_type: i16,
    /// SPLINESEGS - Spline segments for approximation
    pub spline_segments: i16,
    /// SPLINESEGQS - Spline segments for surface fit
    pub spline_segs_surface: i16,
    /// SURFU - Surface U density
    pub surface_u_density: i16,
    /// SURFV - Surface V density
    pub surface_v_density: i16,
    /// SURFTYPE - Surface type
    pub surface_type: i16,
    /// SURFTAB1 - Surface tabulation 1
    pub surface_tab1: i16,
    /// SURFTAB2 - Surface tabulation 2
    pub surface_tab2: i16,
    /// SHADEDGE - Shade edge mode
    pub shade_edge: i16,
    /// SHADEDIF - Shade diffuse percentage
    pub shade_diffuse: i16,
    /// MAXACTVP - Maximum active viewports
    pub max_active_viewports: i16,
    /// ISOLINES - Isolines on surfaces
    pub isolines: i16,
    /// CMLJUST - Multiline justification
    pub multiline_justification: i16,
    /// TEXTQLTY - Text quality for TrueType
    pub text_quality: i16,
    /// SORTENTS - Entity sort flags
    pub sort_entities: i16,
    /// INDEXCTL - Index control flags
    pub index_control: i16,
    /// HIDETEXT - Hide text during HIDE command
    pub hide_text: i16,
    /// XCLIPFRAME - Xref clipping frame visibility
    pub xclip_frame: i16,
    /// HALOGAP - Halo gap percentage
    pub halo_gap: i16,
    /// OBSCOLOR - Obscured line color
    pub obscured_color: i16,
    /// OBSLTYPE - Obscured line type
    pub obscured_linetype: i16,
    /// INTERSECTIONDISPLAY - Intersection polyline display
    pub intersection_display: i16,
    /// INTERSECTIONCOLOR - Intersection polyline color
    pub intersection_color: i16,
    /// DIMASSOC - Dimension associativity (0=no, 1=non-exploded, 2=associative)
    pub dimension_associativity: i16,
    /// PROJECTNAME - Project name
    pub project_name: String,
    
    // ==================== Scale/Size Defaults ====================
    /// LTSCALE - Global linetype scale
    pub linetype_scale: f64,
    /// TEXTSIZE - Default text height
    pub text_height: f64,
    /// TRACEWID - Default trace width
    pub trace_width: f64,
    /// SKETCHINC - Sketch increment
    pub sketch_increment: f64,
    /// THICKNESS - Default thickness
    pub thickness: f64,
    /// PDSIZE - Point display size
    pub point_display_size: f64,
    /// PLINEWID - Default polyline width
    pub polyline_width: f64,
    /// CELTSCALE - Current entity linetype scale
    pub current_entity_linetype_scale: f64,
    /// VIEWTWIST - View twist angle
    pub view_twist: f64,
    /// FILLETRAD - Fillet radius
    pub fillet_radius: f64,
    /// CHAMFERA - Chamfer distance A
    pub chamfer_distance_a: f64,
    /// CHAMFERB - Chamfer distance B
    pub chamfer_distance_b: f64,
    /// CHAMFERC - Chamfer length
    pub chamfer_length: f64,
    /// CHAMFERD - Chamfer angle
    pub chamfer_angle: f64,
    /// ANGBASE - Base angle
    pub angle_base: f64,
    /// ANGDIR - Angular direction (0=counterclockwise, 1=clockwise)
    pub angle_direction: i16,
    /// ELEVATION - Current elevation
    pub elevation: f64,
    /// PELEVATION - Paper space elevation
    pub paper_elevation: f64,
    /// FACETRES - Facet resolution
    pub facet_resolution: f64,
    /// CMLSCALE - Multiline scale
    pub multiline_scale: f64,
    /// USERR1-5 - User real variables
    pub user_real1: f64,
    pub user_real2: f64,
    pub user_real3: f64,
    pub user_real4: f64,
    pub user_real5: f64,
    /// PSVPSCALE - Viewport default view scale factor (R2000+)
    pub viewport_scale_factor: f64,
    /// SHADOWPLANELOCATION - Shadow plane Z location
    pub shadow_plane_location: f64,
    /// LOFTANG1 - Loft angle 1
    pub loft_angle1: f64,
    /// LOFTANG2 - Loft angle 2
    pub loft_angle2: f64,
    /// LOFTMAG1 - Loft magnitude 1
    pub loft_magnitude1: f64,
    /// LOFTMAG2 - Loft magnitude 2
    pub loft_magnitude2: f64,
    /// LOFTPARAM - Loft parameters
    pub loft_param: i16,
    /// LOFTNORMALS - Loft normals mode
    pub loft_normals: i16,
    /// LATITUDE - Geographic latitude
    pub latitude: f64,
    /// LONGITUDE - Geographic longitude
    pub longitude: f64,
    /// NORTHDIRECTION - North direction angle
    pub north_direction: f64,
    /// TIMEZONE - Time zone
    pub timezone: i32,
    /// STEPSPERSEC - Steps per second for walk/fly
    pub steps_per_second: f64,
    /// STEPSIZE - Step size for walk/fly
    pub step_size: f64,
    /// LENSLENGTH - Camera lens length
    pub lens_length: f64,
    /// CAMERAHEIGHT - Camera height
    pub camera_height: f64,
    /// CAMERADISPLAY - Camera display mode
    pub camera_display: bool,
    
    // ==================== Current Entity Settings ====================
    /// CECOLOR - Current entity color
    pub current_entity_color: Color,
    /// CELWEIGHT - Current line weight
    pub current_line_weight: i16,
    /// CEPSNTYPE - Current plot style name type
    pub current_plotstyle_type: i16,
    /// ENDCAPS - Line end cap style
    pub end_caps: i16,
    /// JOINSTYLE - Line join style
    pub join_style: i16,
    /// LWDISPLAY - Lineweight display on/off
    pub lineweight_display: bool,
    /// XEDIT - In-place xref editing
    pub xedit: bool,
    /// EXTNAMES - Extended symbol names (R2000+)
    pub extended_names: bool,
    /// PSTYLEMODE - Plot style mode (0=color, 1=named)
    pub plotstyle_mode: bool,
    /// OLESTARTUP - OLE startup
    pub ole_startup: bool,
    
    // ==================== Dimension Variables ====================
    /// DIMSCALE - Overall dimension scale factor
    pub dim_scale: f64,
    /// DIMASZ - Dimension arrow size
    pub dim_arrow_size: f64,
    /// DIMEXO - Extension line offset
    pub dim_ext_line_offset: f64,
    /// DIMDLI - Dimension line increment
    pub dim_line_increment: f64,
    /// DIMEXE - Extension line extension
    pub dim_ext_line_extension: f64,
    /// DIMRND - Dimension rounding
    pub dim_rounding: f64,
    /// DIMDLE - Dimension line extension
    pub dim_line_extension: f64,
    /// DIMTP - Dimension tolerance plus
    pub dim_tolerance_plus: f64,
    /// DIMTM - Dimension tolerance minus
    pub dim_tolerance_minus: f64,
    /// DIMTXT - Dimension text height
    pub dim_text_height: f64,
    /// DIMCEN - Center mark size
    pub dim_center_mark: f64,
    /// DIMTSZ - Tick size
    pub dim_tick_size: f64,
    /// DIMALTF - Alternate unit scale factor
    pub dim_alt_scale: f64,
    /// DIMLFAC - Linear measurements scale factor
    pub dim_linear_scale: f64,
    /// DIMTVP - Text vertical position
    pub dim_text_vertical_pos: f64,
    /// DIMTFAC - Tolerance text height scale factor
    pub dim_tolerance_scale: f64,
    /// DIMGAP - Dimension line gap
    pub dim_line_gap: f64,
    /// DIMALTRND - Alternate units rounding
    pub dim_alt_rounding: f64,
    /// DIMTOL - Tolerance generation on/off
    pub dim_tolerance: bool,
    /// DIMLIM - Limits generation on/off
    pub dim_limits: bool,
    /// DIMTIH - Text inside horizontal
    pub dim_text_inside_horizontal: bool,
    /// DIMTOH - Text outside horizontal
    pub dim_text_outside_horizontal: bool,
    /// DIMSE1 - Suppress extension line 1
    pub dim_suppress_ext1: bool,
    /// DIMSE2 - Suppress extension line 2
    pub dim_suppress_ext2: bool,
    /// DIMTAD - Text above dimension line
    pub dim_text_above: i16,
    /// DIMZIN - Zero suppression
    pub dim_zero_suppression: i16,
    /// DIMAZIN - Alternate zero suppression
    pub dim_alt_zero_suppression: i16,
    /// DIMALT - Alternate units on/off
    pub dim_alternate_units: bool,
    /// DIMALTD - Alternate decimal places
    pub dim_alt_decimal_places: i16,
    /// DIMTOFL - Force line inside
    pub dim_force_line_inside: bool,
    /// DIMSAH - Separate arrow blocks
    pub dim_separate_arrows: bool,
    /// DIMTIX - Force text inside
    pub dim_force_text_inside: bool,
    /// DIMSOXD - Suppress outside extension dim
    pub dim_suppress_outside_ext: bool,
    /// DIMCLRD - Dimension line color
    pub dim_line_color: Color,
    /// DIMCLRE - Extension line color
    pub dim_ext_line_color: Color,
    /// DIMCLRT - Dimension text color
    pub dim_text_color: Color,
    /// DIMADEC - Angular decimal places
    pub dim_angular_decimal_places: i16,
    /// DIMDEC - Decimal places
    pub dim_decimal_places: i16,
    /// DIMTDEC - Tolerance decimal places
    pub dim_tolerance_decimal_places: i16,
    /// DIMALTU - Alternate units format
    pub dim_alt_units_format: i16,
    /// DIMALTTD - Alternate tolerance decimal places
    pub dim_alt_tolerance_decimal_places: i16,
    /// DIMAUNIT - Angular units format
    pub dim_angular_units: i16,
    /// DIMFRAC - Fraction format
    pub dim_fraction_format: i16,
    /// DIMLUNIT - Linear unit format
    pub dim_linear_unit_format: i16,
    /// DIMDSEP - Decimal separator
    pub dim_decimal_separator: char,
    /// DIMTMOVE - Text movement
    pub dim_text_movement: i16,
    /// DIMJUST - Horizontal text justification
    pub dim_horizontal_justification: i16,
    /// DIMSD1 - Suppress dimension line 1
    pub dim_suppress_line1: bool,
    /// DIMSD2 - Suppress dimension line 2
    pub dim_suppress_line2: bool,
    /// DIMTOLJ - Tolerance vertical justification
    pub dim_tolerance_justification: i16,
    /// DIMTZIN - Tolerance zero suppression
    pub dim_tolerance_zero_suppression: i16,
    /// DIMALTZ - Alternate tolerance zero suppression
    pub dim_alt_tolerance_zero_suppression: i16,
    /// DIMALTTZ - Alternate tolerance zero suppression (tight)
    pub dim_alt_tolerance_zero_tight: i16,
    /// DIMFIT/DIMATFIT - Fit options
    pub dim_fit: i16,
    /// DIMUPT - User positioned text
    pub dim_user_positioned_text: bool,
    /// DIMPOST - Primary units suffix
    pub dim_post: String,
    /// DIMAPOST - Alternate units suffix
    pub dim_alt_post: String,
    /// DIMBLK - Arrow block name
    pub dim_arrow_block: String,
    /// DIMBLK1 - First arrow block name
    pub dim_arrow_block1: String,
    /// DIMBLK2 - Second arrow block name
    pub dim_arrow_block2: String,
    /// DIMLDRBLK - Leader arrow block name
    pub dim_leader_arrow_block: String,
    
    // ==================== Extents and Limits ====================
    /// INSBASE - Model space insertion base point
    pub model_space_insertion_base: Vector3,
    /// EXTMIN - Model space extents min
    pub model_space_extents_min: Vector3,
    /// EXTMAX - Model space extents max
    pub model_space_extents_max: Vector3,
    /// LIMMIN - Model space limits min
    pub model_space_limits_min: Vector2,
    /// LIMMAX - Model space limits max
    pub model_space_limits_max: Vector2,
    
    /// Paper space insertion base point
    pub paper_space_insertion_base: Vector3,
    /// Paper space extents min
    pub paper_space_extents_min: Vector3,
    /// Paper space extents max
    pub paper_space_extents_max: Vector3,
    /// Paper space limits min
    pub paper_space_limits_min: Vector2,
    /// Paper space limits max
    pub paper_space_limits_max: Vector2,
    
    // ==================== UCS Settings ====================
    /// UCSBASE - UCS base name
    pub ucs_base: String,
    /// Model space UCS name
    pub model_space_ucs_name: String,
    /// Paper space UCS name  
    pub paper_space_ucs_name: String,
    /// Model space UCS origin
    pub model_space_ucs_origin: Vector3,
    /// Model space UCS X axis
    pub model_space_ucs_x_axis: Vector3,
    /// Model space UCS Y axis
    pub model_space_ucs_y_axis: Vector3,
    /// Paper space UCS origin
    pub paper_space_ucs_origin: Vector3,
    /// Paper space UCS X axis
    pub paper_space_ucs_x_axis: Vector3,
    /// Paper space UCS Y axis
    pub paper_space_ucs_y_axis: Vector3,
    /// UCSORTHOREF - UCS orthographic reference
    pub ucs_ortho_ref: Handle,
    /// UCSORTHOVIEW - UCS orthographic view type
    pub ucs_ortho_view: i16,
    /// PUCSORTHOREF - Paper space UCS orthographic reference  
    pub paper_ucs_ortho_ref: Handle,
    /// PUCSORTHOVIEW - Paper space UCS orthographic view type
    pub paper_ucs_ortho_view: i16,
    
    // ==================== Handles/References ====================
    /// HANDSEED - Next available handle
    pub handle_seed: u64,
    /// Current layer handle
    pub current_layer_handle: Handle,
    /// Current text style handle
    pub current_text_style_handle: Handle,
    /// Current linetype handle
    pub current_linetype_handle: Handle,
    /// Current dimension style handle
    pub current_dimstyle_handle: Handle,
    /// Current multiline style handle
    pub current_multiline_style_handle: Handle,
    /// Current material handle
    pub current_material_handle: Handle,
    /// Dimension text style handle
    pub dim_text_style_handle: Handle,
    /// Dimension linetype handle
    pub dim_linetype_handle: Handle,
    /// Dimension linetype 1 handle
    pub dim_linetype1_handle: Handle,
    /// Dimension linetype 2 handle
    pub dim_linetype2_handle: Handle,
    /// Dimension arrow block handle
    pub dim_arrow_block_handle: Handle,
    /// Dimension arrow block 1 handle
    pub dim_arrow_block1_handle: Handle,
    /// Dimension arrow block 2 handle
    pub dim_arrow_block2_handle: Handle,
    /// DIMLWD - Dimension line weight
    pub dim_line_weight: i16,
    /// DIMLWE - Extension line weight
    pub dim_ext_line_weight: i16,

    // ==================== Table Control Object Handles ====================
    /// Block table control object
    pub block_control_handle: Handle,
    /// Layer table control object
    pub layer_control_handle: Handle,
    /// Text style table control object
    pub style_control_handle: Handle,
    /// Linetype table control object
    pub linetype_control_handle: Handle,
    /// View table control object
    pub view_control_handle: Handle,
    /// UCS table control object
    pub ucs_control_handle: Handle,
    /// Viewport table control object
    pub vport_control_handle: Handle,
    /// AppId table control object
    pub appid_control_handle: Handle,
    /// Dimension style table control object
    pub dimstyle_control_handle: Handle,
    /// VPEntHdr table control object
    pub vpent_hdr_control_handle: Handle,
    
    // ==================== Dictionary Handles ====================
    /// Named objects dictionary
    pub named_objects_dict_handle: Handle,
    /// ACAD_GROUP dictionary
    pub acad_group_dict_handle: Handle,
    /// ACAD_MLINESTYLE dictionary
    pub acad_mlinestyle_dict_handle: Handle,
    /// ACAD_LAYOUT dictionary (R2000+)
    pub acad_layout_dict_handle: Handle,
    /// ACAD_PLOTSETTINGS dictionary (R2000+)
    pub acad_plotsettings_dict_handle: Handle,
    /// ACAD_PLOTSTYLENAME dictionary (R2000+)
    pub acad_plotstylename_dict_handle: Handle,
    /// ACAD_MATERIAL dictionary (R2007+)
    pub acad_material_dict_handle: Handle,
    /// ACAD_COLOR dictionary (R2007+)
    pub acad_color_dict_handle: Handle,
    /// ACAD_VISUALSTYLE dictionary (R2007+)
    pub acad_visualstyle_dict_handle: Handle,
    
    // ==================== Block Record Handles ====================
    /// *MODEL_SPACE block record
    pub model_space_block_handle: Handle,
    /// *PAPER_SPACE block record
    pub paper_space_block_handle: Handle,
    /// BYLAYER linetype
    pub bylayer_linetype_handle: Handle,
    /// BYBLOCK linetype
    pub byblock_linetype_handle: Handle,
    /// CONTINUOUS linetype
    pub continuous_linetype_handle: Handle,
    
    // ==================== Date/Time ====================
    /// Document creation time (Julian date)
    pub create_date_julian: f64,
    /// Document update time (Julian date)
    pub update_date_julian: f64,
    /// Total editing time in days
    pub total_editing_time: f64,
    /// User elapsed time in days
    pub user_elapsed_time: f64,
    
    // ==================== Metadata ====================
    /// Fingerprint GUID
    pub fingerprint_guid: String,
    /// Version GUID
    pub version_guid: String,
    /// Menu file name
    pub menu_name: String,
    /// DWGCODEPAGE
    pub code_page: String,
    /// LASTSAVEDBY
    pub last_saved_by: String,
    /// HYPERLINKBASE
    pub hyperlink_base: String,
    /// STYLESHEET
    pub stylesheet: String,
    
    // ==================== Misc ====================
    /// MEASUREMENT - Drawing units (0=English, 1=Metric)
    pub measurement: i16,
    /// PROXYGRAPHICS - Show proxy graphics
    pub proxy_graphics: i16,
    /// TREEDEPTH - Tree depth for spatial index
    pub tree_depth: i16,
    /// CMLSTYLE - Current multiline style name
    pub multiline_style: String,
    /// CELTYPE - Current linetype name
    pub current_linetype_name: String,
    /// CLAYER - Current layer name
    pub current_layer_name: String,
    /// TEXTSTYLE - Current text style name
    pub current_text_style_name: String,
    /// DIMSTYLE - Current dimension style name
    pub current_dimstyle_name: String,
}

impl Default for HeaderVariables {
    fn default() -> Self {
        Self {
            // Version-specific flags
            required_versions: 0,
            
            // Drawing mode flags
            associate_dimensions: true,
            update_dimensions_while_dragging: true,
            ortho_mode: false,
            fill_mode: true,
            quick_text_mode: false,
            mirror_text: false,
            regen_mode: true,
            limit_check: false,
            paper_space_limit_check: false,
            polyline_linetype_generation: false,
            paper_space_linetype_scaling: true,
            show_model_space: true,
            user_timer: true,
            world_view: true,
            retain_xref_visibility: true,
            display_silhouette: false,
            spline_frame: false,
            delete_objects: true,
            drag_mode: 2,
            blip_mode: false,
            attribute_request: true,
            attribute_dialog: true,
            
            // Unit settings
            linear_unit_format: 2,  // Decimal
            linear_unit_precision: 4,
            angular_unit_format: 0, // Decimal degrees
            angular_unit_precision: 0,
            insertion_units: 0,     // Unitless
            attribute_visibility: 1,
            point_display_mode: 0,
            user_int1: 0, user_int2: 0, user_int3: 0, user_int4: 0, user_int5: 0,
            coords_mode: 2,
            object_snap_mode: 0,
            pick_style: 1,
            spline_type: 6,
            spline_segments: 8,
            spline_segs_surface: 6,
            surface_u_density: 6,
            surface_v_density: 6,
            surface_type: 6,
            surface_tab1: 6,
            surface_tab2: 6,
            shade_edge: 3,
            shade_diffuse: 70,
            max_active_viewports: 64,
            isolines: 4,
            multiline_justification: 0,
            text_quality: 50,
            sort_entities: 127,
            index_control: 0,
            hide_text: 1,
            xclip_frame: 0,
            halo_gap: 0,
            obscured_color: 257,
            obscured_linetype: 0,
            intersection_display: 0,
            intersection_color: 257,
            dimension_associativity: 2,
            project_name: String::new(),
            
            // Scale/size defaults
            linetype_scale: 1.0,
            text_height: 2.5,
            trace_width: 0.05,
            sketch_increment: 0.1,
            thickness: 0.0,
            point_display_size: 0.0,
            polyline_width: 0.0,
            current_entity_linetype_scale: 1.0,
            view_twist: 0.0,
            fillet_radius: 0.0,
            chamfer_distance_a: 0.0,
            chamfer_distance_b: 0.0,
            chamfer_length: 0.0,
            chamfer_angle: 0.0,
            angle_base: 0.0,
            angle_direction: 0,
            elevation: 0.0,
            paper_elevation: 0.0,
            facet_resolution: 0.5,
            multiline_scale: 1.0,
            user_real1: 0.0, user_real2: 0.0, user_real3: 0.0, user_real4: 0.0, user_real5: 0.0,
            viewport_scale_factor: 0.0,
            shadow_plane_location: 0.0,
            loft_angle1: std::f64::consts::FRAC_PI_2,
            loft_angle2: std::f64::consts::FRAC_PI_2,
            loft_magnitude1: 0.0,
            loft_magnitude2: 0.0,
            loft_param: 7,
            loft_normals: 1,
            latitude: 37.795,
            longitude: -122.394,
            north_direction: 0.0,
            timezone: -8000,
            steps_per_second: 2.0,
            step_size: 6.0,
            lens_length: 50.0,
            camera_height: 0.0,
            camera_display: false,
            
            // Current entity settings
            current_entity_color: Color::ByLayer,
            current_line_weight: -1, // ByLayer
            current_plotstyle_type: 0,
            end_caps: 0,
            join_style: 0,
            lineweight_display: false,
            xedit: true,
            extended_names: true,
            plotstyle_mode: true,
            ole_startup: false,
            
            // Dimension variables
            dim_scale: 1.0,
            dim_arrow_size: 0.18,
            dim_ext_line_offset: 0.0625,
            dim_line_increment: 0.38,
            dim_ext_line_extension: 0.18,
            dim_rounding: 0.0,
            dim_line_extension: 0.0,
            dim_tolerance_plus: 0.0,
            dim_tolerance_minus: 0.0,
            dim_text_height: 0.18,
            dim_center_mark: 0.09,
            dim_tick_size: 0.0,
            dim_alt_scale: 25.4,
            dim_linear_scale: 1.0,
            dim_text_vertical_pos: 0.0,
            dim_tolerance_scale: 1.0,
            dim_line_gap: 0.09,
            dim_alt_rounding: 0.0,
            dim_tolerance: false,
            dim_limits: false,
            dim_text_inside_horizontal: true,
            dim_text_outside_horizontal: true,
            dim_suppress_ext1: false,
            dim_suppress_ext2: false,
            dim_text_above: 0,
            dim_zero_suppression: 0,
            dim_alt_zero_suppression: 0,
            dim_alternate_units: false,
            dim_alt_decimal_places: 2,
            dim_force_line_inside: false,
            dim_separate_arrows: false,
            dim_force_text_inside: false,
            dim_suppress_outside_ext: false,
            dim_line_color: Color::ByBlock,
            dim_ext_line_color: Color::ByBlock,
            dim_text_color: Color::ByBlock,
            dim_angular_decimal_places: 0,
            dim_decimal_places: 4,
            dim_tolerance_decimal_places: 4,
            dim_alt_units_format: 2,
            dim_alt_tolerance_decimal_places: 4,
            dim_angular_units: 0,
            dim_fraction_format: 0,
            dim_linear_unit_format: 2,
            dim_decimal_separator: '.',
            dim_text_movement: 0,
            dim_horizontal_justification: 0,
            dim_suppress_line1: false,
            dim_suppress_line2: false,
            dim_tolerance_justification: 1,
            dim_tolerance_zero_suppression: 0,
            dim_alt_tolerance_zero_suppression: 0,
            dim_alt_tolerance_zero_tight: 0,
            dim_fit: 3,
            dim_user_positioned_text: false,
            dim_post: String::new(),
            dim_alt_post: String::new(),
            dim_arrow_block: String::new(),
            dim_arrow_block1: String::new(),
            dim_arrow_block2: String::new(),
            dim_leader_arrow_block: String::new(),
            
            // Extents and limits - Model space
            model_space_insertion_base: Vector3::ZERO,
            model_space_extents_min: Vector3::new(1e20, 1e20, 1e20),
            model_space_extents_max: Vector3::new(-1e20, -1e20, -1e20),
            model_space_limits_min: Vector2::new(0.0, 0.0),
            model_space_limits_max: Vector2::new(12.0, 9.0),
            
            // Extents and limits - Paper space
            paper_space_insertion_base: Vector3::ZERO,
            paper_space_extents_min: Vector3::new(1e20, 1e20, 1e20),
            paper_space_extents_max: Vector3::new(-1e20, -1e20, -1e20),
            paper_space_limits_min: Vector2::new(0.0, 0.0),
            paper_space_limits_max: Vector2::new(12.0, 9.0),
            
            // UCS settings
            ucs_base: String::new(),
            model_space_ucs_name: String::new(),
            paper_space_ucs_name: String::new(),
            model_space_ucs_origin: Vector3::ZERO,
            model_space_ucs_x_axis: Vector3::new(1.0, 0.0, 0.0),
            model_space_ucs_y_axis: Vector3::new(0.0, 1.0, 0.0),
            paper_space_ucs_origin: Vector3::ZERO,
            paper_space_ucs_x_axis: Vector3::new(1.0, 0.0, 0.0),
            paper_space_ucs_y_axis: Vector3::new(0.0, 1.0, 0.0),
            ucs_ortho_ref: Handle::NULL,
            ucs_ortho_view: 0,
            paper_ucs_ortho_ref: Handle::NULL,
            paper_ucs_ortho_view: 0,
            
            // Handles
            handle_seed: 1,
            current_layer_handle: Handle::NULL,
            current_text_style_handle: Handle::NULL,
            current_linetype_handle: Handle::NULL,
            current_dimstyle_handle: Handle::NULL,
            current_multiline_style_handle: Handle::NULL,
            current_material_handle: Handle::NULL,
            dim_text_style_handle: Handle::NULL,
            dim_linetype_handle: Handle::NULL,
            dim_linetype1_handle: Handle::NULL,
            dim_linetype2_handle: Handle::NULL,
            dim_arrow_block_handle: Handle::NULL,
            dim_arrow_block1_handle: Handle::NULL,
            dim_arrow_block2_handle: Handle::NULL,
            dim_line_weight: -2,      // ByBlock
            dim_ext_line_weight: -2,  // ByBlock
            
            // Table control handles
            block_control_handle: Handle::NULL,
            layer_control_handle: Handle::NULL,
            style_control_handle: Handle::NULL,
            linetype_control_handle: Handle::NULL,
            view_control_handle: Handle::NULL,
            ucs_control_handle: Handle::NULL,
            vport_control_handle: Handle::NULL,
            appid_control_handle: Handle::NULL,
            dimstyle_control_handle: Handle::NULL,
            vpent_hdr_control_handle: Handle::NULL,
            
            // Dictionary handles
            named_objects_dict_handle: Handle::NULL,
            acad_group_dict_handle: Handle::NULL,
            acad_mlinestyle_dict_handle: Handle::NULL,
            acad_layout_dict_handle: Handle::NULL,
            acad_plotsettings_dict_handle: Handle::NULL,
            acad_plotstylename_dict_handle: Handle::NULL,
            acad_material_dict_handle: Handle::NULL,
            acad_color_dict_handle: Handle::NULL,
            acad_visualstyle_dict_handle: Handle::NULL,
            
            // Block record handles
            model_space_block_handle: Handle::NULL,
            paper_space_block_handle: Handle::NULL,
            bylayer_linetype_handle: Handle::NULL,
            byblock_linetype_handle: Handle::NULL,
            continuous_linetype_handle: Handle::NULL,
            
            // Date/time
            create_date_julian: 0.0,
            update_date_julian: 0.0,
            total_editing_time: 0.0,
            user_elapsed_time: 0.0,
            
            // Metadata
            fingerprint_guid: String::new(),
            version_guid: String::new(),
            menu_name: String::new(),
            code_page: String::from("ANSI_1252"),
            last_saved_by: String::new(),
            hyperlink_base: String::new(),
            stylesheet: String::new(),
            
            // Misc
            measurement: 0,
            proxy_graphics: 1,
            tree_depth: 3020,
            multiline_style: String::from("Standard"),
            current_linetype_name: String::from("ByLayer"),
            current_layer_name: String::from("0"),
            current_text_style_name: String::from("Standard"),
            current_dimstyle_name: String::from("Standard"),
        }
    }
}

/// A CAD document containing all drawing data
#[derive(Debug, Clone)]
pub struct CadDocument {
    /// Document version
    pub version: DxfVersion,
    
    /// Header variables containing drawing settings
    pub header: HeaderVariables,
    
    /// Layer table
    pub layers: Table<Layer>,
    
    /// Line type table
    pub line_types: Table<LineType>,
    
    /// Text style table
    pub text_styles: Table<TextStyle>,
    
    /// Block record table
    pub block_records: Table<BlockRecord>,
    
    /// Dimension style table
    pub dim_styles: Table<DimStyle>,
    
    /// Application ID table
    pub app_ids: Table<AppId>,
    
    /// View table
    pub views: Table<View>,
    
    /// Viewport table
    pub vports: Table<VPort>,
    
    /// UCS table
    pub ucss: Table<Ucs>,
    
    /// DXF class definitions (CLASSES section)
    pub classes: DxfClassCollection,

    /// Notifications collected during the last read/write operation
    pub notifications: crate::notification::NotificationCollection,

    /// All entities in the document (indexed by handle)
    entities: HashMap<Handle, EntityType>,

    /// All objects in the document (indexed by handle)
    pub objects: HashMap<Handle, ObjectType>,

    /// Next handle to assign
    next_handle: u64,
}

impl CadDocument {
    /// Create a new empty CAD document
    pub fn new() -> Self {
        let mut doc = CadDocument {
            version: DxfVersion::AC1032, // DXF 2018
            header: HeaderVariables::default(),
            layers: Table::new(),
            line_types: Table::new(),
            text_styles: Table::new(),
            block_records: Table::new(),
            dim_styles: Table::new(),
            app_ids: Table::new(),
            views: Table::new(),
            vports: Table::new(),
            ucss: Table::new(),
            classes: DxfClassCollection::new(),
            notifications: crate::notification::NotificationCollection::new(),
            entities: HashMap::new(),
            objects: HashMap::new(),
            // Start handle allocation above reserved table handles (0x1-0xA)
            // Table handles are well-known fixed values used by AutoCAD
            next_handle: 0x10,
        };
        
        // Initialize with standard entries
        doc.initialize_defaults();
        doc
    }

    /// Create a document with a specific version
    pub fn with_version(version: DxfVersion) -> Self {
        let mut doc = Self::new();
        doc.version = version;
        doc
    }

    /// Initialize default tables with standard entries
    fn initialize_defaults(&mut self) {
        // Allocate table control handles first (these are well-known handles in DWG)
        self.header.block_control_handle = self.allocate_handle();
        self.header.layer_control_handle = self.allocate_handle();
        self.header.style_control_handle = self.allocate_handle();
        self.header.linetype_control_handle = self.allocate_handle();
        self.header.view_control_handle = self.allocate_handle();
        self.header.ucs_control_handle = self.allocate_handle();
        self.header.vport_control_handle = self.allocate_handle();
        self.header.appid_control_handle = self.allocate_handle();
        self.header.dimstyle_control_handle = self.allocate_handle();
        
        // Add standard layer "0"
        let mut layer0 = Layer::layer_0();
        layer0.set_handle(self.allocate_handle());
        // Store the layer handle for CLAYER
        self.header.current_layer_handle = layer0.handle;
        self.layers.add(layer0).ok();

        // Add standard line types
        let mut continuous = LineType::continuous();
        continuous.set_handle(self.allocate_handle());
        self.header.continuous_linetype_handle = continuous.handle;
        self.line_types.add(continuous).ok();

        let mut by_layer = LineType::by_layer();
        by_layer.set_handle(self.allocate_handle());
        self.header.bylayer_linetype_handle = by_layer.handle;
        self.header.current_linetype_handle = by_layer.handle; // Default linetype is ByLayer
        self.line_types.add(by_layer).ok();

        let mut by_block = LineType::by_block();
        by_block.set_handle(self.allocate_handle());
        self.header.byblock_linetype_handle = by_block.handle;
        self.line_types.add(by_block).ok();

        // Add standard text style
        let mut standard_style = TextStyle::standard();
        standard_style.set_handle(self.allocate_handle());
        self.header.current_text_style_handle = standard_style.handle;
        self.text_styles.add(standard_style).ok();

        // Add model space and paper space blocks
        let mut model_space = BlockRecord::model_space();
        model_space.set_handle(self.allocate_handle());
        model_space.block_entity_handle = self.allocate_handle();
        model_space.block_end_handle = self.allocate_handle();
        self.header.model_space_block_handle = model_space.handle;
        self.block_records.add(model_space).ok();

        let mut paper_space = BlockRecord::paper_space();
        paper_space.set_handle(self.allocate_handle());
        paper_space.block_entity_handle = self.allocate_handle();
        paper_space.block_end_handle = self.allocate_handle();
        self.header.paper_space_block_handle = paper_space.handle;
        self.block_records.add(paper_space).ok();

        // Add standard dimension style
        let mut standard_dimstyle = DimStyle::standard();
        standard_dimstyle.set_handle(self.allocate_handle());
        self.header.current_dimstyle_handle = standard_dimstyle.handle;
        self.dim_styles.add(standard_dimstyle).ok();

        // Add standard application ID
        let mut acad = AppId::acad();
        acad.set_handle(self.allocate_handle());
        self.app_ids.add(acad).ok();

        // Add standard viewport
        let mut active_vport = VPort::active();
        active_vport.set_handle(self.allocate_handle());
        self.vports.add(active_vport).ok();
        
        // Allocate dictionary handles (required for DWG format)
        self.header.named_objects_dict_handle = self.allocate_handle();
        self.header.acad_group_dict_handle = self.allocate_handle();
        self.header.acad_mlinestyle_dict_handle = self.allocate_handle();
        self.header.acad_layout_dict_handle = self.allocate_handle();
        self.header.acad_plotsettings_dict_handle = self.allocate_handle();
        self.header.acad_plotstylename_dict_handle = self.allocate_handle();
    }

    /// Allocate a new unique handle
    pub fn allocate_handle(&mut self) -> Handle {
        let handle = Handle::new(self.next_handle);
        self.next_handle += 1;
        handle
    }

    /// Get the next handle value (without allocating)
    pub fn next_handle(&self) -> u64 {
        self.next_handle
    }

    /// Add an entity to the document
    pub fn add_entity(&mut self, mut entity: EntityType) -> Result<Handle> {
        // Allocate a handle if the entity doesn't have one
        let handle = if entity.as_entity().handle().is_null() {
            let h = self.allocate_handle();
            entity.as_entity_mut().set_handle(h);
            h
        } else {
            entity.as_entity().handle()
        };

        // Store the entity
        self.entities.insert(handle, entity);
        Ok(handle)
    }

    /// Get an entity by handle
    pub fn get_entity(&self, handle: Handle) -> Option<&EntityType> {
        self.entities.get(&handle)
    }

    /// Get a mutable entity by handle
    pub fn get_entity_mut(&mut self, handle: Handle) -> Option<&mut EntityType> {
        self.entities.get_mut(&handle)
    }

    /// Remove an entity by handle
    pub fn remove_entity(&mut self, handle: Handle) -> Option<EntityType> {
        self.entities.remove(&handle)
    }

    /// Get the number of entities
    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    /// Iterate over all entities
    pub fn entities(&self) -> impl Iterator<Item = &EntityType> {
        self.entities.values()
    }

    /// Iterate over all entities mutably
    pub fn entities_mut(&mut self) -> impl Iterator<Item = &mut EntityType> {
        self.entities.values_mut()
    }

    /// Resolve handle references after reading a DXF file.
    ///
    /// This performs a simplified version of ACadSharp's two-phase build:
    ///
    /// 1. Assigns owner handles on model-space entities (owner = model space
    ///    block record handle) when the entity has no owner set.
    /// 2. Assigns owner handles on block-owned entities (owner = the block
    ///    record handle) when the entity has no owner set.
    /// 3. Updates `next_handle` to be above the maximum handle seen in the
    ///    document so that subsequent `allocate_handle()` calls produce unique
    ///    values.
    ///
    /// Call this once after loading (the DXF reader calls it automatically).
    pub fn resolve_references(&mut self) {
        // --- 1. Find the max handle in use across the whole document ---
        let mut max_handle: u64 = self.next_handle;

        // Check entities
        for entity in self.entities.values() {
            let h = entity.common().handle.value();
            if h >= max_handle {
                max_handle = h + 1;
            }
        }

        // Check objects
        for (handle, _) in &self.objects {
            let h = handle.value();
            if h >= max_handle {
                max_handle = h + 1;
            }
        }

        // Check block record entities
        for br in self.block_records.iter() {
            let h = br.handle.value();
            if h >= max_handle {
                max_handle = h + 1;
            }
            for entity in &br.entities {
                let h = entity.common().handle.value();
                if h >= max_handle {
                    max_handle = h + 1;
                }
            }
        }

        self.next_handle = max_handle;

        // --- 2. Assign owner handles ---
        let model_handle = self.header.model_space_block_handle;
        let paper_handle = self.header.paper_space_block_handle;

        // Model-space entities (document.entities) — use model space as default owner
        for entity in self.entities.values_mut() {
            let common = match entity {
                EntityType::Dimension(d) => {
                    let base = d.base_mut();
                    &mut base.common
                }
                _ => {
                    // For all other entity types, use as_entity_mut().set_handle pattern
                    // but we need &mut EntityCommon directly — use a helper
                    get_common_mut(entity)
                }
            };
            if common.owner_handle.is_null() {
                common.owner_handle = model_handle;
            }
        }

        // Block record entities — owner is the block record handle
        for br in self.block_records.iter_mut() {
            let br_handle = br.handle;
            for entity in &mut br.entities {
                let common = match entity {
                    EntityType::Dimension(d) => {
                        let base = d.base_mut();
                        &mut base.common
                    }
                    _ => get_common_mut(entity),
                };
                if common.owner_handle.is_null() {
                    common.owner_handle = br_handle;
                }
            }
        }

        // Paper-space entities — if an entity's owner is the paper space block,
        // the entity is already correctly assigned by the reader.
        // We just skip further assignment here.

        let _ = paper_handle; // suppress unused warning; future: paper space logic
    }
}

/// Helper to get a mutable reference to EntityCommon for non-Dimension entities.
fn get_common_mut(entity: &mut EntityType) -> &mut EntityCommon {
    match entity {
        EntityType::Point(e) => &mut e.common,
        EntityType::Line(e) => &mut e.common,
        EntityType::Circle(e) => &mut e.common,
        EntityType::Arc(e) => &mut e.common,
        EntityType::Ellipse(e) => &mut e.common,
        EntityType::Polyline(e) => &mut e.common,
        EntityType::Polyline2D(e) => &mut e.common,
        EntityType::Polyline3D(e) => &mut e.common,
        EntityType::LwPolyline(e) => &mut e.common,
        EntityType::Text(e) => &mut e.common,
        EntityType::MText(e) => &mut e.common,
        EntityType::Spline(e) => &mut e.common,
        EntityType::Dimension(d) => &mut d.base_mut().common,
        EntityType::Hatch(e) => &mut e.common,
        EntityType::Solid(e) => &mut e.common,
        EntityType::Face3D(e) => &mut e.common,
        EntityType::Insert(e) => &mut e.common,
        EntityType::Block(e) => &mut e.common,
        EntityType::BlockEnd(e) => &mut e.common,
        EntityType::Ray(e) => &mut e.common,
        EntityType::XLine(e) => &mut e.common,
        EntityType::Viewport(e) => &mut e.common,
        EntityType::AttributeDefinition(e) => &mut e.common,
        EntityType::AttributeEntity(e) => &mut e.common,
        EntityType::Leader(e) => &mut e.common,
        EntityType::MultiLeader(e) => &mut e.common,
        EntityType::MLine(e) => &mut e.common,
        EntityType::Mesh(e) => &mut e.common,
        EntityType::RasterImage(e) => &mut e.common,
        EntityType::Solid3D(e) => &mut e.common,
        EntityType::Region(e) => &mut e.common,
        EntityType::Body(e) => &mut e.common,
        EntityType::Table(e) => &mut e.common,
        EntityType::Tolerance(e) => &mut e.common,
        EntityType::PolyfaceMesh(e) => &mut e.common,
        EntityType::Wipeout(e) => &mut e.common,
        EntityType::Shape(e) => &mut e.common,
        EntityType::Underlay(e) => &mut e.common,
        EntityType::Seqend(e) => &mut e.common,
        EntityType::Ole2Frame(e) => &mut e.common,
        EntityType::PolygonMesh(e) => &mut e.common,
        EntityType::Unknown(e) => &mut e.common,
    }
}

impl Default for CadDocument {
    fn default() -> Self {
        Self::new()
    }
}


