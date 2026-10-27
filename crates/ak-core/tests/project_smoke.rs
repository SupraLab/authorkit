use ak_core::{NoteKind, Project};
use std::path::PathBuf;

#[test]
fn create_open_link_roundtrip() {
    let dir = std::env::temp_dir().join(format!("ak-test-{}", uuid::Uuid::new_v4()));
    let _ = std::fs::remove_dir_all(&dir);

    let mut project = Project::create(&dir, "Test Novel", "Auteur").expect("create");
    assert!(dir.join("authorkit.toml").exists());
    assert!(!project.manuscript.scenes.is_empty());

    let note = project
        .notes
        .create(&dir, NoteKind::Character, "Alice")
        .expect("note")
        .meta
        .id
        .clone();

    let scene_id = project.manuscript.ordered_scenes()[0].meta.id.clone();
    project
        .manuscript
        .link_note(&scene_id, &note, &dir)
        .expect("link");

    let reopened = Project::open(&dir).expect("open");
    let scene = reopened.manuscript.get_scene(&scene_id).unwrap();
    assert!(scene.meta.links.contains(&note));
    assert!(reopened.notes.get(&note).is_some());

    let _ = std::fs::remove_dir_all(&dir);
    let _: PathBuf = dir;
}
