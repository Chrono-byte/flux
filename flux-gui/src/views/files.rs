//! Files view - Manage tracked dotfiles.

use crate::app::FluxApp;
use crate::state::FileStatus;
use crate::styles::{accent_color, card_bg, text_primary, text_secondary};
use gpui::{Render, Window, div, prelude::*};

pub struct FilesView;

impl Default for FilesView {
    fn default() -> Self {
        Self::new()
    }
}

impl FilesView {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Render for FilesView {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let app = cx.global::<FluxApp>();
        let state = &app.state;
        let files = state.tracked_files.clone();

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
                    .child("Files"),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .child(
                        div()
                            .text_color(text_secondary())
                            .child(format!("{} tracked files", files.len())),
                    )
                    .child(
                        div()
                            .px_4()
                            .py_2()
                            .bg(accent_color())
                            .text_color(gpui::hsla(0.0, 0.0, 1.0, 1.0))
                            .rounded_md()
                            .cursor_pointer()
                            .child("+ Add File"),
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
                            .child("Add New File"),
                    )
                    .child(div().h_4())
                    .child(
                        div().flex().flex_row().gap_4().child(
                            div()
                                .px_4()
                                .py_2()
                                .bg(accent_color())
                                .text_color(gpui::hsla(0.0, 0.0, 1.0, 1.0))
                                .rounded_md()
                                .cursor_pointer()
                                .child("Add File"),
                        ),
                    ),
            )
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(text_primary())
                    .child("Tracked Files"),
            )
            .child(
                div().flex().flex_col().gap_2().children(files.iter().map(|file| {
                    let status_color = match file.status {
                        FileStatus::Synced => gpui::hsla(0.35, 0.7, 0.5, 1.0),
                        FileStatus::OutOfSync => gpui::hsla(0.12, 0.9, 0.55, 1.0),
                        FileStatus::Conflict => gpui::hsla(0.0, 0.8, 0.55, 1.0),
                        FileStatus::Pending => gpui::hsla(0.0, 0.0, 0.53, 1.0),
                    };
                    let status_text = match file.status {
                        FileStatus::Synced => "✓ Synced",
                        FileStatus::OutOfSync => "↻ Out of Sync",
                        FileStatus::Conflict => "⚠ Conflict",
                        FileStatus::Pending => "⏳ Pending",
                    };
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .p_3()
                        .bg(card_bg())
                        .rounded_md()
                        .child(div().w_3().h_3().rounded_full().bg(status_color))
                        .child(
                            div()
                                .ml_3()
                                .flex_1()
                                .child(
                                    div()
                                        .text_color(text_primary())
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .child(file.dest_path.clone()),
                                )
                                .child(
                                    div()
                                        .text_color(text_secondary())
                                        .text_xs()
                                        .mt_1()
                                        .child(format!("{} • {}", file.tool, file.repo_path)),
                                ),
                        )
                        .child(
                            div()
                                .px_2()
                                .py_1()
                                .bg(gpui::hsla(status_color.h, status_color.s, 0.15, 1.0))
                                .text_color(status_color)
                                .rounded_sm()
                                .text_xs()
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .child(status_text),
                        )
                })),
            )
    }
}
