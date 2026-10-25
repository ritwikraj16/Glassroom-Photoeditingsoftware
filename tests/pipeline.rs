use std::path::PathBuf;

// Integration smoke: import, develop, export — via library modules by including paths.
// We test through the binary crate by duplicating minimal calls isn't ideal;
// instead use std process after building helpers.
#[test]
fn demo_jpegs_exist() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("demo-photos");
    let n = std::fs::read_dir(&dir).unwrap().filter(|e| {
        e.as_ref().ok().and_then(|e| e.path().extension().map(|x| x == "jpg")).unwrap_or(false)
    }).count();
    assert!(n >= 5, "expected demo JPEGs");
}
