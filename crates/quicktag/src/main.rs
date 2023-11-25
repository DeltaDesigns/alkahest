mod gui;
mod packages;
mod references;
mod scanner;
mod tagtypes;
mod text;

use std::sync::Arc;

use clap::Parser;
use destiny_pkg::{PackageManager, PackageVersion};
use eframe::egui_wgpu::WgpuConfiguration;
use eframe::{wgpu, IconData};
use env_logger::Env;
use log::info;
use packages::PACKAGE_MANAGER;

use crate::{gui::QuickTagApp, packages::package_manager};

#[derive(clap::Parser, Debug)]
#[command(author, version, about, long_about = None, disable_version_flag(true))]
struct Args {
    /// Path to packages directory
    packages_path: String,

    /// Game version for the specified packages directory
    #[arg(short, value_enum)]
    version: PackageVersion,
}

fn main() -> eframe::Result<()> {
    env_logger::Builder::from_env(
        Env::default().default_filter_or("info,wgpu_core=warn,naga=warn"),
    )
    .init();
    let args = Args::parse();

    info!("Initializing package manager");
    let pm = PackageManager::new(args.packages_path, args.version).unwrap();

    *PACKAGE_MANAGER.write() = Some(Arc::new(pm));

    let native_options = eframe::NativeOptions {
        initial_window_size: Some([400.0, 300.0].into()),
        min_window_size: Some([300.0, 220.0].into()),
        icon_data: Some(
            IconData::try_from_png_bytes(include_bytes!("../quicktag.png"))
                .expect("Failed to load icon"),
        ),
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: WgpuConfiguration {
            supported_backends: wgpu::Backends::PRIMARY,
            device_descriptor: Arc::new(|_adapter| wgpu::DeviceDescriptor {
                features: wgpu::Features::TEXTURE_COMPRESSION_BC
                    | wgpu::Features::TEXTURE_BINDING_ARRAY,
                limits: wgpu::Limits::downlevel_defaults(),
                ..Default::default()
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    eframe::run_native(
        "QuickTag",
        native_options,
        Box::new(|cc| Box::new(QuickTagApp::new(cc, package_manager().version))),
    )
}
