//! Lightweight markdown for chat bubbles — proper list hanging indents.

use egui::{Color32, FontId, RichText, Ui};
use shadcn_egui::style::Weight;
use shadcn_egui::theme::family_for;

use crate::theme::FONT_UI;

/// Render chat markdown visually: headings, block quotes, lists (bullet aligned to
/// first line, wrapped lines indented under the text), bold / italic / inline code.
pub fn show(ui: &mut Ui, markdown: &str, color: Color32) {
    let fonts = Fonts {
        body: FontId::new(FONT_UI, family_for(ui.ctx(), Weight::Normal)),
        bold: FontId::new(FONT_UI, family_for(ui.ctx(), Weight::Semibold)),
        heading: FontId::new(FONT_UI + 1.0, family_for(ui.ctx(), Weight::Semibold)),
        code: FontId::monospace(FONT_UI - 1.0),
    };

    let blocks = parse_blocks(markdown);

    ui.spacing_mut().item_spacing.y = 6.0;
    for (i, block) in blocks.iter().enumerate() {
        if i > 0 {
            ui.add_space(2.0);
        }
        match block {
            Block::Paragraph(text) => {
                show_rich_line(ui, text, color, &fonts, false, false);
            }
            Block::Heading(text) => {
                show_rich_line(ui, text, color, &fonts, true, false);
            }
            Block::Quote(text) => {
                let resp = egui::Frame::new()
                    .inner_margin(egui::Margin {
                        left: 10,
                        right: 0,
                        top: 1,
                        bottom: 1,
                    })
                    .show(ui, |ui| {
                        show_rich_line(ui, text, color.gamma_multiply(0.8), &fonts, false, true);
                    })
                    .response;
                let bar =
                    egui::Rect::from_min_size(resp.rect.min, egui::vec2(3.0, resp.rect.height()));
                ui.painter()
                    .rect_filled(bar, 1.5, color.gamma_multiply(0.35));
            }
            Block::List(items) => {
                ui.spacing_mut().item_spacing.y = 4.0;
                for item in items {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        // Bullet aligned to the first line of text.
                        let bullet_h = ui.fonts_mut(|f| f.row_height(&fonts.body));
                        ui.allocate_ui_with_layout(
                            egui::vec2(10.0, bullet_h),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.label(RichText::new("•").font(fonts.body.clone()).color(color));
                            },
                        );
                        // Remaining width for wrapped item text (hanging indent).
                        let w = (ui.available_width() - 4.0).max(40.0);
                        ui.allocate_ui_with_layout(
                            egui::vec2(w, ui.available_height()),
                            egui::Layout::top_down(egui::Align::LEFT),
                            |ui| {
                                ui.set_max_width(w);
                                show_rich_line(ui, item, color, &fonts, false, false);
                            },
                        );
                    });
                }
            }
        }
    }
}

struct Fonts {
    body: FontId,
    bold: FontId,
    heading: FontId,
    code: FontId,
}

#[derive(Debug, PartialEq)]
enum Block {
    Paragraph(String),
    Heading(String),
    Quote(String),
    List(Vec<String>),
}

fn parse_blocks(markdown: &str) -> Vec<Block> {
    let mut out: Vec<Block> = Vec::new();
    let mut list_buf: Vec<String> = Vec::new();
    let mut para_buf = String::new();
    let mut quote_buf = String::new();

    fn flush(buf: &mut String, out: &mut Vec<Block>, make: fn(String) -> Block) {
        let t = buf.trim();
        if !t.is_empty() {
            out.push(make(t.to_string()));
        }
        buf.clear();
    }
    fn flush_list(items: &mut Vec<String>, out: &mut Vec<Block>) {
        if !items.is_empty() {
            out.push(Block::List(std::mem::take(items)));
        }
    }
    fn join(buf: &mut String, text: &str) {
        if !buf.is_empty() {
            buf.push(' ');
        }
        buf.push_str(text);
    }

    for line in markdown.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            flush_list(&mut list_buf, &mut out);
            flush(&mut para_buf, &mut out, Block::Paragraph);
            flush(&mut quote_buf, &mut out, Block::Quote);
            continue;
        }
        if let Some(rest) = strip_quote_marker(trimmed) {
            flush_list(&mut list_buf, &mut out);
            flush(&mut para_buf, &mut out, Block::Paragraph);
            join(&mut quote_buf, rest);
            continue;
        }
        flush(&mut quote_buf, &mut out, Block::Quote);
        if let Some(h) = strip_heading_marker(trimmed) {
            flush_list(&mut list_buf, &mut out);
            flush(&mut para_buf, &mut out, Block::Paragraph);
            out.push(Block::Heading(h.to_string()));
        } else if let Some(item) = strip_list_marker(trimmed) {
            flush(&mut para_buf, &mut out, Block::Paragraph);
            list_buf.push(item.to_string());
        } else if !list_buf.is_empty() && line.starts_with([' ', '\t']) {
            if let Some(last) = list_buf.last_mut() {
                join(last, trimmed);
            }
        } else {
            flush_list(&mut list_buf, &mut out);
            join(&mut para_buf, trimmed);
        }
    }
    flush_list(&mut list_buf, &mut out);
    flush(&mut para_buf, &mut out, Block::Paragraph);
    flush(&mut quote_buf, &mut out, Block::Quote);
    out
}

