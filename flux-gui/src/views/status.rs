//! Status view - Sync status and operations.

use crate::app::FluxApp;
use crate::state::FileStatus;
use crate::styles::{accent_color, card_bg, text_primary, text_secondary};
use gpui::{Context, Render, Window, div, prelude::*, px};

pub struct StatusView;

impl Render for StatusView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = &cx.global::<FluxApp>().state;
        let sync_summary = state.sync_summary.clone();
        let out_of_sync_files: Vec<_> = state
            .tracked_files
            .iter()
            .filter(|f| f.status == FileStatus::OutOfSync)
            .collect();
        let synced_files: Vec<_> =
            state.tracked_files.iter().filter(|f| f.status == FileStatus::Synced).collect();

        div()
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .gap_6()
            .p_6()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(text_primary())
                            .child("Sync Status"),
                    )
                    .child(
                        div()
                            .px_4()
                            .py_2()
                            .bg(accent_color())
                            .rounded_md()
                            .cursor_pointer()
                            .child(div().text_color(text_primary()).child("Sync All")),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_4()
                    .child(self.render_stat_card(
                        "Synced",
                        sync_summary.synced.to_string(),
                        gpui::hsla(0.46, 0.8, 0.5, 1.0),
                    ))
                    .child(self.render_stat_card(
                        "Out of Sync",
                        sync_summary.out_of_sync.to_string(),
                        gpui::hsla(0.12, 0.8, 0.6, 1.0),
                    ))
                    .child(self.render_stat_card(
                        "Missing",
                        sync_summary.missing.to_string(),
                        gpui::hsla(0.0, 0.8, 0.5, 1.0),
                    ))
                    .child(self.render_stat_card(
                        "Total",
                        sync_summary.total.to_string(),
                        text_primary(),
                    )),
            )
            .child(div().h(px(20.0)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(gpui::hsla(0.12, 0.8, 0.6, 1.0))
                            .child(format!("Out of Sync ({})", out_of_sync_files.len())),
                    )
                    .child(
                        div().flex().flex_col().gap_2().children(out_of_sync_files.iter().map(
                            |file| {
                                div()
                                    .p_3()
                                    .bg(card_bg())
                                    .rounded_md()
                                    .text_color(text_secondary())
                                    .child(&file.dest_path)
                            },
                        )),
                    ),
            )
    }
}

impl StatusView {
    fn render_stat_card(&self, label: &str, value: String, color: gpui::Hsla) -> impl IntoElement {
        div().p_4().bg(card_bg()).rounded_md().w(px(140.0)).child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_xl()
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(color)
                        .child(value),
                )
                .child(div().text_sm().text_color(text_secondary()).child(label)),
        )
    }
}
