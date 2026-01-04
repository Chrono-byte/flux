//! Remotes view - Manage git remotes.

use crate::styles::{text_primary, text_secondary};
use gpui::{Render, Window, div, prelude::*, px};

pub struct RemotesView;

impl Render for RemotesView {
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
                    .child("Remotes"),
            )
            .child(div().p_4().child(
                div().text_color(text_secondary()).child("Remotes content will appear here"),
            ))
    }
}
