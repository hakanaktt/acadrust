//! Common-header encoding for newly constructed Arch records.

use super::DwgObjectWriter;
use crate::classes::arch::ArchObject;
use crate::entities::EntityCommon;
use crate::io::dwg::dwg_stream_readers::object_reader::{
    entities::read_proxy_payload, DwgObjectReader,
};
use crate::io::dwg::dwg_stream_writers::DwgMergedWriter;
use crate::objects::{ProxyObjectReference, ProxyPayload};
use crate::types::{DxfVersion, Handle};

struct ArchitectureObjectBody {
    type_code: i16,
    has_ds_data: bool,
    eed: Vec<(u64, Vec<u8>)>,
    payload: ProxyPayload,
    text: ProxyPayload,
    references: Vec<ProxyObjectReference>,
}

impl DwgObjectWriter<'_> {
    fn architecture_object_body(
        object: &ArchObject,
        version: DxfVersion,
    ) -> crate::error::Result<ArchitectureObjectBody> {
        let raw = object.raw_dwg_data.as_deref().ok_or_else(|| {
            crate::error::DxfError::NotImplemented(format!(
                "Arch object {} has no DWG class body",
                object.dxf_name
            ))
        })?;
        let objects = DwgObjectReader::new(Vec::new(), version, Default::default())?;
        let (type_code, mut reader) = objects.read_raw_record(raw, object.raw_dwg_handle_bits)?;
        let common = objects.read_common_non_entity_data(&mut reader, type_code);
        reader.check_error()?;
        let (payload, text, references) = read_proxy_payload(&mut reader, common.common.handle);
        reader.check_error()?;
        Ok(ArchitectureObjectBody {
            type_code,
            has_ds_data: common.has_ds_data,
            eed: common.common.eed_raw,
            payload,
            text,
            references,
        })
    }

    pub(crate) fn validate_architecture_object(
        object: &ArchObject,
        version: DxfVersion,
    ) -> crate::error::Result<()> {
        Self::architecture_object_body(object, version).map(|_| ())
    }

    pub(super) fn write_architecture_object(&mut self, object: &ArchObject) {
        let body = Self::architecture_object_body(object, self.dxf_version)
            .expect("Arch object record was validated before writing");
        let type_code = self.source_class_type_code(&object.dxf_name, body.type_code);
        let source_eed = if self.document.eed_by_handle.contains_key(&object.handle) {
            Vec::new()
        } else {
            body.eed
        };
        self.write_complete_non_entity_data(
            type_code,
            object.handle,
            object.owner,
            &object.reactors,
            &object.xdictionary_handle,
            source_eed,
            body.has_ds_data,
        );
        self.write_registered_payload(&body.payload, &body.references);
        let text = body.text.data();
        for bit in 0..body.text.bit_count as usize {
            self.writer
                .write_text_bit(text[bit / 8] & (0x80 >> (bit % 8)) != 0);
        }
        self.register_object(object.handle);
    }

    pub(crate) fn encode_architecture_entity(
        &mut self,
        type_code: i16,
        common: &EntityCommon,
        body: impl FnOnce(&mut DwgMergedWriter),
    ) -> (Vec<u8>, i64) {
        self.write_common_entity_data(
            type_code,
            common.handle,
            common.owner_handle,
            &common.layer,
            &common.color,
            &common.line_weight,
            &common.transparency,
            common.invisible,
            common.linetype_scale,
            &common.linetype,
            &common.linetype_handle,
            &common.extended_data,
            &common.reactors,
            &common.xdictionary_handle,
            common.graphic_data.as_deref(),
            common.entity_mode,
            common.material_flags,
            &common.material_handle,
            common.shadow_flags,
            common.plotstyle_flags,
            &common.plotstyle_handle,
            &common.color_book_handle,
            &common.full_visual_style_handle,
            &common.face_visual_style_handle,
            &common.edge_visual_style_handle,
        );
        body(&mut self.writer);
        self.finish_architecture_record()
    }

    pub(crate) fn encode_architecture_object(
        &mut self,
        type_code: i16,
        handle: Handle,
        owner: Handle,
        body: impl FnOnce(&mut DwgMergedWriter),
    ) -> (Vec<u8>, i64) {
        self.write_common_non_entity_data(type_code, handle, owner, &[owner], &None);
        body(&mut self.writer);
        self.finish_architecture_record()
    }

    fn finish_architecture_record(&mut self) -> (Vec<u8>, i64) {
        let raw = self.writer.merge();
        let handle_bits = raw.len() as i64 * 8 - self.writer.handle_start_bits();
        self.writer.reset();
        self.pending_type_code = None;
        (raw, handle_bits)
    }
}
