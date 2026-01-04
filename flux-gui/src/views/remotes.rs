//! Remotes view - Manage git remotes.

use crate::app::FluxApp;
use crate::styles::{accent_color, card_bg, text_primary, text_secondary};
use gpui::{Context, Render, Window, div, prelude::*, px};

pub struct RemotesView;

impl Render for RemotesView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = &cx.global::<FluxApp>().state;
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
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(text_primary())
                            .child("Remotes"),
                    )
                    .child(
                        div()
                            .px_4()
                            .py_2()
                            .bg(accent_color())
                            .rounded_md()
                            .cursor_pointer()
                            .child(div().text_color(text_primary()).child("Add Remote")),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .children(remotes.into_iter().map(|remote| self.render_remote_item(&remote))),
            )
    }
}

impl RemotesView {
    fn render_remote_item(&self, remote: &crate::app::GuiRemoteInfo) -> impl IntoElement {
        div()
            .p_4()
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
                    .child(
                        div()
                            .text_color(text_primary())
                            .font_weight(gpui::FontWeight::BOLD)
                            .child(&remote.name),
                    )
                    .child(div().text_color(text_secondary()).text_sm().child(&remote.url)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(div().text_color(accent_color()).text_sm().child("Push"))
                    .child(div().text_color(accent_color()).text_sm().child("Pull"))
                    .child(div().text_color(text_secondary()).text_sm().child("Edit"))
                    .child(
                        div().text_color(gpui::hsla(0.0, 0.8, 0.5, 1.0)).text_sm().child("Remove"),
                    ),
            )
    }
}
