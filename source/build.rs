#[path = "build_support.rs"]
mod inputs;
fn main() {
    let root = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    for s in inputs::DIRECTORIES.iter().chain(inputs::FILES) {
        println!("cargo:rerun-if-changed={s}");
    }
    println!(
        "cargo:rustc-env=WOW_SOURCE_SHA256={}",
        inputs::fingerprint(&root).expect("source fingerprint")
    );
}
