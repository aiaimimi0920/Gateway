//! Only documented text/font data can leave a layer; it is never executed or added to audits.
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
pub(super) struct Text {
    content: String,
    decorated: Option<bool>,
    font: Option<Font>,
}
#[derive(Clone, Deserialize, Serialize)]
struct Font {
    category: String,
    weight: String,
    width: String,
    italic: bool,
    families: Vec<String>,
    r#match: Option<Match>,
}
#[derive(Clone, Deserialize, Serialize)]
struct Match {
    family_id: String,
    face_id: String,
    family_name: String,
    // Keep the upstream JSON number representation while validating its numeric range.
    weight: serde_json::Number,
    style: String,
}
impl Text {
    pub(super) fn valid(&self) -> bool {
        // The original aggregate JSON read budget bounds these strings; do not clone raw fields.
        self.font.as_ref().is_none_or(|font| {
            [
                "sans serif",
                "serif",
                "slab serif",
                "display sans",
                "display serif",
                "display slab",
                "display decorative",
                "handwriting",
                "monospace",
            ]
            .contains(&font.category.as_str())
                && ["thin", "light", "regular", "medium", "bold", "black"]
                    .contains(&font.weight.as_str())
                && ["condensed", "normal", "wide"].contains(&font.width.as_str())
                && font.families.len() <= 3
                && font.r#match.as_ref().is_none_or(|matched| {
                    matched.family_id.starts_with("font_family:")
                        && matched.family_id.len() > 12
                        && matched.face_id.starts_with("font_face:")
                        && matched.face_id.len() > 10
                        && matched
                            .weight
                            .as_f64()
                            .is_some_and(|weight| (1.0..=1000.0).contains(&weight))
                        && ["normal", "italic"].contains(&matched.style.as_str())
                })
        })
    }
}
