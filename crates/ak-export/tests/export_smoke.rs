use ak_core::Project;
use ak_export::{export_epub, export_markdown};

#[test]
fn export_md_and_epub() {
    let dir = std::env::temp_dir().join(format!("ak-export-{}", uuid::Uuid::new_v4()));
    let _ = std::fs::remove_dir_all(&dir);
    let project = Project::create(&dir, "Export Test", "Auteur").expect("create");

    let md = dir.join("out.md");
    let epub = dir.join("out.epub");
    export_markdown(&project, &md).expect("md");
    export_epub(&project, &epub).expect("epub");
    assert!(md.exists());
    assert!(epub.metadata().unwrap().len() > 20);

    let _ = std::fs::remove_dir_all(&dir);
}
