use ak_core::fsutil::write_atomic;
use ak_core::{list_trash, list_versions, restore_trash_entry, save_version, Project};
use std::fs;
use std::path::PathBuf;

fn tmp() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ak-core-test-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn create_refuses_existing_project() {
    let root = tmp();
    Project::create(&root, "A", "B").unwrap();
    let err = Project::create(&root, "C", "D").unwrap_err();
    assert!(err.to_string().contains("existe déjà"));
    // Original intact
    let p = Project::open(&root).unwrap();
    assert_eq!(p.meta.title, "A");
}

#[test]
fn write_atomic_roundtrip() {
    let root = tmp();
    let path = root.join("hello.txt");
    write_atomic(&path, b"bonjour").unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "bonjour");
    write_atomic(&path, b"monde").unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "monde");
}

#[test]
fn trash_and_restore_chapter_keeps_scenes_order_and_links() {
    let root = tmp();
    let mut p = Project::create(&root, "Roman", "Auteur").unwrap();
    let act_id = p.manuscript.index.acts[0].id.clone();
    let ch_id = p.manuscript.index.acts[0].chapters[0].id.clone();
    // Ensure opening scene + two more
    let s1 = p.manuscript.ordered_scenes()[0].meta.id.clone();
    p.manuscript
        .add_scene(&root, &act_id, &ch_id, "Scène 2")
        .unwrap();
    p.manuscript
        .add_scene(&root, &act_id, &ch_id, "Scène 3")
        .unwrap();
    let note_id = p
        .notes
        .create(&root, ak_core::NoteKind::Character, "Héros")
        .unwrap()
        .meta
        .id
        .clone();
    p.manuscript.link_note(&s1, &note_id, &root).unwrap();

    let order_before: Vec<_> = p
        .manuscript
        .ordered_scenes()
        .iter()
        .map(|s| s.meta.id.clone())
        .collect();
    assert_eq!(order_before.len(), 3);

    p.manuscript.trash_chapter(&act_id, &ch_id, &root).unwrap();
    assert!(p.manuscript.ordered_scenes().is_empty());
    assert_eq!(list_trash(&root).unwrap().len(), 1);

    let trash_id = list_trash(&root).unwrap()[0].id.clone();
    restore_trash_entry(&mut p, &trash_id).unwrap();

    let order_after: Vec<_> = p
        .manuscript
        .ordered_scenes()
        .iter()
        .map(|s| s.meta.id.clone())
        .collect();
    assert_eq!(order_before, order_after);
    let restored = p.manuscript.get_scene(&s1).unwrap();
    assert!(restored.meta.links.contains(&note_id));

    // Round-trip via open
    let p2 = Project::open(&root).unwrap();
    assert_eq!(p2.manuscript.ordered_scenes().len(), 3);
    assert!(p2
        .manuscript
        .get_scene(&s1)
        .unwrap()
        .meta
        .links
        .contains(&note_id));
}

#[test]
fn versions_restore_preserves_current() {
    let root = tmp();
    let p = Project::create(&root, "V", "A").unwrap();
    let sid = p.manuscript.ordered_scenes()[0].meta.id.clone();
    save_version(&root, "scene", &sid, "Scène", "version une", "{}", "auto").unwrap();
    let v1 = list_versions(&root, "scene", &sid).unwrap()[0].id.clone();
    save_version(&root, "scene", &sid, "Scène", "version deux", "{}", "auto").unwrap();

    let restored = ak_core::restore_version_preserving_current(
        &root,
        "scene",
        &sid,
        &v1,
        "Scène",
        "courant en mémoire",
        "{}",
    )
    .unwrap();
    assert_eq!(restored.body, "version une");
    let versions = list_versions(&root, "scene", &sid).unwrap();
    assert!(versions.iter().any(|v| v.reason == "avant restauration"));
    assert!(versions.iter().any(|v| v.body == "courant en mémoire"));
}

#[test]
fn open_reports_orphan_and_broken_refs() {
    let root = tmp();
    let mut p = Project::create(&root, "D", "A").unwrap();
    let sid = p.manuscript.ordered_scenes()[0].meta.id.clone();
    // Fake broken note link
    if let Some(s) = p.manuscript.get_scene_mut(&sid) {
        s.meta.links.push("missing-note-id".into());
        let s = s.clone();
        p.manuscript.save_scene(&s).unwrap();
    }
    // Orphan: write a scene file not in index
    let orphan = root.join("manuscript/scenes/orphan-aaaaaaaa.md");
    fs::write(
        &orphan,
        "---\n{\n  \"id\": \"aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee\",\n  \"title\": \"Orpheline\",\n  \"links\": []\n}\n---\n\ncorps\n",
    )
    .unwrap();

    let opened = Project::open_with_diagnostics(&root).unwrap();
    assert!(!opened.diagnostics.is_empty());
    assert!(opened
        .diagnostics
        .items
        .iter()
        .any(|d| d.code == "orphan_scene"));
    assert!(opened
        .diagnostics
        .items
        .iter()
        .any(|d| d.code == "broken_note_link"));
}
