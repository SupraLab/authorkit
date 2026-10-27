//! Open-diagnostics coverage: orphans, broken refs, unreadable files.

use ak_core::{DiagnosticSeverity, Project};
use std::path::PathBuf;

struct Tmp(PathBuf);
impl Tmp {
    fn new() -> Self {
        let d = std::env::temp_dir().join(format!("ak-diag-{}", uuid::Uuid::new_v4()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        Tmp(d)
    }
}
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn codes(opened: &ak_core::ProjectOpen) -> Vec<&str> {
    opened
        .diagnostics
        .items
        .iter()
        .map(|d| d.code.as_str())
        .collect()
}

#[test]
fn clean_project_has_no_diagnostics() {
    let t = Tmp::new();
    let _ = Project::create(&t.0, "Clean", "Auteur").unwrap();
    let opened = Project::open_with_diagnostics(&t.0).unwrap();
    assert!(opened.diagnostics.is_empty());
    assert!(!opened.diagnostics.has_errors());
}

#[test]
fn orphan_scene_broken_link_and_broken_ref() {
    let t = Tmp::new();
    let mut project = Project::create(&t.0, "Diag", "Auteur").unwrap();
    let scene_id = project.manuscript.ordered_scenes()[0].meta.id.clone();

    // Broken note link on the indexed scene.
    {
        let scene = project.manuscript.get_scene_mut(&scene_id).unwrap();
        scene.meta.links.push("missing-note-id".into());
        let scene = scene.clone();
        project.manuscript.save_scene(&scene).unwrap();
    }

    // Orphan scene file (valid frontmatter, absent from index).
    let orphan_id = uuid::Uuid::new_v4().to_string();
    let orphan_path = t.0.join("manuscript/scenes/orphan-scene.md");
    std::fs::write(
        &orphan_path,
        format!(
            "---\n{{\"id\":\"{orphan_id}\",\"title\":\"Orpheline\",\"links\":[],\"synopsis\":\"\",\"status\":\"to_write\"}}\n---\n\nCorps orphelin.\n"
        ),
    )
    .unwrap();

    // Broken scene ref: index points at a scene that has no file.
    {
        let ch = &mut project.manuscript.index.acts[0].chapters[0];
        ch.scene_ids.push("ghost-scene-id".into());
        project.manuscript.save_index(&t.0).unwrap();
    }

    let opened = Project::open_with_diagnostics(&t.0).unwrap();
    let c = codes(&opened);
    assert!(c.contains(&"orphan_scene"), "got {c:?}");
    assert!(c.contains(&"broken_note_link"), "got {c:?}");
    assert!(c.contains(&"broken_scene_ref"), "got {c:?}");
    assert!(opened.diagnostics.has_errors());

    let orphan = opened
        .diagnostics
        .items
        .iter()
        .find(|d| d.code == "orphan_scene")
        .unwrap();
    assert_eq!(orphan.severity, DiagnosticSeverity::Warning);
    assert_eq!(orphan.id.as_deref(), Some(orphan_id.as_str()));
    assert!(orphan.path.as_ref().unwrap().contains("orphan-scene.md"));

    let broken_link = opened
        .diagnostics
        .items
        .iter()
        .find(|d| d.code == "broken_note_link")
        .unwrap();
    assert_eq!(broken_link.severity, DiagnosticSeverity::Warning);
    assert_eq!(broken_link.id.as_deref(), Some("missing-note-id"));

    let broken_ref = opened
        .diagnostics
        .items
        .iter()
        .find(|d| d.code == "broken_scene_ref")
        .unwrap();
    assert_eq!(broken_ref.severity, DiagnosticSeverity::Error);
    assert_eq!(broken_ref.id.as_deref(), Some("ghost-scene-id"));
}

#[test]
fn unreadable_scene_and_note_frontmatter() {
    let t = Tmp::new();
    let _ = Project::create(&t.0, "Bad FM", "Auteur").unwrap();

    // Scene with missing frontmatter (still a .md in scenes/).
    std::fs::write(
        t.0.join("manuscript/scenes/no-frontmatter.md"),
        "Just plain prose without YAML.\n",
    )
    .unwrap();

    // Scene with unterminated frontmatter.
    std::fs::write(
        t.0.join("manuscript/scenes/open-fm.md"),
        "---\n{\"id\":\"x\",\"title\":\"Y\"}\nbody without closing fence\n",
    )
    .unwrap();

    // Scene with invalid JSON / empty id+title.
    std::fs::write(
        t.0.join("manuscript/scenes/bad-json.md"),
        "---\n{not json}\n---\n\n",
    )
    .unwrap();
    std::fs::write(
        t.0.join("manuscript/scenes/empty-id.md"),
        "---\n{\"id\":\"\",\"title\":\"Has title\"}\n---\n\n",
    )
    .unwrap();
    std::fs::write(
        t.0.join("manuscript/scenes/empty-title.md"),
        "---\n{\"id\":\"sid\",\"title\":\"\"}\n---\n\n",
    )
    .unwrap();

    // Non-.md files are ignored.
    std::fs::write(t.0.join("manuscript/scenes/readme.txt"), "ignore me\n").unwrap();

    // Unreadable note variants.
    std::fs::write(
        t.0.join("notes/characters/no-fm.md"),
        "Pas de frontmatter.\n",
    )
    .unwrap();
    std::fs::write(
        t.0.join("notes/characters/open-fm.md"),
        "---\n{\"id\":\"n1\",\"name\":\"X\",\"kind\":\"character\"}\nbody\n",
    )
    .unwrap();
    std::fs::write(
        t.0.join("notes/settings/bad-json.md"),
        "---\n{bad}\n---\n\n",
    )
    .unwrap();
    std::fs::write(
        t.0.join("notes/lore/empty-id.md"),
        "---\n{\"id\":\"\",\"name\":\"Z\",\"kind\":\"lore\"}\n---\n\n",
    )
    .unwrap();
    std::fs::write(t.0.join("notes/lore/skip.txt"), "not markdown\n").unwrap();

    let opened = Project::open_with_diagnostics(&t.0).unwrap();
    let scenes: Vec<_> = opened
        .diagnostics
        .items
        .iter()
        .filter(|d| d.code == "unreadable_scene")
        .collect();
    let notes: Vec<_> = opened
        .diagnostics
        .items
        .iter()
        .filter(|d| d.code == "unreadable_note")
        .collect();

    assert!(
        scenes.len() >= 5,
        "expected several unreadable scenes, got {}",
        scenes.len()
    );
    assert!(
        notes.len() >= 4,
        "expected several unreadable notes, got {}",
        notes.len()
    );
    assert!(opened.diagnostics.has_errors());
    assert!(scenes
        .iter()
        .all(|d| d.severity == DiagnosticSeverity::Error));
    assert!(notes
        .iter()
        .all(|d| d.severity == DiagnosticSeverity::Error));
    assert!(scenes
        .iter()
        .any(|d| d.message.contains("Frontmatter manquant")));
    assert!(scenes.iter().any(|d| d.message.contains("non terminé")));
    assert!(notes
        .iter()
        .any(|d| d.message.contains("Frontmatter de fiche manquant")));
}
