//! Files view - Manage tracked dotfiles.

use crate::app::FluxApp;
use crate::state::FileStatus;
use crate::styles::{accent_color, border_color, card_bg, text_primary, text_secondary};
use gpui::{Context, Render, Window, div, prelude::*, px};

pub struct FilesView;

impl Render for FilesView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = &cx.global::<FluxApp>().state;
        let files = state.filtered_files();

        div()
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .p_4()
                    .border_b_1()
                    .border_color(border_color())
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_4()
                            .child(div().text_color(text_primary()).child("Search files...")),
                    )
                    .child(
                        div()
                            .text_color(text_secondary())
                            .text_sm()
                            .child(format!("{} files", files.len())),
                    ),
            )
            .child(
                div().flex_1().overflow_y_auto().p_4().child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .children(files.iter().map(|file| self.render_file_item(file))),
                ),
            )
    }
}

impl FilesView {
    fn render_file_item(&self, file: &crate::app::TrackedFileState) -> impl IntoElement {
        let status_color = match file.status {
            FileStatus::Synced => gpui::hsla(0.46, 0.8, 0.5, 1.0),
            FileStatus::OutOfSync => gpui::hsla(0.12, 0.8, 0.6, 1.0),
            FileStatus::Missing => gpui::hsla(0.0, 0.8, 0.5, 1.0),
            FileStatus::Conflict => gpui::hsla(0.0, 0.8, 0.6, 1.0),
            FileStatus::Pending => gpui::hsla(0.12, 0.8, 0.5, 1.0),
        };

        let file_name = file.dest_path.split('/').last().unwrap_or(&file.dest_path);

        div()
            .p_3()
            .bg(card_bg())
            .rounded_md()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .w(px(32.0))
                            .h(px(32.0))
                            .bg(status_color)
                            .rounded_md()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(gpui::hsla(0.0, 0.0, 1.0, 1.0))
                                    .child(file.tool.chars().next().unwrap_or('?').to_string()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(div().text_color(text_primary()).child(file_name))
                            .child(div().text_xs().text_color(text_secondary()).child(&file.tool)),
                    ),
            )
    }
}
