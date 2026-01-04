//! History view - Operation history.

use crate::app::FluxApp;
use crate::state::AppView;
use crate::styles::{card_bg, text_primary, text_secondary};
use gpui::{Context, Render, Window, div, prelude::*, px};

pub struct HistoryView;

impl Render for HistoryView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = &cx.global::<FluxApp>().state;
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
                    .text_lg()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(text_primary())
                    .child("Recent Operations"),
            )
            .child(
                div().flex_1().overflow_y_auto().flex().flex_col().gap_2().children(
                    history.iter().take(20).map(|event| self.render_history_event(event)),
                ),
            )
    }
}

impl HistoryView {
    fn render_history_event(
        &self,
        event: &flux::services::history::HistoryEvent,
    ) -> impl IntoElement {
        let timestamp = event.timestamp.format("%Y-%m-%d %H:%M");
        let command = &event.command;
        let profile = event.profile.as_deref().unwrap_or("");

        div()
            .p_3()
            .bg(card_bg())
            .rounded_md()
            .flex()
            .justify_between()
            .items_center()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().text_color(text_primary()).child(command))
                    .child(
                        div().text_xs().text_color(text_secondary()).child(timestamp.to_string()),
                    ),
            )
            .child(div().text_color(text_secondary()).text_sm().child(profile))
    }
}
