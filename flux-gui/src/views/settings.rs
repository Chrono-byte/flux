//! Settings view - Application settings and configuration.

use crate::app::FluxApp;
use crate::styles::{accent_color, card_bg, text_primary, text_secondary};
use gpui::{Render, Window, div, prelude::*};

pub struct SettingsView;

impl Default for SettingsView {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsView {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let app = cx.global::<FluxApp>();
        let state = &app.state;

        div()
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .gap_6()
            .p_6()
            .child(
                div()
                    .text_2xl()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(text_primary())
                    .child("Settings"),
            )
            .child(div().text_color(text_secondary()).child("Configure Flux application settings"))
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(text_primary())
                    .child("General"),
            )
            .child(
                div().p_4().bg(card_bg()).rounded_md().child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .child(
                            div().flex().flex_row().items_center().justify_between().child(
                                div()
                                    .child(
                                        div()
                                            .text_color(text_primary())
                                            .font_weight(gpui::FontWeight::MEDIUM)
                                            .child("Repository Path"),
                                    )
                                    .child(
                                        div()
                                            .text_color(text_secondary())
                                            .text_sm()
                                            .mt_1()
                                            .child(state.repo_path.display().to_string()),
                                    ),
                            ),
                        )
                        .child(
                            div().flex().flex_row().items_center().justify_between().child(
                                div()
                                    .child(
                                        div()
                                            .text_color(text_primary())
                                            .font_weight(gpui::FontWeight::MEDIUM)
                                            .child("Current Profile"),
                                    )
                                    .child(
                                        div()
                                            .text_color(text_secondary())
                                            .text_sm()
                                            .mt_1()
                                            .child(state.current_profile.clone()),
                                    ),
                            ),
                        ),
                ),
            )
            .child(div().h_4())
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(text_primary())
                    .child("Symlink Resolution Strategy"),
            )
            .child(
                div()
                    .text_color(text_secondary())
                    .text_sm()
                    .child("How symlinks should be created between repository and home directory"),
            )
            .child(div().h_4())
            .child(div().flex().flex_row().gap_2().children(
                ["Auto", "Relative", "Absolute", "Follow", "Replace"].iter().map(|option| {
                    let is_first = option == &"Auto";
                    div()
                        .px_4()
                        .py_2()
                        .bg(if is_first { accent_color() } else { card_bg() })
                        .text_color(if is_first {
                            gpui::hsla(0.0, 0.0, 1.0, 1.0)
                        } else {
                            text_primary()
                        })
                        .rounded_md()
                        .cursor_pointer()
                        .child(*option)
                }),
            ))
            .child(div().h_4())
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(text_primary())
                    .child("Advanced"),
            )
            .child(
                div().p_4().bg(card_bg()).rounded_md().child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .child(
                                    div()
                                        .text_color(text_primary())
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .child("Dry Run Mode"),
                                )
                                .child(
                                    div()
                                        .text_color(text_secondary())
                                        .text_sm()
                                        .mt_1()
                                        .child("Preview changes without applying them"),
                                ),
                        )
                        .child(
                            div()
                                .w_12()
                                .h_6()
                                .bg(gpui::hsla(0.0, 0.0, 0.15, 1.0))
                                .rounded_full()
                                .cursor_pointer(),
                        ),
                ),
            )
            .child(
                div().p_4().bg(card_bg()).rounded_md().child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .child(
                                    div()
                                        .text_color(text_primary())
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .child("Enable Hooks"),
                                )
                                .child(
                                    div()
                                        .text_color(text_secondary())
                                        .text_sm()
                                        .mt_1()
                                        .child("Run pre/post sync hooks"),
                                ),
                        )
                        .child(
                            div().w_12().h_6().bg(accent_color()).rounded_full().cursor_pointer(),
                        ),
                ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_4()
                    .mt_6()
                    .child(
                        div()
                            .px_4()
                            .py_2()
                            .bg(accent_color())
                            .text_color(gpui::hsla(0.0, 0.0, 1.0, 1.0))
                            .rounded_md()
                            .cursor_pointer()
                            .child("Save Settings"),
                    )
                    .child(
                        div()
                            .px_4()
                            .py_2()
                            .bg(card_bg())
                            .text_color(text_primary())
                            .rounded_md()
                            .cursor_pointer()
                            .child("Reset to Defaults"),
                    ),
            )
    }
}
