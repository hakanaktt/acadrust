use crate::entities::Hatch;
use crate::types::{Vector2, Vector3};
use crate::xdata::{ExtendedDataRecord, XDataValue};

fn origin_index(record: &ExtendedDataRecord) -> Option<usize> {
    let mut depth = 0usize;
    for (index, value) in record.values.iter().enumerate() {
        match value {
            XDataValue::ControlString(text) if text == "{" => depth += 1,
            XDataValue::ControlString(text) if text == "}" => depth = depth.saturating_sub(1),
            XDataValue::Point3D(_) if depth == 0 => return Some(index),
            _ => {}
        }
    }
    None
}

impl Hatch {
    /// Pattern origin recorded as the top-level ACAD 1010 point, in hatch coordinates.
    pub fn stored_pattern_origin(&self) -> Option<Vector2> {
        let record = self.common.extended_data.get_record("ACAD")?;
        let XDataValue::Point3D(point) = &record.values[origin_index(record)?] else {
            return None;
        };
        (point.x.is_finite() && point.y.is_finite()).then_some(Vector2::new(point.x, point.y))
    }

    /// Absent origin metadata denotes the coordinate origin, not a pattern line base.
    pub fn pattern_origin(&self) -> Vector2 {
        self.stored_pattern_origin()
            .unwrap_or(Vector2::new(0.0, 0.0))
    }

    /// Record an already-applied origin without changing pattern geometry.
    /// Unrelated application records and nested ACAD payloads remain intact.
    pub fn record_pattern_origin(&mut self, origin: Vector2) -> bool {
        if !origin.x.is_finite() || !origin.y.is_finite() {
            return false;
        }
        let mut record = self
            .common
            .extended_data
            .get_record("ACAD")
            .cloned()
            .unwrap_or_else(|| ExtendedDataRecord::new("ACAD"));
        let value = XDataValue::Point3D(Vector3::new(origin.x, origin.y, 0.0));
        if let Some(index) = origin_index(&record) {
            record.values[index] = value;
        } else {
            record.values.push(value);
        }
        self.common.extended_data.upsert_record(record);
        true
    }

    /// Move pattern lines by the change in origin, preserving their intrinsic offsets.
    pub fn set_pattern_origin(&mut self, origin: Vector2) -> bool {
        if !origin.x.is_finite() || !origin.y.is_finite() {
            return false;
        }
        let previous = self.pattern_origin();
        let dx = origin.x - previous.x;
        let dy = origin.y - previous.y;
        if !dx.is_finite()
            || !dy.is_finite()
            || self.pattern.lines.iter().any(|line| {
                !(line.base_point.x + dx).is_finite() || !(line.base_point.y + dy).is_finite()
            })
        {
            return false;
        }
        for line in &mut self.pattern.lines {
            line.base_point.x += dx;
            line.base_point.y += dy;
        }
        self.record_pattern_origin(origin)
    }

