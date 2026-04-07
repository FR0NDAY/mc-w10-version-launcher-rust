fn main() {
    println!("cargo:rerun-if-changed=mclauncher.rc");
    println!("cargo:rerun-if-changed=mclauncher.manifest");
    embed_resource::compile("mclauncher.rc", embed_resource::NONE)
        .manifest_required()
        .expect("Failed to embed resources (missing Windows SDK rc.exe?)");
}
