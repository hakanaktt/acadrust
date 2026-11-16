//! glTF 2.0 importer.
//!
//! Supports `.gltf` (JSON + separate `.bin`) and `.glb` (binary container)
//! files. Produces one [`Mesh`] entity per mesh primitive. When a scene is
//! present, each node instance is placed with its world transform. Triangle
//! strips and fans are triangulated; other primitive modes are skipped.
//!
//! # Example
//!
//! ```rust,ignore
//! use acadrust::io::import::gltf::GltfImporter;
//!
//! let doc = GltfImporter::from_file("model.glb")?.import()?;
//! ```

pub mod json_parser;

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::document::CadDocument;
use crate::entities::EntityType;
use crate::error::{DxfError, Result};

use super::color_mapping::{color_from_unit_rgb, create_material_layer};
use super::mesh::{matrix_from_trs, mesh_from_indexed, transform_positions, IDENTITY};
use super::{percent_decode, read_path_bytes, read_reader_bytes, resolve_relative, ImportConfig};

use json_parser::JsonValue;

/// glTF file importer.
pub struct GltfImporter {
    data: Vec<u8>,
    base_dir: Option<PathBuf>,
    config: ImportConfig,
}

impl GltfImporter {
    /// Create an importer from a file path.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        Ok(Self {
            data: read_path_bytes(path)?,
            base_dir: path.parent().map(|p| p.to_path_buf()),
            config: ImportConfig::default(),
        })
    }

    /// Create an importer from an in-memory buffer.
    pub fn from_bytes(data: Vec<u8>) -> Self {
        Self {
            data,
            base_dir: None,
            config: ImportConfig::default(),
        }
    }

    /// Create an importer from a reader.
    pub fn from_reader<R: Read>(reader: R) -> Result<Self> {
        Ok(Self {
            data: read_reader_bytes(reader, "glTF data")?,
            base_dir: None,
            config: ImportConfig::default(),
        })
    }

    /// Set the import configuration.
    pub fn with_config(mut self, config: ImportConfig) -> Self {
        self.config = config;
        self
    }

    /// Perform the import and return a [`CadDocument`].
    pub fn import(&self) -> Result<CadDocument> {
        let (json, buffers) = self.parse()?;
        self.build_document(&json, &buffers)
    }

    /// Parse the glTF data into JSON and binary buffer(s).
    fn parse(&self) -> Result<(JsonValue, Vec<Vec<u8>>)> {
        if self.data.len() >= 4 && &self.data[0..4] == b"glTF" {
            self.parse_glb()
        } else {
            self.parse_gltf()
        }
    }

    /// Parse a GLB binary container.
    fn parse_glb(&self) -> Result<(JsonValue, Vec<Vec<u8>>)> {
        if self.data.len() < 12 {
            return Err(DxfError::ImportError("GLB file too short".to_string()));
        }

        let mut cursor = std::io::Cursor::new(&self.data);
        let _magic = read_glb_u32(&mut cursor)?;
        let version = read_glb_u32(&mut cursor)?;
        let _length = read_glb_u32(&mut cursor)?;

        if version != 2 {
            return Err(DxfError::ImportError(format!(
                "Unsupported GLB version: {} (only v2 supported)",
                version
            )));
        }

        let mut json_value = None;
        let mut bin_data: Vec<Vec<u8>> = Vec::new();

        while (cursor.position() as usize) + 8 <= self.data.len() {
            let chunk_length = read_glb_u32(&mut cursor)? as usize;
            let chunk_type = read_glb_u32(&mut cursor)?;
            let pos = cursor.position() as usize;

            if pos + chunk_length > self.data.len() {
                break;
            }

            match chunk_type {
                0x4E4F534A => {
                    // "JSON"
                    let json_str = std::str::from_utf8(&self.data[pos..pos + chunk_length])
                        .map_err(|_| {
                            DxfError::ImportError("Invalid UTF-8 in GLB JSON chunk".to_string())
                        })?;
                    json_value = Some(JsonValue::parse(json_str)?);
                }
                0x004E4942 => {
                    // "BIN\0"
                    bin_data.push(self.data[pos..pos + chunk_length].to_vec());
                }
                _ => {} // Unknown chunk, skip
            }

            cursor.set_position((pos + chunk_length) as u64);
        }

        let json = json_value
            .ok_or_else(|| DxfError::ImportError("GLB missing JSON chunk".to_string()))?;

        Ok((json, bin_data))
    }

    /// Parse a .gltf text file + external buffer(s).
    fn parse_gltf(&self) -> Result<(JsonValue, Vec<Vec<u8>>)> {
        let text = String::from_utf8_lossy(&self.data);
        let json = JsonValue::parse(&text)?;

        let mut buffers = Vec::new();

        if let Some(buffer_descs) = json.get("buffers").and_then(|v| v.as_array()) {
            for buf_desc in buffer_descs {
                let uri = buf_desc
                    .get("uri")
                    .and_then(|value| value.as_str())
                    .ok_or_else(|| {
                        DxfError::ImportError("glTF buffer is missing a uri".to_string())
                    })?;
                buffers.push(self.resolve_uri(uri)?);
            }
        }

        Ok((json, buffers))
    }

    /// Resolve a buffer URI — supports `data:` base64 and file paths.
    fn resolve_uri(&self, uri: &str) -> Result<Vec<u8>> {
        if let Some(data) = uri.strip_prefix("data:") {
            let encoded = data.split_once(',').map(|(_, rest)| rest).ok_or_else(|| {
                DxfError::ImportError("glTF data URI is missing a payload".to_string())
            })?;
            return base64_decode(encoded);
        }

        let base = self.base_dir.as_ref().ok_or_else(|| {
            DxfError::ImportError(format!(
                "glTF buffer '{uri}' needs a file directory to resolve"
            ))
        })?;
        let decoded = percent_decode(uri)?;
        let path = resolve_relative(base, &decoded)?;
        fs::read(&path).map_err(|error| {
            DxfError::ImportError(format!(
                "Cannot read buffer '{}': {}",
                path.display(),
                error
            ))
        })
    }

    fn build_document(&self, json: &JsonValue, buffers: &[Vec<u8>]) -> Result<CadDocument> {
        let mut doc = CadDocument::new();
        let meshes = json.get("meshes").and_then(|value| value.as_array());
        let Some(meshes) = meshes else {
            return Ok(doc);
        };

        let tables = GltfTables {
            accessors: json.get("accessors").and_then(|value| value.as_array()),
            buffer_views: json.get("bufferViews").and_then(|value| value.as_array()),
            materials: json.get("materials").and_then(|value| value.as_array()),
            buffers,
            config: &self.config,
        };

        let instances = placed_meshes(json, meshes).unwrap_or_else(|| {
            (0..meshes.len())
                .map(|index| PlacedMesh {
                    index,
                    world: IDENTITY,
                    name: mesh_name(&meshes[index]),
                })
                .collect()
        });

        for instance in instances {
            let Some(gltf_mesh) = meshes.get(instance.index) else {
                continue;
            };
            append_gltf_mesh(&mut doc, gltf_mesh, &instance, &tables)?;
        }

        Ok(doc)
    }
}

