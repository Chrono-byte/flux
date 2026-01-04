//! Sidebar navigation component

use crate::state::AppView;
use gpui::{Context, Render, Window, div, prelude::*};

/// Sidebar component
pub struct Sidebar {
    /// Current active view
    pub current_view: AppView,
}

impl Render for Sidebar {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let items = vec![
            ("🏠", "Home"),
            ("📁", "Files"),
            ("📊", "Status"),
            ("👥", "Profiles"),
            ("🌐", "Remotes"),
            ("⚙️", "Settings"),
            ("📋", "History"),
        ];

        div()
            .w(crate::styles::dimensions::SIDEBAR_WIDTH)
            .h_full()
            .flex()
            .flex_col()
            .gap_1()
            .p_2()
            .children(items.into_iter().map(|(icon, label)| {
                div()
                    .w_full()
                    .px_3()
                    .py_2()
                    .flex()
                    .gap_2()
                    .child(div().text_base().child(icon))
                    .child(div().text_sm().child(label))
            }))
    }
}
