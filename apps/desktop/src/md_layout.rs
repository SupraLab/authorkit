//! Lightweight markdown highlighter for the Visual editor (`TextEdit::layouter`).
//! Markers stay in the buffer (cursor-correct) but are hidden; content is styled.

use egui::{text::LayoutJob, Color32, FontFamily, FontId, TextFormat, Ui};
use shadcn_egui::style::Weight;
use shadcn_egui::theme::family_for;

/// Gutter (points) reserved before quote text for the painted left border.
pub const QUOTE_GUTTER: f32 = 14.0;

/// Character ranges (inclusive start, exclusive end) of blockquote lines.
#[derive(Debug, Clone, Default)]
pub struct MarkdownExtras {
    pub quote_ranges: Vec<(usize, usize)>,
}

/// Build a `LayoutJob` that styles common markdown while keeping 1:1 char mapping.
pub fn highlight_markdown(text: &str, ui: &Ui, wrap_width: f32, body_size: f32) -> LayoutJob {
    highlight_markdown_ex(text, ui, wrap_width, body_size).0
}

/// Same as [`highlight_markdown`], plus quote ranges for painted decorations.
pub fn highlight_markdown_ex(
    text: &str,
    ui: &Ui,
    wrap_width: f32,
    body_size: f32,
) -> (LayoutJob, MarkdownExtras) {
    let pal = shadcn_egui::Theme::current(ui.ctx()).palette;
    let fg = pal.foreground;
    let muted = pal.muted_foreground;

    let body = FontId::new(body_size, family_for(ui.ctx(), Weight::Normal));
    let bold = FontId::new(body_size, family_for(ui.ctx(), Weight::Bold));
    let mono = FontId::monospace(body_size - 1.0);
    let marker_font = FontId::new(0.01, body.family.clone());

    let heading_fonts: [FontId; 6] = [
        FontId::new(body_size + 12.0, family_for(ui.ctx(), Weight::Bold)),
        FontId::new(body_size + 9.0, family_for(ui.ctx(), Weight::Bold)),
        FontId::new(body_size + 6.0, family_for(ui.ctx(), Weight::Semibold)),
        FontId::new(body_size + 4.0, family_for(ui.ctx(), Weight::Semibold)),
        FontId::new(body_size + 2.0, family_for(ui.ctx(), Weight::Semibold)),
        FontId::new(body_size + 1.0, family_for(ui.ctx(), Weight::Semibold)),
    ];

    let mut job = LayoutJob {
        wrap: egui::text::TextWrapping {
            max_width: wrap_width,
            ..Default::default()
        },
        break_on_newline: true,
        ..Default::default()
    };
    let mut extras = MarkdownExtras::default();

    if text.is_empty() {
        append(&mut job, "", fg, &body, false);
        return (job, extras);
    }

    let mut char_offset = 0usize;
    for line in text.split_inclusive('\n') {
        let line_chars = line.chars().count();
        highlight_line(
            line,
            &mut job,
            &body,
            &bold,
            &heading_fonts,
            &mono,
            &marker_font,
            fg,
            muted,
            char_offset,
            &mut extras,
        );
        char_offset += line_chars;
    }

    (job, extras)
}

/// Split `* item`, `- item` or `+ item` into indent, marker and content.
/// The marker keeps the bullet character and the spaces that follow it, so a
/// visual stand-in can replace it without moving the cursor.
pub fn unordered_item(line: &str) -> Option<(&str, &str, &str)> {
    if is_thematic_break(line) {
        return None;
    }
    let indent_chars = line.chars().take_while(|c| *c == ' ').count();
    let indent_bytes = line
        .char_indices()
        .nth(indent_chars)
        .map(|(i, _)| i)
        .unwrap_or(line.len());
    let rest = &line[indent_bytes..];
    let mut chars = rest.chars();
    let mark = chars.next()?;
    if !matches!(mark, '*' | '-' | '+') {
        return None;
    }
    if chars.next() != Some(' ') {
        return None;
    }
    let extra_spaces = chars.take_while(|c| *c == ' ').count();
    let marker_bytes = mark.len_utf8() + 1 + extra_spaces;
    Some((
        &line[..indent_bytes],
        &rest[..marker_bytes],
        &rest[marker_bytes..],
    ))
}

