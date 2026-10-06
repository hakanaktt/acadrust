//! Roundtrip analysis for a real-world DWG file.
//!
//! Reads a DWG file, writes it back out (both DWG and DXF), saves the outputs
//! to disk next to the source file, then reads them back and reports every
//! detected data loss with deep per-type diagnostics.
//!
//! Usage:
//!   cargo run --example roundtrip_analysis -- [path/to/file.dwg]
//!
//! Defaults to tests/roundtrip/samplekitchen.dwg when no argument is given.
//!
//! Output files written next to the source:
//!   <stem>_rt.dwg   – DWG roundtripped copy
//!   <stem>_rt.dxf   – DXF roundtripped copy
//!   <stem>_rt2.dwg  – DWG double-roundtripped copy (stability check)

use std::collections::BTreeMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use acadrust::entities::EntityType;
use acadrust::types::Handle;
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};

// ═══════════════════════════════════════════════════════════════════════════
//  DISK I/O HELPERS
// ═══════════════════════════════════════════════════════════════════════════

/// Derive a sibling output path: `/dir/<stem><suffix>.<ext>`.
fn sibling_path(source: &Path, suffix: &str, ext: &str) -> PathBuf {
    let stem = source
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "output".to_string());
    let mut p = source.to_path_buf();
    p.set_file_name(format!("{}{}.{}", stem, suffix, ext));
    p
}