fn strip_quote_marker(line: &str) -> Option<&str> {
    // Tolerate emphasis wrapped around the marker, e.g. "**> text**".
    let t = line.trim_start_matches(['*', '_']).trim_start();
    t.strip_prefix('>').map(|rest| {
        let mut r = rest.trim_start();
        while let Some(x) = r.strip_prefix('>') {
            r = x.trim_start();
        }
        r
    })
}

fn strip_heading_marker(line: &str) -> Option<&str> {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    if (1..=6).contains(&hashes) && line[hashes..].starts_with(' ') {
        Some(line[hashes..].trim())
    } else {
        None
    }
}

fn strip_list_marker(line: &str) -> Option<&str> {
    let t = line.trim_start();
    for prefix in ["- ", "* ", "• "] {
        if let Some(rest) = t.strip_prefix(prefix) {
            return Some(rest.trim());
        }
    }
    // Ordered: "1. ", "12. "
    let bytes = t.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i > 0 && t[i..].starts_with(". ") {
        return Some(t[i + 2..].trim());
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Style {
    bold: bool,
    italic: bool,
    code: bool,
}

/// Split inline markdown into styled spans (`**bold**`, `*italic*`, `_italic_`, `` `code` ``).
/// Unmatched markers are dropped so raw syntax never reaches the screen.
fn inline_spans(text: &str) -> Vec<(String, Style)> {
    let chars: Vec<char> = text.chars().collect();
    let mut spans: Vec<(String, Style)> = Vec::new();
    let mut cur = String::new();
    let mut style = Style::default();
    let push = |cur: &mut String, spans: &mut Vec<(String, Style)>, style: Style| {
        if !cur.is_empty() {
            spans.push((std::mem::take(cur), style));
        }
    };
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '`' {
            push(&mut cur, &mut spans, style);
            style.code = !style.code;
            i += 1;
            continue;
        }
        if style.code {
            cur.push(c);
            i += 1;
            continue;
        }
        if c == '*' && chars.get(i + 1) == Some(&'*') {
            push(&mut cur, &mut spans, style);
            style.bold = !style.bold;
            i += 2;
            continue;
        }
        if c == '*' {
            push(&mut cur, &mut spans, style);
            style.italic = !style.italic;
            i += 1;
            continue;
        }
        if c == '_' {
            let prev_alnum = i > 0 && chars[i - 1].is_alphanumeric();
            let next_alnum = chars.get(i + 1).is_some_and(|n| n.is_alphanumeric());
            // Only treat `_` as emphasis at word boundaries (keep snake_case intact).
            if !(prev_alnum && next_alnum) {
                push(&mut cur, &mut spans, style);
                style.italic = !style.italic;
                i += 1;
                continue;
            }
        }
        cur.push(c);
        i += 1;
    }
    push(&mut cur, &mut spans, style);
    spans
}

fn show_rich_line(
    ui: &mut Ui,
    text: &str,
    color: Color32,
    fonts: &Fonts,
    heading: bool,
    force_italic: bool,
) {
    let mut job = egui::text::LayoutJob {
        wrap: egui::text::TextWrapping {
            max_width: ui.available_width(),
            ..Default::default()
        },
        break_on_newline: true,
        ..Default::default()
    };

    for (span, st) in inline_spans(text) {
        let font = if st.code {
            fonts.code.clone()
        } else if heading {
            fonts.heading.clone()
        } else if st.bold {
            fonts.bold.clone()
        } else {
            fonts.body.clone()
        };
        job.append(
            &span,
            0.0,
            egui::TextFormat {
                font_id: font,
                color,
                italics: st.italic || force_italic,
                background: if st.code {
                    color.gamma_multiply(0.10)
                } else {
                    Color32::TRANSPARENT
                },
                ..Default::default()
            },
        );
    }

    let galley = ui.fonts_mut(|f| f.layout_job(job));
    let size = galley.size();
    let (rect, _response) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.painter().galley(rect.min, galley, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_of(s: &str) -> String {
        inline_spans(s).into_iter().map(|(t, _)| t).collect()
    }

    #[test]
    fn inline_markers_never_shown() {
        assert_eq!(
            text_of("**Ici** tout *commença* `x`"),
            "Ici tout commença x"
        );
        assert_eq!(text_of("snake_case_name"), "snake_case_name");
        assert_eq!(text_of("_doux_ ici"), "doux ici");
    }

    #[test]
    fn blocks_parse_quote_heading_list() {
        let b = parse_blocks("# Titre\n> **Ici** tout\n> commença\n\n- a\n- b\nfin");
        assert_eq!(
            b,
            vec![
                Block::Heading("Titre".into()),
                Block::Quote("**Ici** tout commença".into()),
                Block::List(vec!["a".into(), "b".into()]),
                Block::Paragraph("fin".into()),
            ]
        );
        assert_eq!(
            parse_blocks("**> Ici** tout"),
            vec![Block::Quote("Ici** tout".into())]
        );
    }
}