/// Same character count as `marker`, drawn as a bullet plus spaces.
fn bullet_stand_in(marker: &str) -> String {
    let mut shown = String::from("•");
    for _ in 1..marker.chars().count() {
        shown.push(' ');
    }
    shown
}

fn is_thematic_break(line: &str) -> bool {
    let marks: Vec<char> = line.chars().filter(|c| !c.is_whitespace()).collect();
    marks.len() >= 3 && matches!(marks[0], '*' | '-' | '_') && marks.iter().all(|c| *c == marks[0])
}

fn heading_prefix(trim: &str) -> Option<(u8, &str, &str)> {
    let bytes = trim.as_bytes();
    let mut n = 0usize;
    while n < bytes.len() && bytes[n] == b'#' {
        n += 1;
    }
    if n == 0 || n > 6 {
        return None;
    }
    if bytes.get(n) != Some(&b' ') {
        return None;
    }
    let prefix = &trim[..n + 1];
    let rest = &trim[n + 1..];
    Some((n as u8, prefix, rest))
}

#[allow(clippy::too_many_arguments)]
fn highlight_line(
    line: &str,
    job: &mut LayoutJob,
    body: &FontId,
    bold: &FontId,
    headings: &[FontId; 6],
    mono: &FontId,
    marker_font: &FontId,
    fg: Color32,
    muted: Color32,
    char_offset: usize,
    extras: &mut MarkdownExtras,
) {
    let trim = line.trim_end_matches('\n');
    let newline = if line.ends_with('\n') { "\n" } else { "" };
    let line_chars = line.chars().count();

    if let Some((level, prefix, rest)) = heading_prefix(trim) {
        append_marker(job, prefix, marker_font);
        let font = &headings[(level as usize).saturating_sub(1)];
        append(job, rest, fg, font, false);
        append(job, newline, fg, body, false);
        return;
    }
    if let Some((indent, marker, rest)) = unordered_item(trim) {
        append(job, indent, fg, body, false);
        append(job, &bullet_stand_in(marker), fg, body, false);
        style_inline(rest, job, body, bold, mono, marker_font, fg, false);
        append(job, newline, fg, body, false);
        return;
    }
    if let Some(rest) = trim.strip_prefix("> ") {
        extras
            .quote_ranges
            .push((char_offset, char_offset + line_chars));
        // Same char count as "> " so cursors stay aligned with the buffer, but draw
        // spaces so selection recoloring cannot reveal a literal chevron.
        // `leading_space` reserves a gutter for the painted left border.
        job.append(
            "  ",
            QUOTE_GUTTER,
            TextFormat {
                font_id: body.clone(),
                color: Color32::TRANSPARENT,
                ..Default::default()
            },
        );
        style_inline(rest, job, body, bold, mono, marker_font, muted, true);
        append(job, newline, fg, body, false);
        return;
    }

    style_inline(trim, job, body, bold, mono, marker_font, fg, false);
    append(job, newline, fg, body, false);
}

fn style_inline(
    text: &str,
    job: &mut LayoutJob,
    body: &FontId,
    bold: &FontId,
    mono: &FontId,
    marker_font: &FontId,
    fg: Color32,
    force_italics: bool,
) {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] == '*' && i + 1 < chars.len() && chars[i + 1] == '*' {
            if let Some(end) = find_delim(&chars, i + 2, &['*', '*']) {
                append_marker(job, "**", marker_font);
                let inner: String = chars[i + 2..end].iter().collect();
                append(job, &inner, fg, bold, force_italics);
                append_marker(job, "**", marker_font);
                i = end + 2;
                continue;
            }
        }
        if chars[i] == '*' {
            if let Some(end) = find_delim(&chars, i + 1, &['*']) {
                if !(end + 1 < chars.len() && chars[end + 1] == '*') {
                    append_marker(job, "*", marker_font);
                    let inner: String = chars[i + 1..end].iter().collect();
                    append(job, &inner, fg, body, true);
                    append_marker(job, "*", marker_font);
                    i = end + 1;
                    continue;
                }
            }
        }
        if chars[i] == '`' {
            if let Some(end) = find_delim(&chars, i + 1, &['`']) {
                append_marker(job, "`", marker_font);
                let inner: String = chars[i + 1..end].iter().collect();
                append(job, &inner, fg, mono, false);
                append_marker(job, "`", marker_font);
                i = end + 1;
                continue;
            }
        }

        let start = i;
        while i < chars.len() && chars[i] != '*' && chars[i] != '`' {
            i += 1;
        }
        if i > start {
            let plain: String = chars[start..i].iter().collect();
            append(job, &plain, fg, body, force_italics);
        } else {
            let s: String = chars[i].to_string();
            append(job, &s, fg, body, force_italics);
            i += 1;
        }
    }
}

