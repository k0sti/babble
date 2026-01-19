//! Theme and styling for the Proto UI
//!
//! This module provides colors, fonts, and visual styling for the application.

use egui::{Color32, FontFamily, FontId, Rounding, Stroke, Vec2, Visuals};

/// Application theme configuration
#[derive(Clone, Debug)]
pub struct Theme {
    /// Primary accent color
    pub primary: Color32,
    /// Secondary accent color
    pub secondary: Color32,
    /// Success color (green)
    pub success: Color32,
    /// Warning color (yellow/orange)
    pub warning: Color32,
    /// Error color (red)
    pub error: Color32,

    /// Background colors
    pub bg_primary: Color32,
    pub bg_secondary: Color32,
    pub bg_tertiary: Color32,

    /// Text colors
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_muted: Color32,

    /// Recording indicator color
    pub recording: Color32,

    /// Waveform colors
    pub waveform_active: Color32,
    pub waveform_inactive: Color32,

    /// Border radius for buttons
    pub button_rounding: Rounding,
    /// Border radius for cards/panels
    pub card_rounding: Rounding,

    /// Standard spacing
    pub spacing: f32,
    /// Large spacing
    pub spacing_lg: f32,
    /// Small spacing
    pub spacing_sm: f32,
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

impl Theme {
    /// Create a dark theme
    pub fn dark() -> Self {
        Self {
            primary: Color32::from_rgb(99, 102, 241),   // Indigo
            secondary: Color32::from_rgb(139, 92, 246), // Purple
            success: Color32::from_rgb(34, 197, 94),    // Green
            warning: Color32::from_rgb(234, 179, 8),    // Yellow
            error: Color32::from_rgb(239, 68, 68),      // Red

            bg_primary: Color32::from_rgb(17, 24, 39),   // Dark blue-gray
            bg_secondary: Color32::from_rgb(31, 41, 55), // Lighter blue-gray
            bg_tertiary: Color32::from_rgb(55, 65, 81),  // Even lighter

            text_primary: Color32::from_rgb(249, 250, 251),   // Almost white
            text_secondary: Color32::from_rgb(209, 213, 219), // Light gray
            text_muted: Color32::from_rgb(156, 163, 175),     // Medium gray

            recording: Color32::from_rgb(239, 68, 68), // Red

            waveform_active: Color32::from_rgb(99, 102, 241),  // Indigo (matches primary)
            waveform_inactive: Color32::from_rgb(75, 85, 99),  // Gray

            button_rounding: Rounding::same(8.0),
            card_rounding: Rounding::same(12.0),

            spacing: 16.0,
            spacing_lg: 24.0,
            spacing_sm: 8.0,
        }
    }

    /// Create a light theme
    pub fn light() -> Self {
        Self {
            primary: Color32::from_rgb(79, 70, 229),    // Indigo
            secondary: Color32::from_rgb(124, 58, 237), // Purple
            success: Color32::from_rgb(22, 163, 74),    // Green
            warning: Color32::from_rgb(202, 138, 4),    // Yellow
            error: Color32::from_rgb(220, 38, 38),      // Red

            bg_primary: Color32::from_rgb(255, 255, 255),  // White
            bg_secondary: Color32::from_rgb(243, 244, 246), // Light gray
            bg_tertiary: Color32::from_rgb(229, 231, 235), // Lighter gray

            text_primary: Color32::from_rgb(17, 24, 39),  // Dark
            text_secondary: Color32::from_rgb(55, 65, 81), // Gray
            text_muted: Color32::from_rgb(107, 114, 128), // Medium gray

            recording: Color32::from_rgb(220, 38, 38), // Red

            waveform_active: Color32::from_rgb(79, 70, 229),   // Indigo (matches primary)
            waveform_inactive: Color32::from_rgb(156, 163, 175), // Gray

            button_rounding: Rounding::same(8.0),
            card_rounding: Rounding::same(12.0),

            spacing: 16.0,
            spacing_lg: 24.0,
            spacing_sm: 8.0,
        }
    }

    /// Apply this theme to egui
    pub fn apply(&self, ctx: &egui::Context) {
        let mut visuals = Visuals::dark();

        // Panel backgrounds
        visuals.panel_fill = self.bg_primary;
        visuals.window_fill = self.bg_secondary;
        visuals.extreme_bg_color = self.bg_tertiary;

        // Widget colors
        visuals.widgets.noninteractive.bg_fill = self.bg_secondary;
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, self.text_muted);

        visuals.widgets.inactive.bg_fill = self.bg_tertiary;
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, self.text_secondary);

        visuals.widgets.hovered.bg_fill = self.primary.gamma_multiply(0.8);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, self.text_primary);

        visuals.widgets.active.bg_fill = self.primary;
        visuals.widgets.active.fg_stroke = Stroke::new(1.0, self.text_primary);

        // Text selection
        visuals.selection.bg_fill = self.primary.gamma_multiply(0.3);
        visuals.selection.stroke = Stroke::new(1.0, self.primary);

        // Hyperlinks
        visuals.hyperlink_color = self.primary;

        // Window styling
        visuals.window_rounding = self.card_rounding;
        visuals.window_stroke = Stroke::new(1.0, self.bg_tertiary);

        ctx.set_visuals(visuals);

        // Use default fonts (egui's built-in fonts)
        ctx.set_fonts(egui::FontDefinitions::default());

        // Set default style
        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = Vec2::splat(self.spacing_sm);
        style.spacing.window_margin = egui::Margin::same(self.spacing);
        style.spacing.button_padding = Vec2::new(self.spacing, self.spacing_sm);

        // Text styles
        style.text_styles.insert(
            egui::TextStyle::Heading,
            FontId::new(24.0, FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Body,
            FontId::new(14.0, FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        );
        style.text_styles.insert(
            egui::TextStyle::Button,
            FontId::new(14.0, FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Small,
            FontId::new(12.0, FontFamily::Proportional),
        );

        ctx.set_style(style);
    }

    /// Get a button stroke for primary buttons
    pub fn primary_button_stroke(&self) -> Stroke {
        Stroke::new(1.0, self.primary)
    }

    /// Get a button stroke for secondary buttons
    pub fn secondary_button_stroke(&self) -> Stroke {
        Stroke::new(1.0, self.text_muted)
    }
}
