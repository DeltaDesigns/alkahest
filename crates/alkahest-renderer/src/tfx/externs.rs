use std::{fmt::Debug, mem::transmute};

use anyhow::Context;
use binrw::binread;
use destiny_pkg::TagHash;
use glam::{Mat4, Vec4};
use parking_lot::RwLock;
use rustc_hash::{FxHashMap, FxHashSet};
use windows::Win32::Graphics::Direct3D11::ID3D11ShaderResourceView;

use crate::{gpu::texture::Texture, handle::Handle, loaders::AssetManager, util::short_type_name};

#[derive(Default, Clone)]
pub enum TextureView {
    #[default]
    Null,
    /// Used for internal textures such as gbuffers
    RawSRV(ID3D11ShaderResourceView),
    // Tracked(WeakHandle<Texture>),
}

impl TextureView {
    pub fn view(&self, am: &AssetManager) -> Option<ID3D11ShaderResourceView> {
        match self {
            TextureView::Null => None,
            TextureView::RawSRV(v) => Some(v.clone()),
            // TextureView::Tracked(t) => t
            //     .upgrade()
            //     .and_then(|t| am.textures.get(&t).map(|t| t.view.clone())),
        }
    }

    pub fn view_unchecked(&self, am: &AssetManager) -> ID3D11ShaderResourceView {
        self.view(am).unwrap_or_else(|| unsafe { transmute(0u64) })
    }
}

impl Debug for TextureView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TextureView::Null => write!(f, "TextureView::Null"),
            TextureView::RawSRV(_) => write!(f, "TextureView::RawSRV"),
            // TextureView::Tracked(_) => write!(f, "TextureView::Tracked"),
        }
    }
}

impl From<ID3D11ShaderResourceView> for TextureView {
    fn from(v: ID3D11ShaderResourceView) -> Self {
        TextureView::RawSRV(v)
    }
}

// impl From<WeakHandle<Texture>> for TextureView {
//     fn from(t: WeakHandle<Texture>) -> Self {
//         TextureView::Tracked(t)
//     }
// }

#[derive(Default)]
pub struct ExternStorage {
    pub frame: Option<Frame>,
    pub view: Option<View>,
    pub deferred: Option<Deferred>,
    pub deferred_light: Option<DeferredLight>,
    pub transparent: Option<Transparent>,
    pub rigid_model: Option<RigidModel>,
    pub decal: Option<Decal>,
    pub simple_geometry: Option<SimpleGeometry>,

    pub errors: RwLock<FxHashMap<String, TfxExpressionError>>,
}

