use std::io::Cursor;

use acadrust::objects::{
    AssocAction, AssocEvalValue, AssocEvalVariant, AssocValueParam, AssocValueParamVariable,
    AssocVariable, AssociativeData, AssociativeObject, ObjectType,
};
use acadrust::notification::NotificationType;
use acadrust::types::Handle;
use acadrust::{CadDocument, DxfReader, DxfWriter};

fn handle_value(handle: u64) -> AssocEvalVariant {
    AssocEvalVariant {
        code: 330,
        value: AssocEvalValue::Handle(Handle::new(handle)),
    }
}

fn next_parameter() -> AssocValueParam {
    AssocValueParam {
        class_version: 1,
        name: "next".to_string(),
        variables: vec![AssocValueParamVariable {
            value: handle_value(0xEE),
            handle: Handle::new(0xFF),
        }],
        controlled_object_dependency: Handle::new(0xAB),
        ..Default::default()
    }
}

fn value_parameter(variables: Vec<AssocValueParamVariable>) -> AssocValueParam {
    AssocValueParam {
        class_version: 1,
        name: "value".to_string(),
        variables,
        controlled_object_dependency: Handle::new(0xCC),
        ..Default::default()
    }
}

fn round_trip(document: &CadDocument) -> CadDocument {
    let bytes = DxfWriter::new(document).write_to_vec().unwrap();
    DxfReader::from_reader(Cursor::new(bytes))
        .unwrap()
        .read()
        .unwrap()
}

/// Writes an action holding `values` and returns what reading it back gives.
fn round_trip_action_values(values: Vec<AssocValueParam>) -> (Vec<AssocValueParam>, CadDocument) {
    let mut document = CadDocument::new();
    let handle = document.allocate_handle();
    document.objects.insert(
        handle,
        ObjectType::Associative(AssociativeObject {
            handle,
            owner: document.header.named_objects_dict_handle,
            dxf_name: "ASSOCACTION".to_string(),
            cpp_class_name: "AcDbAssocAction".to_string(),
            data: AssociativeData::Action(AssocAction {
                class_version: 2,
                values,
                ..Default::default()
            }),
            ..Default::default()
        }),
    );
    let decoded = round_trip(&document);
    let values = decoded
        .objects
        .values()
        .find_map(|object| match object {
            ObjectType::Associative(AssociativeObject {
                data: AssociativeData::Action(action),
                ..
            }) => Some(action.values.clone()),
            _ => None,
        })
        .unwrap();
    (values, decoded)
}

#[test]
fn empty_action_value_preserves_following_values_and_handles() {
    for variables in [
        vec![AssocValueParamVariable {
            value: AssocEvalVariant::default(),
            handle: Handle::new(0xAA),
        }],
        vec![
            AssocValueParamVariable {
                value: AssocEvalVariant::default(),
                handle: Handle::new(0xAA),
            },
            AssocValueParamVariable {
                value: AssocEvalVariant {
                    code: 40,
                    value: AssocEvalValue::Real(7.0),
                },
                handle: Handle::new(0xBB),
            },
        ],
        vec![
            AssocValueParamVariable {
                value: AssocEvalVariant::default(),
                handle: Handle::new(0xAA),
            },
            AssocValueParamVariable {
                value: AssocEvalVariant::default(),
                handle: Handle::new(0xBB),
            },
        ],
        vec![AssocValueParamVariable {
            value: AssocEvalVariant {
                code: 330,
                value: AssocEvalValue::Handle(Handle::new(0xDD)),
            },
            handle: Handle::new(0xAA),
        }],
        vec![
            AssocValueParamVariable {
                value: AssocEvalVariant {
                    code: 330,
                    value: AssocEvalValue::Handle(Handle::new(0xDD)),
                },
                handle: Handle::new(0xAA),
            },
            AssocValueParamVariable {
                value: AssocEvalVariant {
                    code: 40,
                    value: AssocEvalValue::Real(7.0),
                },
                handle: Handle::new(0xBB),
            },
        ],
    ] {
        let values = vec![value_parameter(variables), next_parameter()];
        let (actual, decoded) = round_trip_action_values(values.clone());
        assert_eq!(actual, values);
        assert!(!decoded.notifications.has_type(NotificationType::Warning));
    }
}

