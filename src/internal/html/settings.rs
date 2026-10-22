/// A display label only. Navigation and persistence must use `BrowserSettings::url`.
pub fn browser_source_name(url: &str) -> String {
    if url.starts_with("data:") {
        return "🌐 Inline HTML".into();
    }
    let mut chars = url.chars();
    let short: String = chars.by_ref().take(80).collect();
    if chars.next().is_some() {
        format!("🌐 {short}…")
    } else {
        format!("🌐 {short}")
    }
}

/// Authored browser viewport, independent of the show's output resolution.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(default)]
pub struct BrowserSettings {
    pub url: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

impl Default for BrowserSettings {
    fn default() -> Self {
        Self {
            url: String::new(),
            width: 1920,
            height: 1080,
            fps: 30,
        }
    }
}

impl BrowserSettings {
    /// Check before allocating textures or sending a command to the host.
    ///
    /// # Errors
    /// Returns an explanation for unsupported URLs or excessive resource requests.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.width == 0 || self.height == 0 || self.width > 4096 || self.height > 4096 {
            return Err("Browser viewport must be between 1 and 4096 pixels per dimension");
        }
        if !(1..=60).contains(&self.fps) {
            return Err("Browser rate must be between 1 and 60 fps");
        }
        let url = self.url.as_str();
        if url.len() > 65536
            || url.chars().any(char::is_control)
            || !(url.starts_with("https://")
                || url.starts_with("http://")
                || url.starts_with("file:///")
                || url.starts_with("data:text/html"))
        {
            return Err("Use an http, https, file:/// or data:text/html browser URL");
        }
        #[cfg(any(feature = "html", feature = "windows-browser"))]
        {
            let parsed = url::Url::parse(url).map_err(|_| "Invalid browser URL")?;
            if matches!(parsed.scheme(), "http" | "https") && parsed.host_str().is_none() {
                return Err("Browser URL requires a host");
            }
        }
        Ok(())
    }
}

/// Snapshot read by UI and API; errors never include the source URL.
#[derive(Clone, serde::Serialize)]
pub struct BrowserSnapshot {
    pub settings: BrowserSettings,
    pub error: Option<String>,
    pub ready: bool,
    pub interactive: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_titles_never_include_inline_documents_or_unbounded_urls() {
        let url = format!("data:text/html;charset=utf-8,{}", "%3Ch1%3Etest%3C%2Fh1%3E".repeat(1000));
        let settings = BrowserSettings { url: url.clone(), ..BrowserSettings::default() };
        assert_eq!(browser_source_name(&url), "🌐 Inline HTML");
        let restored: BrowserSettings = serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        assert_eq!(restored.url, url, "the display label must not replace the saved URL");
        assert_eq!(browser_source_name("https://example.com/chat"), "🌐 https://example.com/chat");
        assert!(browser_source_name(&format!("https://example.com/{}", "界".repeat(1000))).chars().count() <= 83);
    }

    #[test]
    fn url_only_request_gets_compatible_defaults() {
        let settings: BrowserSettings =
            serde_json::from_str(r#"{"url":"https://example.com/chat"}"#).unwrap();
        assert_eq!(
            (settings.width, settings.height, settings.fps),
            (1920, 1080, 30)
        );
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn rejects_unsafe_dimensions_rates_and_non_browser_schemes() {
        let good = BrowserSettings {
            url: "https://example.com/chat".into(),
            ..BrowserSettings::default()
        };
        for size in [0, 4097, u32::MAX] {
            assert!(BrowserSettings {
                width: size,
                ..good.clone()
            }
            .validate()
            .is_err());
            assert!(BrowserSettings {
                height: size,
                ..good.clone()
            }
            .validate()
            .is_err());
        }
        for fps in [0, 61, u32::MAX] {
            assert!(BrowserSettings {
                fps,
                ..good.clone()
            }
            .validate()
            .is_err());
        }
        for url in [
            "javascript:alert(1)",
            "rtmp://example.com",
            "https://example.com/\nsecret",
        ] {
            assert!(BrowserSettings {
                url: url.into(),
                ..good.clone()
            }
            .validate()
            .is_err());
        }
    }

    #[test]
    fn independent_viewports_roundtrip() {
        let settings = BrowserSettings {
            url: "https://example.com/chat?token=private".into(),
            width: 480,
            height: 900,
            fps: 24,
        };
        let restored: BrowserSettings =
            serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        assert_eq!(restored, settings);
    }
}
