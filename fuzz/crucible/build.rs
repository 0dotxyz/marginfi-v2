// declare_fuzz_program! reads the IDL at compile time without telling cargo, so a
// regenerated IDL would not trigger a rebuild on its own.
fn main() {
    println!("cargo:rerun-if-changed=idls/marginfi_program.json");
}
