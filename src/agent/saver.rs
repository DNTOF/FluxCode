//! Token saver: response-style packs that make the agent spend fewer tokens.
//!
//! A pack is a few lines of guidance sent with a message, not a setting of the agent:
//! - Caveman (Lite / Full / Ultra): shorter answers, from "no filler" to telegraphic.
//! - Lean code: changed lines or a small diff, never whole unchanged files.
//! - Quiet tools: read and print only what the task needs.
//!
//! The guidance goes out with the next message when the packs change (or a new session
//! starts), then again every few turns so a long chat doesn't drift; in between nothing
//! is added, so the packs themselves cost almost nothing. Turning every pack off sends
//! one line that cancels the earlier guidance.

use crate::i18n::tr;
use crate::theme::{self, c, GlassExt};
use crate::workbench::Workbench;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex,
    switch::Switch,
    v_flex, Icon, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};

/// Sent again after this many turns with the same packs.
const REMIND_EVERY: u32 = 8;

pub const PACKS: [(&str, &str, &str); 3] = [
    ("caveman", "Caveman", "Short answers: no filler, no recap."),
    ("lean", "Lean code", "Diffs and changed lines, not whole files."),
    ("quiet", "Quiet tools", "Reads and prints only what the task needs."),
];

pub const LEVELS: [(&str, &str); 3] = [("lite", "Lite"), ("full", "Full"), ("ultra", "Ultra")];

#[derive(Default)]
pub struct SaverState {
    pub open: bool,
    /// (session, packs key) the guidance last went out for.
    sent: Option<(String, String)>,
    /// Turns sent since then.
    turns: u32,
    /// When the packs last changed, for the panel's "since" line.
    pub changed: Option<std::time::Instant>,
}

fn caveman(level: &str) -> &'static str {
    match level {
        "lite" => "- Be brief: no greetings, no restating the question, no closing summary. Full sentences are fine.",
        "ultra" => "- Telegraphic: the shortest wording that keeps the meaning (keywords, arrows, symbols). No prose around code.",
        _ => "- Terse: drop filler, hedging and pleasantries; fragments are fine.",
    }
}

fn pack_line(id: &str) -> &'static str {
    match id {
        "lean" => "- Code: show only the changed lines or a minimal diff; never re-print unchanged files or code the user already has; no comments unless asked.",
        "quiet" => "- Tools: search before reading, read line ranges instead of whole files, cap command output (quiet flags, head/tail), don't re-read files already read, don't narrate tool calls.",
        _ => "",
    }
}

/// The guidance for these packs, or None when every pack is off.
pub fn guidance(packs: &[String], level: &str) -> Option<String> {
    let mut lines = Vec::new();
    for (id, ..) in PACKS {
        if packs.iter().any(|p| p == id) {
            lines.push(if id == "caveman" { caveman(level) } else { pack_line(id) });
        }
    }
    if lines.is_empty() {
        return None;
    }
    Some(format!(
        "Response style (FluxCode token saver; keep following it until told otherwise):\n{}\n- Code, commands, file paths and error text always stay exact and complete.",
        lines.join("\n")
    ))
}

/// Rough share of output tokens the packs save (shown as a hint, not a measurement).
pub fn estimate(packs: &[String], level: &str) -> f32 {
    let keep = packs.iter().fold(1.0f32, |k, p| {
        let s = match p.as_str() {
            "caveman" => match level {
                "lite" => 0.12,
                "ultra" => 0.35,
                _ => 0.25,
            },
            "lean" => 0.12,
            "quiet" => 0.15,
            _ => 0.,
        };
        k * (1. - s)
    });
    1. - keep
}

impl Workbench {
    fn saver_key(&self) -> String {
        let s = &self.settings;
        if s.saver_packs.is_empty() {
            String::new()
        } else {
            format!("{}:{}", s.saver_packs.join(","), s.saver_level)
        }
    }

    /// The text block to send with this message, if any (see the module notes).
    pub(crate) fn agent_saver_block(&mut self) -> Option<String> {
        let session = self.agent.session.clone().unwrap_or_default();
        let key = self.saver_key();
        let st = &mut self.agent.saver;
        let same = st.sent.as_ref().is_some_and(|(s, k)| *s == session && *k == key);
        if same {
            st.turns += 1;
            if key.is_empty() || st.turns < REMIND_EVERY {
                return None;
            }
        } else if key.is_empty() {
            // All off: cancel only guidance this session has actually seen.
            let had = st.sent.as_ref().is_some_and(|(s, k)| *s == session && !k.is_empty());
            if !had {
                st.sent = Some((session, key));
                return None;
            }
        }
        st.sent = Some((session, key));
        st.turns = 0;
        Some(guidance(&self.settings.saver_packs, &self.settings.saver_level).unwrap_or_else(|| "Response style: the earlier FluxCode token saver rules no longer apply. Answer normally.".into()))
    }

    fn saver_set(&mut self, pack: &str, on: bool, cx: &mut Context<Self>) {
        let packs = &mut self.settings.saver_packs;
        packs.retain(|p| p != pack);
        if on {
            packs.push(pack.into());
        }
        self.agent.saver.changed = Some(std::time::Instant::now());
        self.settings.save();
        cx.notify();
    }

