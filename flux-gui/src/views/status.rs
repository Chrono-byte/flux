//! Status view - Sync status and operations.

use crate::app::FluxApp;
use crate::styles::{accent_color, card_bg, text_primary, text_secondary};
use gpui::{Render, Window, div, prelude::*};

pub struct StatusView;

impl StatusView {
    pub fn new() -> Self {
        Self
    }
}

impl Render for StatusView {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let app = cx.global::<FluxApp>();
        let state = &app.state;
        let summary = state.sync_summary.clone();

        let synced_color = gpui::hsla(0.35, 0.7, 0.5, 1.0);
        let warning_color = gpui::hsla(0.12, 0.9, 0.55, 1.0);
        let missing_color = gpui::hsla(0.0, 0.0, 0.53, 1.0);
        let conflict_color = gpui::hsla(0.0, 0.8, 0.55, 1.0);

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
                    .child("Sync Status"),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_4()
                    .child(
                        div()
                            .flex_1()
                            .p_4()
                            .bg(card_bg())
                            .rounded_md()
                            .child(
                                div()
                                    .text_color(synced_color)
                                    .text_2xl()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .child(summary.synced.to_string()),
                            )
                            .child(div().text_color(text_secondary()).text_sm().child("Synced")),
                    )
                    .child(
                        div()
                            .flex_1()
                            .p_4()
                            .bg(card_bg())
                            .rounded_md()
                            .child(
                                div()
                                    .text_color(warning_color)
                                    .text_2xl()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .child(summary.out_of_sync.to_string()),
                            )
                            .child(
                                div().text_color(text_secondary()).text_sm().child("Out of Sync"),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .p_4()
                            .bg(card_bg())
                            .rounded_md()
                            .child(
                                div()
                                    .text_color(missing_color)
                                    .text_2xl()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .child(summary.missing.to_string()),
                            )
                            .child(div().text_color(text_secondary()).text_sm().child("Missing")),
                    )
                    .child(
                        div()
                            .flex_1()
                            .p_4()
                            .bg(card_bg())
                            .rounded_md()
                            .child(
                                div()
                                    .text_color(conflict_color)
                                    .text_2xl()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .child(summary.broken.to_string()),
                            )
                            .child(div().text_color(text_secondary()).text_sm().child("Conflicts")),
                    ),
            )
            .child(
                div().flex().flex_row().gap_4().child(
                    div()
                        .px_4()
                        .py_2()
                        .bg(accent_color())
                        .text_color(gpui::hsla(0.0, 0.0, 1.0, 1.0))
                        .rounded_md()
                        .cursor_pointer()
                        .child("Sync All"),
                ),
            )
    }
}
