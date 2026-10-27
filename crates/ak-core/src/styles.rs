use serde::{Deserialize, Serialize};

/// Project-level writing rules injected into every LLM call.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WritingStyles {
    /// Free-form voice / style instructions (Type.ai "writing rules").
    pub rules: String,
    /// Preferred POV, tense, address (tu/vous), etc.
    pub voice_notes: String,
    /// Things the AI must avoid.
    pub avoid: String,
}

impl WritingStyles {
    pub fn as_prompt_block(&self) -> String {
        let mut parts = Vec::new();
        if !self.rules.trim().is_empty() {
            parts.push(format!("Règles d'écriture:\n{}", self.rules.trim()));
        }
        if !self.voice_notes.trim().is_empty() {
            parts.push(format!("Voix:\n{}", self.voice_notes.trim()));
        }
        if !self.avoid.trim().is_empty() {
            parts.push(format!("À éviter:\n{}", self.avoid.trim()));
        }
        parts.join("\n\n")
    }

    pub fn is_empty(&self) -> bool {
        self.rules.trim().is_empty()
            && self.voice_notes.trim().is_empty()
            && self.avoid.trim().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_default_is_empty_and_prompt_blank() {
        let s = WritingStyles::default();
        assert!(s.is_empty());
        assert_eq!(s.as_prompt_block(), "");
    }

    #[test]
    fn whitespace_only_is_empty() {
        let s = WritingStyles {
            rules: "  \n\t ".into(),
            voice_notes: "   ".into(),
            avoid: "\n".into(),
        };
        assert!(s.is_empty());
        assert_eq!(s.as_prompt_block(), "");
    }

    #[test]
    fn rules_only_prompt_block() {
        let s = WritingStyles {
            rules: "  Court et clair.  ".into(),
            ..Default::default()
        };
        assert!(!s.is_empty());
        assert_eq!(s.as_prompt_block(), "Règles d'écriture:\nCourt et clair.");
    }

    #[test]
    fn all_three_fields_prompt_block() {
        let s = WritingStyles {
            rules: "Règle".into(),
            voice_notes: "Voix tu".into(),
            avoid: "Clichés".into(),
        };
        assert!(!s.is_empty());
        assert_eq!(
            s.as_prompt_block(),
            "Règles d'écriture:\nRègle\n\nVoix:\nVoix tu\n\nÀ éviter:\nClichés"
        );
    }
}
