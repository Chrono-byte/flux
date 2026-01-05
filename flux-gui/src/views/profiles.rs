//! Profiles view - Manage dotfiles profiles.

use crate::app::FluxApp;
use crate::styles::{accent_color, card_bg, text_primary, text_secondary};
use gpui::{Render, Window, div, prelude::*};

pub struct ProfilesView;

impl Default for ProfilesView {
    fn default() -> Self {
        Self::new()
    }
}

impl ProfilesView {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Render for ProfilesView {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let app = cx.global::<FluxApp>();
        let state = &app.state;
        let profiles = state.profiles.clone();
        let current_profile = state.current_profile.clone();

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
                    .child("Profiles"),
            )
            .child(div().text_color(text_secondary()).child("Manage your dotfiles profiles"))
            .child(div().h_4())
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(text_primary())
                    .child("Available Profiles"),
            )
            .child(
                div().flex().flex_col().gap_2().children(profiles.iter().map(|profile| {
                    let is_current = *profile == current_profile;
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .p_3()
                        .bg(card_bg())
                        .rounded_md()
                        .child(
                            div()
                                .flex_1()
                                .text_color(if is_current {
                                    accent_color()
                                } else {
                                    text_primary()
                                })
                                .font_weight(if is_current {
                                    gpui::FontWeight::BOLD
                                } else {
                                    gpui::FontWeight::MEDIUM
                                })
                                .child(if is_current {
                                    format!("{profile} (current)")
                                } else {
                                    profile.clone()
                                }),
                        )
                        .child(
                            div()
                                .px_3()
                                .py_1()
                                .bg(if is_current {
                                    accent_color()
                                } else {
                                    gpui::hsla(0.0, 0.0, 0.15, 1.0)
                                })
                                .text_color(if is_current {
                                    gpui::hsla(0.0, 0.0, 1.0, 1.0)
                                } else {
                                    text_secondary()
                                })
                                .rounded_sm()
                                .text_xs()
                                .cursor_pointer()
                                .child(if is_current { "Active" } else { "Switch" }),
                        )
                })),
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
                            .child("Profile Actions"),
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
                                    .child("+ New Profile"),
                            )
                            .child(
                                div()
                                    .px_4()
                                    .py_2()
                                    .bg(card_bg())
                                    .text_color(text_primary())
                                    .rounded_md()
                                    .cursor_pointer()
                                    .child("Export Profile"),
                            ),
                    ),
            )
    }
}
