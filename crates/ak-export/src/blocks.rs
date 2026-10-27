//! Shared Markdown → structured blocks.
//!
//! A deliberately small Markdown subset, matching what the editor produces for
//! fiction: paragraphs, `*italic*` / `_italic_`, `**bold**` / `__bold__`,
//! `***both***`, ATX headings (`# …` to `###### …`) and horizontal rules
//! (`---`, `***`, `___`). Everything else is kept as literal text.
//!
//! Exporters (DOCX, EPUB, Markdown) consume [`Block`]s so that styles are real
//! and no exporter ever prints raw Markdown markers.

/// A run of text sharing the same inline style. `text` may contain `\n`
/// (a soft line break inside a paragraph).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
}

impl Span {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            bold: false,
            italic: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    /// `level` is 1..=6.
    Heading {
        level: u8,
        spans: Vec<Span>,
    },
    Paragraph(Vec<Span>),
    /// Thematic break (`---`).
    Rule,
    /// Decorative scene separator inserted by the exporter (e.g. `* * *`).
    SceneBreak(String),
}

impl Block {
    /// Concatenated plain text of the block (no styling).
    pub fn plain_text(&self) -> String {
        match self {
            Block::Heading { spans, .. } | Block::Paragraph(spans) => {
                spans.iter().map(|s| s.text.as_str()).collect()
            }
            Block::Rule => String::new(),
            Block::SceneBreak(m) => m.clone(),
        }
    }
}

/// Remove work annotations: HTML comments (`<!-- … -->`, possibly multi-line,
/// an unclosed one swallows the rest) and Obsidian-style `%% … %%`.
pub fn strip_annotations(md: &str) -> String {
    let no_html = remove_delimited(md, "<!--", "-->");
    remove_delimited(&no_html, "%%", "%%")
}

fn remove_delimited(s: &str, open: &str, close: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find(open) {
        out.push_str(&rest[..i]);
        let after = &rest[i + open.len()..];
        match after.find(close) {
            Some(j) => rest = &after[j + close.len()..],
            None => return out, // unclosed: drop the remainder
        }
    }
    out.push_str(rest);
    out
}

/// Parse Markdown into blocks. Annotations are **not** stripped here; call
/// [`strip_annotations`] first when needed.
pub fn parse_markdown(md: &str) -> Vec<Block> {
    let md = md.replace("\r\n", "\n").replace('\r', "\n");
    let mut blocks = Vec::new();
    let mut para: Vec<&str> = Vec::new();

    let flush = |para: &mut Vec<&str>, blocks: &mut Vec<Block>| {
        if para.is_empty() {
            return;
        }
        let text = para.iter().map(|l| l.trim()).collect::<Vec<_>>().join("\n");
        para.clear();
        let spans = parse_inline(&text);
        if !spans.is_empty() {
            blocks.push(Block::Paragraph(spans));
        }
    };

    for line in md.lines() {
        let t = line.trim();
        if t.is_empty() {
            flush(&mut para, &mut blocks);
        } else if let Some((level, text)) = heading(t) {
            flush(&mut para, &mut blocks);
            blocks.push(Block::Heading {
                level,
                spans: parse_inline(text),
            });
        } else if is_rule(t) {
            flush(&mut para, &mut blocks);
            blocks.push(Block::Rule);
        } else {
            para.push(line);
        }
    }
    flush(&mut para, &mut blocks);
    blocks
}

fn heading(t: &str) -> Option<(u8, &str)> {
    let hashes = t.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let rest = &t[hashes..];
    if !rest.starts_with(' ') && !rest.starts_with('\t') {
        return None;
    }
    let text = rest.trim().trim_end_matches('#').trim_end();
    Some((hashes as u8, text))
}

fn is_rule(t: &str) -> bool {
    let compact: String = t.chars().filter(|c| !c.is_whitespace()).collect();
    compact.chars().count() >= 3
        && ['-', '*', '_']
            .iter()
            .any(|m| compact.chars().all(|c| c == *m))
}

/// Parse inline emphasis. Unmatched markers stay literal.
pub fn parse_inline(s: &str) -> Vec<Span> {
    let chars: Vec<char> = s.chars().collect();
    let mut spans = Vec::new();
    inline(&chars, false, false, &mut spans);
    merge(spans)
}

fn inline(chars: &[char], bold: bool, italic: bool, out: &mut Vec<Span>) {
    let mut buf = String::new();
    let push = |buf: &mut String, out: &mut Vec<Span>| {
        if !buf.is_empty() {
            out.push(Span {
                text: std::mem::take(buf),
                bold,
                italic,
            });
        }
    };
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && i + 1 < chars.len() && is_escapable(chars[i + 1]) {
            buf.push(chars[i + 1]);
            i += 2;
            continue;
        }
        if c == '*' || c == '_' {
            let run = chars[i..].iter().take_while(|x| **x == c).count();
            let n = run.min(3);
            let prev_alnum = i > 0 && chars[i - 1].is_alphanumeric();
            let next_ok = chars.get(i + run).is_some_and(|x| !x.is_whitespace());
            let can_open = next_ok && !(c == '_' && prev_alnum);
            if can_open && run <= 3 {
                if let Some(close) = find_close(chars, i + run, c, n) {
                    push(&mut buf, out);
                    let (b, it) = match n {
                        1 => (bold, true),
                        2 => (true, italic),
                        _ => (true, true),
                    };
                    inline(&chars[i + run..close], b || bold, it || italic, out);
                    i = close + run;
                    continue;
                }
            }
            // literal run
            for _ in 0..run {
                buf.push(c);
            }
            i += run;
            continue;
        }
        buf.push(c);
        i += 1;
    }
    push(&mut buf, out);
}

