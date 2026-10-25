#[path = "../src/catalog.rs"]
mod catalog;
#[path = "../src/develop.rs"]
mod develop;

use catalog::DevelopParams;
use develop::{export_jpeg, load_image, apply_develop, for_preview};
use std::path::PathBuf;

#[test]
fn develop_and_export_jpeg() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("demo-photos/01-harbor.jpg");
    assert!(src.exists());
    let img = load_image(&src).expect("load");
    let preview = for_preview(&img, 640);
    let mut p = DevelopParams::default();
    p.exposure = 0.8;
    p.contrast = 20.0;
    p.saturation = 15.0;
    let out_img = apply_develop(&preview, &p);
    assert!(out_img.width() > 0);
    let dest = std::env::temp_dir().join("glassroom-smoke-export.jpg");
    export_jpeg(&src, &dest, &p, 90).expect("export");
    assert!(dest.exists());
    let meta = std::fs::metadata(&dest).unwrap();
    assert!(meta.len() > 1000);
    let _ = std::fs::remove_file(&dest);
}