    /// The leaf button under the message box, with the number of packs on.
    pub(crate) fn render_saver_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let n = self.settings.saver_packs.len();
        let open = self.agent.saver.open;
        h_flex()
            .id("ag-saver")
            .flex_none()
            .h(theme::u(24.))
            .px_1p5()
            .gap_1()
            .rounded(px(6.))
            .cursor_pointer()
            .text_xs()
            .text_color(c(if n > 0 { theme::GREEN } else { theme::FG_DIM }))
            .when(n > 0, |d| d.bg(c(theme::GREEN).opacity(0.12)))
            .when(open, |d| d.bg(c(theme::BG_HOVER)))
            .hover(|s| s.bg(c(theme::BG_HOVER)))
            .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new(tr("Token saver")).build(w, cx))
            .on_click(cx.listener(|this, _, _, cx| {
                cx.stop_propagation();
                let may = this.pop_may_open("saver");
                this.agent.saver.open = !this.agent.saver.open && may;
                cx.notify();
            }))
            .child(Icon::new(gpui_kit::assets::IconName::Leaf).size(theme::u(13.)))
            .when(n > 0, |d| d.child(n.to_string()))
    }

    /// The token saver panel, floating over the chat above the message box.
    pub(crate) fn render_saver_panel(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.agent.saver.open {
            return None;
        }
        let s = &self.settings;
        let on = |id: &str| s.saver_packs.iter().any(|p| p == id);
        let pct = (estimate(&s.saver_packs, &s.saver_level) * 100.).round() as u32;
        let since = self.agent.saver.changed.map(|t| {
            let m = t.elapsed().as_secs() / 60;
            if m == 0 { tr("Packs changed just now.").to_string() } else { crate::trf!("Packs changed {0}m ago; they go out with your next message."; 0 => m) }
        });
        let mut rows = v_flex().gap_2();
        for (id, title, sub) in PACKS {
            let checked = on(id);
            rows = rows.child(
                v_flex()
                    .gap_1()
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .child(div().text_sm().font_weight(FontWeight::MEDIUM).text_color(c(theme::FG_STRONG)).child(tr(title)))
                                    .child(div().text_xs().text_color(c(theme::FG_DIM)).child(tr(sub))),
                            )
                            .child(Switch::new(SharedString::from(format!("sv-{id}"))).checked(checked).on_click(cx.listener(move |this, v: &bool, _, cx| this.saver_set(id, *v, cx)))),
                    )
                    .when(id == "caveman" && checked, |v| {
                        // A segmented control: the chosen level sits on a raised pill.
                        let mut seg = h_flex().p_0p5().gap_0p5().rounded(px(8.)).bg(c(theme::BG_HOVER));
                        for (lv, label) in LEVELS {
                            let on = s.saver_level == lv;
                            seg = seg.child(
                                div()
                                    .id(SharedString::from(format!("sv-l-{lv}")))
                                    .flex_1()
                                    .py_0p5()
                                    .rounded(px(6.))
                                    .text_center()
                                    .text_sm()
                                    .cursor_pointer()
                                    .text_color(c(if on { theme::FG_STRONG } else { theme::FG_MUTED }))
                                    .when(on, |d| d.bg(c(theme::BG_ELEVATED)).border_1().border_color(theme::glass_edge()).shadow_sm())
                                    .when(!on, |d| d.hover(|s| s.text_color(c(theme::FG))))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.settings.saver_level = lv.into();
                                        this.agent.saver.changed = Some(std::time::Instant::now());
                                        this.settings.save();
                                        cx.notify();
                                    }))
                                    .child(tr(label)),
                            );
                        }
                        v.child(seg)
                    }),
            );
        }
        Some(
            v_flex()
                .id("ag-saver-panel")
                .occlude()
                .w(theme::u(300.))
                .p_3()
                .gap_3()
                .glass_pop()
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    this.pop_closed_outside("saver");
                    this.agent.saver.open = false;
                    cx.notify();
                }))
                .child(
                    h_flex()
                        .child(div().flex_1().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(tr("Token saver")))
                        .when(pct > 0, |h| h.child(div().text_sm().text_color(c(theme::GREEN)).child(format!("≈ −{pct}%")))),
                )
                .child(div().text_xs().text_color(c(theme::FG_DIM)).child(since.unwrap_or_else(|| tr("Guidance goes out with your next message, then every few turns.").to_string())))
                .child(rows)
                .child(
                    h_flex().child(div().flex_1()).child(Button::new("sv-off").label(tr("Turn all off")).xsmall().ghost().on_click(cx.listener(|this, _, _, cx| {
                        this.settings.saver_packs.clear();
                        this.agent.saver.changed = Some(std::time::Instant::now());
                        this.settings.save();
                        cx.notify();
                    }))),
                )
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{estimate, guidance};

    #[test]
    fn guidance_lists_only_the_packs_that_are_on() {
        assert!(guidance(&[], "full").is_none());
        let g = guidance(&["quiet".into(), "caveman".into()], "ultra").unwrap();
        assert!(g.contains("Telegraphic") && g.contains("Tools:") && !g.contains("Code: show"));
        assert!(g.find("Telegraphic").unwrap() < g.find("Tools:").unwrap());
    }

    #[test]
    fn savings_combine_without_passing_100_percent() {
        assert_eq!(estimate(&[], "full"), 0.);
        let all = estimate(&["caveman".into(), "lean".into(), "quiet".into()], "ultra");
        assert!(all > 0.35 && all < 0.6, "{all}");
    }
}
