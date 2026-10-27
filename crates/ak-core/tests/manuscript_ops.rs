use ak_core::{
    list_versions, load_version, restore_version_preserving_current, save_version, NoteKind,
    Project,
};

fn create_project(title: &str) -> (tempfile::TempDir, Project) {
    let dir = tempfile::tempdir().expect("tempdir");
    let project = Project::create(dir.path(), title, "Auteur").expect("create");
    (dir, project)
}

#[test]
fn rename_link_move_and_word_counts() {
    let (dir, mut p) = create_project("Ops");
    let root = dir.path();

    // Default: Act I / Chapter 1 / Opening scene
    let act1 = p.manuscript.index.acts[0].id.clone();
    let s_open = p.manuscript.ordered_scenes()[0].meta.id.clone();

    let act2 = p.manuscript.add_act("Acte II");
    let ch2 = p.manuscript.add_chapter(&act2, "Chapitre A").unwrap();
    let ch3 = p.manuscript.add_chapter(&act2, "Chapitre B").unwrap();
    let s2 = p
        .manuscript
        .add_scene(root, &act2, &ch2, "Scène Alpha")
        .unwrap()
        .meta
        .id
        .clone();
    let s3 = p
        .manuscript
        .add_scene(root, &act2, &ch2, "Scène Beta")
        .unwrap()
        .meta
        .id
        .clone();

    p.manuscript.rename_act(&act2, "Acte Deux", root).unwrap();
    p.manuscript
        .rename_chapter(&act2, &ch2, "Chapitre Alpha", root)
        .unwrap();
    p.manuscript.rename_scene(&s2, "Alpha renommée").unwrap();

    assert_eq!(
        p.manuscript
            .index
            .acts
            .iter()
            .find(|a| a.id == act2)
            .unwrap()
            .title,
        "Acte Deux"
    );
    assert_eq!(
        p.manuscript
            .index
            .acts
            .iter()
            .find(|a| a.id == act2)
            .unwrap()
            .chapters
            .iter()
            .find(|c| c.id == ch2)
            .unwrap()
            .title,
        "Chapitre Alpha"
    );
    assert_eq!(
        p.manuscript.get_scene(&s2).unwrap().meta.title,
        "Alpha renommée"
    );

    let note_id = p
        .notes
        .create(root, NoteKind::Character, "Héroïne")
        .unwrap()
        .meta
        .id
        .clone();
    p.manuscript.link_note(&s2, &note_id, root).unwrap();
    assert!(p
        .manuscript
        .get_scene(&s2)
        .unwrap()
        .meta
        .links
        .contains(&note_id));
    p.manuscript.unlink_note(&s2, &note_id, root).unwrap();
    assert!(!p
        .manuscript
        .get_scene(&s2)
        .unwrap()
        .meta
        .links
        .contains(&note_id));

    // Move scene across chapters within act2: Beta → "Chapitre B" at index 0
    p.manuscript.move_scene(&s3, &act2, &ch3, 0, root).unwrap();
    let act2_ref = p
        .manuscript
        .index
        .acts
        .iter()
        .find(|a| a.id == act2)
        .unwrap();
    assert!(!act2_ref
        .chapters
        .iter()
        .find(|c| c.id == ch2)
        .unwrap()
        .scene_ids
        .contains(&s3));
    assert_eq!(
        act2_ref
            .chapters
            .iter()
            .find(|c| c.id == ch3)
            .unwrap()
            .scene_ids,
        vec![s3.clone()]
    );
    assert_eq!(p.manuscript.get_scene(&s3).unwrap().chapter_id, ch3);

    // Move chapter to act1
    p.manuscript
        .move_chapter(&act2, &ch3, &act1, 1, root)
        .unwrap();
    let act1_ref = p
        .manuscript
        .index
        .acts
        .iter()
        .find(|a| a.id == act1)
        .unwrap();
    assert!(act1_ref.chapters.iter().any(|c| c.id == ch3));
    assert_eq!(p.manuscript.get_scene(&s3).unwrap().act_id, act1);
    assert!(!p
        .manuscript
        .index
        .acts
        .iter()
        .find(|a| a.id == act2)
        .unwrap()
        .chapters
        .iter()
        .any(|c| c.id == ch3));

    // Reorder acts: act2 → index 0
    p.manuscript.move_act(&act2, 0, root).unwrap();
    assert_eq!(p.manuscript.index.acts[0].id, act2);
    assert_eq!(p.manuscript.index.acts[1].id, act1);

    // Word counts: body text on opening scene
    {
        let scene = p.manuscript.get_scene_mut(&s_open).unwrap();
        scene.body = "un deux trois quatre".into();
        let scene = scene.clone();
        p.manuscript.save_scene(&scene).unwrap();
    }
    assert_eq!(p.manuscript.get_scene(&s_open).unwrap().word_count(), 4);
    assert_eq!(p.manuscript.total_words(), 4);

    let ordered: Vec<_> = p
        .manuscript
        .ordered_scenes()
        .iter()
        .map(|s| s.meta.id.clone())
        .collect();
    // After moves: act2 first (ch2 with s2 only), then act1 (ch1 with s_open, ch3 with s3)
    assert_eq!(ordered, vec![s2.clone(), s_open.clone(), s3.clone()]);

    // Persist + reopen
    let reopened = Project::open(root).unwrap();
    assert_eq!(reopened.manuscript.index.acts[0].title, "Acte Deux");
    assert_eq!(
        reopened
            .manuscript
            .ordered_scenes()
            .iter()
            .map(|s| s.meta.id.clone())
            .collect::<Vec<_>>(),
        ordered
    );
    assert_eq!(reopened.manuscript.total_words(), 4);
}

#[test]
fn versions_save_list_load_and_restore() {
    let (dir, p) = create_project("Versions");
    let root = dir.path();
    let sid = p.manuscript.ordered_scenes()[0].meta.id.clone();

    let v1 = save_version(root, "scene", &sid, "Scène", "corps v1", "{}", "manuel").unwrap();
    let v2 = save_version(root, "scene", &sid, "Scène", "corps v2", "{}", "manuel").unwrap();

    let listed = list_versions(root, "scene", &sid).unwrap();
    assert!(listed.len() >= 2);
    assert!(listed.iter().any(|v| v.id == v1.id));
    assert!(listed.iter().any(|v| v.id == v2.id));
    // Newest first
    assert_eq!(listed[0].id, v2.id);

    let loaded = load_version(root, "scene", &sid, &v1.id).unwrap();
    assert_eq!(loaded.body, "corps v1");

    let restored = restore_version_preserving_current(
        root,
        "scene",
        &sid,
        &v1.id,
        "Scène",
        "corps courant",
        "{}",
    )
    .unwrap();
    assert_eq!(restored.body, "corps v1");
    let after = list_versions(root, "scene", &sid).unwrap();
    assert!(after.iter().any(|v| v.reason == "avant restauration"));
    assert!(after.iter().any(|v| v.body == "corps courant"));
}
