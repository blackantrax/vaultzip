// Embeds the icon and version information into the Windows executable.
// On other platforms this does nothing.
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../assets/vaultzip.ico");
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../assets/vaultzip.ico");
        res.set("ProductName", "VaultZip");
        res.set("FileDescription", "VaultZip command line archiver");
        res.set("CompanyName", "Omerta Security");
        res.set("LegalCopyright", "Copyright (c) 2026 Omerta Security");
        res.set("OriginalFilename", "vaultzip.exe");
        res.set("InternalName", "vaultzip");
        res.compile().expect("failed to embed Windows resources");
    }
}
