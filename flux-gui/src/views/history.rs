//! History view - Operation history.

use crate::styles::{text_primary, text_secondary};
use gpui::{Render, Window, div, prelude::*, px};

pub struct HistoryView;

impl Render for HistoryView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
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
            .child(div().p_4().child(
                div().text_color(text_secondary()).child("History content will appear here"),
            ))
    }
}
