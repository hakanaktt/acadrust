use crate::objects::{Dictionary, DictionaryVariable, ObjectType};
use crate::types::Handle;
use crate::CadDocument;

pub(crate) const VARIABLES: &str = "AcDbVariableDictionary";

impl CadDocument {
    /// Read a serialized drawing variable without modifying its dictionary.
    pub fn drawing_variable(&self, name: &str) -> Option<&str> {
        let ObjectType::Dictionary(root) =
            self.objects.get(&self.header.named_objects_dict_handle)?
        else {
            return None;
        };
        let ObjectType::Dictionary(variables) = self.objects.get(&root.get(VARIABLES)?)? else {
            return None;
        };
        let ObjectType::DictionaryVariable(value) = self.objects.get(&variables.get(name)?)? else {
            return None;
        };
        Some(&value.value)
    }

    /// Update or create a drawing variable, retaining existing object handles.
    /// Conflicting dictionary/object types are left intact.
    pub fn set_drawing_variable(&mut self, name: &str, value: &str) -> bool {
        if name.is_empty() {
            return false;
        }
        let mut root_handle = self.header.named_objects_dict_handle;
        if root_handle == Handle::NULL {
            root_handle = self.allocate_handle();
            let mut root = Dictionary::new();
            root.handle = root_handle;
            self.objects
                .insert(root_handle, ObjectType::Dictionary(root));
            self.header.named_objects_dict_handle = root_handle;
        }
        let Some(ObjectType::Dictionary(root)) = self.objects.get(&root_handle) else {
            return false;
        };
        let variables_handle = root.get(VARIABLES);
        let variables_handle = if let Some(handle) = variables_handle {
            if !matches!(self.objects.get(&handle), Some(ObjectType::Dictionary(_))) {
                return false;
            }
            handle
        } else {
            let handle = self.allocate_handle();
            let mut variables = Dictionary::new();
            variables.handle = handle;
            variables.owner = root_handle;
            self.objects
                .insert(handle, ObjectType::Dictionary(variables));
            if let Some(ObjectType::Dictionary(root)) = self.objects.get_mut(&root_handle) {
                root.add_entry(VARIABLES, handle);
            }
            handle
        };
        let Some(ObjectType::Dictionary(variables)) = self.objects.get(&variables_handle) else {
            return false;
        };
        let packed = value.to_owned();
        if let Some(handle) = variables.get(name) {
            let Some(ObjectType::DictionaryVariable(variable)) = self.objects.get_mut(&handle)
            else {
                return false;
            };
            variable.value = packed;
        } else {
            let handle = self.allocate_handle();
            let mut variable = DictionaryVariable::new(name, &packed);
            variable.handle = handle;
            variable.owner_handle = variables_handle;
            self.objects
                .insert(handle, ObjectType::DictionaryVariable(variable));
            if let Some(ObjectType::Dictionary(variables)) = self.objects.get_mut(&variables_handle)
            {
                variables.add_entry(name, handle);
            }
        }
        true
    }

    /// Default hatch origin in drawing coordinates. Missing/malformed entries use zero.
    pub fn hatch_origin(&self) -> [f64; 2] {
        let read = || {
            let mut parts = self.drawing_variable("HPORIGIN")?.split(';');
            let x = parts.next()?.trim().parse::<f64>().ok()?;
            let y = parts.next()?.trim().parse::<f64>().ok()?;
            (parts.next().is_none() && x.is_finite() && y.is_finite()).then_some([x, y])
        };
        read().unwrap_or([0.0, 0.0])
    }

    /// Store a finite hatch origin using the standard semicolon-separated point value.
    pub fn set_hatch_origin(&mut self, origin: [f64; 2]) -> bool {
        if !origin.iter().all(|value| value.is_finite()) {
            return false;
        }
        self.set_drawing_variable("HPORIGIN", &format!("{};{}", origin[0], origin[1]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DwgReader, DwgWriter, DxfReader, DxfWriter};
    use std::io::Cursor;

    #[test]
    fn new_document_has_variable_dictionary_after_dwg_round_trip() {
        let document = CadDocument::new();
        let bytes = DwgWriter::write_to_vec(&document).unwrap();
        let read = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
        let ObjectType::Dictionary(root) =
            read.objects.get(&read.header.named_objects_dict_handle).unwrap()
        else {
            panic!("named objects root is not a dictionary")
        };
        let handle = root.get(VARIABLES).expect("AcDbVariableDictionary missing");
        assert!(matches!(read.objects.get(&handle), Some(ObjectType::Dictionary(_))));
    }

    #[test]
    fn drawing_variables_create_update_and_preserve_object_handles() {
        let mut document = CadDocument::new();
        assert!(document.set_drawing_variable("TEST", "first"));
        let root_handle = document.header.named_objects_dict_handle;
        let ObjectType::Dictionary(root) = document.objects.get(&root_handle).unwrap() else {
            panic!("named objects root is not a dictionary")
        };
        let variables_handle = root.get(VARIABLES).unwrap();
        let ObjectType::Dictionary(variables) = document.objects.get(&variables_handle).unwrap()
        else {
            panic!("drawing variables object is not a dictionary")
        };
        let value_handle = variables.get("TEST").unwrap();

        assert!(document.set_drawing_variable("test", "second"));
        assert_eq!(document.drawing_variable("TEST"), Some("second"));
        let ObjectType::Dictionary(variables) = document.objects.get(&variables_handle).unwrap()
        else {
            unreachable!()
        };
        assert_eq!(variables.get("TEST"), Some(value_handle));
    }

    #[test]
    fn conflicting_types_and_malformed_hatch_origins_are_left_alone() {
        let mut document = CadDocument::new();
        let wrong_handle = document.allocate_handle();
        let mut wrong = DictionaryVariable::new("wrong", "unchanged");
        wrong.handle = wrong_handle;
        document
            .objects
            .insert(wrong_handle, ObjectType::DictionaryVariable(wrong));
        let root_handle = document.header.named_objects_dict_handle;
        let ObjectType::Dictionary(root) = document.objects.get_mut(&root_handle).unwrap() else {
            unreachable!()
        };
        root.insert(VARIABLES, wrong_handle);
        let object_count = document.objects.len();
        assert!(!document.set_drawing_variable("TEST", "value"));
        assert_eq!(document.objects.len(), object_count);

        let mut malformed = CadDocument::new();
        assert!(malformed.set_drawing_variable("HPORIGIN", "1;not-a-number"));
        assert_eq!(malformed.hatch_origin(), [0.0, 0.0]);
        assert!(!malformed.set_hatch_origin([f64::NAN, 2.0]));
        assert_eq!(
            malformed.drawing_variable("HPORIGIN"),
            Some("1;not-a-number")
        );
    }

    #[test]
    fn hatch_origin_round_trips_through_both_file_formats() {
        let mut document = CadDocument::new();
        assert!(document.set_hatch_origin([12.5, -8.25]));

        let dxf = DxfWriter::new(&document).write_to_vec().unwrap();
        let dxf = DxfReader::from_reader(Cursor::new(dxf))
            .unwrap()
            .read()
            .unwrap();
        assert_eq!(dxf.hatch_origin(), [12.5, -8.25]);

        let dwg = DwgWriter::write_to_vec(&document).unwrap();
        let dwg = DwgReader::from_stream(Cursor::new(dwg)).read().unwrap();
        assert_eq!(dwg.hatch_origin(), [12.5, -8.25]);
    }
}
