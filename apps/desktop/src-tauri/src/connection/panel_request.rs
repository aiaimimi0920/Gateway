//! Strict presentation-only input for the unprivileged console's embedded settings slot.
use serde::{Deserialize, Serialize};
use tauri::{LogicalPosition, LogicalSize, Rect, Url};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    pub category: String,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub theme: String,
    pub locale: String,
}

impl Layout {
    pub fn parse(url: &Url) -> Option<Self> {
        if url.as_str().len() > 2048
            || url.scheme() != "gateway-window"
            || url.host_str() != Some("settings")
            || url.path() != "/show"
            || !url.username().is_empty()
            || url.password().is_some()
            || url.port().is_some()
            || url.fragment().is_some()
        {
            return None;
        }
        let pairs: Vec<_> = url.query_pairs().collect();
        if pairs.len() != 1 || pairs[0].0 != "layout" {
            return None;
        }
        let layout: Self = serde_json::from_str(&pairs[0].1).ok()?;
        (matches!(layout.category.as_str(), "backend" | "desktop")
            && matches!(layout.theme.as_str(), "dark" | "light")
            && matches!(layout.locale.as_str(), "zh-CN" | "en-US")
            && layout.width > 0
            && layout.height > 0
            && [layout.x, layout.y, layout.width, layout.height]
                .iter()
                .all(|value| *value <= 100_000))
        .then_some(layout)
    }

    pub fn same_content(&self, other: &Self) -> bool {
        self.category == other.category && self.theme == other.theme && self.locale == other.locale
    }

    pub fn bounds(&self, viewport: LogicalSize<f64>) -> Option<Rect> {
        let width = f64::from(self.width).min(viewport.width - f64::from(self.x));
        let height = f64::from(self.height).min(viewport.height - f64::from(self.y));
        (width > 0.0 && height > 0.0).then(|| Rect {
            position: LogicalPosition::new(f64::from(self.x), f64::from(self.y)).into(),
            size: LogicalSize::new(width, height).into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(value: serde_json::Value) -> Url {
        let mut url = Url::parse("gateway-window://settings/show").unwrap();
        url.query_pairs_mut()
            .append_pair("layout", &value.to_string());
        url
    }

    fn valid() -> serde_json::Value {
        serde_json::json!({"category":"desktop","x":210,"y":160,"width":900,"height":600,"theme":"dark","locale":"zh-CN"})
    }

    #[test]
    fn accepts_only_presentation_metadata() {
        for category in ["backend", "desktop"] {
            let mut value = valid();
            value["category"] = category.into();
            assert!(Layout::parse(&request(value)).is_some());
        }
        for (key, value) in [
            ("category", serde_json::json!("diagnostics")),
            ("category", serde_json::json!("security")),
            ("theme", serde_json::json!("dark';alert(1)")),
            ("locale", serde_json::json!("unknown")),
            ("x", serde_json::json!(-1)),
            ("width", serde_json::json!(0)),
            ("height", serde_json::json!(100_001)),
            ("y", serde_json::json!(1.5)),
            ("token", serde_json::json!("secret")),
            ("path", serde_json::json!("C:/private")),
            ("autostart", serde_json::json!(true)),
        ] {
            let mut input = valid();
            input[key] = value;
            assert!(Layout::parse(&request(input)).is_none(), "{key}");
        }
    }

    #[test]
    fn geometry_changes_preserve_the_form_but_category_changes_do_not() {
        let first = Layout::parse(&request(valid())).unwrap();
        let mut resized = first.clone();
        resized.x = 52;
        resized.width = 1200;
        assert!(first.same_content(&resized));
        resized.category = "backend".into();
        assert!(!first.same_content(&resized));
    }

    #[test]
    fn rejects_ambiguous_or_nonlocal_navigation_and_clamps_bounds() {
        let url = request(valid());
        for input in [
            url.as_str().replace("gateway-window:", "https:"),
            url.as_str().replace("//settings/", "//user@settings/"),
            format!("{url}#x"),
            format!("{url}&layout={{}}"),
            format!("{url}&command=save_profile"),
        ] {
            assert!(Layout::parse(&Url::parse(&input).unwrap()).is_none());
        }
        let layout = Layout::parse(&url).unwrap();
        let bounds = layout.bounds(LogicalSize::new(1000.0, 700.0)).unwrap();
        assert_eq!(bounds.size, LogicalSize::new(790.0, 540.0).into());
        assert!(layout.bounds(LogicalSize::new(200.0, 100.0)).is_none());
    }
}
