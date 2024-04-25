use std::fmt::format;

use alkahest_data::{
    technique::{STechnique, STechniqueShader},
    tfx::TfxShaderStage,
    ExtendedHash,
};
use alkahest_pm::package_manager;
use anyhow::{ensure, Context};
use destiny_pkg::TagHash;
use tiger_parse::PackageManagerExt;
use windows::Win32::Graphics::Direct3D11::ID3D11SamplerState;

use crate::{
    gpu::{buffer::ConstantBufferCached, GpuContext, SharedGpuContext},
    tfx::{
        bytecode::{interpreter::TfxBytecodeInterpreter, opcodes::TfxBytecodeOp},
        technique::{ShaderModule, Technique, TechniqueStage},
    },
};

pub fn load_technique(gctx: SharedGpuContext, hash: TagHash) -> anyhow::Result<Technique> {
    let stech: STechnique = package_manager().read_tag_struct(hash)?;

    Ok(Technique {
        stage_vertex: load_technique_stage(
            gctx.clone(),
            &stech.shader_vertex,
            TfxShaderStage::Vertex,
        )?,
        stage_geometry: load_technique_stage(
            gctx.clone(),
            &stech.shader_geometry,
            TfxShaderStage::Geometry,
        )?,
        stage_pixel: load_technique_stage(
            gctx.clone(),
            &stech.shader_pixel,
            TfxShaderStage::Pixel,
        )?,
        stage_compute: load_technique_stage(
            gctx.clone(),
            &stech.shader_compute,
            TfxShaderStage::Compute,
        )?,
        tech: stech,
    })
}

fn load_technique_stage(
    gctx: SharedGpuContext,
    shader: &STechniqueShader,
    stage: TfxShaderStage,
) -> anyhow::Result<Option<TechniqueStage>> {
    if shader.shader.is_none() {
        return Ok(None);
    }

    let cbuffer = if shader.constant_buffer.is_some() {
        let buffer_header_ref = package_manager()
            .get_entry(shader.constant_buffer)
            .unwrap()
            .reference;

        let data_raw = package_manager().read_tag(buffer_header_ref).unwrap();

        let data = bytemuck::cast_slice(&data_raw);
        let buf = ConstantBufferCached::create_array_init(gctx.clone(), data).unwrap();

        Some(buf)
    } else if !shader.unk50.is_empty() {
        let buf = ConstantBufferCached::create_array_init(
            gctx.clone(),
            bytemuck::cast_slice(&shader.unk50),
        )
        .unwrap();

        Some(buf)
    } else {
        None
    };

    let bytecode = match TfxBytecodeOp::parse_all(&shader.bytecode, binrw::Endian::Little) {
        Ok(opcodes) => Some(TfxBytecodeInterpreter::new(opcodes)),
        Err(e) => {
            debug!(
                "Failed to parse VS TFX bytecode: {e:?} (data={})",
                hex::encode(&shader.bytecode)
            );
            None
        }
    };

    let mut stage = TechniqueStage {
        stage,
        shader: shader.clone(),

        samplers: vec![],
        textures: vec![],
        shader_module: ShaderModule::load(&gctx, shader.shader)
            .with_context(|| format!("Failed to load shader module {}", shader.shader))?,

        cbuffer,
        bytecode,
    };

    for sampler in shader.samplers.iter() {
        stage.samplers.push(load_sampler(&gctx, sampler.hash32()).ok());
    }

    Ok(Some(stage))
}

pub fn load_sampler(gctx: &GpuContext, hash: TagHash) -> anyhow::Result<ID3D11SamplerState> {
    let entry = package_manager()
        .get_entry(hash)
        .context("Sampler entry not found")?;
    ensure!(
        entry.file_type == 34 && entry.file_subtype == 1,
        "Sampler header type mismatch"
    );
    let sampler_header_ref = entry.reference;
    let sampler_data = package_manager().read_tag(sampler_header_ref).unwrap();

    let mut sampler = None;
    unsafe {
        gctx.device
            .CreateSamplerState(sampler_data.as_ptr() as _, Some(&mut sampler))?;
    };

    Ok(sampler.unwrap())
}
