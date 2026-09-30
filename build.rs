//! Builds the C++ shim around libtorrent-rasterbar (the engine qBittorrent uses).

fn main() {
    let lt = pkg_config::Config::new()
        .atleast_version("2.0")
        .probe("libtorrent-rasterbar")
        .expect("libtorrent-rasterbar 2.x and its pkg-config file are needed (pacman -S libtorrent-rasterbar boost)");
    let mut build = cxx_build::bridge("src/engine/ffi.rs");
    build
        .file("engine/shim.cpp")
        .std("c++17")
        .flag_if_supported("-Wno-deprecated-declarations")
        // cxx's generated Vec constructors trip a GCC false positive.
        .flag_if_supported("-Wno-maybe-uninitialized");
    for path in &lt.include_paths {
        build.include(path);
    }
    for (key, value) in &lt.defines {
        build.define(key, value.as_deref());
    }
    build.compile("torrents-shim");
    println!("cargo:rerun-if-changed=engine/shim.cpp");
    println!("cargo:rerun-if-changed=engine/shim.h");
    println!("cargo:rerun-if-changed=src/engine/ffi.rs");
}
