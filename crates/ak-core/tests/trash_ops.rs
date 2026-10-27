use ak_core::trash::{purge_trash_entry, TrashEntryKind};
use ak_core::{list_trash, restore_trash_entry, NoteKind, Project};

fn create_project(title: &str) -> (tempfile::TempDir, Project) {
    let dir = tempfile::tempdir().expect("tempdir");
    let project = Project::create(dir.path(), title, "Auteur").expect("create");
    (dir, project)
}

#[test]
fn trash_restore_purge_scene_chapter_act_and_note() {
    let (dir, mut p) = create_project("Corbeille");
    let root = dir.path();

    let act1 = p.manuscript.index.acts[0].id.clone();
    let s_open = p.manuscript.ordered_scenes()[0].meta.id.clone();

    // Extra structure for act/chapter trash
    let act2 = p.manuscript.add_act("Acte à jeter");
    let ch2 = p.manuscript.add_chapter(&act2, "Chapitre à jeter").unwrap();
    let s_extra = p
        .manuscript
        .add_scene(root, &act2, &ch2, "Scène à jeter")
        .unwrap()
        .meta
        .id
        .clone();
    let ch_solo = p.manuscript.add_chapter(&act1, "Chapitre solo").unwrap();
    let s_solo = p
        .manuscript
        .add_scene(root, &act1, &ch_solo, "Scène solo")
        .unwrap()
        .meta
        .id
        .clone();

    let note_id = p
        .notes
        .create(root, NoteKind::Setting, "Lieu perdu")
        .unwrap()
        .meta
        .id
        .clone();

    // --- trash scene ---
    let scene_entry = p.manuscript.trash_scene(&s_open, root).unwrap();
    assert!(matches!(scene_entry.kind, TrashEntryKind::Scene { .. }));
    assert!(p.manuscript.get_scene(&s_open).is_none());
    assert_eq!(list_trash(root).unwrap().len(), 1);

    // --- trash chapter (with its scene) ---
    let chapter_entry = p.manuscript.trash_chapter(&act1, &ch_solo, root).unwrap();
    assert!(matches!(chapter_entry.kind, TrashEntryKind::Chapter { .. }));
    assert!(p.manuscript.get_scene(&s_solo).is_none());
    assert!(!p
        .manuscript
        .index
        .acts
        .iter()
        .find(|a| a.id == act1)
        .unwrap()
        .chapters
        .iter()
        .any(|c| c.id == ch_solo));

    // --- trash act ---
    let act_entry = p.manuscript.trash_act(&act2, root).unwrap();
    assert!(matches!(act_entry.kind, TrashEntryKind::Act { .. }));
    assert!(p.manuscript.get_scene(&s_extra).is_none());
    assert!(!p.manuscript.index.acts.iter().any(|a| a.id == act2));

    // --- trash note ---
    let note_entry = p.notes.trash_note(&note_id, root).unwrap();
    assert!(matches!(note_entry.kind, TrashEntryKind::Note { .. }));
    assert!(p.notes.get(&note_id).is_none());

    let trash = list_trash(root).unwrap();
    assert_eq!(trash.len(), 4);

    // Restore scene
    restore_trash_entry(&mut p, &scene_entry.id).unwrap();
    assert!(p.manuscript.get_scene(&s_open).is_some());
    assert_eq!(list_trash(root).unwrap().len(), 3);

    // Restore chapter
    restore_trash_entry(&mut p, &chapter_entry.id).unwrap();
    assert!(p.manuscript.get_scene(&s_solo).is_some());
    assert!(p
        .manuscript
        .index
        .acts
        .iter()
        .find(|a| a.id == act1)
        .unwrap()
        .chapters
        .iter()
        .any(|c| c.id == ch_solo));

    // Restore act
    restore_trash_entry(&mut p, &act_entry.id).unwrap();
    assert!(p.manuscript.index.acts.iter().any(|a| a.id == act2));
    assert!(p.manuscript.get_scene(&s_extra).is_some());

    // Restore note
    restore_trash_entry(&mut p, &note_entry.id).unwrap();
    assert!(p.notes.get(&note_id).is_some());
    assert!(list_trash(root).unwrap().is_empty());

    // Purge: trash again then permanently delete without restore
    let purged = p.manuscript.trash_scene(&s_extra, root).unwrap();
    assert_eq!(list_trash(root).unwrap().len(), 1);
    purge_trash_entry(root, &purged.id).unwrap();
    assert!(list_trash(root).unwrap().is_empty());
    assert!(p.manuscript.get_scene(&s_extra).is_none());

    // Round-trip via open after restores
    let reopened = Project::open(root).unwrap();
    assert!(reopened.manuscript.get_scene(&s_open).is_some());
    assert!(reopened.manuscript.get_scene(&s_solo).is_some());
    assert!(reopened.notes.get(&note_id).is_some());
    assert!(reopened.manuscript.get_scene(&s_extra).is_none());
}

#[test]
fn trash_empty_list_and_purge_unknown_is_ok() {
    let (dir, _p) = create_project("Vide");
    let root = dir.path();
    assert!(list_trash(root).unwrap().is_empty());
    purge_trash_entry(root, "no-such-id").unwrap();
}
