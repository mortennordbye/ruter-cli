fn main() {
    // Build scripts are compiled for the host, so the *target* OS has to come
    // from Cargo's environment rather than a cfg! check.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("set by cargo");
    let plist = format!("{manifest_dir}/macos/Info.plist");

    println!("cargo:rerun-if-changed=macos/Info.plist");
    // Give the binary a bundle identity. Core Location refuses to hand out a
    // position to a process macOS cannot name, and an unbundled CLI has no
    // identity at all unless the plist is linked into the Mach-O directly.
    println!("cargo:rustc-link-arg=-Wl,-sectcreate,__TEXT,__info_plist,{plist}");
}