/// Issue 363: native DXF writes nothing for an empty value, so `[empty,
/// handle]` and `[handle, empty]` write the same groups. The reader keeps
/// the `[handle, empty]` reading and warns that it may be wrong.
#[test]
fn empty_value_before_handle_value_is_read_deterministically_with_a_warning() {
    let values = vec![
        value_parameter(vec![
            AssocValueParamVariable {
                value: AssocEvalVariant::default(),
                handle: Handle::new(0xAA),
            },
            AssocValueParamVariable {
                value: handle_value(0xDD),
                handle: Handle::new(0xBB),
            },
        ]),
        next_parameter(),
    ];
    let (actual, decoded) = round_trip_action_values(values);

    assert_eq!(
        actual,
        vec![
            value_parameter(vec![
                AssocValueParamVariable {
                    value: handle_value(0xAA),
                    handle: Handle::new(0xDD),
                },
                AssocValueParamVariable {
                    value: AssocEvalVariant::default(),
                    handle: Handle::new(0xBB),
                },
            ]),
            next_parameter(),
        ]
    );
    let warnings = decoded.notifications.of_type(NotificationType::Warning);
    assert!(
        warnings
            .iter()
            .any(|warning| warning.message.contains("value parameter")),
        "{warnings:?}"
    );
}

#[test]
fn standalone_variable_empty_value_keeps_code_zero() {
    let mut document = CadDocument::new();
    let handle = document.allocate_handle();
    let variable = AssocVariable {
        name: "empty".to_string(),
        value: AssocEvalVariant::default(),
        has_cached_value: true,
        cached_value: "cached".to_string(),
        flag: true,
        ..Default::default()
    };
    document.objects.insert(
        handle,
        ObjectType::Associative(AssociativeObject {
            handle,
            owner: document.header.named_objects_dict_handle,
            dxf_name: "ASSOCVARIABLE".to_string(),
            cpp_class_name: "AcDbAssocVariable".to_string(),
            data: AssociativeData::Variable(variable.clone()),
            ..Default::default()
        }),
    );

    let decoded = round_trip(&document);
    let actual = decoded
        .objects
        .values()
        .find_map(|object| match object {
            ObjectType::Associative(AssociativeObject {
                data: AssociativeData::Variable(variable),
                ..
            }) => Some(variable),
            _ => None,
        })
        .unwrap();

    assert_eq!(actual, &variable);
}

#[test]
fn standalone_variable_handle_value_does_not_require_parameter_handles() {
    let mut document = CadDocument::new();
    let handle = document.allocate_handle();
    let variable = AssocVariable {
        name: "handle".to_string(),
        value: AssocEvalVariant {
            code: 330,
            value: AssocEvalValue::Handle(Handle::new(0xAA)),
        },
        has_cached_value: true,
        cached_value: "cached".to_string(),
        flag: true,
        ..Default::default()
    };
    document.objects.insert(
        handle,
        ObjectType::Associative(AssociativeObject {
            handle,
            owner: document.header.named_objects_dict_handle,
            dxf_name: "ASSOCVARIABLE".to_string(),
            cpp_class_name: "AcDbAssocVariable".to_string(),
            data: AssociativeData::Variable(variable.clone()),
            ..Default::default()
        }),
    );

    let bytes = DxfWriter::new(&document).write_to_vec().unwrap();
    let decoded = DxfReader::from_reader(Cursor::new(bytes))
        .unwrap()
        .read()
        .unwrap();
    let actual = decoded
        .objects
        .values()
        .find_map(|object| match object {
            ObjectType::Associative(AssociativeObject {
                data: AssociativeData::Variable(variable),
                ..
            }) => Some(variable),
            _ => None,
        })
        .unwrap();

    assert_eq!(actual, &variable);
}
