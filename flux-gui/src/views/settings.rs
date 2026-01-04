//! Settings view - Application settings and configuration.

use crate::app::FluxApp;
use crate::styles::{accent_color, card_bg, text_primary, text_secondary};
use gpui::{Context, Render, Window, div, prelude::*, px};

pub struct SettingsView;

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = &cx.global::<FluxApp>().state;
        let repo_path = state.repo_path.display().to_string();
        let config_result = &state.config;

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
            .child(
                div().flex().flex_col().gap_4().child(
                    div()
                        .p_4()
                        .bg(card_bg())
                        .rounded_md()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(
                            div()
                                .text_lg()
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(text_primary())
                                .child("General"),
                        )
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .child(div().text_color(text_secondary()).child("Repository"))
                                .child(div().text_color(text_primary()).child(repo_path)),
                        ),
                ),
            )
    }
}
