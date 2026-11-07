use std::path::Path;
use wow_steamos_core::release;
fn run() -> Result<(), String> {
    let a: Vec<_> = std::env::args().collect();
    match a.get(1).map(String::as_str){
Some("verify-source") if a.len()==3=>release::source(Path::new(&a[2]))?,
Some("manifest") if a.len()==3=>println!("{}",serde_json::to_string_pretty(&release::manifest(Path::new(&a[2]))?).unwrap()),
Some("verify-binary") if a.len()==4=>release::check_manifest(Path::new(&a[2]),&release::read(Path::new(&a[3]))?)?,
Some("evidence") if a.len()==4=>{release::evidence(&release::hash_file(Path::new(&a[2]))?,&release::read(Path::new(&a[3]))?)?;println!("Evidence matches this archive. This validates a manual record, not hardware or publication.");},
_=>return Err("Use: wow-release verify-source ROOT | manifest BINARY | verify-binary BINARY MANIFEST | evidence ARCHIVE RECORD".into())}
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
