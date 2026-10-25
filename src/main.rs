//! Glassroom — local catalog photo library + JPEG develop.
//! Copyright 2026 Glassroom contributors. Licensed under Apache-2.0.

mod app;
mod catalog;
mod develop;

use app::GlassroomApp;
use catalog::{default_catalog_path, Catalog};
use eframe::egui;
use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(i) = args.iter().position(|a| a == "--import") {
        if let Some(dir) = args.get(i + 1) {
            let catalog = Catalog::open(default_catalog_path()).expect("open catalog");
            match catalog.import_folder(&PathBuf::from(dir)) {
                Ok((n, _)) => {
                    eprintln!("Imported / linked {n} JPEG(s) from {dir}");
                    return Ok(());
                }
                Err(e) => {
                    eprintln!("Import failed: {e}");
                    std::process::exit(1);
                }
            }
        } else {
            eprintln!("Usage: glassroom --import <folder>");
            std::process::exit(2);
        }
    }

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([900.0, 600.0])
            .with_title("Glassroom"),
        ..Default::default()
    };

    eframe::run_native(
        "Glassroom",
        native_options,
        Box::new(|cc| Ok(Box::new(GlassroomApp::new(cc)))),
    )
}
