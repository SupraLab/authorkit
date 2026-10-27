# Glossary

| Term | Meaning |
|------|---------|
| **Project** | A folder containing `authorkit.toml`; one book. |
| **Manuscript** | The ordered structure of acts → chapters → scenes (`manuscript/index.json`). |
| **Scene** | The unit of writing; one Markdown file with JSON frontmatter. |
| **Note** ("fiche") | Worldbuilding entry: Character ("Personnage"), Setting ("Lieu") or Lore ("Concept"). |
| **Link** | A note id attached to a scene (`SceneMeta.links`); linked notes are sent as context. |
| **Styles** | Project-wide writing rules, voice notes and things to avoid (`WritingStyles`). |
| **Premise** | One-paragraph pitch of the book (`ProjectMeta.premise`). |
| **Synopsis** | Short summary of a scene; used as "summaries" context for other scenes. |
| **Tool / brush** | A one-shot AI transformation of the selected text (`Brush { id, label, instruction }`). Displayed as "Outils" / "Tools". |
| **Continue mode** | AI continuation at the cursor: `continue`, `sentence`, `paragraph`, `generate`. |
| **Composer** | The chat input area, with mode and optional attachment. |
| **Attachment** | Text linked from the editor into the composer, with range and anchors. |
| **Staging** | Preparing the composer (mode + attachment) without sending. |
| **Edit proposal** | Assistant message carrying an AI edit that can be accepted, regenerated or ignored. |
| **Anchor** | Up to 40 characters before/after a range, used to relocate it after edits (`resolve_anchor`). |
| **Context** | What the model receives: instructions + documentary data built by `ak_llm::assemble`. |
| **Piece** | One part of the context (styles, premise, summary, note, scene, passage) with a status. |
| **Budget** | Character limits for context pieces and history (`ContextBudget`, `RequestBudget`). |
| **Job** | One background LLM request (`LlmJob`) with a `JobPurpose` and `JobKind`. |
| **Review** | Coherence analysis of a scene: a list of `ReviewItem`s with ToDo/Done/Ignored status. |
| **Blueprint** | Generated novel plan (title, premise, characters, chapters, opening) that can be applied to a project. |
| **BYOK** | Bring your own key: the user supplies their own provider API key. |
| **Trash** | Soft-delete store in `.authorkit-rust/trash/`. |
| **Version** | Snapshot of a document in `.authorkit-rust/versions/`. |
