//! The agent's full-screen workspace: chats on the left, the conversation in the
//! middle, the files it changed on the right, each a floating glass column.
//!
//! The docked chat stays compact (one column beside the code); this is where the agent
//! gets the room for everything. Columns give way as the window narrows: the changes
//! column goes first, then the chats, and each can be shown or hidden from the top bar.

use super::{ago, Item, Status};
use crate::i18n::tr;
use crate::theme::{self, c, GlassExt};
use crate::workbench::{tool_button, Workbench};
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex, v_flex, Icon, IconName, Selectable, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};

/// Below these window widths the changes column, then the chats column, step aside.
const WIDE: f32 = 1180.;
const MEDIUM: f32 = 860.;

pub struct FullState {
    /// The user hid (false) or showed the side columns from the top bar.
    pub left: bool,
    pub right: bool,
}

impl Default for FullState {
    fn default() -> Self {
        FullState { left: true, right: true }
    }
}

impl Workbench {
    pub(crate) fn render_agent_full(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let w = f32::from(window.viewport_size().width);
        let left = self.agent.full.left && w >= MEDIUM;
        let right = self.agent.full.right && w >= WIDE;
        let gap = theme::u(10.);
        h_flex()
            .id("ag-full")
            .size_full()
            .p(gap)
            .gap(gap)
            .when(left, |d| d.child(self.render_full_chats(cx)))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .glass()
                    .overflow_hidden()
                    .child(self.render_full_top(w, cx))
                    .children(self.render_worktree_strip(cx))
                    .child(h_flex().flex_1().min_h_0().justify_center().child(div().h_full().w_full().max_w(theme::u(900.)).child(self.render_agent(window, cx)))),
            )
            .when(right, |d| d.child(self.render_full_changes(cx)))
            .with_animation("ag-full-in", Animation::new(std::time::Duration::from_millis(180)).with_easing(ease_out_quint()), |d, t| d.opacity(t))
            .into_any_element()
    }

    /// The bar over the conversation: agent, its switch, side column toggles, tools.
    fn render_full_top(&self, w: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let toggle = |id: &'static str, icon: IconName, on: bool, can: bool, tip: &'static str| {
            Button::new(id).icon(Icon::new(icon)).xsmall().ghost().selected(on && can).tooltip(tip)
        };
        h_flex()
            .h(theme::u(42.))
            .flex_none()
            .px_3()
            .gap_1()
            .border_b_1()
            .border_color(theme::glass_edge())
            .child(toggle("agf-left", IconName::PanelLeft, self.agent.full.left, w >= MEDIUM, tr("Chats")).on_click(cx.listener(|this, _, _, cx| {
                this.agent.full.left = !this.agent.full.left;
                cx.notify();
            })))
            .child(self.render_agent_picker(cx))
            .child(self.render_agent_switch(cx))
            .child(div().flex_1())
            .child(self.render_agent_tools(cx))
            .child(self.render_agent_more(cx))
            .child(toggle("agf-right", IconName::PanelRight, self.agent.full.right, w >= WIDE, tr("Changes")).on_click(cx.listener(|this, _, _, cx| {
                this.agent.full.right = !this.agent.full.right;
                cx.notify();
            })))
    }

    /// Left column: new chat and the saved chats, the open one marked.
    fn render_full_chats(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let a = &self.agent;
        let busy = a.status == Status::Busy;
        let mut list = v_flex().id("agf-chats").flex_1().min_h_0().px_2().gap_px().overflow_y_scroll();
        let fresh = a.chat_id.is_none() && a.items.iter().any(|i| matches!(i, Item::User(_)));
        if fresh {
            let title = a.items.iter().find_map(|i| if let Item::User(t) = i { Some(t.lines().next().unwrap_or("").to_string()) } else { None }).unwrap_or_default();
            list = list.child(chat_row("agf-now", title, tr("Now").into(), true, busy));
        }
        for (i, chat) in a.history.iter().enumerate() {
            let id = chat.id;
            let current = a.chat_id == Some(id);
            list = list.child(
                chat_row(SharedString::from(format!("agf-c-{i}")), chat.title.clone(), ago(chat.updated).into(), current, current && busy)
                    .on_click(cx.listener(move |this, _, _, cx| this.agent_open_chat(id, cx))),
            );
        }
        if a.history.is_empty() && !fresh {
            list = list.child(div().px_2().py_2().text_xs().text_color(c(theme::FG_DIM)).child(tr("No chats yet. Each answer saves the chat here.")));
        }
        v_flex()
            .w(theme::u(250.))
            .flex_none()
            .h_full()
            .glass()
            .overflow_hidden()
            .child(
                v_flex().p_2().gap_1().child(
                    h_flex()
                        .id("agf-new")
                        .h(theme::u(34.))
                        .px_2()
                        .gap_2()
                        .rounded(px(10.))
                        .cursor_pointer()
                        .text_sm()
                        .text_color(c(theme::FG_STRONG))
                        .hover(|s| s.bg(c(theme::BG_HOVER)))
                        .on_click(cx.listener(|this, _, _, cx| this.agent_new_chat(cx)))
                        .child(Icon::new(IconName::Plus).size(theme::u(15.)))
                        .child(tr("New chat")),
                ),
            )
            .child(div().px_4().pt_1().pb_1().text_xs().font_weight(FontWeight::MEDIUM).text_color(c(theme::FG_DIM)).child(tr("Chats")))
            .child(list)
    }

    /// Right column: every file the agent changed, with its counts, a diff on click.
    fn render_full_changes(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let a = &self.agent;
        let (add, del) = a.changes.iter().fold((0, 0), |t, ch| (t.0 + ch.added, t.1 + ch.removed));
        let mut list = v_flex().id("agf-changes").flex_1().min_h_0().px_2().gap_px().overflow_y_scroll();
        for (i, ch) in a.changes.iter().enumerate() {
            let name = ch.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let dir = ch.path.parent().map(|p| self.rel_display(&p.to_string_lossy())).unwrap_or_default();
            let (icon, tint) = crate::files::icon_for(&name);
            list = list.child(
                h_flex()
                    .id(("agf-ch", i))
                    .px_2()
                    .py_1()
                    .gap_2()
                    .rounded(px(8.))
                    .cursor_pointer()
                    .text_sm()
                    .hover(|s| s.bg(c(theme::BG_HOVER)))
                    .on_click(cx.listener(move |this, _, w, cx| {
                        this.agent.user_nav = Some(std::time::Instant::now());
                        this.agent_show_diff(i, w, cx)
                    }))
                    .child(Icon::new(icon).flex_none().size(theme::u(14.)).text_color(theme::tint(tint)))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().truncate().text_color(c(theme::FG_STRONG)).child(name))
                            .child(div().truncate().text_xs().text_color(c(theme::FG_DIM)).child(dir)),
                    )
                    .child(if ch.counted {
                        h_flex()
                            .flex_none()
                            .gap_1()
                            .text_xs()
                            .child(div().text_color(c(theme::GREEN)).child(format!("+{}", ch.added)))
                            .child(div().text_color(c(theme::DANGER)).child(format!("−{}", ch.removed)))
                            .into_any_element()
                    } else {
                        div().flex_none().text_xs().text_color(c(theme::FG_DIM)).child("…").into_any_element()
                    })
                    .child(
                        div()
                            .id(("agf-undo", i))
                            .flex_none()
                            .p_1()
                            .rounded(px(6.))
                            .text_color(c(theme::FG_DIM))
                            .hover(|s| s.bg(c(theme::BG_ELEVATED)).text_color(c(theme::FG_STRONG)))
                            .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new(tr("Undo this file")).build(w, cx))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                this.agent_undo(i, cx)
                            }))
                            .child(Icon::new(gpui_kit::assets::IconName::Undo2).size(theme::u(13.))),
                    ),
            );
        }
        if a.changes.is_empty() {
            list = list.child(div().px_2().py_2().text_xs().text_color(c(theme::FG_DIM)).child(tr("Files the agent edits show up here, with a diff a click away.")));
        }
        v_flex()
            .w(theme::u(300.))
            .flex_none()
            .h_full()
            .glass()
            .overflow_hidden()
            .child(
                h_flex()
                    .h(theme::u(42.))
                    .flex_none()
                    .px_3()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme::glass_edge())
                    .child(Icon::new(gpui_kit::assets::IconName::FileDiff).size(theme::u(14.)).text_color(c(theme::FG_MUTED)))
                    .child(div().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(tr("Changes")))
                    .child(div().text_xs().text_color(c(theme::FG_DIM)).child(a.changes.len().to_string()))
                    .child(div().flex_1())
                    .when(!a.changes.is_empty(), |h| {
                        h.child(div().text_xs().text_color(c(theme::GREEN)).child(format!("+{add}"))).child(div().text_xs().text_color(c(theme::DANGER)).child(format!("−{del}")))
                    })
                    .child(tool_button("agf-exit", IconName::Minimize, tr("Back beside the editor"), cx, |this, _, cx| {
                        this.agent_full = false;
                        cx.notify();
                    })),
            )
            .child(list.pt_1())
    }
}

/// One chat in the left column: title, when, and a pulse while it is answering.
fn chat_row(id: impl Into<ElementId>, title: String, when: SharedString, current: bool, busy: bool) -> Stateful<Div> {
    h_flex()
        .id(id)
        .w_full()
        .px_2()
        .py_1p5()
        .gap_2()
        .rounded(px(10.))
        .cursor_pointer()
        .when(current, |d| d.bg(c(theme::BG_HOVER)).border_1().border_color(theme::glass_edge()))
        .hover(|s| s.bg(c(theme::BG_HOVER)))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .child(div().text_sm().truncate().text_color(c(if current { theme::FG_STRONG } else { theme::FG })).child(if title.is_empty() { tr("Untitled chat").to_string() } else { title }))
                .child(div().text_xs().text_color(c(theme::FG_DIM)).child(when)),
        )
        .when(busy, |d| {
            d.child(
                div()
                    .flex_none()
                    .size(px(7.))
                    .rounded_full()
                    .bg(c(theme::ACCENT))
                    .with_animation("agf-pulse", Animation::new(std::time::Duration::from_millis(1100)).repeat(), |d, t| d.opacity(0.35 + 0.65 * (1. - (t * 2. - 1.).abs()))),
            )
        })
}
