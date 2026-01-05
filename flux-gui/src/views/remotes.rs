//! Remotes view - Manage git remotes.

use crate::app::FluxApp;
use crate::styles::{accent_color, card_bg, text_primary, text_secondary};
use gpui::{Render, Window, div, prelude::*};

pub struct RemotesView;

impl Default for RemotesView {
    fn default() -> Self {
        Self::new()
    }
}

impl RemotesView {
    #[must_use] 
    pub fn new() -> Self {
        Self
    }
}

impl Render for RemotesView {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let app = cx.global::<FluxApp>();
        let state = &app.state;
        let remotes = state.remotes.clone();

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
                    .child("Remotes"),
            )
            .child(
                div()
                    .text_color(text_secondary())
                    .child("Manage git remotes for your dotfiles repository"),
            )
            .child(
                div()
                    .w_full()
                    .p_4()
                    .bg(card_bg())
                    .rounded_md()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(text_primary())
                            .child("Add Remote"),
                    )
                    .child(div().h_4())
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_color(text_secondary())
                                            .text_sm()
                                            .child("Remote Name"),
                                    )
                                    .child(
                                        div()
                                            .w_full()
                                            .p_2()
                                            .bg(gpui::hsla(0.0, 0.0, 0.12, 1.0))
                                            .text_color(text_primary())
                                            .rounded_md()
                                            .child("origin"),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_color(text_secondary())
                                            .text_sm()
                                            .child("Remote URL"),
                                    )
                                    .child(
                                        div()
                                            .w_full()
                                            .p_2()
                                            .bg(gpui::hsla(0.0, 0.0, 0.12, 1.0))
                                            .text_color(text_primary())
                                            .rounded_md()
                                            .child("git@github.com:user/dotfiles.git"),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_4()
                                    .child(
                                        div()
                                            .px_4()
                                            .py_2()
                                            .bg(accent_color())
                                            .text_color(gpui::hsla(0.0, 0.0, 1.0, 1.0))
                                            .rounded_md()
                                            .cursor_pointer()
                                            .child("Add Remote"),
                                    )
                                    .child(
                                        div()
                                            .px_4()
                                            .py_2()
                                            .bg(card_bg())
                                            .text_color(text_primary())
                                            .rounded_md()
                                            .cursor_pointer()
                                            .child("Cancel"),
                                    ),
                            ),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .p_4()
                    .bg(card_bg())
                    .rounded_md()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(text_primary())
                            .child("Git Operations"),
                    )
                    .child(div().h_4())
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_4()
                            .child(
                                div()
                                    .px_4()
                                    .py_2()
                                    .bg(accent_color())
                                    .text_color(gpui::hsla(0.0, 0.0, 1.0, 1.0))
                                    .rounded_md()
                                    .cursor_pointer()
                                    .child("Push"),
                            )
                            .child(
                                div()
                                    .px_4()
                                    .py_2()
                                    .bg(accent_color())
                                    .text_color(gpui::hsla(0.0, 0.0, 1.0, 1.0))
                                    .rounded_md()
                                    .cursor_pointer()
                                    .child("Pull"),
                            )
                            .child(
                                div()
                                    .px_4()
                                    .py_2()
                                    .bg(card_bg())
                                    .text_color(text_primary())
                                    .rounded_md()
                                    .cursor_pointer()
                                    .child("Fetch"),
                            ),
                    ),
            )
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(text_primary())
                    .child("Configured Remotes"),
            )
            .child(
                div().flex().flex_col().gap_3().children(remotes.iter().map(|remote| {
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .p_4()
                        .bg(card_bg())
                        .rounded_md()
                        .child(
                            div()
                                .w_10()
                                .h_10()
                                .bg(gpui::hsla(0.0, 0.0, 0.15, 1.0))
                                .rounded_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(div().text_lg().text_color(text_secondary()).child("git")),
                        )
                        .child(
                            div()
                                .ml_4()
                                .flex_1()
                                .child(
                                    div()
                                        .text_color(text_primary())
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .child(remote.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_color(text_secondary())
                                        .text_sm()
                                        .mt_1()
                                        .child(remote.url.clone()),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap_2()
                                .child(
                                    div()
                                        .px_3()
                                        .py_1()
                                        .bg(gpui::hsla(0.0, 0.0, 0.15, 1.0))
                                        .text_color(text_primary())
                                        .rounded_sm()
                                        .text_sm()
                                        .cursor_pointer()
                                        .child("Edit"),
                                )
                                .child(
                                    div()
                                        .px_3()
                                        .py_1()
                                        .bg(gpui::hsla(0.0, 0.8, 0.55, 1.0))
                                        .text_color(gpui::hsla(0.0, 0.0, 1.0, 1.0))
                                        .rounded_sm()
                                        .text_sm()
                                        .cursor_pointer()
                                        .child("Remove"),
                                ),
                        )
                })),
            )
            .child(if remotes.is_empty() {
                div().w_full().p_8().text_center().bg(card_bg()).rounded_md().child(
                    div()
                        .text_color(text_secondary())
                        .child("No remotes configured. Add a remote to sync your dotfiles."),
                )
            } else {
                div()
            })
    }
}
