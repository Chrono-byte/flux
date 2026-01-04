//! Profiles view - Manage dotfiles profiles.

use crate::app::FluxApp;
use crate::styles::{accent_color, card_bg, text_primary, text_secondary, window_bg};
use gpui::{Context, Render, Window, div, prelude::*, px};

pub struct ProfilesView;

impl Render for ProfilesView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = &cx.global::<FluxApp>().state;
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
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(text_primary())
                            .child("Profiles"),
                    )
                    .child(
                        div()
                            .px_4()
                            .py_2()
                            .bg(accent_color())
                            .rounded_md()
                            .cursor_pointer()
                            .child(div().text_color(window_bg()).child("New Profile")),
                    ),
            )
            .child(
                div().flex().flex_col().gap_2().children(profiles.into_iter().map(|profile| {
                    let is_active = profile == current_profile;
                    self.render_profile_item(&profile, is_active)
                })),
            )
    }
}

impl ProfilesView {
    fn render_profile_item(&self, name: &str, is_active: bool) -> impl IntoElement {
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
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .w(px(40.0))
                            .h(px(40.0))
                            .bg(if is_active {
                                accent_color()
                            } else {
                                gpui::hsla(0.0, 0.0, 0.15, 1.0)
                            })
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .text_color(if is_active {
                                        window_bg()
                                    } else {
                                        text_primary()
                                    })
                                    .child(name.chars().next().unwrap_or('?').to_string()),
                            ),
                    )
                    .child(div().text_color(text_primary()).child(name)),
            )
            .child(div().flex().gap_2().child(if is_active {
                div().text_color(accent_color()).text_sm().child("Active")
            } else {
                div().text_color(text_secondary()).text_sm().child("Switch")
            }))
    }
}
