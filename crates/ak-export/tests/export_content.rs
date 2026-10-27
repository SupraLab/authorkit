//! Content-level export tests: they unzip the produced files and assert on
//! the actual text, styles and order — not on file sizes.

use std::io::Read;
use std::path::{Path, PathBuf};

use ak_core::Project;
use ak_export::*;

// ---------------------------------------------------------------------------
// fixtures
// ---------------------------------------------------------------------------

struct Tmp(PathBuf);
impl Tmp {
    fn new() -> Self {
        let d = std::env::temp_dir().join(format!("ak-export-content-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        Tmp(d)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

type SceneDef<'a> = (&'a str, &'a str);
type ChapterDef<'a> = (&'a str, Vec<SceneDef<'a>>);
type ActDef<'a> = (&'a str, Vec<ChapterDef<'a>>);

fn project(root: &Path, title: &str, author: &str, acts: Vec<ActDef>) -> Project {
    let mut p = Project::create(root, title, author).expect("create");
    p.manuscript.index.acts.clear();
    p.manuscript.scenes.clear();
    for (act_title, chapters) in acts {
        let a = p.manuscript.add_act(act_title);
        for (ch_title, scenes) in chapters {
            let c = p.manuscript.add_chapter(&a, ch_title).unwrap();
            for (s_title, body) in scenes {
                let id = p
                    .manuscript
                    .add_scene(root, &a, &c, s_title)
                    .unwrap()
                    .meta
                    .id
                    .clone();
                p.manuscript.get_scene_mut(&id).unwrap().body = body.to_string();
            }
        }
    }
    p
}

const DIALOGUE: &str = "— Où vas-tu ? demanda *Zoé*, **inquiète**.\n\
— « Là-bas », répondit Émile. 日本語 😀 café\n\
<!-- TODO: réécrire ce passage -->\n\n\
---\n\n\
Elle ne dit rien. %%note privée%%Fin du passage.";

fn sample(root: &Path) -> Project {
    project(
        root,
        "Le Café & les <Étoiles>",
        "Zoé Müller",
        vec![
            (
                "Acte I",
                vec![
                    (
                        "Chapitre 1",
                        vec![("Ouverture", DIALOGUE), ("Suite", "Deuxième scène.")],
                    ),
                    (
                        "Chapitre 2",
                        vec![("Nuit", "# Intertitre\n\nTexte de nuit.")],
                    ),
                ],
            ),
            (
                "Acte II",
                vec![("Chapitre 3", vec![("Aube", "Dernière scène.")])],
            ),
        ],
    )
}

fn zip_text(path: &Path, name: &str) -> String {
    let mut z = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    let mut s = String::new();
    z.by_name(name)
        .unwrap_or_else(|_| panic!("{name} missing in {}", path.display()))
        .read_to_string(&mut s)
        .unwrap();
    s
}

fn zip_names(path: &Path) -> Vec<String> {
    let z = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    z.file_names().map(str::to_string).collect()
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

// --- tiny DOCX reader --------------------------------------------------------

#[derive(Debug)]
struct DRun {
    text: String,
    bold: bool,
    italic: bool,
}
#[derive(Debug)]
struct DPara {
    style: Option<String>,
    page_break: bool,
    centered: bool,
    runs: Vec<DRun>,
}
impl DPara {
    fn text(&self) -> String {
        self.runs.iter().map(|r| r.text.as_str()).collect()
    }
}

fn docx_paragraphs(path: &Path) -> Vec<DPara> {
    let xml = zip_text(path, "word/document.xml");
    let body = xml.split("<w:body>").nth(1).unwrap();
    body.split("<w:p ")
        .skip(1)
        .map(|seg| {
            let style = seg
                .split("<w:pStyle w:val=\"")
                .nth(1)
                .map(|r| r.split('"').next().unwrap().to_string());
            let ppr = seg.split("</w:pPr>").next().unwrap_or("");
            let runs = seg
                .split("<w:r>")
                .skip(1)
                .map(|r| {
                    let rpr = r.split("</w:rPr>").next().unwrap_or("");
                    let mut text = String::new();
                    let mut rest = r;
                    loop {
                        let t = rest.find("<w:t ");
                        let b = rest.find("<w:br ");
                        match (t, b) {
                            (Some(ti), bi) if bi.is_none_or(|bi| ti < bi) => {
                                let after = &rest[ti..];
                                let start = after.find('>').unwrap() + 1;
                                let end = after.find("</w:t>").unwrap();
                                text.push_str(&unescape(&after[start..end]));
                                rest = &after[end + 6..];
                            }
                            (_, Some(bi)) => {
                                text.push('\n');
                                rest = &rest[bi + 6..];
                            }
                            _ => break,
                        }
                    }
                    DRun {
                        text,
                        bold: rpr.contains("<w:b />"),
                        italic: rpr.contains("<w:i />"),
                    }
                })
                .collect();
            DPara {
                style,
                page_break: ppr.contains("<w:pageBreakBefore"),
                centered: ppr.contains("<w:jc w:val=\"center\""),
                runs,
            }
        })
        .collect()
}

fn texts(paras: &[DPara]) -> Vec<String> {
    paras.iter().map(|p| p.text()).collect()
}

// --- tiny EPUB reader ----------------------------------------------------------

fn epub_spine_files(path: &Path) -> Vec<String> {
    let opf = zip_text(path, "OEBPS/content.opf");
    let manifest: Vec<(String, String)> = opf
        .split("<item ")
        .skip(1)
        .map(|i| {
            let id = i.split("id=\"").nth(1).unwrap().split('"').next().unwrap();
            let href = i
                .split("href=\"")
                .nth(1)
                .unwrap()
                .split('"')
                .next()
                .unwrap();
            (id.to_string(), href.to_string())
        })
        .collect();
    opf.split("<itemref idref=\"")
        .skip(1)
        .map(|r| {
            let id = r.split('"').next().unwrap();
            manifest.iter().find(|(i, _)| i == id).unwrap().1.clone()
        })
        .collect()
}

fn epub_body(path: &Path, href: &str) -> String {
    zip_text(path, &format!("OEBPS/{href}"))
}

fn epub_all_text(path: &Path) -> String {
    epub_spine_files(path)
        .iter()
        .map(|h| epub_body(path, h))
        .collect::<Vec<_>>()
        .join("\n")
}

fn pos(hay: &str, needle: &str) -> usize {
    hay.find(needle)
        .unwrap_or_else(|| panic!("`{needle}` not found in:\n{hay}"))
}

// ---------------------------------------------------------------------------
// DOCX
// ---------------------------------------------------------------------------

#[test]
fn docx_has_real_styles_and_clean_text() {
    let t = Tmp::new();
    let p = sample(&t.path("proj"));
    let out = t.path("out.docx");
    export_docx(&p, &out).unwrap();
    let paras = docx_paragraphs(&out);
    let all = texts(&paras).join("\n");

    // Title page, Unicode and XML-significant characters survive escaping.
    assert_eq!(paras[0].style.as_deref(), Some("Title"));
    assert_eq!(paras[0].text(), "Le Café & les <Étoiles>");
    assert_eq!(paras[1].style.as_deref(), Some("Author"));
    assert_eq!(paras[1].text(), "Zoé Müller");

    // Structure: act → chapter → scene headings with real heading styles.
    let headings: Vec<(String, String)> = paras
        .iter()
        .filter_map(|p| match p.style.as_deref() {
            Some(s) if s.starts_with("Heading") => Some((s.to_string(), p.text())),
            _ => None,
        })
        .collect();
    let h = |s: &str, t: &str| (s.to_string(), t.to_string());
    assert_eq!(
        headings,
        vec![
            h("Heading1", "Acte I"),
            h("Heading2", "Chapitre 1"),
            h("Heading3", "Ouverture"),
            h("Heading3", "Suite"),
            h("Heading2", "Chapitre 2"),
            h("Heading3", "Nuit"),
            h("Heading4", "Intertitre"), // body heading nested below the scene title
            h("Heading1", "Acte II"),
            h("Heading2", "Chapitre 3"),
            h("Heading3", "Aube"),
        ]
    );

    // Dialogue paragraph: inline styles are real runs, markers are gone.
    let dlg = paras
        .iter()
        .find(|p| p.text().contains("Où vas-tu ?"))
        .expect("dialogue paragraph");
    let italic: Vec<_> = dlg
        .runs
        .iter()
        .filter(|r| r.italic)
        .map(|r| r.text.as_str())
        .collect();
    let bold: Vec<_> = dlg
        .runs
        .iter()
        .filter(|r| r.bold)
        .map(|r| r.text.as_str())
        .collect();
    assert_eq!(italic, vec!["Zoé"]);
    assert_eq!(bold, vec!["inquiète"]);
    assert_eq!(
        dlg.text(),
        "— Où vas-tu ? demanda Zoé, inquiète.\n— « Là-bas », répondit Émile. 日本語 😀 café"
    );
    assert!(
        !all.replace("* * *", "").contains('*'),
        "raw emphasis markers leaked: {all}"
    );
    assert!(
        !all.contains("<!--") && !all.contains("TODO"),
        "annotation leaked"
    );
    assert!(!all.contains("note privée") && !all.contains("%%"));
    assert!(all.contains("Elle ne dit rien. Fin du passage."));

    // Horizontal rule is a centred separator paragraph.
    assert!(paras.iter().any(|p| p.centered && p.text() == "* * *"));

    // Styles are defined in styles.xml (not just referenced).
    let styles = zip_text(&out, "word/styles.xml");
    for id in ["Title", "Heading1", "Heading2", "Heading3", "Heading4"] {
        assert!(styles.contains(&format!("w:styleId=\"{id}\"")), "{id}");
    }
    assert!(styles.contains("<w:outlineLvl w:val=\"0\""));
}

#[test]
fn docx_preserves_manuscript_order() {
    let t = Tmp::new();
    let root = t.path("proj");
    let mut p = project(
        &root,
        "Ordre",
        "A",
        vec![(
            "Acte",
            vec![("Ch", vec![("S1", "un"), ("S2", "deux"), ("S3", "trois")])],
        )],
    );
    // Reorder in the index only: scene storage order stays S1,S2,S3.
    p.manuscript.index.acts[0].chapters[0].scene_ids.reverse();
    let out = t.path("o.docx");
    export_docx(&p, &out).unwrap();
    let all = texts(&docx_paragraphs(&out));
    let order: Vec<usize> = ["trois", "deux", "un"]
        .iter()
        .map(|w| all.iter().position(|t| t == w).unwrap())
        .collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{all:?}");
}

#[test]
fn docx_options_titles_breaks_and_meta() {
    let t = Tmp::new();
    let p = sample(&t.path("proj"));

    // Default: page break before the first section (after title page) and each chapter.
    let out = t.path("a.docx");
    export_docx(&p, &out).unwrap();
    let breaks = docx_paragraphs(&out)
        .iter()
        .filter(|p| p.page_break)
        .count();
    assert_eq!(breaks, 3, "3 chapters, each starts a page");

    // No chapter breaks.
    let opts = ExportOptions {
        chapter_breaks: false,
        ..Default::default()
    };
    let out = t.path("b.docx");
    export_docx_with(&p, &out, &opts).unwrap();
    assert_eq!(
        docx_paragraphs(&out)
            .iter()
            .filter(|p| p.page_break)
            .count(),
        0
    );

    // Hide act + scene titles; override meta; no title page.
    let opts = ExportOptions {
        title: Some("Nouveau titre".into()),
        author: Some("Autre Auteur".into()),
        language: Some("en-GB".into()),
        include_act_titles: false,
        include_scene_titles: false,
        title_page: false,
        ..Default::default()
    };
    let out = t.path("c.docx");
    export_docx_with(&p, &out, &opts).unwrap();
    let paras = docx_paragraphs(&out);
    let all = texts(&paras);
    assert!(!all
        .iter()
        .any(|t| t == "Acte I" || t == "Ouverture" || t == "Nouveau titre"));
    assert!(all.iter().any(|t| t == "Chapitre 1"));
    // Chapters are now the top level.
    let ch = paras.iter().find(|p| p.text() == "Chapitre 1").unwrap();
    assert_eq!(ch.style.as_deref(), Some("Heading1"));
    // Scenes of the same chapter are separated by a break marker.
    let i = all
        .iter()
        .position(|t| t.starts_with("— Où vas-tu"))
        .unwrap();
    assert_eq!(all[i + 1], "* * *"); // from the `---` rule inside the scene text
    let j = all.iter().position(|t| t == "Deuxième scène.").unwrap();
    assert_eq!(all[j - 1], "* * *", "scene break between scenes");
    let props = zip_text(&out, "docProps/custom.xml");
    assert!(
        props.contains("Nouveau titre")
            && props.contains("Autre Auteur")
            && props.contains("en-GB")
    );
}

#[test]
fn docx_keeps_annotations_when_asked() {
    let t = Tmp::new();
    let p = sample(&t.path("proj"));
    let opts = ExportOptions {
        strip_annotations: false,
        ..Default::default()
    };
    let out = t.path("o.docx");
    export_docx_with(&p, &out, &opts).unwrap();
    let all = texts(&docx_paragraphs(&out)).join("\n");
    assert!(all.contains("TODO: réécrire ce passage") || all.contains("<!-- TODO"));
}

// ---------------------------------------------------------------------------
// EPUB
// ---------------------------------------------------------------------------

#[test]
fn epub_structure_styles_and_content() {
    let t = Tmp::new();
    let p = sample(&t.path("proj"));
    let out = t.path("o.epub");
    export_epub(&p, &out).unwrap();

    // Container rules: mimetype first and stored.
    let z = zip::ZipArchive::new(std::fs::File::open(&out).unwrap()).unwrap();
    assert_eq!(z.name_for_index(0), Some("mimetype"));
    drop(z);
    assert_eq!(zip_text(&out, "mimetype"), "application/epub+zip");

    // Metadata.
    let opf = zip_text(&out, "OEBPS/content.opf");
    assert!(opf.contains("<dc:title>Le Café &amp; les &lt;Étoiles&gt;</dc:title>"));
    assert!(opf.contains("<dc:creator>Zoé Müller</dc:creator>"));
    assert!(opf.contains("<dc:language>fr</dc:language>"));
    assert!(opf.contains("dcterms:modified"));

    // Spine: title page, then one file per chapter, in manuscript order.
    let files = epub_spine_files(&out);
    assert_eq!(
        files,
        vec![
            "titlepage.xhtml",
            "chap1.xhtml",
            "chap2.xhtml",
            "chap3.xhtml"
        ]
    );
    let chap1 = epub_body(&out, "chap1.xhtml");
    assert!(chap1.contains("<h1>Acte I</h1>"));
    assert!(chap1.contains("<h2>Chapitre 1</h2>"));
    assert!(chap1.contains("<h3>Ouverture</h3>"));
    assert!(chap1.contains("<em>Zoé</em>"));
    assert!(chap1.contains("<strong>inquiète</strong>"));
    assert!(chap1.contains("<hr/>"));
    assert!(
        chap1.contains("<br/>"),
        "dialogue lines keep their line break"
    );
    assert!(chap1.contains("— « Là-bas », répondit Émile. 日本語 😀 café"));
    assert!(chap1.contains("lang=\"fr\""));
    assert!(pos(&chap1, "Ouverture") < pos(&chap1, "Suite"));

    let chap2 = epub_body(&out, "chap2.xhtml");
    assert!(
        !chap2.contains("<h1>Acte"),
        "act title only on its first chapter"
    );
    assert!(chap2.contains("<h4>Intertitre</h4>"));
    let chap3 = epub_body(&out, "chap3.xhtml");
    assert!(chap3.contains("<h1>Acte II</h1>"));

    // No raw Markdown, no annotations.
    let all = epub_all_text(&out);
    assert!(!all.contains("**") && !all.contains("*Zoé*"));
    assert!(!all.contains("<!--") && !all.contains("TODO") && !all.contains("note privée"));
    assert!(all.contains("Elle ne dit rien. Fin du passage."));

    // Title page + nav.
    let tp = epub_body(&out, "titlepage.xhtml");
    assert!(tp.contains("Le Café &amp; les &lt;Étoiles&gt;") && tp.contains("<em>Zoé Müller</em>"));
    let nav = zip_text(&out, "OEBPS/nav.xhtml");
    assert!(
        pos(&nav, "Chapitre 1") < pos(&nav, "Chapitre 2")
            && pos(&nav, "Chapitre 2") < pos(&nav, "Chapitre 3")
    );
    assert!(nav.contains("href=\"chap2.xhtml#sec2\""));
    assert!(zip_names(&out).contains(&"OEBPS/style.css".to_string()));
}

#[test]
fn epub_options() {
    let t = Tmp::new();
    let p = sample(&t.path("proj"));
    let opts = ExportOptions {
        title: Some("Titre édité".into()),
        author: Some("Édith".into()),
        language: Some("en".into()),
        include_act_titles: false,
        include_scene_titles: false,
        chapter_breaks: false,
        title_page: false,
        ..Default::default()
    };
    let out = t.path("o.epub");
    export_epub_with(&p, &out, &opts).unwrap();
    assert_eq!(epub_spine_files(&out), vec!["content.xhtml"]);
    let c = epub_body(&out, "content.xhtml");
    assert!(c.contains("<h1>Chapitre 1</h1>") && c.contains("<h1>Chapitre 3</h1>"));
    assert!(!c.contains("Acte I") && !c.contains("Ouverture"));
    assert!(c.contains("<p class=\"scenebreak\">* * *</p>"));
    assert!(pos(&c, "sec1") < pos(&c, "sec2") && pos(&c, "sec2") < pos(&c, "sec3"));
    let opf = zip_text(&out, "OEBPS/content.opf");
    assert!(opf.contains("<dc:title>Titre édité</dc:title>"));
    assert!(opf.contains("<dc:creator>Édith</dc:creator>"));
    assert!(opf.contains("<dc:language>en</dc:language>"));
    // Nav still names chapters even though they point into one file.
    let nav = zip_text(&out, "OEBPS/nav.xhtml");
    assert!(nav.contains("content.xhtml#sec2") && nav.contains("Chapitre 2"));
}

#[test]
fn invalid_language_falls_back_and_empty_project_exports() {
    let t = Tmp::new();
    let p = project(&t.path("proj"), "Vide", "", vec![]);
    let opts = ExportOptions {
        language: Some("<script>".into()),
        ..Default::default()
    };
    let out = t.path("o.epub");
    export_epub_with(&p, &out, &opts).unwrap();
    let opf = zip_text(&out, "OEBPS/content.opf");
    assert!(opf.contains("<dc:language>fr</dc:language>"));
    assert!(!opf.contains("dc:creator"), "no empty creator");
    let dx = t.path("o.docx");
    export_docx(&p, &dx).unwrap();
    assert_eq!(docx_paragraphs(&dx)[0].text(), "Vide");
    assert!(
        is_valid_lang("fr")
            && is_valid_lang("en-GB")
            && !is_valid_lang("f")
            && !is_valid_lang("a b")
    );
}

#[test]
fn control_characters_do_not_corrupt_xml() {
    let t = Tmp::new();
    let p = project(
        &t.path("proj"),
        "T",
        "A",
        vec![(
            "Acte",
            vec![("Ch", vec![("S", "avant\u{0}\u{1}\u{8} après & <b>")])],
        )],
    );
    let e = t.path("o.epub");
    export_epub(&p, &e).unwrap();
    let all = epub_all_text(&e);
    assert!(all.contains("avant après &amp; &lt;b&gt;"));
    let d = t.path("o.docx");
    export_docx(&p, &d).unwrap();
    assert!(texts(&docx_paragraphs(&d))
        .iter()
        .any(|t| t == "avant après & <b>"));
}

// ---------------------------------------------------------------------------
// Markdown + shared model
// ---------------------------------------------------------------------------

#[test]
fn markdown_export_honours_options() {
    let t = Tmp::new();
    let p = sample(&t.path("proj"));
    let out = t.path("o.md");
    export_markdown(&p, &out).unwrap();
    let md = std::fs::read_to_string(&out).unwrap();
    assert!(md.starts_with("# Le Café & les <Étoiles>\n\n*Zoé Müller*"));
    assert!(
        md.contains("### Ouverture") && md.contains("*Zoé*"),
        "body stays Markdown"
    );
    assert!(!md.contains("TODO") && !md.contains("note privée"));
    assert!(pos(&md, "Chapitre 1") < pos(&md, "Chapitre 3"));

    let opts = ExportOptions {
        include_scene_titles: false,
        include_act_titles: false,
        title: Some("Autre".into()),
        strip_annotations: false,
        ..Default::default()
    };
    export_markdown_with(&p, &out, &opts).unwrap();
    let md = std::fs::read_to_string(&out).unwrap();
    assert!(md.starts_with("# Autre\n\n"));
    assert!(!md.contains("### Ouverture") && !md.contains("Acte I"));
    assert!(md.contains("TODO"));
}

#[test]
fn markdown_empty_doc_premise_and_scene_breaks() {
    let t = Tmp::new();
    // Empty author → no italic byline; premise is written before the body.
    let mut p = Project::create(&t.path("empty"), "Seul titre", "").unwrap();
    p.meta.premise = "Une porte s'ouvre.".into();
    p.manuscript.index.acts.clear();
    p.manuscript.scenes.clear();
    let nested = t.path("out/nested/empty.md");
    export_markdown(&p, &nested).unwrap();
    let md = std::fs::read_to_string(&nested).unwrap();
    assert_eq!(md, "# Seul titre\n\nUne porte s'ouvre.\n\n---\n\n");
    assert!(!md.contains('*'));

    // Two scenes, titles off → default scene_break between them.
    let p = project(
        &t.path("breaks"),
        "Breaks",
        "A",
        vec![("Acte", vec![("Ch", vec![("S1", "un"), ("S2", "deux")])])],
    );
    let opts = ExportOptions {
        include_act_titles: false,
        include_chapter_titles: false,
        include_scene_titles: false,
        scene_break: Some("§".into()),
        strip_annotations: false,
        ..Default::default()
    };
    let out = t.path("breaks.md");
    export_markdown_with(&p, &out, &opts).unwrap();
    let md = std::fs::read_to_string(&out).unwrap();
    assert!(md.contains("un\n\n§\n\ndeux"), "got:\n{md}");
    assert!(!md.contains("### "));
}

#[test]
fn shared_document_model_is_the_single_source() {
    let t = Tmp::new();
    let p = sample(&t.path("proj"));
    let doc = build_document(&p, &ExportOptions::default());
    assert_eq!(doc.sections.len(), 3);
    assert_eq!(
        doc.sections
            .iter()
            .map(|s| s.nav_title.as_str())
            .collect::<Vec<_>>(),
        vec!["Chapitre 1", "Chapitre 2", "Chapitre 3"]
    );
    let text = doc.plain_text();
    assert!(text.contains("Zoé") && !text.contains('*') && !text.contains("TODO"));
    assert!(doc.sections[0].blocks.contains(&Block::Rule));
    // DOCX and EPUB carry the same words in the same order.
    let d = t.path("o.docx");
    let e = t.path("o.epub");
    export_docx(&p, &d).unwrap();
    export_epub(&p, &e).unwrap();
    let dx = texts(&docx_paragraphs(&d)).join("\n");
    for needle in ["Ouverture", "Suite", "Nuit", "Aube"] {
        assert!(dx.contains(needle));
    }
    assert!(pos(&dx, "Ouverture") < pos(&dx, "Nuit") && pos(&dx, "Nuit") < pos(&dx, "Aube"));
}
