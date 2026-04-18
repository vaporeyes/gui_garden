use std::fs;
use std::path::Path;
use egui::{Color32, Context, Rgba, Stroke, Visuals};
use serde::{Deserialize, Serialize};

/// Theme configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    /// Name of the theme
    pub name: String,
    
    /// Whether this is a dark theme
    pub is_dark: bool,
    
    /// Primary accent color (hex string like "#FF5500")
    pub accent_color: String,
    
    /// Background color (hex string)
    pub background_color: String,
    
    /// Text color (hex string)
    pub text_color: String,
    
    /// Link color (hex string)
    pub link_color: String,
    
    /// Code background color (hex string)
    pub code_background: String,
    
    /// Custom CSS content
    pub custom_css: Option<String>,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        // Mirrors the josh.contact / astro-blog dark palette:
        // warm amber accent on deep near-black surfaces.
        Self {
            name: "Garden Dark".to_string(),
            is_dark: true,
            accent_color: "#f59e0b".to_string(),
            background_color: "#0c0c0b".to_string(),
            text_color: "#ece9e1".to_string(),
            link_color: "#fbbf24".to_string(),
            code_background: "#151413".to_string(),
            custom_css: None,
        }
    }
}

/// Theme presets as functions rather than constants
pub fn get_default_themes() -> Vec<ThemeConfig> {
    vec![
        ThemeConfig {
            name: "Garden Dark".to_string(),
            is_dark: true,
            accent_color: "#f59e0b".to_string(),
            background_color: "#0c0c0b".to_string(),
            text_color: "#ece9e1".to_string(),
            link_color: "#fbbf24".to_string(),
            code_background: "#151413".to_string(),
            custom_css: None,
        },
        ThemeConfig {
            name: "Garden Light".to_string(),
            is_dark: false,
            accent_color: "#b45309".to_string(),
            background_color: "#faf7f0".to_string(),
            text_color: "#26231f".to_string(),
            link_color: "#b45309".to_string(),
            code_background: "#f0ece1".to_string(),
            custom_css: None,
        },
        ThemeConfig {
            name: "Obsidian Dark".to_string(),
            is_dark: true,
            accent_color: "#50fa7b".to_string(),
            background_color: "#282a36".to_string(),
            text_color: "#f8f8f2".to_string(),
            link_color: "#8be9fd".to_string(),
            code_background: "#44475a".to_string(),
            custom_css: None,
        },
        ThemeConfig {
            name: "Obsidian Light".to_string(),
            is_dark: false,
            accent_color: "#50fa7b".to_string(),
            background_color: "#fafafa".to_string(),
            text_color: "#40434c".to_string(),
            link_color: "#277bcf".to_string(),
            code_background: "#f2f3f5".to_string(),
            custom_css: None,
        },
    ]
}

/// Theme manager
pub struct ThemeManager {
    /// Current theme
    pub current_theme: ThemeConfig,
    
    /// Path to themes directory
    pub themes_dir: Option<String>,
    
    /// Available themes
    pub available_themes: Vec<String>,
}

impl Default for ThemeManager {
    fn default() -> Self {
        let default_themes = get_default_themes();
        Self {
            current_theme: ThemeConfig::default(),
            themes_dir: None,
            available_themes: default_themes.iter().map(|t| t.name.clone()).collect(),
        }
    }
}

impl ThemeManager {
    /// Create a new theme manager
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Set the themes directory. Part of a disk-based theme loading feature
    /// that is implemented but not yet wired up to the app UI; kept so
    /// `set_theme`'s JSON fallback remains reachable when it is.
    #[allow(dead_code)]
    pub fn set_themes_directory<P: AsRef<Path>>(&mut self, path: P) {
        let path_str = path.as_ref().to_string_lossy().to_string();
        self.themes_dir = Some(path_str);
        self.load_available_themes();
    }

