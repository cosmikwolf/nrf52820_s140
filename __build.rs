fn main() {
    // Tell the linker where to find memory.x
    println!("cargo:rustc-link-search=.");
    
    // Only re-run the build script when memory.x is changed
    println!("cargo:rerun-if-changed=memory.x");
}