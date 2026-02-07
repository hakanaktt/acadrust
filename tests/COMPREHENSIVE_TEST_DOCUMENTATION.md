# Comprehensive Entity Test Suite

## Overview

This test file (`tests/all_entities_output_test.rs`) creates DXF files containing examples of all 30+ entity types supported by the acadrust library in both ASCII and Binary formats.

## Generated Files

The test suite generates two DXF files:
- **`test_output_all_entities_ascii.dxf`** - ASCII format DXF file (~10.5 KB)
- **`test_output_all_entities_binary.dxb`** - Binary format DXF file (~9.5 KB)

## Entities Included

The test creates one instance of each supported entity type:

### Basic Geometric Entities (1-8)
1. **Point** - Single point in 3D space (red)
2. **Line** - Line segment between two points (green)
3. **Circle** - Circle defined by center and radius (blue)
4. **Arc** - Circular arc with start and end angles (yellow)
5. **Ellipse** - Ellipse with major/minor axes (cyan)
6. **LwPolyline** - Lightweight 2D polyline with bulges (magenta)
7. **Polyline3D** - 3D polyline with elevation
8. **Spline** - Cubic spline curve with control points

### Text Entities (9-10)
9. **Text** - Single-line text entity
10. **MText** - Multi-line text with formatting

### Solid and Face Entities (11-12)
11. **Solid** - 2D solid-filled quad/triangle
12. **Face3D** - 3D face with four corners

### Construction Entities (13-14)
13. **Ray** - Semi-infinite line from a point
14. **XLine** - Infinite construction line

### Hatch Entity (15)
15. **Hatch** - Solid fill with rectangular boundary

### Block-Related Entities (16-18)
16. **Insert** - Block reference/instance
17. **AttributeDefinition** - Attribute template in block
18. **AttributeEntity** - Attribute instance value

### Leader Entities (19-21)
19. **Leader** - Simple leader with arrow
20. **MultiLeader** - Advanced leader with text/block
21. **MLine** - Multi-line entity with parallel lines

### Advanced 3D Entities (22-25)
22. **Mesh** - Subdivision mesh with faces
23. **Solid3D** - 3D solid with ACIS data
24. **Region** - 2D region with ACIS data
25. **Body** - 3D body with ACIS data

### Table and Advanced Entities (26-27)
26. **Table** - Table entity with rows and columns
27. **Tolerance** - Geometric tolerance/feature control frame

### Legacy and Specialized Entities (28-30)
28. **PolyfaceMesh** - Legacy polyface mesh
29. **Shape** - Shape reference entity
30. **Viewport** - Paper space viewport

## Test Functions

The test suite includes the following test functions:

### `test_write_all_entities_ascii()`
Creates an ASCII DXF file containing all entity types. Verifies:
- File is created successfully
- No errors during write operation
- Correct entity count

### `test_write_all_entities_binary()`
Creates a Binary DXF file containing all entity types. Verifies:
- File is created successfully  
- No errors during write operation
- Correct entity count

### `test_entity_count()`
Validates that the document contains exactly 30 entities.

### `test_all_entity_types_present()`
Verifies:
- All unique entity types are present
- Lists all entity type names
- Ensures at least 20 unique types exist

### `test_document_structure()`
Checks:
- Document has entities
- All entities have valid (non-null) handles
- Document structure is valid

## Running the Tests

```bash
# Run all tests
cargo test --test all_entities_output_test

# Run with output
cargo test --test all_entities_output_test -- --nocapture

# Run specific test
cargo test --test all_entities_output_test test_write_all_entities_ascii
```

## Entity Layout

Entities are positioned in a grid layout with 20-unit spacing to prevent overlap. They start at coordinates (0, 0) and progress horizontally, wrapping to a new row every few entities.

## Color Scheme

Each entity type uses a distinct color to make them easily identifiable:
- RED (1): Point, Text
- GREEN (3): Line
- BLUE (5): Circle, MText
- YELLOW (2): Arc
- CYAN (4): Ellipse, AttributeEntity
- MAGENTA (6): LwPolyline
- Custom RGB colors: Used for most other entities

## Technical Details

### DXF Version
- **Version**: AC1032 (AutoCAD 2018+)
- Supports all modern entity types and features

### Document Structure
- Standard layers (Layer "0")
- Standard line types (CONTINUOUS, ByLayer, ByBlock)
- Standard text style (STANDARD)
- Standard dimension style

### ACIS Entities
Entities that use ACIS solid modeling (Solid3D, Region, Body) contain placeholder ACIS SAT data strings. In a real application, these would contain actual solid model data.

## Viewing the Output

The generated DXF files can be opened in:
- AutoCAD (any version 2018+)
- LibreCAD
- QCAD
- DraftSight
- Any DXF-compatible CAD viewer

## Performance

Test execution time: ~60ms (compilation excluded)
- ASCII file generation: <30ms
- Binary file generation: <30ms

## File Sizes

- ASCII DXF: ~10.5 KB
- Binary DXF: ~9.5 KB (10% smaller)

Binary format is more compact due to efficient encoding of numeric values.

