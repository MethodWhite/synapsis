fn main() {
    println!(
        "cargo:rustc-env=SYNAPSIS_VERSION={}",
        env!("CARGO_PKG_VERSION")
    );
}