impl ExternStorage {
    pub fn get_value_or_default<T: Sized + Default + 'static>(
        &self,
        ext: TfxExtern,
        offset: usize,
    ) -> T {
        self.get_value(ext, offset).unwrap_or_default()
    }

    pub fn get_value<T: Sized + 'static>(
        &self,
        ext: TfxExtern,
        offset: usize,
    ) -> anyhow::Result<T> {
        match self.get_value_inner::<T>(ext, offset) {
            ExternValue::Value(v) => Ok(v),
            ExternValue::Unimplemented => {
                self.errors
                    .write()
                    .entry(format!(
                        "Extern field @ 0x{offset:X} for {ext:?} is unimplemented (type {})",
                        short_type_name::<T>()
                    ))
                    .or_insert_with(|| TfxExpressionError {
                        error_type: TfxExpressionErrorType::Unimplemented {
                            field_offset: offset,
                        },
                        repeat_count: 0,
                        // occurences: FxHashSet::default(),
                    })
                    .repeat_count += 1;

                Err(anyhow::anyhow!("Unimplemented field: {ext:?}@0x{offset:X}"))
            }

            ExternValue::InvalidType(t) => {
                self.errors
                    .write()
                    .entry(format!(
                        "Extern field @ 0x{offset:X} for {ext:?} has invalid type (expected {})",
                        short_type_name::<T>()
                    ))
                    .or_insert_with(|| TfxExpressionError {
                        error_type: TfxExpressionErrorType::InvalidType(t),
                        repeat_count: 0,
                        // occurences: FxHashSet::default(),
                    })
                    .repeat_count += 1;

                Err(anyhow::anyhow!("Invalid type: {ext:?}@0x{offset:X}"))
            }
            ExternValue::FieldNotFound => {
                self.errors
                    .write()
                    .entry(format!(
                        "Extern field @ 0x{offset:X} for {ext:?} not found (type {})",
                        short_type_name::<T>()
                    ))
                    .or_insert_with(|| TfxExpressionError {
                        error_type: TfxExpressionErrorType::Unimplemented {
                            field_offset: offset,
                        },
                        repeat_count: 0,
                        // occurences: FxHashSet::default(),
                    })
                    .repeat_count += 1;

                Err(anyhow::anyhow!("Field not found: {ext:?}@0x{offset:X}"))
            }
            ExternValue::ExternNotFound => {
                self.errors
                    .write()
                    .entry(format!("Extern {ext:?} not found"))
                    .or_insert_with(|| TfxExpressionError {
                        error_type: TfxExpressionErrorType::ExternNotSet("Extern not found"),
                        repeat_count: 0,
                        // occurences: FxHashSet::default(),
                    })
                    .repeat_count += 1;

                Err(anyhow::anyhow!("Extern not found: {ext:?}"))
            }
            ExternValue::ExternNotSet => {
                self.errors
                    .write()
                    .entry(format!("Extern {ext:?} not set"))
                    .or_insert_with(|| TfxExpressionError {
                        error_type: TfxExpressionErrorType::ExternNotSet("Extern not set"),
                        repeat_count: 0,
                        // occurences: FxHashSet::default(),
                    })
                    .repeat_count += 1;

                Err(anyhow::anyhow!("Extern not set: {ext:?}"))
            }
        }
    }

    fn get_value_inner<T: Sized + 'static>(&self, ext: TfxExtern, offset: usize) -> ExternValue<T> {
        macro_rules! extern_lookup {
            ($(
                $ext:ident => $field:expr,
            )*) => {
                match ext {
                    $(
                        TfxExtern::$ext => $field.as_ref().map(|v| v.get_field(offset)).unwrap_or_else(|| ExternValue::ExternNotSet),
                    )*
                    _ => {
                        ExternValue::ExternNotFound
                    },
                }
            };
        }

        extern_lookup! {
            Frame => self.frame,
            View => self.view,
            Deferred => self.deferred,
            DeferredLight => self.deferred_light,
            Transparent => self.transparent,
            RigidModel => self.rigid_model,
            Decal => self.decal,
            SimpleGeometry => self.simple_geometry,
        }
    }

    pub fn get_field_path(ext: TfxExtern, offset: usize) -> Option<String> {
        macro_rules! extern_lookup {
            ($(
                $field:ident
            ),*) => {
                match ext {
                    $(
                        TfxExtern::$field => <$field as Extern>::get_field_name(offset).map(|f| format!("{}->{f}", <$field as Extern>::get_name())),
                    )*
                    _ => {
                        None
                    },
                }
            };
        }

        extern_lookup! {
            Frame,
            View,
            Deferred,
            DeferredLight,
            Transparent,
            RigidModel,
            Decal,
            SimpleGeometry
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum ExternValue<T> {
    Value(T),
    Unimplemented,
    InvalidType(&'static str),
    FieldNotFound,

    ExternNotFound,
    ExternNotSet,
}

#[allow(clippy::missing_safety_doc)]
pub trait Extern {
    fn get_name() -> &'static str;

    fn get_field<T: Sized + 'static>(&self, offset: usize) -> ExternValue<T>;

    fn get_field_name(offset: usize) -> Option<&'static str>;
}

/*
## Syntax

struct Name("internal_name") {
    0xOFFSET => field_name: field_type,
    // Optionally, you can mark a field as unimplemented
    0xOFFSET => field_name: field_type > unimplemented(true),
    // Marking the field as unimplemented(false) will still return the value
    0xOFFSET => field_name: field_type > unimplemented(true),
}
*/

macro_rules! extern_struct {
    (struct $name:ident ($name_c:literal) { $($field_offset:expr => $field:ident: $field_type:ty  $(> unimplemented($unimp:expr))? ,)* }) => {
        #[repr(C)]
        #[derive(Debug, Default, Clone)]
        pub struct $name {
            $(pub $field: $field_type,)*
        }

        impl Extern for $name {
            fn get_name() -> &'static str {
                $name_c
            }

            fn get_field<T: Sized + 'static>(&self, offset: usize) -> ExternValue<T> {
                let ptr = self as *const _ as *const u8;

                match offset {
                    $($field_offset => {
                        $(
                            if $unimp {
                                return ExternValue::Unimplemented;
                            }
                        )*
                        if std::any::TypeId::of::<T>() == std::any::TypeId::of::<$field_type>() {
                            unsafe {
                                let ptr = ptr.add(std::mem::offset_of!(Self, $field)) as *const T;
                                ExternValue::Value(ptr.read())
                            }
                        } else {
                            ExternValue::InvalidType(concat!(stringify!($field), ": ", stringify!($field_type)))
                        }
                    })*
                    _ => ExternValue::FieldNotFound
                }
            }

            fn get_field_name(offset: usize) -> Option<&'static str> {
                match offset {
                    $($field_offset => Some(stringify!($field)),)*
                    _ => None
                }
            }
        }
    };

}

