//! History view - Operation history.

use crate::app::FluxApp;
use crate::styles::{card_bg, text_primary, text_secondary};
use gpui::{Render, Window, div, prelude::*};

pub struct HistoryView;

impl Default for HistoryView {
    fn default() -> Self {
        Self::new()
    }
}

impl HistoryView {
    #[must_use] 
    pub fn new() -> Self {
        Self
    }
}

impl Render for HistoryView {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let app = cx.global::<FluxApp>();
        let state = &app.state;
        let history = state.history.clone();

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
                    .child("History"),
            )
            .child(div().text_color(text_secondary()).child("View past operations and changes"))
            .child(
                div().flex().flex_row().gap_4().mb_6().child(
                    div().flex_1().p_2().bg(card_bg()).rounded_md().child(
                        div().text_color(text_secondary()).text_sm().child("Search history..."),
                    ),
                ),
            )
            .child(
                div().flex().flex_col().gap_3().children(history.iter().map(|event| {
                    div()
                        .flex()
                        .flex_row()
                        .items_start()
                        .p_4()
                        .bg(card_bg())
                        .rounded_md()
                        .child(
                            div()
                                .w_20()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_color(text_primary())
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .child(event.timestamp.format("%Y-%m-%d").to_string()),
                                )
                                .child(
                                    div()
                                        .text_color(text_secondary())
                                        .text_sm()
                                        .child(event.timestamp.format("%H:%M:%S").to_string()),
                                ),
                        )
                        .child(
                            div()
                                .flex_1()
                                .child(
                                    div()
                                        .text_color(text_primary())
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .child(event.command.clone()),
                                )
                                .child(
                                    div().flex().flex_row().gap_4().mt_2().child(
                                        div().text_color(text_secondary()).text_sm().child(
                                            if event.dry_run { "Dry Run" } else { "Executed" },
                                        ),
                                    ),
                                ),
                        )
                })),
            )
            .child(if history.is_empty() {
                div().w_full().p_8().text_center().bg(card_bg()).rounded_md().child(
                    div()
                        .text_color(text_secondary())
                        .child("No history yet. Operations will appear here as you use Flux."),
                )
            } else {
                div()
            })
    }
}