fn save_bytes(path: &Path, bytes: &[u8]) {
    match std::fs::write(path, bytes) {
        Ok(_) => println!("  Saved {} bytes → {}", bytes.len(), path.display()),
        Err(e) => eprintln!("  WARNING: could not write {}: {}", path.display(), e),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  HELPERS
// ═══════════════════════════════════════════════════════════════════════════

fn entity_variant_name(entity: &EntityType) -> String {
    let dbg = format!("{:?}", entity);
    if let Some(paren_pos) = dbg.find('(') {
        dbg[..paren_pos].to_string()
    } else {
        dbg
    }
}

fn entity_type_counts(doc: &CadDocument) -> BTreeMap<String, usize> {
    let mut map = BTreeMap::new();
    for entity in doc.entities() {
        *map.entry(entity_variant_name(entity)).or_insert(0) += 1;
    }
    map
}

fn normalize_common(common: &mut acadrust::entities::EntityCommon) {
    common.handle = Handle::NULL;
    common.owner_handle = Handle::NULL;
    common.reactors.clear();
    common.xdictionary_handle = None;
}

fn normalize_entity(entity: &mut EntityType) {
    normalize_common(entity.common_mut());
    match entity {
        EntityType::Polyline3D(p) => {
            for v in &mut p.vertices {
                v.handle = Handle::NULL;
                v.layer = String::new();
            }
        }
        EntityType::PolyfaceMesh(pf) => {
            pf.seqend_handle = None;
            for v in &mut pf.vertices {
                normalize_common(&mut v.common);
            }
            for f in &mut pf.faces {
                normalize_common(&mut f.common);
            }
        }
        EntityType::Hatch(h) => {
            for path in &mut h.paths {
                path.boundary_handles.clear();
            }
        }
        EntityType::MLine(ml) => {
            ml.style_handle = None;
        }
        EntityType::MultiLeader(mld) => {
            mld.style_handle = None;
            mld.line_type_handle = None;
            mld.arrowhead_handle = None;
            mld.text_style_handle = None;
            mld.block_content_handle = None;
            mld.context.text_style_handle = None;
            mld.context.block_content_handle = None;
            mld.context.scale_handle = None;
            for root in &mut mld.context.leader_roots {
                for line in &mut root.lines {
                    line.line_type_handle = None;
                    line.arrowhead_handle = None;
                }
            }
            for attr in &mut mld.block_attributes {
                attr.attribute_definition_handle = None;
            }
        }
        EntityType::Tolerance(t) => {
            t.dimension_style_handle = None;
        }
        EntityType::Shape(s) => {
            s.style_handle = None;
        }
        EntityType::Leader(l) => {
            l.annotation_handle = Handle::NULL;
        }
        EntityType::Viewport(v) => {
            v.ucs_handle = Handle::NULL;
            v.base_ucs_handle = Handle::NULL;
            v.background_handle = Handle::NULL;
            v.shade_plot_handle = Handle::NULL;
            v.visual_style_handle = Handle::NULL;
        }
        EntityType::Dimension(d) => {
            let base = d.base_mut();
            base.block_name = String::new();
            base.actual_measurement = 0.0;
        }
        EntityType::Insert(ins) => {
            for attr in &mut ins.attributes {
                normalize_common(&mut attr.common);
                attr.attdef_handle = Handle::NULL;
            }
        }
        EntityType::RasterImage(r) => {
            r.definition_handle = None;
            r.definition_reactor_handle = None;
        }
        EntityType::Mesh(m) => {
            for edge in &mut m.edges {
                if edge.crease.is_none() {
                    edge.crease = Some(0.0);
                }
            }
            m.edges
                .sort_by(|a, b| a.start.cmp(&b.start).then(a.end.cmp(&b.end)));
        }
        EntityType::Text(t) => {
            t.style = t.style.to_uppercase();
        }
        EntityType::MText(m) => {
            m.style = m.style.to_uppercase();
        }
        _ => {}
    }
}

fn field_diff(orig: &str, rt: &str) -> Vec<String> {
    let orig_lines: Vec<&str> = orig.lines().collect();
    let rt_lines: Vec<&str> = rt.lines().collect();
    let max = orig_lines.len().max(rt_lines.len());
    let mut diffs = Vec::new();
    for i in 0..max {
        let o = orig_lines.get(i).unwrap_or(&"<missing>");
        let r = rt_lines.get(i).unwrap_or(&"<missing>");
        if o != r {
            diffs.push(format!(
                "    line {}: ORIG: {}\n             RT:   {}",
                i + 1,
                o.trim(),
                r.trim()
            ));
        }
    }
    diffs
}

// ═══════════════════════════════════════════════════════════════════════════
//  DOCUMENT ANALYSIS
// ═══════════════════════════════════════════════════════════════════════════

fn print_document_summary(label: &str, doc: &CadDocument) {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║  {}  ║", format!("{:<56}", label));
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  Version:    {:?}", doc.version);
    println!("║  Entities:   {}", doc.entity_count());
    println!("║  Layers:     {}", doc.layers.len());
    println!("║  LineTypes:  {}", doc.line_types.len());
    println!("║  TextStyles: {}", doc.text_styles.len());
    println!("║  DimStyles:  {}", doc.dim_styles.len());
    println!("║  BlockRecs:  {}", doc.block_records.len());
    println!("║  AppIds:     {}", doc.app_ids.len());
    println!("║  Views:      {}", doc.views.len());
    println!("║  VPorts:     {}", doc.vports.len());
    println!("║  UCSs:       {}", doc.ucss.len());
    println!("║  Objects:    {}", doc.objects.len());
    println!("║  Classes:    {}", doc.classes.len());

    let counts = entity_type_counts(doc);
    println!("║  ─── Entity breakdown ───");
    for (name, count) in &counts {
        println!("║    {:<30} {}", name, count);
    }

    // Notifications
    let notifs: Vec<_> = doc.notifications.iter().collect();
    if !notifs.is_empty() {
        println!("║  ─── Notifications ({}) ───", notifs.len());
        // Show all Error notifications
        for n in notifs
            .iter()
            .filter(|n| format!("{}", n.notification_type) == "Error")
        {
            println!("║    [{}] {}", n.notification_type, n.message);
        }
        // Show first 5 Warning notifications
        for n in notifs
            .iter()
            .filter(|n| format!("{}", n.notification_type) != "Error")
            .take(5)
        {
            println!(
                "║    [{}] {}...",
                n.notification_type,
                &n.message[..n.message.len().min(80)]
            );
        }
        if notifs.len() > 5 {
            println!(
                "║    ... and {} more notifications",
                notifs.len().saturating_sub(5)
            );
        }
    }
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
}

// ═══════════════════════════════════════════════════════════════════════════
//  COMPARISON
// ═══════════════════════════════════════════════════════════════════════════

struct LossReport {
    format: String,
    count_diffs: Vec<String>,
    entity_diffs: Vec<String>,
    table_diffs: Vec<String>,
    header_diffs: Vec<String>,
    object_diffs: Vec<String>,
}

impl LossReport {
    fn new(format: &str) -> Self {
        Self {
            format: format.to_string(),
            count_diffs: Vec::new(),
            entity_diffs: Vec::new(),
            table_diffs: Vec::new(),
            header_diffs: Vec::new(),
            object_diffs: Vec::new(),
        }
    }

    fn total(&self) -> usize {
        self.count_diffs.len()
            + self.entity_diffs.len()
            + self.table_diffs.len()
            + self.header_diffs.len()
            + self.object_diffs.len()
    }

    fn print(&self) {
        let total = self.total();
        println!("┌──────────────────────────────────────────────────────────────┐");
        println!(
            "│  {} Roundtrip: {} difference(s) found{}│",
            self.format,
            total,
            " ".repeat(38usize.saturating_sub(self.format.len() + format!("{}", total).len()))
        );
        println!("└──────────────────────────────────────────────────────────────┘");

        if total == 0 {
            println!("  ✓ PERFECT roundtrip — no data loss detected!\n");
            return;
        }

        if !self.count_diffs.is_empty() {
            println!("\n  ── Count Mismatches ──");
            for d in &self.count_diffs {
                println!("  • {}", d);
            }
        }

        if !self.table_diffs.is_empty() {
            println!("\n  ── Table Differences ──");
            for d in &self.table_diffs {
                println!("  • {}", d);
            }
        }

        if !self.header_diffs.is_empty() {
            println!("\n  ── Header Variable Differences ──");
            for d in &self.header_diffs {
                println!("  • {}", d);
            }
        }

        if !self.object_diffs.is_empty() {
            println!("\n  ── Object Differences ──");
            for d in &self.object_diffs {
                println!("  • {}", d);
            }
        }

        if !self.entity_diffs.is_empty() {
            println!(
                "\n  ── Entity Data Differences ({}) ──",
                self.entity_diffs.len()
            );
            // Show up to 50 diffs, summarize the rest
            for (i, d) in self.entity_diffs.iter().enumerate() {
                if i >= 50 {
                    println!(
                        "  ... and {} more entity differences",
                        self.entity_diffs.len() - 50
                    );
                    break;
                }
                println!("  • {}", d);
            }
        }
        println!();
    }
}

fn compare_documents(orig: &CadDocument, rt: &CadDocument, format: &str) -> LossReport {
    let mut report = LossReport::new(format);

    // ── Entity counts ─────────────────────────────────────────────
    let orig_count = orig.entity_count();
    let rt_count = rt.entity_count();
    if orig_count != rt_count {
        report.count_diffs.push(format!(
            "Total entity count: {} → {} (Δ{})",
            orig_count,
            rt_count,
            rt_count as isize - orig_count as isize
        ));
    }

    let orig_types = entity_type_counts(orig);
    let rt_types = entity_type_counts(rt);
    for (name, &orig_c) in &orig_types {
        let rt_c = rt_types.get(name).copied().unwrap_or(0);
        if orig_c != rt_c {
            report.count_diffs.push(format!(
                "  {}: {} → {} (Δ{})",
                name,
                orig_c,
                rt_c,
                rt_c as isize - orig_c as isize
            ));
        }
    }
    for (name, &rt_c) in &rt_types {
        if !orig_types.contains_key(name) {
            report
                .count_diffs
                .push(format!("  {} appeared in roundtrip: 0 → {}", name, rt_c));
        }
    }

    // ── Tables ────────────────────────────────────────────────────
    macro_rules! cmp_table {
        ($name:expr, $field:ident) => {
            if orig.$field.len() != rt.$field.len() {
                report.table_diffs.push(format!(
                    "{}: {} → {}",
                    $name,
                    orig.$field.len(),
                    rt.$field.len()
                ));
            }
        };
    }
    cmp_table!("Layers", layers);
    cmp_table!("LineTypes", line_types);
    cmp_table!("TextStyles", text_styles);
    cmp_table!("BlockRecords", block_records);
    cmp_table!("DimStyles", dim_styles);
    cmp_table!("AppIds", app_ids);
    cmp_table!("Views", views);
    cmp_table!("VPorts", vports);
    cmp_table!("UCSs", ucss);

    // ── Objects ───────────────────────────────────────────────────
    if orig.objects.len() != rt.objects.len() {
        report.object_diffs.push(format!(
            "Object count: {} → {}",
            orig.objects.len(),
            rt.objects.len()
        ));
    }

    // ── Classes ───────────────────────────────────────────────────
    if orig.classes.len() != rt.classes.len() {
        report.object_diffs.push(format!(
            "Class count: {} → {}",
            orig.classes.len(),
            rt.classes.len()
        ));
    }

    // ── Header variables ──────────────────────────────────────────
    compare_header(&mut report, &orig.header, &rt.header);

    // ── Per-entity comparison ─────────────────────────────────────
    compare_entities(&mut report, orig, rt);

    report
}

fn compare_header(
    report: &mut LossReport,
    orig: &acadrust::document::HeaderVariables,
    rt: &acadrust::document::HeaderVariables,
) {
    macro_rules! cmp {
        ($field:ident) => {
            if orig.$field != rt.$field {
                report.header_diffs.push(format!(
                    "{}: {:?} → {:?}",
                    stringify!($field),
                    orig.$field,
                    rt.$field
                ));
            }
        };
    }
    cmp!(associate_dimensions);
    cmp!(ortho_mode);
    cmp!(fill_mode);
    cmp!(quick_text_mode);
    cmp!(mirror_text);
    cmp!(regen_mode);
    cmp!(limit_check);
    cmp!(show_model_space);
    cmp!(world_view);
    cmp!(retain_xref_visibility);
    cmp!(display_silhouette);
    cmp!(linear_unit_format);
    cmp!(linear_unit_precision);
    cmp!(angular_unit_format);
    cmp!(angular_unit_precision);
    cmp!(insertion_units);
    cmp!(linetype_scale);
    cmp!(text_height);
    cmp!(dim_scale);
    cmp!(dim_arrow_size);
    cmp!(dim_text_height);
    cmp!(dim_tolerance);
    cmp!(dim_limits);
    cmp!(dim_decimal_places);
    cmp!(model_space_insertion_base);
    cmp!(model_space_limits_min);
    cmp!(model_space_limits_max);
    cmp!(measurement);
}

fn compare_entities(report: &mut LossReport, orig: &CadDocument, rt: &CadDocument) {
    let mut orig_by_type: BTreeMap<String, Vec<&EntityType>> = BTreeMap::new();
    let mut rt_by_type: BTreeMap<String, Vec<&EntityType>> = BTreeMap::new();

    for e in orig.entities() {
        orig_by_type
            .entry(entity_variant_name(e))
            .or_default()
            .push(e);
    }
    for e in rt.entities() {
        rt_by_type
            .entry(entity_variant_name(e))
            .or_default()
            .push(e);
    }

    for (type_name, orig_entities) in &orig_by_type {
        let rt_entities = match rt_by_type.get(type_name) {
            Some(v) => v,
            None => {
                report.entity_diffs.push(format!(
                    "{}: all {} entities LOST",
                    type_name,
                    orig_entities.len()
                ));
                continue;
            }
        };

        let count = orig_entities.len().min(rt_entities.len());
        for i in 0..count {
            let mut o = orig_entities[i].clone();
            let mut r = rt_entities[i].clone();

            // Check common fields before normalization
            let o_common = o.common();
            let r_common = r.common();
            let mut common_diffs = Vec::new();
            if o_common.layer != r_common.layer {
                common_diffs.push(format!(
                    "layer: {:?} → {:?}",
                    o_common.layer, r_common.layer
                ));
            }
            if o_common.color != r_common.color {
                common_diffs.push(format!(
                    "color: {:?} → {:?}",
                    o_common.color, r_common.color
                ));
            }
            if o_common.line_weight != r_common.line_weight {
                common_diffs.push(format!(
                    "line_weight: {:?} → {:?}",
                    o_common.line_weight, r_common.line_weight
                ));
            }
            if o_common.linetype != r_common.linetype {
                common_diffs.push(format!(
                    "linetype: {:?} → {:?}",
                    o_common.linetype, r_common.linetype
                ));
            }
            if (o_common.linetype_scale - r_common.linetype_scale).abs() > 1e-10 {
                common_diffs.push(format!(
                    "linetype_scale: {} → {}",
                    o_common.linetype_scale, r_common.linetype_scale
                ));
            }
            if o_common.invisible != r_common.invisible {
                common_diffs.push(format!(
                    "invisible: {} → {}",
                    o_common.invisible, r_common.invisible
                ));
            }
            for cd in common_diffs {
                report
                    .entity_diffs
                    .push(format!("{}[{}] common.{}", type_name, i, cd));
            }

            // Normalize and compare geometry
            normalize_entity(&mut o);
            normalize_entity(&mut r);

            if o != r {
                let o_dbg = format!("{:#?}", o);
                let r_dbg = format!("{:#?}", r);
                let diffs = field_diff(&o_dbg, &r_dbg);
                if diffs.len() <= 5 {
                    for d in &diffs {
                        report
                            .entity_diffs
                            .push(format!("{}[{}] field diff:\n{}", type_name, i, d));
                    }
                } else {
                    report.entity_diffs.push(format!(
                        "{}[{}]: {} field differences (showing first 5):\n{}",
                        type_name,
                        i,
                        diffs.len(),
                        diffs[..5].join("\n")
                    ));
                }
            }
        }
    }

    for type_name in rt_by_type.keys() {
        if !orig_by_type.contains_key(type_name) {
            report.entity_diffs.push(format!(
                "{}: {} entities APPEARED (not in original)",
                type_name,
                rt_by_type[type_name].len()
            ));
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  MAIN
// ═══════════════════════════════════════════════════════════════════════════

fn main() {
    let input_str = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "tests/roundtrip/samplekitchen.dwg".to_string());
    let source_path = Path::new(&input_str).to_path_buf();

    println!("=================================================================");
    println!("  Roundtrip Analysis: {}", source_path.display());
    println!("=================================================================\n");

    // ── Step 1: Read original DWG ─────────────────────────────────
    println!("Reading original DWG...");
    let mut reader = match DwgReader::from_file(&source_path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("ERROR: Could not open file: {}", e);
            std::process::exit(1);
        }
    };
    let original = match reader.read() {
        Ok(doc) => doc,
        Err(e) => {
            eprintln!("ERROR: Could not read DWG: {}", e);
            std::process::exit(1);
        }
    };

    print_document_summary("ORIGINAL (from DWG file)", &original);

    // ── Step 2: DWG roundtrip ─────────────────────────────────────
    println!("Performing DWG roundtrip (write → read → disk)...");
    let dwg_out_path = sibling_path(&source_path, "_rt", "dwg");
    let dwg_rt = match DwgWriter::write_to_vec(&original) {
        Ok(bytes) => {
            save_bytes(&dwg_out_path, &bytes);
            let mut r = DwgReader::from_stream(Cursor::new(bytes));
            match r.read() {
                Ok(doc) => Some(doc),
                Err(e) => {
                    eprintln!("  WARNING: DWG re-read failed: {}", e);
                    None
                }
            }
        }
        Err(e) => {
            eprintln!("  WARNING: DWG write failed: {}", e);
            None
        }
    };

    if let Some(ref rt) = dwg_rt {
        print_document_summary("DWG ROUNDTRIPPED", rt);
    }

    // ── Step 3: DXF roundtrip ─────────────────────────────────────
    println!("Performing DXF roundtrip (write → read → disk)...");
    let dxf_out_path = sibling_path(&source_path, "_rt", "dxf");
    let dxf_rt = {
        let writer = DxfWriter::new(&original);
        match writer.write_to_vec() {
            Ok(bytes) => {
                save_bytes(&dxf_out_path, &bytes);
                match DxfReader::from_reader(Cursor::new(bytes)) {
                    Ok(r) => match r.read() {
                        Ok(doc) => Some(doc),
                        Err(e) => {
                            eprintln!("  WARNING: DXF re-read failed: {}", e);
                            None
                        }
                    },
                    Err(e) => {
                        eprintln!("  WARNING: DXF reader init failed: {}", e);
                        None
                    }
                }
            }
            Err(e) => {
                eprintln!("  WARNING: DXF write failed: {}", e);
                None
            }
        }
    };

    if let Some(ref rt) = dxf_rt {
        print_document_summary("DXF ROUNDTRIPPED", rt);
    }

    // ── Step 4: Data loss analysis ────────────────────────────────
    println!("\n=================================================================");
    println!("  DATA LOSS ANALYSIS");
    println!("=================================================================\n");

    if let Some(ref rt) = dwg_rt {
        let report = compare_documents(&original, rt, "DWG");
        report.print();
        println!();
        deep_analysis(&original, rt, "DWG");
    } else {
        println!("DWG roundtrip: FAILED (could not complete write→read cycle)");
    }

    if let Some(ref rt) = dxf_rt {
        let report = compare_documents(&original, rt, "DXF");
        report.print();
        println!();
        deep_analysis(&original, rt, "DXF");
    } else {
        println!("DXF roundtrip: FAILED (could not complete write→read cycle)");
    }

    // ── Step 5: DWG double roundtrip stability ────────────────────
    println!("=================================================================");
    println!("  DWG DOUBLE-ROUNDTRIP STABILITY");
    println!("=================================================================\n");

    if let Some(ref rt1) = dwg_rt {
        println!("Performing DWG double roundtrip...");
        let dwg2_out_path = sibling_path(&source_path, "_rt2", "dwg");
        match DwgWriter::write_to_vec(rt1) {
            Ok(bytes2) => {
                save_bytes(&dwg2_out_path, &bytes2);
                let mut r2 = DwgReader::from_stream(Cursor::new(bytes2));
                match r2.read() {
                    Ok(rt2) => {
                        let report = compare_documents(rt1, &rt2, "DWG double-roundtrip (RT1→RT2)");
                        report.print();
                        if !report.entity_diffs.is_empty() {
                            deep_analysis(rt1, &rt2, "DWG double-roundtrip");
                        }
                    }
                    Err(e) => eprintln!("DWG double roundtrip re-read failed: {}", e),
                }
            }
            Err(e) => eprintln!("DWG double roundtrip write failed: {}", e),
        }
    }

    println!("=================================================================");
    println!("  Analysis complete.");
    println!("  Output files:");
    println!("    {}", dwg_out_path.display());
    println!("    {}", dxf_out_path.display());
    println!(
        "    {}",
        sibling_path(&source_path, "_rt2", "dwg").display()
    );
    println!("=================================================================");
}

// ═══════════════════════════════════════════════════════════════════════════
//  DEEP PER-TYPE ANALYSIS
// ═══════════════════════════════════════════════════════════════════════════

fn deep_analysis(orig: &CadDocument, rt: &CadDocument, format: &str) {
    println!(
        "─── Deep Analysis: {} ───────────────────────────────────────",
        format
    );

    deep_polyface_mesh(orig, rt);
    deep_solid3d(orig, rt);
    deep_dimension(orig, rt);
    deep_insert(orig, rt);
    deep_notifications(rt, format);

    println!();
}

// ── PolyfaceMesh ──────────────────────────────────────────────────────────

fn deep_polyface_mesh(orig: &CadDocument, rt: &CadDocument) {
    let orig_meshes: Vec<_> = orig
        .entities()
        .filter_map(|e| {
            if let EntityType::PolyfaceMesh(m) = e {
                Some(m)
            } else {
                None
            }
        })
        .collect();
    let rt_meshes: Vec<_> = rt
        .entities()
        .filter_map(|e| {
            if let EntityType::PolyfaceMesh(m) = e {
                Some(m)
            } else {
                None
            }
        })
        .collect();

    if orig_meshes.is_empty() {
        return;
    }

    println!(
        "\n  PolyfaceMesh ({} in original, {} after roundtrip):",
        orig_meshes.len(),
        rt_meshes.len()
    );

    if rt_meshes.is_empty() {
        println!(
            "    ALL PolyfaceMesh entities LOST — check if they were re-read as generic Polyline."
        );
        return;
    }

    // Build a handle→mesh map for rt so we can match by handle when possible.
    let rt_by_handle: std::collections::HashMap<
        u64,
        &acadrust::entities::polyface_mesh::PolyfaceMesh,
    > = rt_meshes
        .iter()
        .map(|m| (m.common.handle.value(), *m))
        .collect();

    let mut lost_vertices = 0usize;
    let mut lost_faces = 0usize;
    let count = orig_meshes.len().min(rt_meshes.len());
    let mut unmatched = 0usize;

    for (i, o) in orig_meshes.iter().enumerate() {
        // Try to match by handle first; fall back to positional match.
        let r = rt_by_handle
            .get(&o.common.handle.value())
            .copied()
            .or_else(|| rt_meshes.get(i).copied());

        let r = match r {
            Some(r) => r,
            None => {
                unmatched += 1;
                continue;
            }
        };

        let ov = o.vertices.len();
        let rv = r.vertices.len();
        let of_ = o.faces.len();
        let rf = r.faces.len();

        if ov != rv || of_ != rf {
            println!(
                "    [{}] v:{}→{}  f:{}→{}  layer={:?}  handle={:#X}→{:#X}  owner={:#X}→{:#X}",
                i,
                ov,
                rv,
                of_,
                rf,
                o.common.layer,
                o.common.handle.value(),
                r.common.handle.value(),
                o.common.owner_handle.value(),
                r.common.owner_handle.value()
            );
            lost_vertices += ov.saturating_sub(rv);
            lost_faces += of_.saturating_sub(rf);
        }
    }

    if unmatched > 0 {
        println!(
            "    {} original mesh(es) had no match in roundtrip output.",
            unmatched
        );
    }

    if lost_vertices == 0 && lost_faces == 0 && unmatched == 0 {
        println!("    All PolyfaceMesh vertex/face counts preserved.");
    } else {
        println!(
            "    Summary: {} vertices lost, {} faces lost across {} affected meshes",
            lost_vertices, lost_faces, count
        );
    }
}

// ── Solid3D ───────────────────────────────────────────────────────────────

fn deep_solid3d(orig: &CadDocument, rt: &CadDocument) {
    let orig_solids: Vec<_> = orig
        .entities()
        .filter_map(|e| {
            if let EntityType::Solid3D(s) = e {
                Some(s)
            } else {
                None
            }
        })
        .collect();
    let rt_solids: Vec<_> = rt
        .entities()
        .filter_map(|e| {
            if let EntityType::Solid3D(s) = e {
                Some(s)
            } else {
                None
            }
        })
        .collect();

    if orig_solids.is_empty() {
        return;
    }

    println!("\n  Solid3D ({} entities):", orig_solids.len());

    let mut loss_categories: BTreeMap<String, usize> = BTreeMap::new();

    let count = orig_solids.len().min(rt_solids.len());
    for i in 0..count {
        let o = &orig_solids[i];
        let r = &rt_solids[i];

        let o_acis = &o.acis_data;
        let r_acis = &r.acis_data;

        let mut issues = Vec::new();

        if o_acis.version != r_acis.version {
            issues.push(format!("version {:?}→{:?}", o_acis.version, r_acis.version));
        }
        if o_acis.is_binary != r_acis.is_binary {
            issues.push(format!(
                "is_binary {}→{}",
                o_acis.is_binary, r_acis.is_binary
            ));
        }
        if !o_acis.sab_data.is_empty() && r_acis.sab_data.is_empty() {
            issues.push(format!(
                "sab_data {} bytes → 0 bytes",
                o_acis.sab_data.len()
            ));
        }
        if o_acis.sab_data.len() != r_acis.sab_data.len()
            && !o_acis.sab_data.is_empty()
            && !r_acis.sab_data.is_empty()
        {
            issues.push(format!(
                "sab_data {} → {} bytes",
                o_acis.sab_data.len(),
                r_acis.sab_data.len()
            ));
        }
        if o_acis.sat_data != r_acis.sat_data {
            let o_lines = o_acis.sat_data.lines().count();
            let r_lines = r_acis.sat_data.lines().count();
            issues.push(format!("sat_data {}-line → {}-line", o_lines, r_lines));
        }
        if o.uid != r.uid {
            issues.push(format!("uid {:?}→{:?}", o.uid, r.uid));
        }
        if o.point_of_reference != r.point_of_reference {
            issues.push(format!(
                "point_of_reference {:?}→{:?}",
                o.point_of_reference, r.point_of_reference
            ));
        }
        if o.wires.len() != r.wires.len() {
            issues.push(format!("wires {} → {}", o.wires.len(), r.wires.len()));
        }
        if o.silhouettes.len() != r.silhouettes.len() {
            issues.push(format!(
                "silhouettes {} → {}",
                o.silhouettes.len(),
                r.silhouettes.len()
            ));
        }

        if !issues.is_empty() {
            let key = issues
                .iter()
                .map(|s| {
                    // strip specific values to get a category key
                    if s.starts_with("sab_data") {
                        "sab_data_lost".to_string()
                    } else if s.starts_with("sat_data") {
                        "sat_data_changed".to_string()
                    } else if s.starts_with("version") {
                        "version_downgrade".to_string()
                    } else {
                        s.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join("+");
            *loss_categories.entry(key).or_insert(0) += 1;

            if i < 3 {
                println!(
                    "    [{}] layer={:?}: {}",
                    i,
                    o.common.layer,
                    issues.join(", ")
                );
            }
        }
    }

    if orig_solids.len() > rt_solids.len() {
        println!(
            "    MISSING: {} Solid3D entities not present after roundtrip",
            orig_solids.len() - rt_solids.len()
        );
    }

    if !loss_categories.is_empty() {
        println!("    Loss categories across all {} solids:", count);
        for (cat, n) in &loss_categories {
            println!("      [{} × {}]", n, cat);
        }
    } else if count == orig_solids.len() {
        println!("    All Solid3D ACIS data preserved.");
    }
}

// ── Dimension ─────────────────────────────────────────────────────────────

fn deep_dimension(orig: &CadDocument, rt: &CadDocument) {
    let orig_dims: Vec<_> = orig
        .entities()
        .filter_map(|e| {
            if let EntityType::Dimension(d) = e {
                Some(d)
            } else {
                None
            }
        })
        .collect();
    let rt_dims: Vec<_> = rt
        .entities()
        .filter_map(|e| {
            if let EntityType::Dimension(d) = e {
                Some(d)
            } else {
                None
            }
        })
        .collect();

    if orig_dims.is_empty() {
        return;
    }

    println!(
        "\n  Dimension ({} in original, {} after roundtrip):",
        orig_dims.len(),
        rt_dims.len()
    );

    let count = orig_dims.len().min(rt_dims.len());
    let mut field_loss: BTreeMap<String, usize> = BTreeMap::new();

    for i in 0..count {
        let ob = orig_dims[i].base();
        let rb = rt_dims[i].base();

        macro_rules! track {
            ($field:ident) => {
                if ob.$field != rb.$field {
                    *field_loss
                        .entry(stringify!($field).to_string())
                        .or_insert(0) += 1;
                    if i < 2 {
                        println!(
                            "    [{}] {}: {:?} → {:?}",
                            i,
                            stringify!($field),
                            ob.$field,
                            rb.$field
                        );
                    }
                }
            };
        }
        track!(definition_point);
        track!(text_middle_point);
        track!(insertion_point);
        track!(text);
        track!(normal);
        track!(text_rotation);
        track!(horizontal_direction);
        track!(style_name);
        track!(line_spacing_factor);
        track!(attachment_point);
    }

    if field_loss.is_empty() {
        println!("    All Dimension base fields preserved.");
    } else {
        println!("    Fields lost across {} dimensions:", count);
        for (field, n) in &field_loss {
            println!("      {} lost in {}/{} dimensions", field, n, count);
        }
    }
}

// ── Insert (attribute data) ───────────────────────────────────────────────

fn deep_insert(orig: &CadDocument, rt: &CadDocument) {
    let orig_inserts: Vec<_> = orig
        .entities()
        .filter_map(|e| {
            if let EntityType::Insert(i) = e {
                Some(i)
            } else {
                None
            }
        })
        .collect();
    let rt_inserts: Vec<_> = rt
        .entities()
        .filter_map(|e| {
            if let EntityType::Insert(i) = e {
                Some(i)
            } else {
                None
            }
        })
        .collect();

    if orig_inserts.is_empty() {
        return;
    }

    let count = orig_inserts.len().min(rt_inserts.len());
    let mut attr_losses = 0usize;
    let mut name_losses = 0usize;
    let mut point_losses = 0usize;

    let mut changed_names: Vec<(String, String)> = Vec::new();
    for i in 0..count {
        let o = &orig_inserts[i];
        let r = &rt_inserts[i];

        if o.block_name != r.block_name {
            name_losses += 1;
            changed_names.push((o.block_name.clone(), r.block_name.clone()));
        }
        if o.insert_point != r.insert_point {
            point_losses += 1;
        }
        if o.attributes.len() != r.attributes.len() {
            attr_losses += 1;
        }
    }

    println!("\n  Insert ({} entities):", orig_inserts.len());
    if name_losses == 0 && point_losses == 0 && attr_losses == 0 {
        println!("    All Insert block_name, insert_point, and attribute counts preserved.");
    } else {
        if name_losses > 0 {
            println!("    block_name changed: {} inserts", name_losses);
            for (orig, rt) in changed_names.iter().take(10) {
                println!("      {:?} -> {:?}", orig, rt);
            }
        }
        if point_losses > 0 {
            println!("    insert_point changed: {} inserts", point_losses);
        }
        if attr_losses > 0 {
            println!("    attribute count changed: {} inserts", attr_losses);
        }
    }
}

// ── Notifications from re-read document ──────────────────────────────────

fn deep_notifications(rt: &CadDocument, format: &str) {
    let notifs: Vec<_> = rt.notifications.iter().collect();
    let errors: Vec<_> = notifs
        .iter()
        .filter(|n| {
            matches!(
                n.notification_type,
                acadrust::notification::NotificationType::Error
                    | acadrust::notification::NotificationType::Warning
            ) && !n.message.starts_with("Reading DWG file")
                && !n.message.starts_with("AC1021")
                && !n.message.starts_with("AC18")
                && !n.message.starts_with("  Section")
        })
        .collect();

    if errors.is_empty() {
        println!(
            "\n  {} re-read notifications: none (clean re-read).",
            format
        );
        return;
    }

    // Deduplicate by message prefix (first 60 chars)
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for n in &errors {
        let key = n.message.chars().take(80).collect::<String>();
        *seen.entry(key).or_insert(0) += 1;
    }

    println!(
        "\n  {} re-read warnings/errors ({} unique types):",
        format,
        seen.len()
    );
    for (msg, count) in seen.iter().take(20) {
        if *count > 1 {
            println!("    [{} ×] {}", count, msg);
        } else {
            println!("    {}", msg);
        }
    }
    if seen.len() > 20 {
        println!("    ... and {} more notification types", seen.len() - 20);
    }
}
