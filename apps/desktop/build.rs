fn main() {
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/brand/authorkit.ico");
        res.set("ProductName", "AuthorKit");
        res.set("FileDescription", "AuthorKit — local-first writing studio");
        res.set(
            "LegalCopyright",
            "Copyright (c) 2026 Julien Moulin / SupraLab",
        );
        if let Err(e) = res.compile() {
            // Don't fail hosts without a resource compiler.
            println!("cargo:warning=winres: {e}");
        }
    }
}
