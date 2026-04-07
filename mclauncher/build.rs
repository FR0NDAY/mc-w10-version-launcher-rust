fn main() {
    println!("cargo:rerun-if-changed=mclauncher.rc");
    println!("cargo:rerun-if-changed=mclauncher.manifest");
    let _ = embed_resource::compile("mclauncher.rc", embed_resource::NONE);
}
