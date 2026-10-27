use ak_core::Project;
use docx_rs::*;
use std::path::Path;

use crate::blocks::{Block, Span};
use crate::error::{Error, Result};
use crate::model::{build_document, ExportOptions};

pub fn export_docx(project: &Project, out: &Path) -> Result<()> {
    export_docx_with(project, out, &ExportOptions::default())
}

/// DOCX with real Word styles: `Title`, `Heading1..6` (navigable outline),
/// bold / italic runs, `<w:br/>` for soft line breaks, optional page break
/// before each chapter.
///
/// Thematic breaks (`---`) and scene separators are centred paragraphs.
/// Title / author / language are written as visible title page content and as
/// custom document properties (the docx writer has no core-property setters).
pub fn export_docx_with(project: &Project, out: &Path, opts: &ExportOptions) -> Result<()> {
    let doc_model = build_document(project, opts);
    let meta = &doc_model.meta;

    let mut doc = Docx::new()
        .add_style(
            Style::new("Title", StyleType::Paragraph)
                .name("Title")
                .size(36)
                .bold()
                .align(AlignmentType::Center),
        )
        .add_style(
            Style::new("Author", StyleType::Paragraph)
                .name("Author")
                .italic()
                .align(AlignmentType::Center),
        )
        .custom_property("Title", meta.title.as_str())
        .custom_property("Author", meta.author.as_str())
        .custom_property("Language", meta.language.as_str());
    for level in 1..=6u8 {
        doc = doc.add_style(heading_style(level));
    }

    let mut has_title_page = false;
    if opts.title_page && !meta.title.is_empty() {
        has_title_page = true;
        doc = doc.add_paragraph(
            Paragraph::new()
                .style("Title")
                .add_run(Run::new().add_text(sanitize(&meta.title)).size(36).bold()),
        );
        if !meta.author.is_empty() {
            doc = doc.add_paragraph(
                Paragraph::new()
                    .style("Author")
                    .add_run(Run::new().add_text(sanitize(&meta.author)).italic()),
            );
        }
    }

    for (i, section) in doc_model.sections.iter().enumerate() {
        let mut page_break = opts.chapter_breaks && (i > 0 || has_title_page);
        for block in &section.blocks {
            let mut p = match block {
                Block::Heading { level, spans } => {
                    let level = (*level).clamp(1, 6);
                    paragraph_with_runs(
                        Paragraph::new().style(&format!("Heading{level}")),
                        spans,
                        true,
                    )
                }
                Block::Paragraph(spans) => paragraph_with_runs(Paragraph::new(), spans, false),
                Block::Rule => Paragraph::new()
                    .align(AlignmentType::Center)
                    .add_run(Run::new().add_text("* * *")),
                Block::SceneBreak(marker) => Paragraph::new()
                    .align(AlignmentType::Center)
                    .add_run(Run::new().add_text(sanitize(marker))),
            };
            if page_break {
                p = p.page_break_before(true);
                page_break = false;
            }
            doc = doc.add_paragraph(p);
        }
    }

    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(Error::Io)?;
    }
    let file = std::fs::File::create(out).map_err(Error::Io)?;
    doc.build()
        .pack(file)
        .map_err(|e| Error::Docx(e.to_string()))?;
    Ok(())
}

fn heading_style(level: u8) -> Style {
    // Half-points. Matches the sizes used before styles were introduced.
    let size = match level {
        1 => 32,
        2 => 28,
        _ => 24,
    };
    let mut s = Style::new(format!("Heading{level}"), StyleType::Paragraph)
        .name(format!("heading {level}"))
        .size(size)
        .bold()
        .outline_lvl(level as usize - 1);
    if level >= 5 {
        s = s.italic();
    }
    s
}

fn paragraph_with_runs(mut p: Paragraph, spans: &[Span], in_heading: bool) -> Paragraph {
    for span in spans {
        let mut run = Run::new();
        for (k, part) in sanitize(&span.text).split('\n').enumerate() {
            if k > 0 {
                run = run.add_break(BreakType::TextWrapping);
            }
            run = run.add_text(part);
        }
        // Headings are already bold via their style; explicit bold would be redundant
        // but harmless. Keep italic/bold only where the author asked for it.
        if span.bold && !in_heading {
            run = run.bold();
        }
        if span.italic {
            run = run.italic();
        }
        p = p.add_run(run);
    }
    p
}

/// Drop characters that are illegal in XML 1.0 (docx is XML).
fn sanitize(s: &str) -> String {
    s.chars()
        .filter(|c| {
            matches!(*c, '\t' | '\n' | '\r') || (*c >= ' ' && *c != '\u{FFFE}' && *c != '\u{FFFF}')
        })
        .collect()
}