struct GltfTables<'a> {
    accessors: Option<&'a [JsonValue]>,
    buffer_views: Option<&'a [JsonValue]>,
    materials: Option<&'a [JsonValue]>,
    buffers: &'a [Vec<u8>],
    config: &'a ImportConfig,
}

struct PlacedMesh {
    index: usize,
    world: [f64; 16],
    name: String,
}

fn append_gltf_mesh(
    doc: &mut CadDocument,
    gltf_mesh: &JsonValue,
    instance: &PlacedMesh,
    tables: &GltfTables<'_>,
) -> Result<()> {
    let primitives = match gltf_mesh
        .get("primitives")
        .and_then(|value| value.as_array())
    {
        Some(primitives) => primitives,
        None => return Ok(()),
    };

    for primitive in primitives {
        let Some(attributes) = primitive.get("attributes") else {
            continue;
        };
        let Some(position_accessor) = attributes
            .get("POSITION")
            .and_then(|value| value.as_usize())
        else {
            continue;
        };
        let positions = read_accessor_vec3(
            position_accessor,
            tables.accessors,
            tables.buffer_views,
            tables.buffers,
        )?;
        if positions.is_empty() {
            continue;
        }

        let indices =
            if let Some(index) = primitive.get("indices").and_then(|value| value.as_usize()) {
                read_accessor_scalar(index, tables.accessors, tables.buffer_views, tables.buffers)?
            } else {
                (0..positions.len()).collect()
            };
        let mode = primitive
            .get("mode")
            .and_then(|value| value.as_usize())
            .unwrap_or(4);
        let triangles = triangle_indices(&indices, mode);
        if triangles.is_empty() {
            continue;
        }

        let (material_name, color) = if let Some(material_index) =
            primitive.get("material").and_then(|value| value.as_usize())
        {
            get_material_color(material_index, tables.materials)
        } else {
            (String::new(), [0.8f32, 0.8, 0.8, 1.0])
        };

        let transformed = transform_positions_f32(&positions, &instance.world);
        let mesh = mesh_from_indexed(
            &transformed,
            &triangles,
            tables.config.scale_factor,
            tables.config.merge_vertices,
            tables.config.merge_tolerance,
        );
        if mesh.faces.is_empty() {
            continue;
        }

        let layer_label = if material_name.is_empty() {
            instance.name.clone()
        } else {
            material_name.clone()
        };
        let color = if material_name.is_empty() {
            tables.config.default_color
        } else {
            color_from_unit_rgb(color[0], color[1], color[2])
        };
        let layer = create_material_layer(doc, &tables.config.layer_prefix, &layer_label, color);
        let mut mesh_entity = mesh;
        mesh_entity.common.layer = layer;
        mesh_entity.common.color = color;
        doc.add_entity(EntityType::Mesh(mesh_entity))?;
    }
    Ok(())
}

