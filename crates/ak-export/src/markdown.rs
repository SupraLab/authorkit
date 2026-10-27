use ak_core::Project;
use std::path::Path;

use crate::error::Result;
use crate::model::{body_text, ExportOptions};

pub fn export_markdown(project: &Project, out: &Path) -> Result<()> {
    export_markdown_with(project, out, &ExportOptions::default())
}

/// Markdown keeps the author's own Markdown body; options control titles,
/// metadata and annotation stripping.
pub fn export_markdown_with(project: &Project, out: &Path, opts: &ExportOptions) -> Result<()> {
    let meta = opts.meta(project);
    let mut md = String::new();
    md.push_str(&format!("# {}\n\n", meta.title));
    if !meta.author.is_empty() {
        md.push_str(&format!("*{}*\n\n", meta.author));
    }
    if !project.meta.premise.is_empty() {
        md.push_str(&format!("{}\n\n---\n\n", project.meta.premise));
    }

    for act in &project.manuscript.index.acts {
        if opts.include_act_titles {
            md.push_str(&format!("# {}\n\n", act.title));
        }
        for ch in &act.chapters {
            if opts.include_chapter_titles {
                md.push_str(&format!("## {}\n\n", ch.title));
            }
            let mut first = true;
            for sid in &ch.scene_ids {
                if let Some(scene) = project.manuscript.get_scene(sid) {
                    if opts.include_scene_titles {
                        md.push_str(&format!("### {}\n\n", scene.meta.title));
                    } else if !first {
                        if let Some(m) = &opts.scene_break {
                            md.push_str(&format!("{m}\n\n"));
                        }
                    }
                    first = false;
                    md.push_str(body_text(&scene.body, opts).trim());
                    md.push_str("\n\n");
                }
            }
        }
    }

    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(out, md)?;
    Ok(())
}