extern_struct! {
    struct Frame("frame") {
        0x00 => unk00: f32 > unimplemented(false),
        0x04 => unk04: f32 > unimplemented(false),
        0x0c => unk0c: f32 > unimplemented(true),
        0x10 => unk10: f32 > unimplemented(true),
        0x14 => unk14: f32 > unimplemented(true),
        0x1c => unk1c: f32 > unimplemented(false),
        0x20 => unk20: f32 > unimplemented(true),
        0x24 => unk24: f32 > unimplemented(true),
        0x28 => unk28: f32 > unimplemented(true),
        0x2c => unk2c: f32 > unimplemented(true),
        0x40 => unk40: f32 > unimplemented(true),
        0x70 => unk70: f32 > unimplemented(true),
        0x78 => unk78: u64 > unimplemented(true),
        0x80 => unk80: u64 > unimplemented(true),
        0x88 => unk88: u64 > unimplemented(true),
        0x90 => unk90: u64 > unimplemented(true),
        0x98 => unk98: u64 > unimplemented(true),
        0xa0 => unka0: u64 > unimplemented(true),
        0xa8 => specular_lobe_lookup: TextureView,
        0xb0 => specular_lobe_3d_lookup: TextureView,
        0xb8 => specular_tint_lookup: TextureView,
        0xc0 => iridescence_lookup: TextureView,
        0xd0 => unkd0: Vec4 > unimplemented(true),
        0x150 => unk150: Vec4 > unimplemented(true),
        0x160 => unk160: Vec4 > unimplemented(true),
        0x170 => unk170: Vec4 > unimplemented(true),
        0x180 => unk180: Vec4 > unimplemented(true),
        0x190 => unk190: f32 > unimplemented(true),
        0x194 => unk194: f32 > unimplemented(true),
        0x1a0 => unk1a0: Vec4 > unimplemented(false),
        0x1b0 => unk1b0: Vec4 > unimplemented(false),
        0x1e0 => unk1e0: u64 > unimplemented(true),
        0x1e8 => unk1e8: u64 > unimplemented(true),
        0x1f0 => unk1f0: u64 > unimplemented(true),
    }
}

extern_struct! {
    struct View("view") {
        0x00 => resolution_width: f32,
        0x04 => resolution_height: f32,
        0x10 => view_miscellaneous: Vec4 > unimplemented(false),
        0x20 => position: Vec4,
        // TODO(cohae): Used for shadow generation it seems
        0x30 => unk30: Vec4 > unimplemented(false),
        0x40 => world_to_camera: Mat4,
        0x80 => camera_to_projective: Mat4,
        0xc0 => camera_to_world: Mat4,
        0x100 => projective_to_camera: Mat4,
        0x140 => world_to_projective: Mat4,
        0x180 => projective_to_world: Mat4,
        0x1c0 => target_pixel_to_world: Mat4,
        0x200 => target_pixel_to_camera: Mat4,
        0x240 => unk240: Mat4 > unimplemented(true),
        0x280 => combined_tptoc_wtoc: Mat4,
        0x2c0 => unk2c0: Mat4 > unimplemented(true),
    }
}

