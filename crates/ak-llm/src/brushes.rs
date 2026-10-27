use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Brush {
    pub id: String,
    pub label: String,
    pub instruction: String,
}

pub fn builtin_brushes() -> Vec<Brush> {
    vec![
        Brush {
            id: "improve".into(),
            label: "Améliorer".into(),
            instruction:
                "Améliore la qualité littéraire du texte (choix des mots, rythme, clarté) \
                en conservant le sens, le ton et la voix. Renvoie uniquement le texte réécrit."
                    .into(),
        },
        Brush {
            id: "shorten".into(),
            label: "Raccourcir".into(),
            instruction: "Raccourcis le texte en gardant l'essentiel et la voix. \
                Renvoie uniquement le texte réécrit."
                .into(),
        },
        Brush {
            id: "lengthen".into(),
            label: "Allonger".into(),
            instruction:
                "Allonge le texte avec des détails sensoriels et du sous-texte pertinents, \
                sans diluer l'action. Renvoie uniquement le texte réécrit."
                    .into(),
        },
        Brush {
            id: "grammar".into(),
            label: "Grammaire".into(),
            instruction: "Corrige grammaire, orthographe et ponctuation uniquement. \
                Ne change pas le style. Renvoie uniquement le texte corrigé."
                .into(),
        },
        Brush {
            id: "simplify".into(),
            label: "Simplifier".into(),
            instruction: "Simplifie le langage tout en restant littéraire. \
                Renvoie uniquement le texte réécrit."
                .into(),
        },
        Brush {
            id: "readable".into(),
            label: "Lisibilité".into(),
            instruction: "Améliore la lisibilité: découpe les phrases trop longues, \
                clarifie les passages denses. Renvoie uniquement le texte réécrit."
                .into(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn builtin_brushes_are_non_empty_with_unique_expected_ids() {
        let brushes = builtin_brushes();
        assert!(!brushes.is_empty());

        let ids: Vec<&str> = brushes.iter().map(|b| b.id.as_str()).collect();
        let unique: HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(unique.len(), ids.len(), "brush ids must be unique: {ids:?}");

        for expected in [
            "improve", "shorten", "lengthen", "grammar", "simplify", "readable",
        ] {
            assert!(
                ids.contains(&expected),
                "missing expected brush id `{expected}` in {ids:?}"
            );
        }
    }
}
