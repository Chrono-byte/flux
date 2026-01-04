//! Home view - Dashboard with overview and quick actions.

use crate::styles::{card_bg, text_primary, text_secondary};
use gpui::{Render, Window, div, prelude::*, px};

pub struct HomeView;

impl Render for HomeView {
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
                    .child("Welcome to Flux"),
            )
            .child(div().p_4().bg(card_bg()).rounded_md().child(
                div().text_color(text_secondary()).child("Dashboard content will appear here"),
            ))
    }
}
