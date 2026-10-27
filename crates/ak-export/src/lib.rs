//! Export manuscript to Markdown, EPUB, and DOCX.
//!
//! DOCX and EPUB share one pipeline: scenes are parsed from Markdown into
//! structured [`Block`]s ([`blocks`]), assembled in manuscript order into a
//! [`Document`] ([`model`]) according to [`ExportOptions`], then rendered with
//! real styles (Word heading styles / bold / italic runs, XHTML `<h1>`,
//! `<em>`, `<strong>`, `<hr/>`).

mod blocks;
mod docx_export;
mod epub;
mod error;
mod markdown;
mod model;

pub use blocks::{parse_inline, parse_markdown, strip_annotations, Block, Span};
pub use docx_export::{export_docx, export_docx_with};
pub use epub::{export_epub, export_epub_with};
pub use error::{Error, Result};
pub use markdown::{export_markdown, export_markdown_with};
pub use model::{build_document, is_valid_lang, Document, ExportOptions, Meta, Section};