fn is_escapable(c: char) -> bool {
    matches!(c, '*' | '_' | '\\' | '#' | '`' | '<' | '>')
}

/// Find the closing run of exactly `n` `marker` chars at or after `from`.
fn find_close(chars: &[char], from: usize, marker: char, n: usize) -> Option<usize> {
    let mut j = from;
    while j < chars.len() {
        if chars[j] == '\\' {
            j += 2;
            continue;
        }
        if chars[j] == marker {
            let run = chars[j..].iter().take_while(|x| **x == marker).count();
            let prev_ws = j == from || chars[j - 1].is_whitespace();
            let next_alnum = chars.get(j + run).is_some_and(|x| x.is_alphanumeric());
            if run == n && !prev_ws && !(marker == '_' && next_alnum) {
                return Some(j);
            }
            j += run;
            continue;
        }
        j += 1;
    }
    None
}

fn merge(spans: Vec<Span>) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::new();
    for s in spans {
        match out.last_mut() {
            Some(last) if last.bold == s.bold && last.italic == s.italic => {
                last.text.push_str(&s.text)
            }
            _ => out.push(s),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(spans: &[(&str, bool, bool)]) -> Block {
        Block::Paragraph(
            spans
                .iter()
                .map(|(t, b, i)| Span {
                    text: (*t).into(),
                    bold: *b,
                    italic: *i,
                })
                .collect(),
        )
    }

    #[test]
    fn paragraphs_and_hard_wrapped_lines() {
        let b = parse_markdown("Premier.\nSuite.\n\n\n  Second  \n");
        assert_eq!(
            b,
            vec![
                p(&[("Premier.\nSuite.", false, false)]),
                p(&[("Second", false, false)])
            ]
        );
    }

    #[test]
    fn inline_styles() {
        let b = parse_markdown("Elle dit *bonjour*, **fort** et ***tout***. _Aussi_ __ça__.");
        assert_eq!(
            b,
            vec![p(&[
                ("Elle dit ", false, false),
                ("bonjour", false, true),
                (", ", false, false),
                ("fort", true, false),
                (" et ", false, false),
                ("tout", true, true),
                (". ", false, false),
                ("Aussi", false, true),
                (" ", false, false),
                ("ça", true, false),
                (".", false, false),
            ])]
        );
    }

    #[test]
    fn nested_styles() {
        let b = parse_markdown("**gras *et italique* ici**");
        assert_eq!(
            b,
            vec![p(&[
                ("gras ", true, false),
                ("et italique", true, true),
                (" ici", true, false)
            ])]
        );
    }

    #[test]
    fn unmatched_and_spaced_markers_stay_literal() {
        let b = parse_markdown("2 * 3 = 6, **ouvert, snake_case_name et a*b");
        let text = b[0].plain_text();
        assert_eq!(text, "2 * 3 = 6, **ouvert, snake_case_name et a*b");
        assert!(matches!(&b[0], Block::Paragraph(s) if s.len() == 1 && !s[0].italic && !s[0].bold));
    }

    #[test]
    fn escapes() {
        let b = parse_markdown(r"\*pas italique\* et \_non\_");
        assert_eq!(b[0].plain_text(), "*pas italique* et _non_");
    }

    #[test]
    fn headings_and_rules() {
        let b = parse_markdown("# Titre *un*\n\n## Deux ##\n\n---\n\nTexte\n\n* * *\n\n#nope\n");
        assert_eq!(
            b[0],
            Block::Heading {
                level: 1,
                spans: vec![
                    Span::plain("Titre "),
                    Span {
                        text: "un".into(),
                        bold: false,
                        italic: true
                    },
                ]
            }
        );
        assert_eq!(
            b[1],
            Block::Heading {
                level: 2,
                spans: vec![Span::plain("Deux")]
            }
        );
        assert_eq!(b[2], Block::Rule);
        assert_eq!(b[3].plain_text(), "Texte");
        assert_eq!(b[4], Block::Rule);
        assert_eq!(b[5].plain_text(), "#nope");
    }

    #[test]
    fn dialogue_and_unicode() {
        let md = "— Où vas-tu ? demanda *Zoé*.\n— « Là-bas », répondit-il. 日本語 😀 café";
        let b = parse_markdown(md);
        assert_eq!(b.len(), 1);
        let t = b[0].plain_text();
        assert!(t.contains("— Où vas-tu ? demanda Zoé."));
        assert!(t.contains("« Là-bas »"));
        assert!(t.contains("日本語 😀 café"));
        assert!(t.contains('\n'), "dialogue lines stay separate");
    }

    #[test]
    fn annotations_are_stripped() {
        let md = "Avant <!-- note\nmultiligne --> après.\n\n<!-- Résumé: x -->\n\nVisible %%privé%% fin.\n\n<!-- non fermé\nperdu";
        let s = strip_annotations(md);
        assert!(!s.contains("note"));
        assert!(!s.contains("Résumé"));
        assert!(!s.contains("privé"));
        assert!(!s.contains("perdu"));
        let b = parse_markdown(&s);
        let all: Vec<String> = b.iter().map(|b| b.plain_text()).collect();
        assert_eq!(all, vec!["Avant  après.", "Visible  fin."]);
    }
}
