//! Reusable UI components for Flux GUI.

use crate::styles::{accent_color, border_color, card_bg, text_primary, text_secondary};
use gpui::{Context, Entity, Render, Window, div, prelude::*, px};

/// A text input field component
#[derive(Clone)]
pub struct TextInput {
    pub value: String,
    pub placeholder: String,
    pub is_focused: bool,
}

impl TextInput {
    /// Create a new text input
    #[must_use]
    pub fn new(placeholder: impl Into<String>) -> Self {
        Self {
            value: String::new(),
            placeholder: placeholder.into(),
            is_focused: false,
        }
    }

    /// Set the current value
    #[must_use]
    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self
    }
}

/// Render a text input field
pub fn render_text_input(
    input: &TextInput,
    on_change: Option<impl Fn(String) + 'static>,
) -> impl IntoElement {
    let value = input.value.clone();
    let placeholder = input.placeholder.clone();
    let is_focused = input.is_focused;

    div()
        .w_full()
        .px_3()
        .py_2()
        .bg(card_bg())
        .border_1()
        .border_color(if is_focused {
            accent_color()
        } else {
            border_color()
        })
        .rounded_sm()
        .child(
            div()
                .text_color(if value.is_empty() {
                    text_secondary()
                } else {
                    text_primary()
                })
                .text_sm()
                .child(if value.is_empty() { placeholder } else { value }),
        )
}

/// A confirmation dialog
#[derive(Clone)]
pub struct ConfirmDialog {
    pub title: String,
    pub message: String,
    pub confirm_text: String,
    pub cancel_text: String,
}

impl ConfirmDialog {
    /// Create a new confirmation dialog
    #[must_use]
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            confirm_text: "Confirm".to_string(),
            cancel_text: "Cancel".to_string(),
        }
    }

    /// Set the confirm button text
    #[must_use]
    pub fn with_confirm_text(mut self, text: impl Into<String>) -> Self {
        self.confirm_text = text.into();
        self
    }

    /// Set the cancel button text
    #[must_use]
    pub fn with_cancel_text(mut self, text: impl Into<String>) -> Self {
        self.cancel_text = text.into();
        self
    }
}

/// Render a confirmation dialog
pub fn render_confirm_dialog(
    dialog: &ConfirmDialog,
    on_confirm: Option<impl Fn() + 'static>,
    on_cancel: Option<impl Fn() + 'static>,
) -> impl IntoElement {
    let title = dialog.title.clone();
    let message = dialog.message.clone();
    let confirm_text = dialog.confirm_text.clone();
    let cancel_text = dialog.cancel_text.clone();

    div()
        .w_full()
        .p_4()
        .bg(card_bg())
        .border_1()
        .border_color(border_color())
        .rounded_md()
        .child(
            div()
                .text_lg()
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(text_primary())
                .child(title),
        )
        .child(div().h_2())
        .child(div().text_color(text_secondary()).child(message))
        .child(div().h_4())
        .child(
            div()
                .flex()
                .flex_row()
                .gap_3()
                .justify_end()
                .child(
                    div()
                        .px_4()
                        .py_2()
                        .bg(card_bg())
                        .text_color(text_primary())
                        .border_1()
                        .border_color(border_color())
                        .rounded_sm()
                        .cursor_pointer()
                        .child(cancel_text),
                )
                .child(
                    div()
                        .px_4()
                        .py_2()
                        .bg(accent_color())
                        .text_color(gpui::hsla(0.0, 0.0, 1.0, 1.0))
                        .rounded_sm()
                        .cursor_pointer()
                        .child(confirm_text),
                ),
        )
}

/// A simple badge/tag component
pub fn render_badge(label: &str, color: gpui::Hsla) -> impl IntoElement {
    div()
        .px_2()
        .py_1()
        .bg(gpui::hsla(color.h, color.s, 0.15, 1.0))
        .text_color(color)
        .rounded_sm()
        .text_xs()
        .font_weight(gpui::FontWeight::MEDIUM)
        .child(label)
}

/// A progress indicator
pub fn render_progress_bar(progress: f32) -> impl IntoElement {
    div().w_full().h_1().bg(card_bg()).rounded_full().overflow_hidden().child(
        div().h_full().bg(accent_color()).w(gpui::Pixels::from(
            (progress.clamp(0.0, 1.0) * 100.0).to_string(),
        )),
    )
}

/// An expandable section
#[derive(Clone)]
pub struct ExpandableSection {
    pub title: String,
    pub is_expanded: bool,
}

impl ExpandableSection {
    /// Create a new expandable section
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            is_expanded: false,
        }
    }
}

/// Render an expandable section header
pub fn render_section_header(
    section: &ExpandableSection,
    on_toggle: Option<impl Fn() + 'static>,
) -> impl IntoElement {
    let title = section.title.clone();
    let is_expanded = section.is_expanded;

    div()
        .flex()
        .flex_row()
        .items_center()
        .cursor_pointer()
        .child(div().text_sm().child(if is_expanded { "▼" } else { "▶" }))
        .child(
            div()
                .ml_2()
                .text_color(text_primary())
                .font_weight(gpui::FontWeight::MEDIUM)
                .child(title),
        )
}
