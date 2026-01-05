//! Styling constants and utilities for the Flux GUI.

#[must_use]
pub fn window_bg() -> gpui::Hsla {
    gpui::hsla(0.0, 0.0, 0.12, 1.0)
}

#[must_use]
pub fn text_primary() -> gpui::Hsla {
    gpui::hsla(0.0, 0.0, 1.0, 1.0)
}

#[must_use]
pub fn sidebar_bg() -> gpui::Hsla {
    gpui::hsla(0.0, 0.0, 0.1, 1.0)
}

#[must_use]
pub fn accent_color() -> gpui::Hsla {
    gpui::hsla(0.46, 0.8, 0.5, 1.0)
}

#[must_use]
pub fn text_secondary() -> gpui::Hsla {
    gpui::hsla(0.0, 0.0, 0.53, 1.0)
}

#[must_use]
pub fn border_color() -> gpui::Hsla {
    gpui::hsla(0.0, 0.0, 0.15, 1.0)
}

#[must_use]
pub fn card_bg() -> gpui::Hsla {
    gpui::hsla(0.0, 0.0, 0.08, 1.0)
}

pub mod dimensions {
    use gpui::px;

    /// Sidebar width
    pub const SIDEBAR_WIDTH: gpui::Pixels = px(200.0);
    /// Header height
    pub const HEADER_HEIGHT: gpui::Pixels = px(50.0);
    /// Titlebar height
    pub const TITLEBAR_HEIGHT: gpui::Pixels = px(36.0);
}
