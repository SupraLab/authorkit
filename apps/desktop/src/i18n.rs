//! Interface language — French by default, English optional. No Franglais in chrome.

use ak_core::NoteKind;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    #[default]
    Fr,
    En,
}

impl Lang {
    pub fn label(self) -> &'static str {
        match self {
            Self::Fr => "Français",
            Self::En => "English",
        }
    }

    pub fn all() -> [Lang; 2] {
        [Self::Fr, Self::En]
    }

    pub fn is_fr(self) -> bool {
        matches!(self, Self::Fr)
    }
}

/// Convenience: pick FR or EN string.
#[inline]
pub fn tr(lang: Lang, fr: &'static str, en: &'static str) -> &'static str {
    match lang {
        Lang::Fr => fr,
        Lang::En => en,
    }
}

impl Lang {
    // —— Chrome / header ——
    pub fn settings(self) -> &'static str {
        tr(self, "Paramètres", "Settings")
    }
    pub fn settings_llm_tip(self) -> &'static str {
        tr(self, "Paramètres", "Settings")
    }
    pub fn publish(self) -> &'static str {
        tr(self, "Publier", "Publish")
    }
    pub fn publish_tip(self) -> &'static str {
        tr(self, "Publier / exporter", "Publish / export")
    }
    pub fn novel_styles(self) -> &'static str {
        tr(self, "Styles du roman", "Writing styles")
    }
    pub fn novel_styles_tip(self) -> &'static str {
        tr(
            self,
            "Styles du roman (voix, règles IA)",
            "Writing styles (voice, AI rules)",
        )
    }
    pub fn theme_tip(self) -> &'static str {
        tr(self, "Thème clair / sombre", "Light / dark theme")
    }
    pub fn sidebar_tip(self) -> &'static str {
        tr(self, "Panneau latéral", "Side panel")
    }
    pub fn nav_tip(self) -> &'static str {
        tr(self, "Navigation", "Navigation")
    }

    // —— Settings tabs ——
    pub fn tab_llm(self) -> &'static str {
        tr(self, "IA & clés", "AI & keys")
    }
    pub fn tab_interface(self) -> &'static str {
        tr(self, "Interface", "Interface")
    }
    pub fn provider(self) -> &'static str {
        tr(self, "Fournisseur", "Provider")
    }
    pub fn model(self) -> &'static str {
        tr(self, "Modèle", "Model")
    }
    pub fn custom_model(self) -> &'static str {
        tr(self, "Ou modèle personnalisé", "Or custom model")
    }
    pub fn choose_model(self) -> &'static str {
        tr(self, "Choisir un modèle…", "Choose a model…")
    }
    pub fn base_url(self) -> &'static str {
        tr(self, "URL de base", "Base URL")
    }
    pub fn api_key(self) -> &'static str {
        tr(self, "Clé API", "API key")
    }
    pub fn api_key_hint(self) -> &'static str {
        tr(self, "sk-… / clé fournisseur", "sk-… / provider key")
    }
    pub fn api_key_replace_hint(self) -> &'static str {
        tr(
            self,
            "Laisser vide pour garder la clé · ou coller une nouvelle",
            "Leave blank to keep · or paste a new key",
        )
    }
    pub fn api_key_saved(self) -> &'static str {
        tr(self, "clé enregistrée", "key saved")
    }
    pub fn llm_key_saved(self) -> &'static str {
        tr(self, "Clé API enregistrée.", "API key saved.")
    }
    pub fn llm_key_missing(self) -> &'static str {
        tr(
            self,
            "Ajoutez une clé API avant d’enregistrer.",
            "Add an API key before saving.",
        )
    }
    pub fn save(self) -> &'static str {
        tr(self, "Enregistrer", "Save")
    }
    pub fn cancel(self) -> &'static str {
        tr(self, "Annuler", "Cancel")
    }
    pub fn close(self) -> &'static str {
        tr(self, "Fermer", "Close")
    }
    pub fn language(self) -> &'static str {
        tr(self, "Langue de l’interface", "Interface language")
    }
    pub fn appearance(self) -> &'static str {
        tr(self, "Apparence", "Appearance")
    }
    pub fn theme_light(self) -> &'static str {
        tr(self, "Clair", "Light")
    }
    pub fn theme_dark(self) -> &'static str {
        tr(self, "Sombre", "Dark")
    }
    pub fn llm_saved(self) -> &'static str {
        tr(self, "Paramètres IA enregistrés.", "AI settings saved.")
    }

    // —— Nav ——
    pub fn manuscript(self) -> &'static str {
        tr(self, "Manuscrit", "Manuscript")
    }
    pub fn universe(self) -> &'static str {
        tr(self, "Univers", "World")
    }
    pub fn new_act(self) -> &'static str {
        tr(self, "Nouvel acte", "New act")
    }
    pub fn no_acts(self) -> &'static str {
        tr(self, "Aucun acte — cliquez +", "No acts — click +")
    }
    pub fn chapter(self) -> &'static str {
        tr(self, "Chapitre", "Chapter")
    }
    pub fn scene(self) -> &'static str {
        tr(self, "Scène", "Scene")
    }
    pub fn rename(self) -> &'static str {
        tr(self, "Renommer", "Rename")
    }
    pub fn delete(self) -> &'static str {
        tr(self, "Supprimer", "Delete")
    }

    pub fn note_kind_plural(self, kind: NoteKind) -> &'static str {
        match (self, kind) {
            (Self::Fr, NoteKind::Character) => "Personnages",
            (Self::Fr, NoteKind::Setting) => "Lieux",
            (Self::Fr, NoteKind::Lore) => "Concepts",
            (Self::En, NoteKind::Character) => "Characters",
            (Self::En, NoteKind::Setting) => "Places",
            (Self::En, NoteKind::Lore) => "Concepts",
        }
    }

    pub fn note_kind_singular(self, kind: NoteKind) -> &'static str {
        match (self, kind) {
            (Self::Fr, NoteKind::Character) => "Personnage",
            (Self::Fr, NoteKind::Setting) => "Lieu",
            (Self::Fr, NoteKind::Lore) => "Concept",
            (Self::En, NoteKind::Character) => "Character",
            (Self::En, NoteKind::Setting) => "Place",
            (Self::En, NoteKind::Lore) => "Concept",
        }
    }

    pub fn new_note_tip(self, kind: NoteKind) -> String {
        match self {
            Self::Fr => format!("Nouveau {}", self.note_kind_singular(kind).to_lowercase()),
            Self::En => format!("New {}", self.note_kind_singular(kind).to_lowercase()),
        }
    }

    pub fn create(self) -> &'static str {
        tr(self, "Créer", "Create")
    }

    pub fn new_scene_title(self) -> &'static str {
        tr(self, "Nouvelle scène", "New scene")
    }

    pub fn create_note_title(self, kind: NoteKind) -> String {
        self.new_note_tip(kind)
    }

    pub fn name_required(self) -> &'static str {
        tr(
            self,
            "Le nom ne peut pas être vide.",
            "Name cannot be empty.",
        )
    }

    pub fn name_hint(self) -> &'static str {
        tr(self, "Nom", "Name")
    }

    // —— Editor ——
    pub fn visual(self) -> &'static str {
        tr(self, "Visuel", "Visual")
    }
    pub fn code(self) -> &'static str {
        tr(self, "Code", "Code")
    }
    pub fn visual_tip(self) -> &'static str {
        tr(
            self,
            "Mode Visuel — écrire avec mise en forme",
            "Visual mode — write with formatting",
        )
    }
    pub fn code_tip(self) -> &'static str {
        tr(
            self,
            "Mode Code — source Markdown brute",
            "Code mode — raw Markdown source",
        )
    }
    pub fn context(self) -> &'static str {
        tr(self, "Contexte", "Context")
    }
    pub fn link(self) -> &'static str {
        tr(self, "+ Relier", "+ Link")
    }
    pub fn link_tip(self) -> &'static str {
        tr(
            self,
            "Lier un personnage, lieu ou concept",
            "Link a character, place, or concept",
        )
    }
    pub fn link_title(self) -> &'static str {
        tr(self, "Relier au contexte", "Link to context")
    }
    pub fn filter_hint(self) -> &'static str {
        tr(self, "Filtrer…", "Filter…")
    }
    pub fn no_notes_link(self) -> &'static str {
        tr(
            self,
            "Aucune note disponible à relier.",
            "No notes available to link.",
        )
    }
    pub fn no_results(self) -> &'static str {
        tr(self, "Aucun résultat.", "No results.")
    }
    pub fn title_hint(self) -> &'static str {
        tr(self, "Titre…", "Title…")
    }
    pub fn write_hint(self) -> &'static str {
        tr(
            self,
            "Écrivez ici…  sélection → format / outils · / pour l’IA",
            "Write here…  selection → format / tools · / for AI",
        )
    }
    pub fn md_hint(self) -> &'static str {
        tr(self, "Source Markdown…", "Markdown source…")
    }
    pub fn bold(self) -> &'static str {
        tr(self, "Gras", "Bold")
    }
    pub fn italic(self) -> &'static str {
        tr(self, "Italique", "Italic")
    }
    pub fn heading(self) -> &'static str {
        tr(self, "Titre", "Heading")
    }
    pub fn quote(self) -> &'static str {
        tr(self, "Citation", "Quote")
    }
    pub fn list(self) -> &'static str {
        tr(self, "Liste à puces", "Bulleted list")
    }
    pub fn more_brushes(self) -> &'static str {
        tr(
            self,
            "Plus d’outils / commandes IA",
            "More tools / AI commands",
        )
    }
    pub fn brushes(self) -> &'static str {
        tr(self, "Outils", "Tools")
    }
    pub fn ai_commands(self) -> &'static str {
        tr(self, "Commandes IA…", "AI commands…")
    }

    // —— Status ——
    pub fn words(self, n: usize) -> String {
        match self {
            Self::Fr if n <= 1 => format!("{n} mot"),
            Self::Fr => format!("{n} mots"),
            Self::En if n == 1 => format!("{n} word"),
            Self::En => format!("{n} words"),
        }
    }
    pub fn words_total(self, n: usize) -> String {
        match self {
            Self::Fr => format!("{} au total", self.words(n)),
            Self::En => format!("{} total", self.words(n)),
        }
    }
    pub fn structure(self, acts: usize, chapters: usize, scenes: usize) -> String {
        let (a, c, s) = match self {
            Self::Fr => (
                if acts <= 1 { "acte" } else { "actes" },
                "chap.",
                if scenes <= 1 { "scène" } else { "scènes" },
            ),
            Self::En => (
                if acts == 1 { "act" } else { "acts" },
                "ch.",
                if scenes == 1 { "scene" } else { "scenes" },
            ),
        };
        format!("{acts} {a} · {chapters} {c} · {scenes} {s}")
    }

    // —— Welcome ——
    pub fn recents(self) -> &'static str {
        tr(self, "Récents", "Recents")
    }
    pub fn tagline(self) -> &'static str {
        tr(
            self,
            "Imaginer, structurer, réécrire, tout dans un même espace",
            "Imagine, structure, rewrite, all in one workspace",
        )
    }
    pub fn get_started(self) -> &'static str {
        tr(self, "Commencer", "Get started")
    }
    pub fn new_project(self) -> &'static str {
        tr(self, "Nouveau projet", "New project")
    }
    pub fn open_project(self) -> &'static str {
        tr(self, "Ouvrir un projet", "Open a project")
    }
    pub fn new_idea(self) -> &'static str {
        tr(self, "Nouvelle idée de roman", "New novel idea")
    }
    pub fn new_idea_tip(self) -> &'static str {
        tr(
            self,
            "Générer un blueprint avec l’IA (BYOK)",
            "Generate a blueprint with AI (BYOK)",
        )
    }
    pub fn empty_hint(self) -> &'static str {
        tr(
            self,
            "Créez un projet ou ouvrez un dossier AuthorKit.",
            "Create a project or open an AuthorKit folder.",
        )
    }
    pub fn configure_byok(self) -> &'static str {
        tr(self, "Configurer BYOK", "Configure BYOK")
    }

    // —— Modals ——
    pub fn confirm_delete(self) -> &'static str {
        tr(self, "Confirmer la suppression", "Confirm deletion")
    }
    pub fn delete_confirm(self, kind: &str, name: &str) -> String {
        match self {
            Self::Fr => format!("Supprimer {kind} « {name} » ? Cette action est définitive."),
            Self::En => format!("Delete {kind} “{name}”? This cannot be undone."),
        }
    }
    pub fn kind_act(self) -> &'static str {
        tr(self, "cet acte", "this act")
    }
    pub fn kind_chapter(self) -> &'static str {
        tr(self, "ce chapitre", "this chapter")
    }
    pub fn kind_scene(self) -> &'static str {
        tr(self, "cette scène", "this scene")
    }
    pub fn kind_note(self) -> &'static str {
        tr(self, "cet élément", "this element")
    }
    pub fn styles_desc(self) -> &'static str {
        tr(
            self,
            "Voix, règles et préférences injectées dans chaque appel IA.",
            "Voice, rules and preferences injected into every AI call.",
        )
    }
    pub fn rules(self) -> &'static str {
        tr(self, "Règles", "Rules")
    }
    pub fn voice(self) -> &'static str {
        tr(self, "Voix", "Voice")
    }
    pub fn avoid(self) -> &'static str {
        tr(self, "À éviter", "Avoid")
    }
    pub fn styles_saved(self) -> &'static str {
        tr(self, "Styles enregistrés.", "Styles saved.")
    }
    pub fn export_md(self) -> &'static str {
        tr(self, "Exporter Markdown…", "Export Markdown…")
    }
    pub fn export_epub(self) -> &'static str {
        tr(self, "Exporter EPUB…", "Export EPUB…")
    }
    pub fn export_docx(self) -> &'static str {
        tr(self, "Exporter DOCX…", "Export DOCX…")
    }
    pub fn publish_desc(self, title: &str, words: usize) -> String {
        match self {
            Self::Fr => format!("{title} — {words} mots"),
            Self::En => format!("{title} — {words} words"),
        }
    }

    pub fn saved(self) -> &'static str {
        tr(self, "Enregistré", "Saved")
    }
    pub fn scene_missing(self) -> &'static str {
        tr(self, "Scène introuvable.", "Scene not found.")
    }
    pub fn note_missing(self) -> &'static str {
        tr(self, "Note introuvable.", "Note not found.")
    }
}
