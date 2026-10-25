//! The colours a diagram is drawn in.
//!
//! Everything else about how it looks — sizes, weights, the font — is the same
//! whichever theme is chosen, so a theme is nothing but a palette.

/// Which palette to draw with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    /// Light, turning dark where the viewer has asked for dark.
    #[default]
    Auto,
    Light,
    Dark,
    /// White lines on deep blue.
    Blueprint,
    /// Black on white and nothing else, for printing.
    Mono,
}

impl Theme {
    /// The name the CLI and the options object use for each theme.
    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "auto" => Some(Self::Auto),
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            "blueprint" => Some(Self::Blueprint),
            "mono" => Some(Self::Mono),
            _ => None,
        }
    }

    /// The rules that colour every part of the diagram.
    pub fn css(self) -> String {
        match self {
            Self::Auto => format!(
                "{}  @media (prefers-color-scheme: dark) {{\n{}  }}\n",
                LIGHT.css("  "),
                DARK.css("    ")
            ),
            Self::Light => LIGHT.css("  "),
            Self::Dark => DARK.css("  "),
            Self::Blueprint => BLUEPRINT.css("  "),
            Self::Mono => MONO.css("  "),
        }
    }
}

struct Palette {
    canvas: &'static str,
    entity: &'static str,
    header: &'static str,
    border: &'static str,
    text: &'static str,
    muted: &'static str,
    edge: &'static str,
    label_bg: &'static str,
    label: &'static str,
    cardinality_bg: &'static str,
    pk: &'static str,
    fk: &'static str,
    /// Text on a key badge
    badge_text: &'static str,
    flag: &'static str,
}

impl Palette {
    fn css(&self, indent: &str) -> String {
        let rules = [
            (".canvas", format!("fill: {}", self.canvas)),
            (".entity-bg", format!("fill: {}", self.entity)),
            (".entity-header", format!("fill: {}", self.header)),
            (".entity-border", format!("stroke: {}", self.border)),
            (".entity-separator", format!("stroke: {}", self.border)),
            (".entity-name", format!("fill: {}", self.text)),
            (".column-text", format!("fill: {}", self.text)),
            (".column-type", format!("fill: {}", self.muted)),
            (".badge-pk", format!("fill: {}", self.pk)),
            (".badge-fk", format!("fill: {}", self.fk)),
            (".badge-flag", format!("stroke: {}", self.flag)),
            (
                ".badge-pk-text, .badge-fk-text",
                format!("fill: {}", self.badge_text),
            ),
            (".badge-flag-text", format!("fill: {}", self.muted)),
            (".edge", format!("stroke: {}", self.edge)),
            (".edge-symbol", format!("stroke: {}", self.edge)),
            (
                ".edge-symbol-zero",
                format!("stroke: {}; fill: {}", self.edge, self.canvas),
            ),
            (".edge-label-bg", format!("fill: {}", self.label_bg)),
            (".edge-label", format!("fill: {}", self.label)),
            (".cardinality-bg", format!("fill: {}", self.cardinality_bg)),
            (".cardinality", format!("fill: {}", self.text)),
        ];
        rules
            .iter()
            .map(|(selector, body)| format!("{indent}{selector} {{ {body}; }}\n"))
            .collect()
    }
}

const LIGHT: Palette = Palette {
    canvas: "#fff",
    entity: "#fff",
    header: "#e0e0e0",
    border: "#333",
    text: "#222",
    muted: "#6a737d",
    edge: "#666",
    label_bg: "rgba(234,234,234,0.9)",
    label: "#444",
    cardinality_bg: "rgba(224,224,224,0.95)",
    pk: "#f6d365",
    fk: "#9ec5fe",
    badge_text: "#24292f",
    flag: "#b0b7c0",
};

const DARK: Palette = Palette {
    canvas: "#0d1117",
    entity: "#161b22",
    header: "#262c36",
    border: "#6e7681",
    text: "#e6edf3",
    muted: "#8b949e",
    edge: "#8b949e",
    label_bg: "rgba(48,54,61,0.92)",
    label: "#c9d1d9",
    cardinality_bg: "rgba(60,67,76,0.95)",
    pk: "#9e7c1c",
    fk: "#2f5f9e",
    badge_text: "#f0f6fc",
    flag: "#57606a",
};

const BLUEPRINT: Palette = Palette {
    canvas: "#0b3d6e",
    entity: "#0e4a85",
    header: "#14599e",
    border: "#cfe3f7",
    text: "#f2f8fd",
    muted: "#9cc3e8",
    edge: "#cfe3f7",
    label_bg: "rgba(11,61,110,0.92)",
    label: "#f2f8fd",
    cardinality_bg: "rgba(20,89,158,0.95)",
    pk: "#ffd166",
    fk: "#7fd1ff",
    badge_text: "#0b3d6e",
    flag: "#9cc3e8",
};

const MONO: Palette = Palette {
    canvas: "#fff",
    entity: "#fff",
    header: "#fff",
    border: "#000",
    text: "#000",
    muted: "#555",
    edge: "#000",
    label_bg: "rgba(255,255,255,0.95)",
    label: "#000",
    cardinality_bg: "rgba(255,255,255,0.95)",
    pk: "#000",
    fk: "#777",
    badge_text: "#fff",
    flag: "#555",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_theme_is_known_by_its_name() {
        for name in ["auto", "light", "dark", "blueprint", "mono"] {
            assert!(Theme::from_name(name).is_some(), "{name}");
        }
        assert_eq!(Theme::from_name("sepia"), None);
    }

    #[test]
    fn only_auto_follows_the_viewer() {
        assert!(Theme::Auto.css().contains("prefers-color-scheme: dark"));
        for theme in [Theme::Light, Theme::Dark, Theme::Blueprint, Theme::Mono] {
            assert!(!theme.css().contains("@media"), "{theme:?}");
        }
    }
}