/// `None` when the file has no scene to walk. An empty scene is `Some` and
/// imports nothing, rather than every mesh in the library.
fn placed_meshes(json: &JsonValue, meshes: &[JsonValue]) -> Option<Vec<PlacedMesh>> {
    let nodes = json.get("nodes")?.as_array()?;
    if nodes.is_empty() {
        return None;
    }
    let scenes = json.get("scenes")?.as_array()?;
    if scenes.is_empty() {
        return None;
    }
    let scene_index = json
        .get("scene")
        .and_then(|value| value.as_usize())
        .unwrap_or(0);
    let scene = scenes.get(scene_index).or_else(|| scenes.first())?;
    let mut placed = Vec::new();
    let mut path = Vec::new();
    if let Some(roots) = scene.get("nodes").and_then(|value| value.as_array()) {
        for root in roots {
            if let Some(index) = root.as_usize() {
                collect_placed(index, &IDENTITY, nodes, meshes, &mut path, &mut placed);
            }
        }
    }
    Some(placed)
}

fn collect_placed(
    index: usize,
    parent: &[f64; 16],
    nodes: &[JsonValue],
    meshes: &[JsonValue],
    path: &mut Vec<usize>,
    placed: &mut Vec<PlacedMesh>,
) {
    if path.len() >= 128 || path.contains(&index) || index >= nodes.len() {
        return;
    }
    path.push(index);
    let node = &nodes[index];
    let world = super::mesh::mul_matrix(parent, &gltf_node_matrix(node));
    if let Some(mesh_index) = node.get("mesh").and_then(|value| value.as_usize()) {
        if mesh_index < meshes.len() {
            placed.push(PlacedMesh {
                index: mesh_index,
                world,
                name: placed_name(node, mesh_index, meshes),
            });
        }
    }
    if let Some(children) = node.get("children").and_then(|value| value.as_array()) {
        for child in children {
            if let Some(child_index) = child.as_usize() {
                collect_placed(child_index, &world, nodes, meshes, path, placed);
            }
        }
    }
    path.pop();
}

