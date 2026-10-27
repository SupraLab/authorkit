use ak_core::Project;
use std::io::Write;
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::blocks::{Block, Span};
use crate::error::Result;
use crate::model::{build_document, ExportOptions};

const CSS: &str = "body { line-height: 1.5; }\n\
p { margin: 0; text-indent: 1.2em; }\n\
h1, h2, h3, h4, h5, h6 { text-indent: 0; margin: 1.5em 0 0.8em; }\n\
.titlepage { text-align: center; margin-top: 30%; }\n\
.titlepage p { text-indent: 0; }\n\
.scenebreak { text-align: center; text-indent: 0; margin: 1.2em 0; }\n\
hr { margin: 1.2em 30%; }\n";

pub fn export_epub(project: &Project, out: &Path) -> Result<()> {
    export_epub_with(project, out, &ExportOptions::default())
}

/// EPUB 3: one XHTML file per chapter (or a single file when
/// `chapter_breaks` is off), real `<h1..h6>`, `<em>`, `<strong>`, `<hr/>`,
/// a navigation document, title/author/language metadata, and CSS.
pub fn export_epub_with(project: &Project, out: &Path, opts: &ExportOptions) -> Result<()> {
    let doc = build_document(project, opts);
    let meta = &doc.meta;
    let lang = escape_xml(&meta.language);

    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::create(out)?;
    let mut zip = ZipWriter::new(file);
    let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let opts_z = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    zip.start_file("mimetype", stored)?;
    zip.write_all(b"application/epub+zip")?;

    zip.start_file("META-INF/container.xml", opts_z)?;
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#,
    )?;

    let page = |title: &str, body: &str| {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" lang="{lang}" xml:lang="{lang}">
<head><meta charset="utf-8"/><title>{title}</title><link rel="stylesheet" type="text/css" href="style.css"/></head>
<body>
{body}
</body>
</html>"#,
            title = escape_xml(title),
        )
    };

    let mut manifest: Vec<String> = Vec::new();
    let mut spine: Vec<String> = Vec::new();
    let mut nav: Vec<String> = Vec::new();
    let mut files: Vec<(String, String)> = Vec::new();

    if opts.title_page && !meta.title.is_empty() {
        let mut body = format!(
            "<div class=\"titlepage\">\n<h1>{}</h1>\n",
            escape_xml(&meta.title)
        );
        if !meta.author.is_empty() {
            body.push_str(&format!("<p><em>{}</em></p>\n", escape_xml(&meta.author)));
        }
        body.push_str("</div>");
        files.push(("titlepage.xhtml".into(), page(&meta.title, &body)));
        manifest.push(
            r#"<item id="titlepage" href="titlepage.xhtml" media-type="application/xhtml+xml"/>"#
                .into(),
        );
        spine.push(r#"<itemref idref="titlepage"/>"#.into());
    }

    let render_section = |idx: usize, blocks: &[Block]| -> String {
        let mut s = format!("<section id=\"sec{idx}\" epub:type=\"chapter\">\n");
        if blocks.is_empty() {
            s.push_str("<p></p>\n");
        }
        for b in blocks {
            s.push_str(&block_xhtml(b));
            s.push('\n');
        }
        s.push_str("</section>");
        s
    };

    if doc.sections.is_empty() {
        let body = render_section(1, &[]);
        files.push(("chap1.xhtml".into(), page(&meta.title, &body)));
        manifest.push(
            r#"<item id="chap1" href="chap1.xhtml" media-type="application/xhtml+xml"/>"#.into(),
        );
        spine.push(r#"<itemref idref="chap1"/>"#.into());
        nav.push(format!(
            r##"<li><a href="chap1.xhtml#sec1">{}</a></li>"##,
            escape_xml(&meta.title)
        ));
    } else if opts.chapter_breaks {
        for (i, sec) in doc.sections.iter().enumerate() {
            let n = i + 1;
            let href = format!("chap{n}.xhtml");
            files.push((
                href.clone(),
                page(&sec.nav_title, &render_section(n, &sec.blocks)),
            ));
            manifest.push(format!(
                r#"<item id="chap{n}" href="{href}" media-type="application/xhtml+xml"/>"#
            ));
            spine.push(format!(r#"<itemref idref="chap{n}"/>"#));
            nav.push(format!(
                r##"<li><a href="{href}#sec{n}">{}</a></li>"##,
                escape_xml(&nav_label(&sec.nav_title, n))
            ));
        }
    } else {
        let mut body = String::new();
        for (i, sec) in doc.sections.iter().enumerate() {
            let n = i + 1;
            body.push_str(&render_section(n, &sec.blocks));
            body.push('\n');
            nav.push(format!(
                r##"<li><a href="content.xhtml#sec{n}">{}</a></li>"##,
                escape_xml(&nav_label(&sec.nav_title, n))
            ));
        }
        files.push(("content.xhtml".into(), page(&meta.title, &body)));
        manifest.push(
            r#"<item id="content" href="content.xhtml" media-type="application/xhtml+xml"/>"#
                .into(),
        );
        spine.push(r#"<itemref idref="content"/>"#.into());
    }

    let nav_doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" lang="{lang}" xml:lang="{lang}">
<head><meta charset="utf-8"/><title>{title}</title></head>
<body>
<nav epub:type="toc"><ol>
{items}
</ol></nav>
</body>
</html>"#,
        title = escape_xml(&meta.title),
        items = nav.join("\n"),
    );
    zip.start_file("OEBPS/nav.xhtml", opts_z)?;
    zip.write_all(nav_doc.as_bytes())?;
    manifest.push(
        r#"<item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>"#
            .into(),
    );
    zip.start_file("OEBPS/style.css", opts_z)?;
    zip.write_all(CSS.as_bytes())?;
    manifest.push(r#"<item id="css" href="style.css" media-type="text/css"/>"#.into());

    for (href, content) in &files {
        zip.start_file(format!("OEBPS/{href}"), opts_z)?;
        zip.write_all(content.as_bytes())?;
    }

    let creator = if meta.author.is_empty() {
        String::new()
    } else {
        format!(
            "\n    <dc:creator>{}</dc:creator>",
            escape_xml(&meta.author)
        )
    };
    let opf = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" unique-identifier="bookid" version="3.0" xml:lang="{lang}">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="bookid">{id}</dc:identifier>
    <dc:title>{title}</dc:title>{creator}
    <dc:language>{lang}</dc:language>
    <meta property="dcterms:modified">{modified}</meta>
  </metadata>
  <manifest>
    {manifest}
  </manifest>
  <spine>
    {spine}
  </spine>
</package>"#,
        id = book_id(&meta.title, &meta.author),
        title = escape_xml(&meta.title),
        modified = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ"),
        manifest = manifest.join("\n    "),
        spine = spine.join("\n    "),
    );
    zip.start_file("OEBPS/content.opf", opts_z)?;
    zip.write_all(opf.as_bytes())?;
    zip.finish()?;
    Ok(())
}

fn nav_label(title: &str, n: usize) -> String {
    if title.trim().is_empty() {
        format!("Chapitre {n}")
    } else {
        title.to_string()
    }
}

fn block_xhtml(b: &Block) -> String {
    match b {
        Block::Heading { level, spans } => {
            let l = (*level).clamp(1, 6);
            format!("<h{l}>{}</h{l}>", spans_xhtml(spans))
        }
        Block::Paragraph(spans) => format!("<p>{}</p>", spans_xhtml(spans)),
        Block::Rule => "<hr/>".into(),
        Block::SceneBreak(m) => format!("<p class=\"scenebreak\">{}</p>", escape_xml(m)),
    }
}

fn spans_xhtml(spans: &[Span]) -> String {
    let mut out = String::new();
    for s in spans {
        let mut t = escape_xml(&s.text).replace('\n', "<br/>\n");
        if s.italic {
            t = format!("<em>{t}</em>");
        }
        if s.bold {
            t = format!("<strong>{t}</strong>");
        }
        out.push_str(&t);
    }
    out
}

/// Escape for XML text/attributes and drop characters illegal in XML 1.0.
fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' | '\n' | '\r' => out.push(c),
            c if c < ' ' || c == '\u{FFFE}' || c == '\u{FFFF}' => {}
            c => out.push(c),
        }
    }
    out
}

/// Stable identifier from title + author (FNV-1a), unique even for non-ASCII titles.
fn book_id(title: &str, author: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in title
        .bytes()
        .chain(std::iter::once(0))
        .chain(author.bytes())
    {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("urn:authorkit:{h:016x}")
}
