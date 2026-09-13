fn main() {
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    std::fs::write(out.join("generated.rs"), "pub fn generated(x: bool) -> u8 { if x { 1 } else { 2 } }").unwrap();
}
