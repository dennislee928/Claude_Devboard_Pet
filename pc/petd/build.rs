//! Stamps Windows executables with publisher information so the file
//! properties dialog, SmartScreen prompts and the signed installer all show a
//! real product name and company instead of "Unknown publisher".

fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set("CompanyName", "DevPet Project")
            .set("ProductName", "DevPet")
            .set("FileDescription", "DevPet desk pet — shows what Claude Code is doing")
            .set("LegalCopyright", "Copyright (c) 2026 Dennis Lee. MIT licensed.")
            .set("OriginalFilename", "petd.exe")
            .set("ProductVersion", env!("CARGO_PKG_VERSION"));
        if let Err(e) = res.compile() {
            println!("cargo:warning=version resource not embedded: {e}");
        }
    }
}