extern_struct! {
    struct Deferred("deferred") {
        0x00 => unk00: Vec4 > unimplemented(false),
        0x10 => unk10: Vec4 > unimplemented(true),
        0x20 => unk20: Vec4 > unimplemented(true),
        0x30 => unk30: f32 > unimplemented(true),
        0x38 => deferred_depth: TextureView,
        0x48 => deferred_rt0: TextureView,
        0x50 => deferred_rt1: TextureView,
        0x58 => deferred_rt2: TextureView,
        0x60 => light_diffuse: TextureView,
        0x68 => light_specular: TextureView,
        0x70 => light_ibl_specular: TextureView,
        0x78 => unk78: TextureView > unimplemented(true),
        0x80 => unk80: TextureView > unimplemented(true),
        0x88 => unk88: TextureView > unimplemented(true),
        0x90 => unk90: TextureView > unimplemented(true),
        0x98 => unk98: TextureView > unimplemented(true),
    }
}

extern_struct! {
    struct DeferredLight("deferred_light") {
        0x40 => unk40: Mat4 > unimplemented(false),
        0x80 => unk80: Mat4 > unimplemented(false),
        0xc0 => unkc0: Vec4 > unimplemented(false),
        0xd0 => unkd0: Vec4 > unimplemented(false),
        0xe0 => unke0: Vec4 > unimplemented(false),
        0xf0 => unkf0: Vec4 > unimplemented(false),
        0x100 => unk100: Vec4 > unimplemented(false),
        0x110 => unk110: f32 > unimplemented(false),
        0x114 => unk114: f32 > unimplemented(false),
        0x118 => unk118: f32 > unimplemented(false),
        0x11c => unk11c: f32 > unimplemented(false),
        0x120 => unk120: f32 > unimplemented(false),
    }
}

extern_struct! {
    struct Transparent("transparent") {
        0x00 => unk00: TextureView > unimplemented(false), // atmos_ss_far_lookup(_low_res)
        0x08 => unk08: TextureView > unimplemented(false), // atmos_ss_far_lookup_downsampled
        0x10 => unk10: TextureView > unimplemented(false), // atmos_ss_near_lookup(_low_res)
        0x18 => unk18: TextureView > unimplemented(false), // atmos_ss_near_lookup_downsampled
        0x20 => unk20: TextureView > unimplemented(false),
        0x28 => unk28: TextureView > unimplemented(false),
        0x30 => unk30: TextureView > unimplemented(false),
        0x38 => unk38: TextureView > unimplemented(false),
        0x40 => unk40: TextureView > unimplemented(false),
        0x48 => unk48: TextureView > unimplemented(false),
        0x50 => unk50: TextureView > unimplemented(false),
        0x58 => unk58: TextureView > unimplemented(false),
        0x60 => unk60: TextureView > unimplemented(false),
        0x70 => unk70: Vec4 > unimplemented(true),
        0x80 => unk80: Vec4 > unimplemented(true),
        0x90 => unk90: Vec4 > unimplemented(true),
        0xa0 => unka0: Vec4 > unimplemented(true),
        0xb0 => unkb0: Vec4 > unimplemented(true),
    }
}

extern_struct! {
    struct SimpleGeometry("simple_geometry") {
        0x00 => transform: Mat4,
    }
}

extern_struct! {
    struct Decal("decal") {
        0x00 => unk00: TextureView > unimplemented(true),
        0x08 => unk08: TextureView, // rt1 copy
        0x10 => unk10: Vec4 > unimplemented(true),
        0x20 => unk20: Vec4 > unimplemented(true),
    }
}

extern_struct! {
    struct RigidModel("rigid_model") {
        0x00 => mesh_to_world: Mat4,
        0x40 => position_scale: Vec4,
        0x50 => position_offset: Vec4,
        0x60 => texcoord0_scale_offset: Vec4,
        0x70 => dynamic_sh_ao_values: Vec4,
    }
}