    /// Scale the stored pattern geometry about its recorded origin.
    pub fn scale_pattern_about_origin(&mut self, factor: f64) {
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }
        let origin = self.pattern_origin();
        for line in &mut self.pattern.lines {
            line.base_point.x = origin.x + (line.base_point.x - origin.x) * factor;
            line.base_point.y = origin.y + (line.base_point.y - origin.y) * factor;
            line.offset.x *= factor;
            line.offset.y *= factor;
            for dash in &mut line.dash_lengths {
                *dash *= factor;
            }
        }
    }

    /// Rotate the stored pattern geometry about its recorded origin.
    pub fn rotate_pattern_about_origin(&mut self, angle: f64) {
        if !angle.is_finite() {
            return;
        }
        let origin = self.pattern_origin();
        let (sin, cos) = angle.sin_cos();
        for line in &mut self.pattern.lines {
            let x = line.base_point.x - origin.x;
            let y = line.base_point.y - origin.y;
            line.base_point.x = origin.x + x * cos - y * sin;
            line.base_point.y = origin.y + x * sin + y * cos;
            let x = line.offset.x;
            let y = line.offset.y;
            line.offset.x = x * cos - y * sin;
            line.offset.y = x * sin + y * cos;
            line.angle += angle;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::hatch::{HatchPattern, HatchPatternLine};
    use crate::entities::EntityType;
    use crate::types::Transform;
    use crate::xdata::ExtendedDataRecord;
    use crate::{CadDocument, DwgReader, DwgWriter};
    use std::io::Cursor;

    fn patterned_hatch() -> Hatch {
        let mut pattern = HatchPattern::new("TEST");
        pattern.add_line(HatchPatternLine {
            angle: 0.0,
            base_point: Vector2::new(2.0, 1.0),
            offset: Vector2::new(1.0, 0.0),
            dash_lengths: vec![2.0, -1.0],
        });
        Hatch::with_pattern(pattern)
    }

    #[test]
    fn recording_an_origin_preserves_nested_and_unrelated_data() {
        let mut hatch = patterned_hatch();
        let mut acad = ExtendedDataRecord::new("ACAD");
        acad.values = vec![
            XDataValue::ControlString("{".to_string()),
            XDataValue::Point3D(Vector3::new(90.0, 91.0, 92.0)),
            XDataValue::ControlString("}".to_string()),
            XDataValue::String("keep".to_string()),
            XDataValue::Point3D(Vector3::new(1.0, 2.0, 3.0)),
        ];
        hatch.common.extended_data.add_record(acad);
        let mut other = ExtendedDataRecord::new("OTHER");
        other.values.push(XDataValue::Integer16(7));
        hatch.common.extended_data.add_record(other.clone());

        assert!(hatch.record_pattern_origin(Vector2::new(4.0, 5.0)));
        assert_eq!(hatch.pattern_origin(), Vector2::new(4.0, 5.0));
        let acad = hatch.common.extended_data.get_record("ACAD").unwrap();
        assert_eq!(
            acad.values[1],
            XDataValue::Point3D(Vector3::new(90.0, 91.0, 92.0))
        );
        assert_eq!(acad.values[3], XDataValue::String("keep".to_string()));
        assert_eq!(hatch.common.extended_data.get_record("OTHER"), Some(&other));
    }

    #[test]
    fn changing_scaling_and_rotating_origin_move_pattern_geometry_consistently() {
        let mut hatch = patterned_hatch();
        assert!(hatch.set_pattern_origin(Vector2::new(1.0, 1.0)));
        assert_eq!(hatch.pattern.lines[0].base_point, Vector2::new(3.0, 2.0));
        let before = hatch.clone();
        assert!(!hatch.set_pattern_origin(Vector2::new(f64::NAN, 0.0)));
        assert_eq!(hatch, before);

        hatch.scale_pattern_about_origin(2.0);
        assert_eq!(hatch.pattern.lines[0].base_point, Vector2::new(5.0, 3.0));
        assert_eq!(hatch.pattern.lines[0].offset, Vector2::new(2.0, 0.0));
        assert_eq!(hatch.pattern.lines[0].dash_lengths, vec![4.0, -2.0]);

        hatch.rotate_pattern_about_origin(std::f64::consts::FRAC_PI_2);
        let line = &hatch.pattern.lines[0];
        assert!((line.base_point.x + 1.0).abs() < 1e-12);
        assert!((line.base_point.y - 5.0).abs() < 1e-12);
        assert!(line.offset.x.abs() < 1e-12);
        assert!((line.offset.y - 2.0).abs() < 1e-12);
    }

    #[test]
    fn translation_and_transform_move_the_recorded_origin_with_the_hatch() {
        let mut translated = patterned_hatch();
        assert!(translated.record_pattern_origin(Vector2::new(1.0, 2.0)));
        crate::entities::translate::translate_hatch(&mut translated, Vector3::new(5.0, -1.0, 0.0));
        assert_eq!(translated.pattern_origin(), Vector2::new(6.0, 1.0));
        assert_eq!(
            translated.pattern.lines[0].base_point,
            Vector2::new(7.0, 0.0)
        );

        let mut scaled = patterned_hatch();
        assert!(scaled.record_pattern_origin(Vector2::new(1.0, 2.0)));
        crate::entities::transform::transform_hatch(&mut scaled, &Transform::from_scale(2.0));
        assert_eq!(scaled.pattern_origin(), Vector2::new(2.0, 4.0));
        assert_eq!(scaled.pattern.lines[0].base_point, Vector2::new(4.0, 2.0));
    }

    #[test]
    fn dwg_writes_the_current_origin_for_hatch_and_mpolygon() {
        let mut document = CadDocument::new();
        let app_handle = document.app_ids.get("ACAD").unwrap().handle.value();
        let stale = crate::io::dwg::eed_codec::encode_values_with_encoding(
            true,
            &[XDataValue::Point3D(Vector3::new(1.0, 2.0, 0.0))],
            encoding_rs::WINDOWS_1252,
            30,
            |_| 0,
        );
        for (is_mpolygon, origin) in [(false, [4.0, 5.0]), (true, [6.0, 7.0])] {
            let mut hatch = patterned_hatch();
            hatch.is_mpolygon = is_mpolygon;
            hatch
                .common
                .extended_data
                .raw_dwg_eed
                .push((app_handle, stale.clone()));
            assert!(hatch.record_pattern_origin(Vector2::new(origin[0], origin[1])));
            document.add_entity(EntityType::Hatch(Box::new(hatch))).unwrap();
        }

        let bytes = DwgWriter::write_to_vec(&document).unwrap();
        let roundtrip = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
        let origins = roundtrip
            .entities()
            .filter_map(|entity| match entity {
                EntityType::Hatch(hatch) => Some((hatch.is_mpolygon, hatch.pattern_origin())),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(
            origins.contains(&(false, Vector2::new(4.0, 5.0))),
            "{origins:?}"
        );
        assert!(
            origins.contains(&(true, Vector2::new(6.0, 7.0))),
            "{origins:?}"
        );
    }
}
