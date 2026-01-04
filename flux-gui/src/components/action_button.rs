//! Action button component

use gpui::{Context, Render, Window, div, prelude::*};

/// Button variant
pub enum ButtonVariant {
    /// Primary button (accent color)
    Primary,
    /// Secondary button (muted)
    Secondary,
    /// Danger button (red)
    Danger,
}

/// Action button component
pub struct ActionButton {
    /// Button label
    pub label: String,
    /// Button variant
    pub variant: ButtonVariant,
    /// Is button disabled
    pub disabled: bool,
}

impl ActionButton {
    /// Create a new primary button
    pub fn primary(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            variant: ButtonVariant::Primary,
            disabled: false,
        }
    }
}

impl Render for ActionButton {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .h(crate::styles::dimensions::BUTTON_HEIGHT)
            .px_4()
            .flex()
            .items_center()
            .justify_center()
            .child(div().text_sm().child(self.label.clone()))
    }
}