fn append_marker(job: &mut LayoutJob, text: &str, marker_font: &FontId) {
    if text.is_empty() {
        return;
    }
    // Same length as the real markers (cursor-correct) but drawn as spaces so a
    // text selection cannot recolor and reveal `**`, `#`, etc.
    let _ = marker_font;
    let hidden: String = " ".repeat(text.chars().count());
    job.append(
        &hidden,
        0.0,
        TextFormat {
            font_id: FontId::new(0.01, FontFamily::Proportional),
            color: Color32::TRANSPARENT,
            ..Default::default()
        },
    );
}

fn append(job: &mut LayoutJob, text: &str, color: Color32, font: &FontId, italics: bool) {
    if text.is_empty() {
        return;
    }
    job.append(
        text,
        0.0,
        TextFormat {
            font_id: font.clone(),
            color,
            italics,
            ..Default::default()
        },
    );
}

fn find_delim(chars: &[char], start: usize, delim: &[char]) -> Option<usize> {
    let mut i = start;
    while i + delim.len() <= chars.len() {
        if &chars[i..i + delim.len()] == delim {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Merge consecutive quote ranges into blocks (for painting a continuous bar).
pub fn merge_quote_blocks(ranges: &[(usize, usize)]) -> Vec<(usize, usize)> {
    if ranges.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut cur = ranges[0];
    for &(a, b) in &ranges[1..] {
        if a <= cur.1 {
            cur.1 = cur.1.max(b);
        } else {
            out.push(cur);
            cur = (a, b);
        }
    }
    out.push(cur);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unordered_item_splits_markers_and_keeps_char_count() {
        let (indent, marker, content) = unordered_item("* les flux").unwrap();
        assert_eq!(indent, "");
        assert_eq!(marker, "* ");
        assert_eq!(content, "les flux");
        assert_eq!(
            bullet_stand_in(marker).chars().count(),
            marker.chars().count()
        );

        let (indent, marker, content) = unordered_item("  - deux  espaces").unwrap();
        assert_eq!(indent, "  ");
        assert_eq!(marker, "- ");
        assert_eq!(content, "deux  espaces");

        let (_, marker, content) = unordered_item("+  item").unwrap();
        assert_eq!(marker, "+  ");
        assert_eq!(content, "item");
        assert_eq!(bullet_stand_in(marker).chars().count(), 3);

        assert!(unordered_item("**gras**").is_none());
        assert!(unordered_item("*pas d'espace").is_none());
        assert!(unordered_item("***").is_none());
        assert!(unordered_item("* * *").is_none());
        assert!(unordered_item("à côté").is_none());
    }

    #[test]
    fn heading_prefix_levels() {
        assert_eq!(heading_prefix("# Hi").map(|(l, ..)| l), Some(1));
        assert_eq!(heading_prefix("###### Hi").map(|(l, ..)| l), Some(6));
        assert_eq!(heading_prefix("####### Hi"), None);
        assert_eq!(heading_prefix("#No"), None);
    }

    #[test]
    fn merge_quotes() {
        let blocks = merge_quote_blocks(&[(0, 5), (5, 10), (20, 25)]);
        assert_eq!(blocks, vec![(0, 10), (20, 25)]);
    }
}