#[test]
fn test_externs() {
    let deferred = Deferred {
        unk00: Vec4::new(1.0, 2.0, 3.0, 4.0),
        ..Default::default()
    };

    assert_eq!(deferred.get_field::<Vec4>(0x00), ExternValue::Unimplemented);

    let view = View {
        resolution_width: 1920.0,
        resolution_height: 1080.0,
        ..Default::default()
    };

    assert_eq!(view.get_field::<f32>(0x00), ExternValue::Value(1920.0));
    assert_eq!(view.get_field::<f32>(0x04), ExternValue::Value(1080.0));
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[binread]
#[br(repr(u8))]
pub enum TfxExtern {
    None = 0,
    Frame = 1,
    View = 2,
    Deferred = 3,
    DeferredLight = 4,
    DeferredUberLight = 5,
    DeferredShadow = 6,
    Atmosphere = 7,
    RigidModel = 8,
    EditorMesh = 9,
    EditorMeshMaterial = 10,
    EditorDecal = 11,
    EditorTerrain = 12,
    EditorTerrainPatch = 13,
    EditorTerrainDebug = 14,
    SimpleGeometry = 15,
    UiFont = 16,
    CuiView = 17,
    CuiObject = 18,
    CuiBitmap = 19,
    CuiVideo = 20,
    CuiStandard = 21,
    CuiHud = 22,
    CuiScreenspaceBoxes = 23,
    TextureVisualizer = 24,
    Generic = 25,
    Particle = 26,
    ParticleDebug = 27,
    GearDyeVisualizationMode = 28,
    ScreenArea = 29,
    Mlaa = 30,
    Msaa = 31,
    Hdao = 32,
    DownsampleTextureGeneric = 33,
    DownsampleDepth = 34,
    Ssao = 35,
    VolumetricObscurance = 36,
    Postprocess = 37,
    TextureSet = 38,
    Transparent = 39,
    Vignette = 40,
    GlobalLighting = 41,
    ShadowMask = 42,
    ObjectEffect = 43,
    Decal = 44,
    DecalSetTransform = 45,
    DynamicDecal = 46,
    DecoratorWind = 47,
    TextureCameraLighting = 48,
    VolumeFog = 49,
    Fxaa = 50,
    Smaa = 51,
    Letterbox = 52,
    DepthOfField = 53,
    PostprocessInitialDownsample = 54,
    CopyDepth = 55,
    DisplacementMotionBlur = 56,
    DebugShader = 57,
    MinmaxDepth = 58,
    SdsmBiasAndScale = 59,
    SdsmBiasAndScaleTextures = 60,
    ComputeShadowMapData = 61,
    ComputeLocalLightShadowMapData = 62,
    BilateralUpsample = 63,
    HealthOverlay = 64,
    LightProbeDominantLight = 65,
    LightProbeLightInstance = 66,
    Water = 67,
    LensFlare = 68,
    ScreenShader = 69,
    Scaler = 70,
    GammaControl = 71,
    SpeedtreePlacements = 72,
    Reticle = 73,
    Distortion = 74,
    WaterDebug = 75,
    ScreenAreaInput = 76,
    WaterDepthPrepass = 77,
    OverheadVisibilityMap = 78,
    ParticleCompute = 79,
    CubemapFiltering = 80,
    ParticleFastpath = 81,
    VolumetricsPass = 82,
    TemporalReprojection = 83,
    FxaaCompute = 84,
    VbCopyCompute = 85,
    UberDepth = 86,
    GearDye = 87,
    Cubemaps = 88,
    ShadowBlendWithPrevious = 89,
    DebugShadingOutput = 90,
    Ssao3d = 91,
    WaterDisplacement = 92,
    PatternBlending = 93,
    UiHdrTransform = 94,
    PlayerCenteredCascadedGrid = 95,
    SoftDeform = 96,
}

pub struct TfxExpressionError {
    pub error_type: TfxExpressionErrorType,
    pub repeat_count: usize,
    // pub occurences: FxHashSet<TagHash>,
}

pub enum TfxExpressionErrorType {
    Unimplemented { field_offset: usize },
    InvalidType(&'static str),
    ExternNotSet(&'static str),
}
