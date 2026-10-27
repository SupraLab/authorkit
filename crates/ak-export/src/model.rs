//! Export options and the format-neutral document model shared by DOCX and EPUB.

use ak_core::Project;

use crate::blocks::{parse_markdown, strip_annotations, Block, Span};

/// What to put in an export. `Default` reproduces the historical behaviour
/// (all titles, chapter breaks) plus annotation stripping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportOptions {
    /// Overrides `project.meta.title` when non-blank.
    pub title: Option<String>,
    /// Overrides `project.meta.author` when non-blank.
    pub author: Option<String>,
    /// BCP 47 tag (`fr`, `en-GB`…). Overrides `project.meta.language` when valid.
    pub language: Option<String>,
    pub include_act_titles: bool,
    pub include_chapter_titles: bool,
    pub include_scene_titles: bool,
    /// DOCX: page break before each chapter. EPUB: one XHTML file per chapter
    /// (when `false`, a single content file with one `<section>` per chapter).
    pub chapter_breaks: bool,
    /// Drop `<!-- … -->` and `%% … %%` annotations from the body text.
    pub strip_annotations: bool,
    /// Title page (title + author) at the start of DOCX / EPUB.
    pub title_page: bool,
    /// Separator inserted between two scenes of a chapter when scene titles
    /// are not shown. `None` → no separator.
    pub scene_break: Option<String>,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            title: None,
            author: None,
            language: None,
            include_act_titles: true,
            include_chapter_titles: true,
            include_scene_titles: true,
            chapter_breaks: true,
            strip_annotations: true,
            title_page: true,
            scene_break: Some("* * *".into()),
        }
    }
}

/// Resolved metadata (options override project values).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meta {
    pub title: String,
    pub author: String,
    pub language: String,
}

impl ExportOptions {
    pub fn meta(&self, project: &Project) -> Meta {
        let pick = |over: &Option<String>, base: &str| match over {
            Some(s) if !s.trim().is_empty() => s.trim().to_string(),
            _ => base.trim().to_string(),
        };
        let lang = self
            .language
            .as_deref()
            .map(str::trim)
            .filter(|l| is_valid_lang(l))
            .map(str::to_string)
            .or_else(|| Some(project.meta.language.trim().to_string()).filter(|l| is_valid_lang(l)))
            .unwrap_or_else(|| "fr".into());
        Meta {
            title: pick(&self.title, &project.meta.title),
            author: pick(&self.author, &project.meta.author),
            language: lang,
        }
    }
}

/// Loose BCP 47 check: 2–8 letter primary tag, then `-` separated alnum subtags.
pub fn is_valid_lang(l: &str) -> bool {
    let mut parts = l.split('-');
    let ok_primary = parts
        .next()
        .is_some_and(|p| (2..=8).contains(&p.len()) && p.chars().all(|c| c.is_ascii_alphabetic()));
    ok_primary
        && parts.all(|p| (1..=8).contains(&p.len()) && p.chars().all(|c| c.is_ascii_alphanumeric()))
}

/// One chapter's worth of content (the unit of an EPUB file / DOCX page break).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// Label for navigation (always the real chapter title, even if headings are hidden).
    pub nav_title: String,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    pub meta: Meta,
    pub sections: Vec<Section>,
}

impl Document {
    /// All plain text in reading order (handy for tests and word counts).
    pub fn plain_text(&self) -> String {
        self.sections
            .iter()
            .flat_map(|s| s.blocks.iter())
            .map(|b| b.plain_text())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Build the document in manuscript order (acts → chapters → scenes, as in
/// `manuscript/index.json`). Scenes missing from disk are skipped.
pub fn build_document(project: &Project, opts: &ExportOptions) -> Document {
    let meta = opts.meta(project);

    // Structural heading levels depend on which titles are shown.
    let mut level = 0u8;
    let act_level = opts.include_act_titles.then(|| {
        level += 1;
        level
    });
    let chapter_level = opts.include_chapter_titles.then(|| {
        level += 1;
        level
    });
    let scene_level = opts.include_scene_titles.then(|| {
        level += 1;
        level
    });
    let body_offset = level; // body headings are nested below the deepest title

    let mut sections: Vec<Section> = Vec::new();

    for act in &project.manuscript.index.acts {
        let mut pending_act: Option<Block> = act_level.map(|l| heading(l, &act.title));
        if act.chapters.is_empty() {
            if let Some(h) = pending_act.take() {
                sections.push(Section {
                    nav_title: act.title.clone(),
                    blocks: vec![h],
                });
            }
            continue;
        }
        for ch in &act.chapters {
            let mut blocks: Vec<Block> = Vec::new();
            if let Some(h) = pending_act.take() {
                blocks.push(h);
            }
            if let Some(l) = chapter_level {
                blocks.push(heading(l, &ch.title));
            }
            let mut first_scene = true;
            for sid in &ch.scene_ids {
                let Some(scene) = project.manuscript.get_scene(sid) else {
                    continue;
                };
                if let Some(l) = scene_level {
                    blocks.push(heading(l, &scene.meta.title));
                } else if !first_scene {
                    if let Some(marker) = &opts.scene_break {
                        blocks.push(Block::SceneBreak(marker.clone()));
                    }
                }
                first_scene = false;
                let body = if opts.strip_annotations {
                    strip_annotations(&scene.body)
                } else {
                    scene.body.clone()
                };
                for b in parse_markdown(&body) {
                    blocks.push(match b {
                        Block::Heading { level, spans } => Block::Heading {
                            level: (level + body_offset).min(6),
                            spans,
                        },
                        other => other,
                    });
                }
            }
            sections.push(Section {
                nav_title: ch.title.clone(),
                blocks,
            });
        }
    }

    Document { meta, sections }
}

fn heading(level: u8, title: &str) -> Block {
    Block::Heading {
        level,
        spans: vec![Span::plain(title.trim())],
    }
}

/// Markdown scene/act text honouring `strip_annotations` (used by the Markdown exporter).
pub(crate) fn body_text(body: &str, opts: &ExportOptions) -> String {
    if opts.strip_annotations {
        strip_annotations(body)
    } else {
        body.to_string()
    }
}
