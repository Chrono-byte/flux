//! Home view - Dashboard with overview and quick actions.

use crate::app::FluxApp;
use crate::styles::{accent_color, card_bg, text_primary, text_secondary};
use gpui::{Render, Window, div, prelude::*};

pub struct HomeView;

impl Default for HomeView {
    fn default() -> Self {
        Self::new()
    }
}

impl HomeView {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Render for HomeView {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let app = cx.global::<FluxApp>();
        let state = &app.state;
        let sync_summary = state.sync_summary.clone();
        let git_status = state.git_status.clone();

        let synced_color = gpui::hsla(0.35, 0.7, 0.5, 1.0);
        let warning_color = gpui::hsla(0.12, 0.9, 0.55, 1.0);
        let missing_color = gpui::hsla(0.0, 0.0, 0.53, 1.0);
        let conflict_color = gpui::hsla(0.0, 0.8, 0.55, 1.0);

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
            .child(
                div()
                    .text_color(text_secondary())
                    .child(format!("Repository: {}", state.repo_path.display())),
            )
            .child(div().h_4())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_4()
                    .child(
                        div()
                            .flex_1()
                            .p_4()
                            .bg(card_bg())
                            .rounded_md()
                            .child(
                                div()
                                    .text_color(synced_color)
                                    .text_3xl()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .child(sync_summary.synced.to_string()),
                            )
                            .child(div().text_color(text_secondary()).text_sm().child("Synced")),
                    )
                    .child(
                        div()
                            .flex_1()
                            .p_4()
                            .bg(card_bg())
                            .rounded_md()
                            .child(
                                div()
                                    .text_color(warning_color)
                                    .text_3xl()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .child(sync_summary.out_of_sync.to_string()),
                            )
                            .child(
                                div().text_color(text_secondary()).text_sm().child("Out of Sync"),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .p_4()
                            .bg(card_bg())
                            .rounded_md()
                            .child(
                                div()
                                    .text_color(missing_color)
                                    .text_3xl()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .child(sync_summary.missing.to_string()),
                            )
                            .child(div().text_color(text_secondary()).text_sm().child("Missing")),
                    )
                    .child(
                        div()
                            .flex_1()
                            .p_4()
                            .bg(card_bg())
                            .rounded_md()
                            .child(
                                div()
                                    .text_color(conflict_color)
                                    .text_3xl()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .child(sync_summary.broken.to_string()),
                            )
                            .child(div().text_color(text_secondary()).text_sm().child("Conflicts")),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_4()
                    .child(
                        div()
                            .flex_1()
                            .p_4()
                            .bg(card_bg())
                            .rounded_md()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(text_primary())
                                    .child("Git Status"),
                            )
                            .child(div().h_4())
                            .child(div().flex().flex_row().justify_between().child(
                                div().text_color(text_secondary()).child(if git_status.is_repo {
                                    format!("Branch: {}", git_status.branch)
                                } else {
                                    "Not a git repository".to_string()
                                }),
                            )),
                    )
                    .child(
                        div()
                            .flex_1()
                            .p_4()
                            .bg(card_bg())
                            .rounded_md()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(text_primary())
                                    .child("Quick Actions"),
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
                                            .child("Sync All"),
                                    )
                                    .child(
                                        div()
                                            .px_4()
                                            .py_2()
                                            .bg(card_bg())
                                            .text_color(text_primary())
                                            .rounded_md()
                                            .cursor_pointer()
                                            .child("Refresh"),
                                    ),
                            ),
                    ),
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
                            .child("Recent Activity"),
                    )
                    .child(div().h_4())
                    .child(div().text_color(text_secondary()).child("No recent activity")),
            )
    }
}