fn gltf_node_matrix(node: &JsonValue) -> [f64; 16] {
    if let Some(values) = node.get("matrix").and_then(|value| value.as_array()) {
        if values.len() >= 16 {
            let mut matrix = [0.0; 16];
            for (slot, value) in matrix.iter_mut().zip(values.iter()) {
                *slot = value.as_f64().unwrap_or(0.0);
            }
            return matrix;
        }
    }
    let translation = json_vec3(node.get("translation"), [0.0; 3]);
    let rotation = json_vec4(node.get("rotation"), [0.0, 0.0, 0.0, 1.0]);
    let scale = json_vec3(node.get("scale"), [1.0, 1.0, 1.0]);
    matrix_from_trs(translation, rotation, scale)
}

fn json_vec3(value: Option<&JsonValue>, default: [f64; 3]) -> [f64; 3] {
    let Some(array) = value.and_then(|value| value.as_array()) else {
        return default;
    };
    if array.len() < 3 {
        return default;
    }
    [
        array[0].as_f64().unwrap_or(default[0]),
        array[1].as_f64().unwrap_or(default[1]),
        array[2].as_f64().unwrap_or(default[2]),
    ]
}

fn json_vec4(value: Option<&JsonValue>, default: [f64; 4]) -> [f64; 4] {
    let Some(array) = value.and_then(|value| value.as_array()) else {
        return default;
    };
    if array.len() < 4 {
        return default;
    }
    [
        array[0].as_f64().unwrap_or(default[0]),
        array[1].as_f64().unwrap_or(default[1]),
        array[2].as_f64().unwrap_or(default[2]),
        array[3].as_f64().unwrap_or(default[3]),
    ]
}

