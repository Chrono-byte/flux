//! Header component

use gpui::{Context, Render, Window, div, prelude::*};

/// Header component
pub struct Header {
    /// Profile name to display
    pub profile: String,
}

impl Render for Header {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w_full()
            .h(crate::styles::dimensions::HEADER_HEIGHT)
            .flex()
            .flex_row()
            .items_center()
            .px_4()
            .gap_4()
            .child(div().text_lg().child("Flux"))
            .child(div().flex_1())
            .child(div().text_sm().child(format!("Profile: {}", self.profile)))
    }
}
