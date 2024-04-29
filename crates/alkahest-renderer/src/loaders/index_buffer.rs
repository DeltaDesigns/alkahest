use alkahest_data::{buffers::IndexBufferHeader, dxgi::DxgiFormat};
use alkahest_pm::package_manager;
use anyhow::Context;
use destiny_pkg::TagHash;
use tiger_parse::PackageManagerExt;
use windows::Win32::Graphics::Direct3D11::{
    ID3D11Buffer, D3D11_BIND_INDEX_BUFFER, D3D11_BUFFER_DESC, D3D11_SUBRESOURCE_DATA,
    D3D11_USAGE_IMMUTABLE,
};

use crate::{gpu::SharedGpuContext, util::d3d::D3dResource};

pub struct IndexBuffer {
    pub buffer: ID3D11Buffer,
    pub size: u64,
    pub format: DxgiFormat,
}

pub(crate) fn load_index_buffer(
    gctx: &SharedGpuContext,
    hash: TagHash,
) -> anyhow::Result<IndexBuffer> {
    let entry = package_manager()
        .get_entry(hash)
        .context("Entry not found")?;

    let header: IndexBufferHeader = package_manager()
        .read_tag_struct(hash)
        .context("Failed to read header data")?;
    let data = package_manager()
        .read_tag(entry.reference)
        .context("Failed to read buffer data")?;

    let mut buffer = None;
    unsafe {
        gctx.device.CreateBuffer(
            &D3D11_BUFFER_DESC {
                ByteWidth: header.data_size as u32,
                Usage: D3D11_USAGE_IMMUTABLE,
                BindFlags: D3D11_BIND_INDEX_BUFFER.0 as u32,
                CPUAccessFlags: 0,
                MiscFlags: 0,
                StructureByteStride: 0,
            },
            Some(&D3D11_SUBRESOURCE_DATA {
                pSysMem: data.as_ptr() as _,
                ..Default::default()
            }),
            Some(&mut buffer),
        )?;
    }
    let buffer = buffer.unwrap();
    buffer.set_debug_name(&format!("IndexBuffer: {hash}"));

    Ok(IndexBuffer {
        buffer,
        size: header.data_size,
        format: if header.is_32bit {
            DxgiFormat::R32_UINT
        } else {
            DxgiFormat::R16_UINT
        },
    })
}