    /// Load available themes from the themes directory.
    #[allow(dead_code)]
    pub fn load_available_themes(&mut self) {
        // Start with predefined themes
        let default_themes = get_default_themes();
        self.available_themes = default_themes.iter().map(|t| t.name.clone()).collect();
        
        if let Some(themes_dir) = &self.themes_dir {
            if let Ok(entries) = fs::read_dir(themes_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    
                    if path.is_file() && path.extension().map_or(false, |ext| ext == "json") {
                        if let Ok(content) = fs::read_to_string(&path) {
                            if let Ok(theme) = serde_json::from_str::<ThemeConfig>(&content) {
                                if !self.available_themes.contains(&theme.name) {
                                    self.available_themes.push(theme.name);
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // Sort themes
        self.available_themes.sort();
    }
    
    /// Parsed accent color of the current theme, for inline use in widgets.
    pub fn accent(&self) -> Color32 {
        hex_to_color32(&self.current_theme.accent_color)
    }

    /// Parsed muted text color (used for metadata rows and secondary labels).
    pub fn muted_text(&self) -> Color32 {
        // Soften the theme's text color by blending toward the background.
        let text = hex_to_color32(&self.current_theme.text_color);
        let bg = hex_to_color32(&self.current_theme.background_color);
        blend(text, bg, 0.55)
    }

    /// Apply the current theme to the UI
    pub fn apply_theme(&self, ctx: &Context) {
        let mut visuals = if self.current_theme.is_dark {
            Visuals::dark()
        } else {
            Visuals::light()
        };
        
        // Set accent color
        let accent_color = hex_to_color32(&self.current_theme.accent_color);
        visuals.selection.bg_fill = accent_color;
        visuals.selection.stroke = Stroke::new(1.0, accent_color);
        visuals.hyperlink_color = hex_to_color32(&self.current_theme.link_color);
        visuals.code_bg_color = hex_to_color32(&self.current_theme.code_background);
        
        // Set background color
        let bg_color = hex_to_color32(&self.current_theme.background_color);
        visuals.panel_fill = bg_color;
        visuals.window_fill = bg_color;
        
        // Set text color
        let text_color = hex_to_color32(&self.current_theme.text_color);
        visuals.override_text_color = Some(text_color);
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, text_color);
        
        // Make window corners rounded (currently not supported in this version of egui)
        
        // Apply the visuals
        ctx.set_visuals(visuals);
        
        // Apply custom CSS if provided
        if let Some(css) = &self.current_theme.custom_css {
            // In a real implementation, we would inject this CSS
            // into the web page using JavaScript
            eprintln!("Custom CSS is not supported in egui yet: {}", css);
        }
    }
    
    /// Set the current theme by name
    pub fn set_theme(&mut self, name: &str) {
        // Try to find it in predefined themes first
        let default_themes = get_default_themes();
        for theme in default_themes {
            if theme.name == name {
                self.current_theme = theme;
                return;
            }
        }
        
        // Try to load from themes directory
        if let Some(themes_dir) = &self.themes_dir {
            let path = Path::new(themes_dir).join(format!("{}.json", name));
            if path.exists() {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(theme) = serde_json::from_str::<ThemeConfig>(&content) {
                        self.current_theme = theme;
                        return;
                    }
                }
            }
        }
        
        // Fallback to default theme
        self.current_theme = ThemeConfig::default();
    }
    
    /// Toggle between light and dark themes
    pub fn toggle_dark_mode(&mut self, ctx: &Context) {
        let current_name = self.current_theme.name.clone();
        
        if self.current_theme.is_dark {
            // Switch to light theme with the same name if possible
            let light_name = current_name.replace("Dark", "Light");
            if self.available_themes.contains(&light_name) {
                self.set_theme(&light_name);
            } else {
                // Default light theme
                self.set_theme("Default Light");
            }
        } else {
            // Switch to dark theme with the same name if possible
            let dark_name = current_name.replace("Light", "Dark");
            if self.available_themes.contains(&dark_name) {
                self.set_theme(&dark_name);
            } else {
                // Default dark theme
                self.set_theme("Default Dark");
            }
        }
        
        self.apply_theme(ctx);
    }
}

/// Convert a hex color string to an egui Color32
pub fn hex_to_color32(hex: &str) -> Color32 {
    let hex = hex.trim_start_matches('#');

    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);

    Color32::from_rgb(r, g, b)
}

/// Linear-space blend between two colors. `t=0.0` returns `a`, `t=1.0` returns `b`.
fn blend(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let ar = Rgba::from(a);
    let br = Rgba::from(b);
    let r = ar.r() * (1.0 - t) + br.r() * t;
    let g = ar.g() * (1.0 - t) + br.g() * t;
    let bl = ar.b() * (1.0 - t) + br.b() * t;
    let al = ar.a() * (1.0 - t) + br.a() * t;
    Color32::from(Rgba::from_rgba_premultiplied(r, g, bl, al))
}