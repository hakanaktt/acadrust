//! Integration tests for DXF reading

use acadrust::DxfReader;
use std::fs;
use std::io::Write;

/// Test that DxfReader can be created from a non-existent file (should error)
#[test]
fn test_dxf_reader_from_nonexistent_file() {
    let result = DxfReader::from_file("nonexistent.dxf");
    assert!(result.is_err(), "Should fail to open non-existent file");
}

/// Test reading a minimal DXF file with basic entities
#[test]
#[ignore] // Ignore for now - needs proper DXF format
fn test_read_minimal_dxf() {
    let dxf_content = r#"  0
SECTION
  2
HEADER
  9
$ACADVER
  1
AC1032
  0
ENDSEC
  0
SECTION
  2
TABLES
  0
TABLE
  2
LAYER
  5
2
330
0
100
AcDbSymbolTable
 70
1
  0
LAYER
  5
10
330
2
100
AcDbSymbolTableRecord
100
AcDbLayerTableRecord
  2
0
 70
0
 62
7
  6
Continuous
  0
ENDTAB
  0
ENDSEC
  0
SECTION
  2
BLOCKS
  0
ENDSEC
  0
SECTION
  2
ENTITIES
  0
POINT
  5
100
330
1F
100
AcDbEntity
  8
0
100
AcDbPoint
 10
1.0
 20
2.0
 30
3.0
  0
LINE
  5
101
330
1F
100
AcDbEntity
  8
0
100
AcDbLine
 10
0.0
 20
0.0
 30
0.0
 11
10.0
 21
10.0
 31
0.0
  0
CIRCLE
  5
102
330
1F
100
AcDbEntity
  8
0
100
AcDbCircle
 10
5.0
 20
5.0
 30
0.0
 40
2.5
  0
ENDSEC
  0
SECTION
  2
OBJECTS
  0
ENDSEC
  0
EOF
"#;

    // Write to temporary file
    let temp_path = "test_minimal.dxf";
    let mut file = fs::File::create(temp_path).unwrap();
    file.write_all(dxf_content.as_bytes()).unwrap();
    drop(file);

    let result = DxfReader::from_file(temp_path).and_then(|r| r.read());

    // Clean up
    let _ = fs::remove_file(temp_path);

    assert!(result.is_ok(), "Failed to read DXF: {:?}", result.err());

    let doc = result.unwrap();
    
    // Check version
    assert_eq!(doc.version.to_string(), "AC1032");
    
    // Check entities
    let entities: Vec<_> = doc.entities().collect();
    assert_eq!(entities.len(), 3, "Expected 3 entities, got {}", entities.len());
}

/// Test reading DXF with extended data
#[test]
#[ignore] // Ignore for now - needs proper DXF format
fn test_read_dxf_with_tables() {
    let dxf_content = r#"  0
SECTION
  2
HEADER
  9
$ACADVER
  1
AC1032
  0
ENDSEC
  0
SECTION
  2
TABLES
  0
TABLE
  2
LAYER
  5
2
330
0
100
AcDbSymbolTable
 70
2
  0
LAYER
  5
10
330
2
100
AcDbSymbolTableRecord
100
AcDbLayerTableRecord
  2
0
 70
0
 62
7
  6
Continuous
  0
LAYER
  5
11
330
2
100
AcDbSymbolTableRecord
100
AcDbLayerTableRecord
  2
MyLayer
 70
0
 62
1
  6
Continuous
  0
ENDTAB
  0
ENDSEC
  0
EOF
"#;

    // Write to temporary file
    let temp_path = "test_tables.dxf";
    let mut file = fs::File::create(temp_path).unwrap();
    file.write_all(dxf_content.as_bytes()).unwrap();
    drop(file);

    let result = DxfReader::from_file(temp_path).and_then(|r| r.read());

    // Clean up
    let _ = fs::remove_file(temp_path);

    assert!(result.is_ok());
    let doc = result.unwrap();

    // Check layers
    assert!(doc.layers.get("0").is_some());
    assert!(doc.layers.get("MyLayer").is_some());
}


