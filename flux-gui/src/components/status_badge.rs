//! Status badge component

use crate::state::FileStatus;
use gpui::{Context, Render, Window, div, prelude::*};

/// Status badge component
pub struct StatusBadge {
    /// Status to display
    pub status: FileStatus,
}

impl Render for StatusBadge {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().px_2().py_1().child(div().text_xs().child(self.status.to_string()))
    }
}