fn mesh_name(mesh: &JsonValue) -> String {
    mesh.get("name")
        .and_then(|value| value.as_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("mesh")
        .to_string()
}

fn placed_name(node: &JsonValue, mesh_index: usize, meshes: &[JsonValue]) -> String {
    if let Some(name) = node.get("name").and_then(|value| value.as_str()) {
        if !name.is_empty() {
            return name.to_string();
        }
    }
    meshes
        .get(mesh_index)
        .map(mesh_name)
        .unwrap_or_else(|| "mesh".to_string())
}

fn transform_positions_f32(positions: &[[f32; 3]], world: &[f64; 16]) -> Vec<[f64; 3]> {
    let widened: Vec<[f64; 3]> = positions
        .iter()
        .map(|position| [position[0] as f64, position[1] as f64, position[2] as f64])
        .collect();
    if super::mesh::is_identity(world) {
        widened
    } else {
        transform_positions(&widened, world)
    }
}

/// glTF primitive modes: 4 triangles, 5 triangle strip, 6 triangle fan.
fn triangle_indices(indices: &[usize], mode: usize) -> Vec<[usize; 3]> {
    match mode {
        5 => {
            let mut triangles = Vec::new();
            if indices.len() >= 3 {
                for offset in 0..indices.len() - 2 {
                    let (a, b, c) = (indices[offset], indices[offset + 1], indices[offset + 2]);
                    if a == b || b == c || a == c {
                        continue;
                    }
                    if offset % 2 == 0 {
                        triangles.push([a, b, c]);
                    } else {
                        triangles.push([b, a, c]);
                    }
                }
            }
            triangles
        }
        6 => {
            let mut triangles = Vec::new();
            if indices.len() >= 3 {
                for offset in 1..indices.len() - 1 {
                    triangles.push([indices[0], indices[offset], indices[offset + 1]]);
                }
            }
            triangles
        }
        4 => indices
            .chunks_exact(3)
            .map(|tri| [tri[0], tri[1], tri[2]])
            .collect(),
        _ => Vec::new(),
    }
}

/// Read Vec3 positions from an accessor.
fn read_accessor_vec3(
    accessor_idx: usize,
    accessors: Option<&[JsonValue]>,
    buffer_views: Option<&[JsonValue]>,
    buffers: &[Vec<u8>],
) -> Result<Vec<[f32; 3]>> {
    let accessors =
        accessors.ok_or_else(|| DxfError::ImportError("Missing accessors array".to_string()))?;
    let accessor = accessors
        .get(accessor_idx)
        .ok_or_else(|| DxfError::ImportError(format!("Accessor {} out of range", accessor_idx)))?;

    let count = accessor
        .get("count")
        .and_then(|v| v.as_usize())
        .unwrap_or(0);
    let component_type = accessor
        .get("componentType")
        .and_then(|v| v.as_usize())
        .unwrap_or(0);
    let bv_idx = accessor.get("bufferView").and_then(|v| v.as_usize());
    let byte_offset_acc = accessor
        .get("byteOffset")
        .and_then(|v| v.as_usize())
        .unwrap_or(0);

    if component_type != 5126 {
        // 5126 = FLOAT
        return Err(DxfError::ImportError(format!(
            "Unsupported POSITION component type: {} (expected FLOAT/5126)",
            component_type
        )));
    }

    let bv_idx = match bv_idx {
        Some(i) => i,
        None => return Ok(Vec::new()),
    };

    let buffer_views = buffer_views
        .ok_or_else(|| DxfError::ImportError("Missing bufferViews array".to_string()))?;
    let bv = buffer_views
        .get(bv_idx)
        .ok_or_else(|| DxfError::ImportError(format!("BufferView {} out of range", bv_idx)))?;

    let buffer_idx = bv.get("buffer").and_then(|v| v.as_usize()).unwrap_or(0);
    let byte_offset_bv = bv.get("byteOffset").and_then(|v| v.as_usize()).unwrap_or(0);
    let byte_stride = bv
        .get("byteStride")
        .and_then(|v| v.as_usize())
        .unwrap_or(12); // 3 floats

    let buffer = buffers
        .get(buffer_idx)
        .ok_or_else(|| DxfError::ImportError(format!("Buffer {} not loaded", buffer_idx)))?;

    let start = byte_offset_bv.checked_add(byte_offset_acc).ok_or_else(|| {
        DxfError::ImportError("Buffer offset overflow in glTF accessor".to_string())
    })?;

    let max_possible = if start >= buffer.len() || byte_stride == 0 {
        0
    } else {
        (buffer.len() - start) / byte_stride
    };
    let alloc_cap = count.min(max_possible);
    let mut positions = Vec::with_capacity(alloc_cap);

    for i in 0..count {
        let stride_offset = match i.checked_mul(byte_stride) {
            Some(so) => so,
            None => break,
        };
        let offset = match start.checked_add(stride_offset) {
            Some(o) => o,
            None => break,
        };
        if offset
            .checked_add(12)
            .map_or(true, |end| end > buffer.len())
        {
            break;
        }
        let x = f32::from_le_bytes([
            buffer[offset],
            buffer[offset + 1],
            buffer[offset + 2],
            buffer[offset + 3],
        ]);
        let y = f32::from_le_bytes([
            buffer[offset + 4],
            buffer[offset + 5],
            buffer[offset + 6],
            buffer[offset + 7],
        ]);
        let z = f32::from_le_bytes([
            buffer[offset + 8],
            buffer[offset + 9],
            buffer[offset + 10],
            buffer[offset + 11],
        ]);
        positions.push([x, y, z]);
    }

    Ok(positions)
}

/// Read scalar indices from an accessor.
fn read_accessor_scalar(
    accessor_idx: usize,
    accessors: Option<&[JsonValue]>,
    buffer_views: Option<&[JsonValue]>,
    buffers: &[Vec<u8>],
) -> Result<Vec<usize>> {
    let accessors =
        accessors.ok_or_else(|| DxfError::ImportError("Missing accessors array".to_string()))?;
    let accessor = accessors
        .get(accessor_idx)
        .ok_or_else(|| DxfError::ImportError(format!("Accessor {} out of range", accessor_idx)))?;

    let count = accessor
        .get("count")
        .and_then(|v| v.as_usize())
        .unwrap_or(0);
    let component_type = accessor
        .get("componentType")
        .and_then(|v| v.as_usize())
        .unwrap_or(0);
    let bv_idx = accessor.get("bufferView").and_then(|v| v.as_usize());
    let byte_offset_acc = accessor
        .get("byteOffset")
        .and_then(|v| v.as_usize())
        .unwrap_or(0);

    let bv_idx = match bv_idx {
        Some(i) => i,
        None => return Ok(Vec::new()),
    };

    let buffer_views = buffer_views
        .ok_or_else(|| DxfError::ImportError("Missing bufferViews array".to_string()))?;
    let bv = buffer_views
        .get(bv_idx)
        .ok_or_else(|| DxfError::ImportError(format!("BufferView {} out of range", bv_idx)))?;

    let buffer_idx = bv.get("buffer").and_then(|v| v.as_usize()).unwrap_or(0);
    let byte_offset_bv = bv.get("byteOffset").and_then(|v| v.as_usize()).unwrap_or(0);

    let buffer = buffers
        .get(buffer_idx)
        .ok_or_else(|| DxfError::ImportError(format!("Buffer {} not loaded", buffer_idx)))?;

    let start = byte_offset_bv.checked_add(byte_offset_acc).ok_or_else(|| {
        DxfError::ImportError("Buffer offset overflow in glTF accessor".to_string())
    })?;

    let item_size = match component_type {
        5121 => 1,
        5123 => 2,
        5125 => 4,
        _ => 1,
    };
    let max_possible = if start >= buffer.len() {
        0
    } else {
        (buffer.len() - start) / item_size
    };
    let alloc_cap = count.min(max_possible);
    let mut indices = Vec::with_capacity(alloc_cap);

    match component_type {
        5121 => {
            // UNSIGNED_BYTE
            for i in 0..count {
                let offset = match start.checked_add(i) {
                    Some(o) => o,
                    None => break,
                };
                if offset >= buffer.len() {
                    break;
                }
                indices.push(buffer[offset] as usize);
            }
        }
        5123 => {
            // UNSIGNED_SHORT
            for i in 0..count {
                let offset = match i.checked_mul(2).and_then(|so| start.checked_add(so)) {
                    Some(o) => o,
                    None => break,
                };
                if offset.checked_add(2).map_or(true, |end| end > buffer.len()) {
                    break;
                }
                let v = u16::from_le_bytes([buffer[offset], buffer[offset + 1]]);
                indices.push(v as usize);
            }
        }
        5125 => {
            // UNSIGNED_INT
            for i in 0..count {
                let offset = match i.checked_mul(4).and_then(|so| start.checked_add(so)) {
                    Some(o) => o,
                    None => break,
                };
                if offset.checked_add(4).map_or(true, |end| end > buffer.len()) {
                    break;
                }
                let v = u32::from_le_bytes([
                    buffer[offset],
                    buffer[offset + 1],
                    buffer[offset + 2],
                    buffer[offset + 3],
                ]);
                indices.push(v as usize);
            }
        }
        _ => {
            return Err(DxfError::ImportError(format!(
                "Unsupported index component type: {}",
                component_type
            )));
        }
    }

    Ok(indices)
}

/// Extract material name and base color from a glTF material.
fn get_material_color(mat_idx: usize, materials: Option<&[JsonValue]>) -> (String, [f32; 4]) {
    let default = (String::new(), [0.8f32, 0.8, 0.8, 1.0]);

    let materials = match materials {
        Some(m) => m,
        None => return default,
    };
    let mat = match materials.get(mat_idx) {
        Some(m) => m,
        None => return default,
    };

    let name = mat
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // PBR metallic-roughness base color
    let mut color = [0.8f32, 0.8, 0.8, 1.0];
    if let Some(pbr) = mat.get("pbrMetallicRoughness") {
        if let Some(bc) = pbr.get("baseColorFactor").and_then(|v| v.as_array()) {
            if bc.len() >= 3 {
                color[0] = bc[0].as_f64().unwrap_or(0.8) as f32;
                color[1] = bc[1].as_f64().unwrap_or(0.8) as f32;
                color[2] = bc[2].as_f64().unwrap_or(0.8) as f32;
                if bc.len() >= 4 {
                    color[3] = bc[3].as_f64().unwrap_or(1.0) as f32;
                }
            }
        }
    }

    (name, color)
}

fn read_glb_u32(cursor: &mut impl Read) -> Result<u32> {
    cursor
        .read_u32::<LittleEndian>()
        .map_err(|error| DxfError::ImportError(format!("truncated GLB container: {error}")))
}

/// Simple base64 decoder (standard alphabet, no padding required).
fn base64_decode(input: &str) -> Result<Vec<u8>> {
    const DECODE: [u8; 128] = {
        let mut t = [255u8; 128];
        let mut i = 0u8;
        while i < 26 {
            t[(b'A' + i) as usize] = i;
            t[(b'a' + i) as usize] = i + 26;
            i += 1;
        }
        let mut i = 0u8;
        while i < 10 {
            t[(b'0' + i) as usize] = i + 52;
            i += 1;
        }
        t[b'+' as usize] = 62;
        t[b'/' as usize] = 63;
        t
    };

    let bytes: Vec<u8> = input
        .bytes()
        .filter(|&b| b != b'=' && b != b'\n' && b != b'\r' && b != b' ')
        .collect();

    let mut output = Vec::with_capacity(bytes.len() * 3 / 4);
    let chunks = bytes.chunks(4);

    for chunk in chunks {
        let mut buf = [0u32; 4];
        for (i, &b) in chunk.iter().enumerate() {
            if b >= 128 || DECODE[b as usize] == 255 {
                return Err(DxfError::ImportError(
                    "Invalid base64 character".to_string(),
                ));
            }
            buf[i] = DECODE[b as usize] as u32;
        }
        let n = (buf[0] << 18) | (buf[1] << 12) | (buf[2] << 6) | buf[3];
        output.push((n >> 16) as u8);
        if chunk.len() > 2 {
            output.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            output.push(n as u8);
        }
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_decode() {
        let decoded = base64_decode("SGVsbG8=").unwrap();
        assert_eq!(&decoded, b"Hello");
    }

    #[test]
    fn test_glb_magic() {
        let importer = GltfImporter::from_bytes(b"not glb".to_vec());
        // Should parse as gltf text — will fail JSON parse
        assert!(importer.import().is_err());
    }

    #[test]
    fn test_import_minimal_gltf() {
        // Minimal glTF with embedded base64 buffer:
        // 1 triangle, 3 vertices, 3 indices
        let positions: [f32; 9] = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        let indices: [u16; 3] = [0, 1, 2];

        let mut buf = Vec::new();
        for p in &positions {
            buf.extend_from_slice(&p.to_le_bytes());
        }
        for i in &indices {
            buf.extend_from_slice(&i.to_le_bytes());
        }
        // Pad to 4 bytes
        while buf.len() % 4 != 0 {
            buf.push(0);
        }

        let b64 = base64_encode(&buf);
        let uri = format!("data:application/octet-stream;base64,{}", b64);

        let json = format!(
            r#"{{
  "asset": {{"version": "2.0"}},
  "buffers": [{{"uri": "{uri}", "byteLength": {}}}],
  "bufferViews": [
    {{"buffer": 0, "byteOffset": 0, "byteLength": 36}},
    {{"buffer": 0, "byteOffset": 36, "byteLength": 6}}
  ],
  "accessors": [
    {{"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "max": [1,1,0], "min": [0,0,0]}},
    {{"bufferView": 1, "componentType": 5123, "count": 3, "type": "SCALAR"}}
  ],
  "meshes": [{{"name": "Triangle", "primitives": [{{"attributes": {{"POSITION": 0}}, "indices": 1}}]}}]
}}"#,
            buf.len()
        );

        let importer = GltfImporter::from_bytes(json.as_bytes().to_vec());
        let doc = importer.import().unwrap();
        let entities: Vec<_> = doc.entities().collect();
        assert_eq!(entities.len(), 1);
        if let EntityType::Mesh(mesh) = &entities[0] {
            assert_eq!(mesh.vertices.len(), 3);
            assert_eq!(mesh.faces.len(), 1);
        } else {
            panic!("Expected Mesh");
        }
    }

    #[test]
    fn node_translation_moves_the_triangle() {
        let positions: [f32; 9] = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        let indices: [u16; 3] = [0, 1, 2];
        let mut buf = Vec::new();
        for position in &positions {
            buf.extend_from_slice(&position.to_le_bytes());
        }
        for index in &indices {
            buf.extend_from_slice(&index.to_le_bytes());
        }
        while buf.len() % 4 != 0 {
            buf.push(0);
        }
        let uri = format!(
            "data:application/octet-stream;base64,{}",
            base64_encode(&buf)
        );
        let json = format!(
            r#"{{
  "asset": {{"version": "2.0"}},
  "buffers": [{{"uri": "{uri}", "byteLength": {}}}],
  "bufferViews": [
    {{"buffer": 0, "byteOffset": 0, "byteLength": 36}},
    {{"buffer": 0, "byteOffset": 36, "byteLength": 6}}
  ],
  "accessors": [
    {{"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3"}},
    {{"bufferView": 1, "componentType": 5123, "count": 3, "type": "SCALAR"}}
  ],
  "meshes": [{{"primitives": [{{"attributes": {{"POSITION": 0}}, "indices": 1}}]}}],
  "nodes": [{{"mesh": 0, "translation": [10, 0, 0]}}],
  "scenes": [{{"nodes": [0]}}],
  "scene": 0
}}"#,
            buf.len()
        );
        let doc = GltfImporter::from_bytes(json.into_bytes())
            .import()
            .unwrap();
        let entities: Vec<_> = doc.entities().collect();
        let EntityType::Mesh(mesh) = &entities[0] else {
            panic!("Expected Mesh");
        };
        assert!(mesh
            .vertices
            .iter()
            .any(|vertex| (vertex.x - 10.0).abs() < 1e-5 && vertex.y == 0.0));
    }

    #[test]
    fn triangle_strip_alternates_winding() {
        let triangles = triangle_indices(&[0, 1, 2, 3], 5);
        assert_eq!(triangles, vec![[0, 1, 2], [2, 1, 3]]);
    }

    #[test]
    fn gltf_huge_count_tiny_buffer_does_not_oom() {
        let json = r#"{
  "asset": {"version": "2.0"},
  "buffers": [{"uri": "data:application/octet-stream;base64,AAAA", "byteLength": 4}],
  "bufferViews": [{"buffer": 0, "byteOffset": 0, "byteLength": 4}],
  "accessors": [{"bufferView": 0, "componentType": 5126, "count": 2000000000, "type": "VEC3"}],
  "meshes": [{"primitives": [{"attributes": {"POSITION": 0}}]}]
}"#;
        let doc = GltfImporter::from_bytes(json.as_bytes().to_vec())
            .import()
            .unwrap();
        assert_eq!(doc.entities().count(), 0);
    }

    /// Simple base64 encoder for tests.
    fn base64_encode(data: &[u8]) -> String {
        const TABLE: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in data.chunks(3) {
            let b0 = chunk[0] as u32;
            let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
            let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
            let n = (b0 << 16) | (b1 << 8) | b2;
            out.push(TABLE[(n >> 18 & 63) as usize] as char);
            out.push(TABLE[(n >> 12 & 63) as usize] as char);
            if chunk.len() > 1 {
                out.push(TABLE[(n >> 6 & 63) as usize] as char);
            } else {
                out.push('=');
            }
            if chunk.len() > 2 {
                out.push(TABLE[(n & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
        out
    }
}
