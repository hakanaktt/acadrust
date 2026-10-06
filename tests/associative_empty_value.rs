use std::io::Cursor;

use acadrust::objects::{
    AssocAction, AssocEvalValue, AssocEvalVariant, AssocValueParam, AssocValueParamVariable,
    AssocVariable, AssociativeData, AssociativeObject, ObjectType,
};
use acadrust::types::Handle;
use acadrust::{CadDocument, DxfReader, DxfWriter};

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
        let mut document = CadDocument::new();
        let handle = document.allocate_handle();
        let parameter = AssocValueParam {
            class_version: 1,
            name: "value".to_string(),
            variables,
            controlled_object_dependency: Handle::new(0xCC),
            ..Default::default()
        };
        let next_parameter = AssocValueParam {
            class_version: 1,
            name: "next".to_string(),
            variables: vec![AssocValueParamVariable {
                value: AssocEvalVariant {
                    code: 330,
                    value: AssocEvalValue::Handle(Handle::new(0xEE)),
                },
                handle: Handle::new(0xFF),
            }],
            controlled_object_dependency: Handle::new(0xAB),
            ..Default::default()
        };
        document.objects.insert(
            handle,
            ObjectType::Associative(AssociativeObject {
                handle,
                owner: document.header.named_objects_dict_handle,
                dxf_name: "ASSOCACTION".to_string(),
                cpp_class_name: "AcDbAssocAction".to_string(),
                data: AssociativeData::Action(AssocAction {
                    class_version: 2,
                    values: vec![parameter.clone(), next_parameter.clone()],
                    ..Default::default()
                }),
                ..Default::default()
            }),
        );

        let bytes = DxfWriter::new(&document).write_to_vec().unwrap();
        let decoded = DxfReader::from_reader(Cursor::new(bytes))
            .unwrap()
            .read()
            .unwrap();
        let action = decoded
            .objects
            .values()
            .find_map(|object| match object {
                ObjectType::Associative(AssociativeObject {
                    data: AssociativeData::Action(action),
                    ..
                }) => Some(action),
                _ => None,
            })
            .unwrap();

        assert_eq!(action.values, vec![parameter, next_parameter]);
    }
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
        reserved: 7,
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
