//! The Marketplace in the Extensions view: live search, rows laid out like VS Code's
//! (icon, name, installs and rating, one line of description, publisher and the
//! button), and an extension's page in the editor area beside the built-in ones.

use super::market::{self, Busy, Item};
use super::view::tile;
use super::Filter;
use crate::i18n::tr;
use crate::theme::{self, c};
use crate::workbench::*;
use gpui_kit::assets::IconName as Ico;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex,
    spinner::Spinner,
    text::TextView,
    v_flex, Icon, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};
use std::path::PathBuf;
use std::time::Duration;

/// Page ids of Marketplace extensions start with this; built-in ids have no colon.
pub const PAGE: &str = "vsx:";
/// Quiet time after a keystroke before the Marketplace is asked.
const DEBOUNCE_MS: u64 = 300;
/// Icons downloaded at the same time.
const ICON_WORKERS: usize = 6;

/// A Marketplace extension's icon, or the Blocks tile until it is on disk.
fn vsx_icon(path: Option<&PathBuf>, size: f32) -> AnyElement {
    match path {
        Some(p) => img(p.clone())
            .flex_none()
            .size(theme::u(size))
            .rounded(px(size * 0.12))
            .object_fit(ObjectFit::Contain)
            .into_any_element(),
        None => tile("", size).into_any_element(),
    }
}

/// What a row shows: the item, and its icon once it is on disk.
struct Row {
    item: Item,
    icon: Option<PathBuf>,
}

impl Workbench {
    /// The query box changed: search again once typing pauses, on the tabs that show
    /// the Marketplace (the others search when they're left).
    pub(crate) fn market_query_changed(&mut self, cx: &mut Context<Self>) {
        if matches!(self.ext.filter, Filter::All | Filter::Market) {
            self.market_search(DEBOUNCE_MS, cx);
        }
        cx.notify();
    }

    /// A tab was picked: search now when the shown answer is for another query.
    pub(crate) fn market_tab_shown(&mut self, cx: &mut Context<Self>) {
        let q = self.ext.query.read(cx).value().trim().to_string();
        if matches!(self.ext.filter, Filter::All | Filter::Market)
            && self.ext.market.answered.as_deref() != Some(q.as_str())
        {
            self.market_search(0, cx);
        }
    }

    /// Ask the Marketplace for the query box's text after `delay` ms. Each call
    /// bumps the generation; a wait or an answer that belongs to an older one is
    /// dropped, so a slow reply never replaces a newer list.
    pub(crate) fn market_search(&mut self, delay: u64, cx: &mut Context<Self>) {
        let q = self.ext.query.read(cx).value().trim().to_string();
        let m = &mut self.ext.market;
        m.kicked.set(true);
        m.generation += 1;
        let generation = m.generation;
        cx.spawn(async move |this, cx| {
            if delay > 0 {
                cx.background_executor()
                    .timer(Duration::from_millis(delay))
                    .await;
            }
            let current = this
                .update(cx, |this, cx| {
                    let m = &mut this.ext.market;
                    m.loading = m.generation == generation;
                    cx.notify();
                    m.loading
                })
                .unwrap_or(false);
            if !current {
                return;
            }
            let asked = q.clone();
            let res = cx
                .background_spawn(async move { market::search(&asked) })
                .await;
            _ = this.update(cx, |this, cx| {
                let m = &mut this.ext.market;
                if m.generation != generation {
                    return;
                }
                m.loading = false;
                match res {
                    Ok(list) => {
                        m.results = list;
                        m.error = None;
                        m.answered = Some(q);
                        this.market_fetch_icons(cx);
                    }
                    Err(e) => m.error = Some(e),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Download the icons the list doesn't have yet, a few at a time.
    fn market_fetch_icons(&mut self, cx: &mut Context<Self>) {
        let m = &mut self.ext.market;
        let mut todo: Vec<(String, Vec<String>, PathBuf)> = Vec::new();
        for it in &m.results {
            let id = it.id();
            if m.icons.contains_key(&id) || !m.icons_tried.insert(id.clone()) {
                continue;
            }
            let Some(to) = it.icon_cache() else { continue };
            let urls: Vec<String> = [market::ICON_SMALL, market::ICON_DEFAULT]
                .iter()
                .filter_map(|k| it.asset(k))
                .collect();
            todo.push((id, urls, to));
        }
        for w in 0..ICON_WORKERS {
            let part: Vec<_> = todo.iter().skip(w).step_by(ICON_WORKERS).cloned().collect();
            if part.is_empty() {
                break;
            }
            cx.spawn(async move |this, cx| {
                for (id, urls, to) in part {
                    let path = to.clone();
                    if cx
                        .background_spawn(async move { market::fetch_icon(&urls, &path) })
                        .await
                    {
                        _ = this.update(cx, |this, cx| {
                            this.ext.market.icons.insert(id, to);
                            cx.notify();
                        });
                    }
                }
            })
            .detach();
        }
    }

    pub(crate) fn market_install(&mut self, item: Item, cx: &mut Context<Self>) {
        let id = item.id();
        if self.ext.market.busy.contains_key(&id) {
            return;
        }
        self.ext.market.busy.insert(id.clone(), Busy::Installing);
        self.ext.note = None;
        let name = item.display.clone();
        cx.spawn(async move |this, cx| {
            let res = cx
                .background_spawn(async move {
                    market::install(&item).map(|_| market::installed_list())
                })
                .await;
            _ = this.update(cx, |this, cx| {
                this.ext.market.busy.remove(&id);
                match res {
                    Ok(list) => this.ext.market.installed = list,
                    Err(e) => {
                        this.ext.note =
                            Some(crate::trf!("Couldn't install {name}: {e}"; name => name, e => e))
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn market_uninstall(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(folder) = self.ext.market.installed(&id).map(|i| i.folder.clone()) else {
            return;
        };
        if self.ext.market.busy.contains_key(&id) {
            return;
        }
        self.ext.market.busy.insert(id.clone(), Busy::Removing);
        self.ext.note = None;
        cx.spawn(async move |this, cx| {
            let res = cx
                .background_spawn(async move {
                    market::uninstall(&folder).map(|_| market::installed_list())
                })
                .await;
            _ = this.update(cx, |this, cx| {
                this.ext.market.busy.remove(&id);
                match res {
                    Ok(list) => this.ext.market.installed = list,
                    Err(e) => {
                        this.ext.note = Some(crate::trf!("Couldn't uninstall it: {e}"; e => e))
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Show an extension's page in the editor area and fetch its README once.
    pub(crate) fn market_open(&mut self, item: Item, cx: &mut Context<Self>) {
        let id = item.id();
        let loading = self.ext.market.open.as_ref().is_some_and(|o| o.id() == id);
        self.ext.page = Some(format!("{PAGE}{id}"));
        self.settings_open = false;
        self.whats_new = None;
        self.collab.page = false;
        if !self.ext.market.readme.contains_key(&id) && !loading {
            let folder = self.ext.market.installed(&id).map(|i| i.folder.clone());
            let it = item.clone();
            cx.spawn(async move |this, cx| {
                let text = cx
                    .background_spawn(async move { market::readme(&it, folder.as_deref()) })
                    .await;
                _ = this.update(cx, |this, cx| {
                    this.ext.market.readme.insert(id, text);
                    cx.notify();
                });
            })
            .detach();
        }
        self.ext.market.open = Some(item);
        cx.notify();
    }

    /// Install, the busy state, or Installed (which opens the page).
    fn market_button(
        &self,
        id: (&'static str, usize),
        item: &Item,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let key = item.id();
        match self.ext.market.busy.get(&key) {
            Some(b) => Button::new(id)
                .label(if *b == Busy::Installing {
                    tr("Installing")
                } else {
                    tr("Removing")
                })
                .xsmall()
                .ghost()
                .loading(true)
                .into_any_element(),
            None if self.ext.market.installed(&key).is_some() => {
                let it = item.clone();
                Button::new(id)
                    .label(tr("Installed"))
                    .xsmall()
                    .ghost()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.market_open(it.clone(), cx);
                    }))
                    .into_any_element()
            }
            None => {
                let it = item.clone();
                Button::new(id)
                    .label(tr("Install"))
                    .xsmall()
                    .primary()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.market_install(it.clone(), cx);
                    }))
                    .into_any_element()
            }
        }
    }

    /// One row, like VS Code's list: icon; name with installs and rating; a line
    /// of description; the publisher (with its check when verified) and the button.
    fn market_row(
        &self,
        keys: (&'static str, &'static str),
        ix: usize,
        row: Row,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let it = &row.item;
        let id = it.id();
        let selected =
            self.ext.page.as_deref().and_then(|p| p.strip_prefix(PAGE)) == Some(id.as_str());
        let stats = h_flex()
            .flex_none()
            .gap_2()
            .text_xs()
            .text_color(c(theme::FG_DIM))
            .when(it.installs > 0, |h| {
                h.child(
                    h_flex()
                        .gap_0p5()
                        .child(Icon::new(Ico::Download).size(theme::u(11.)))
                        .child(market::format_count(it.installs)),
                )
            })
            .when_some(it.rating, |h, r| {
                h.child(
                    h_flex()
                        .gap_0p5()
                        .child(
                            Icon::new(Ico::Star)
                                .size(theme::u(11.))
                                .text_color(c(theme::WARN)),
                        )
                        .child(format!("{r:.1}")),
                )
            });
        let open = row.item.clone();
        h_flex()
            .id((keys.0, ix))
            .flex_none()
            .w_full()
            .min_w_0()
            .overflow_hidden()
            .px_3()
            .py_1p5()
            .gap_2p5()
            .items_start()
            .cursor_pointer()
            .when(selected, |d| d.bg(c(theme::BG_HOVER)))
            .hover(|d| d.bg(c(theme::BG_HOVER)))
            .child(div().pt_0p5().child(vsx_icon(row.icon.as_ref(), 36.)))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(
                        h_flex()
                            .w_full()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(c(theme::FG_STRONG))
                                    .child(it.display.clone()),
                            )
                            .child(stats),
                    )
                    .when(!it.short.is_empty(), |v| {
                        v.child(
                            div()
                                .w_full()
                                .truncate()
                                .text_xs()
                                .text_color(c(theme::FG_MUTED))
                                .child(it.short.clone()),
                        )
                    })
                    .child(
                        h_flex()
                            .w_full()
                            .pt_0p5()
                            .gap_2()
                            .child(
                                h_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_1()
                                    .text_xs()
                                    .text_color(c(theme::FG_DIM))
                                    .when(it.verified, |h| {
                                        h.child(
                                            Icon::new(Ico::BadgeCheck)
                                                .size(theme::u(12.))
                                                .text_color(c(theme::ACCENT_TEXT)),
                                        )
                                    })
                                    .child(
                                        div()
                                            .min_w_0()
                                            .truncate()
                                            .child(it.publisher_display.clone()),
                                    ),
                            )
                            .child(self.market_button((keys.1, ix), it, cx)),
                    ),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.market_open(open.clone(), cx)))
            .into_any_element()
    }

    /// The Marketplace part of the list (All and Marketplace tabs): a header with a
    /// spinner while it asks, then the answer.
    pub(crate) fn market_section(&self, cx: &mut Context<Self>) -> Div {
        let m = &self.ext.market;
        let empty_query = self.ext.query.read(cx).value().trim().is_empty();
        let title: SharedString = if empty_query {
            tr("VS Code Marketplace · most installed")
        } else {
            tr("VS Code Marketplace")
        }
        .into();
        let mut col = v_flex().w_full().child(
            h_flex()
                .px_3()
                .pt_3()
                .pb_1()
                .gap_2()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(theme::FG_MUTED))
                .child(div().flex_1().min_w_0().truncate().child(title))
                .when(m.loading, |h| {
                    h.child(
                        Spinner::new()
                            .icon(Ico::LoaderCircle)
                            .xsmall()
                            .color(c(theme::FG_DIM)),
                    )
                }),
        );
        let note =
            |t: SharedString, color| div().px_3().py_2().text_xs().text_color(c(color)).child(t);
        if let Some(e) = &m.error {
            col = col.child(note(
                crate::trf!("Couldn't reach the Marketplace: {e}"; e => e).into(),
                theme::DANGER,
            ));
        } else if m.answered.is_some() && m.results.is_empty() && !m.loading {
            col = col.child(note(
                tr("No Marketplace extension matches.").into(),
                theme::FG_DIM,
            ));
        } else if m.answered.is_none() && !m.loading {
            col = col.child(note(
                tr("Type to search the VS Code Marketplace.").into(),
                theme::FG_DIM,
            ));
        }
        for (i, it) in m.results.iter().enumerate() {
            let row = Row {
                icon: m.icons.get(&it.id()).cloned(),
                item: it.clone(),
            };
            col = col.child(self.market_row(("vsx-row", "vsx-btn"), i, row, cx));
        }
        col
    }

    /// Installed VS Code extensions matching the query (Installed tab), and how many.
    pub(crate) fn market_installed_rows(&self, q: &str, cx: &mut Context<Self>) -> (Div, usize) {
        let m = &self.ext.market;
        let mut col = v_flex().w_full();
        let mut n = 0;
        for inst in &m.installed {
            let hay = format!("{} {} {}", inst.display, inst.id, inst.description).to_lowercase();
            if !q.split_whitespace().all(|w| hay.contains(w)) {
                continue;
            }
            // The search's entry when it lists the same extension: its numbers, and
            // the newer version for the page's Update.
            let item = m
                .results
                .iter()
                .find(|r| r.id() == inst.id)
                .cloned()
                .unwrap_or_else(|| inst.item());
            let icon = inst.icon.clone().or_else(|| m.icons.get(&inst.id).cloned());
            col = col.child(self.market_row(("vsx-irow", "vsx-ibtn"), n, Row { item, icon }, cx));
            n += 1;
        }
        (col, n)
    }

    /// A Marketplace extension's page.
    pub(crate) fn render_market_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(it) = self.ext.market.open.clone() else {
            return div().into_any_element();
        };
        let id = it.id();
        let m = &self.ext.market;
        let inst = m.installed(&id).cloned();
        let busy = m.busy.get(&id).copied();
        let gallery = !it.asset_uri.is_empty();

        let mut actions = h_flex().flex_none().gap_1();
        actions = match (busy, &inst) {
            (Some(b), _) => actions.child(
                Button::new("vsx-busy")
                    .label(if b == Busy::Installing {
                        tr("Installing")
                    } else {
                        tr("Removing")
                    })
                    .small()
                    .loading(true),
            ),
            (None, Some(i)) => {
                let uid = id.clone();
                let a = actions.child(
                    Button::new("vsx-uninstall")
                        .label(tr("Uninstall"))
                        .small()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.market_uninstall(uid.clone(), cx)
                        })),
                );
                if gallery && i.version != it.version {
                    let up = it.clone();
                    a.child(
                        Button::new("vsx-update")
                            .label(crate::trf!("Update to {v}"; v => it.version))
                            .small()
                            .primary()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.market_install(up.clone(), cx)
                            })),
                    )
                } else {
                    a
                }
            }
            (None, None) => {
                let ins = it.clone();
                actions.child(
                    Button::new("vsx-install")
                        .label(tr("Install"))
                        .small()
                        .primary()
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.market_install(ins.clone(), cx)),
                        ),
                )
            }
        };
        let url = format!(
            "https://marketplace.visualstudio.com/items?itemName={}.{}",
            it.publisher, it.name
        );
        actions = actions
            .child(
                Button::new("vsx-web")
                    .icon(Icon::new(Ico::ExternalLink))
                    .small()
                    .ghost()
                    .tooltip(tr("Open on the Marketplace website"))
                    .on_click(move |_, _, cx| cx.open_url(&url)),
            )
            .child(
                Button::new("vsx-close")
                    .icon(Icon::new(Ico::Close))
                    .small()
                    .ghost()
                    .tooltip(tr("Close (Esc)"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.ext.page = None;
                        cx.notify();
                    })),
            );

        let title = h_flex()
            .w_full()
            .flex_wrap()
            .gap_x_4()
            .gap_y_1()
            .child(
                div()
                    .flex_1()
                    .min_w(theme::u(160.))
                    .truncate()
                    .text_xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(c(theme::FG_STRONG))
                    .child(it.display.clone()),
            )
            .child(actions);
        let meta = h_flex()
            .w_full()
            .flex_wrap()
            .gap_x_3()
            .gap_y_1()
            .text_xs()
            .text_color(c(theme::FG_DIM))
            .child(
                h_flex()
                    .gap_1()
                    .text_color(c(theme::FG_MUTED))
                    .when(it.verified, |h| {
                        h.child(
                            Icon::new(Ico::BadgeCheck)
                                .size(theme::u(12.))
                                .text_color(c(theme::ACCENT_TEXT)),
                        )
                    })
                    .child(it.publisher_display.clone()),
            )
            .when(inst.is_some(), |h| {
                h.child(
                    h_flex()
                        .gap_1()
                        .text_color(c(theme::GREEN))
                        .child(Icon::new(Ico::Check).size(theme::u(12.)))
                        .child(tr("Installed")),
                )
            })
            .when(it.installs > 0, |h| {
                h.child(
                    h_flex()
                        .gap_1()
                        .child(Icon::new(Ico::Download).size(theme::u(12.)))
                        .child(market::format_count(it.installs)),
                )
            })
            .when_some(it.rating, |h, r| {
                h.child(
                    h_flex()
                        .gap_1()
                        .child(
                            Icon::new(Ico::Star)
                                .size(theme::u(12.))
                                .text_color(c(theme::WARN)),
                        )
                        .child(format!("{r:.1} ({})", it.rating_count)),
                )
            });
        let icon = inst
            .as_ref()
            .and_then(|i| i.icon.clone())
            .or_else(|| m.icons.get(&id).cloned());
        let head = h_flex()
            .w_full()
            .gap_4()
            .items_start()
            .child(vsx_icon(icon.as_ref(), 64.))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(title)
                    .child(meta)
                    .when(!it.short.is_empty(), |v| {
                        v.child(
                            div()
                                .pt_1p5()
                                .text_sm()
                                .text_color(c(theme::FG))
                                .child(it.short.clone()),
                        )
                    }),
            );

        // Facts, as label and value rows.
        let mut facts: Vec<(&'static str, String)> =
            vec![("Identifier", format!("{}.{}", it.publisher, it.name))];
        facts.push((
            "Version",
            inst.as_ref()
                .filter(|i| i.version != it.version && gallery)
                .map(|i| format!("{} ({} {})", it.version, tr("installed"), i.version))
                .unwrap_or_else(|| it.version.clone()),
        ));
        if !it.updated.is_empty() {
            facts.push(("Last updated", it.updated.clone()));
        }
        if !it.categories.is_empty() {
            facts.push(("Categories", it.categories.join(", ")));
        }
        if it.installs > 0 {
            facts.push(("Installs", it.installs.to_string()));
        }
        if let Some(r) = it.rating {
            facts.push((
                "Rating",
                format!(
                    "{r:.1} / 5 · {}",
                    crate::trf!("{n} ratings"; n => it.rating_count)
                ),
            ));
        }
        let mut table = v_flex()
            .rounded(px(6.))
            .border_1()
            .border_color(c(theme::BORDER))
            .overflow_hidden();
        for (k, (label, value)) in facts.into_iter().enumerate() {
            table = table.child(
                h_flex()
                    .px_3()
                    .py_1p5()
                    .gap_3()
                    .items_start()
                    .when(k > 0, |h| h.border_t_1().border_color(c(theme::BORDER)))
                    .child(
                        div()
                            .flex_none()
                            .w(theme::u(110.))
                            .text_xs()
                            .text_color(c(theme::FG_DIM))
                            .child(tr(label)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_xs()
                            .text_color(c(theme::FG))
                            .child(value),
                    ),
            );
        }

        // What FluxCode does with it: the declared parts, and the honest limit.
        let declared: SharedString = match &inst {
            Some(i) if !i.contributes.is_empty() => {
                crate::trf!("It declares {list}."; list => i.contributes.join(", ")).into()
            }
            Some(_) => tr("It declares no themes, languages or snippets.").into(),
            None => tr("Install it to see what it declares (themes, languages, snippets).").into(),
        };
        let here = v_flex()
            .gap_1()
            .child(div().text_sm().text_color(c(theme::FG)).child(declared))
            .child(div().text_xs().text_color(c(theme::FG_DIM)).child(tr(
                "VS Code's JavaScript extension host isn't available in FluxCode, so code that runs inside VS Code (commands, debuggers, language servers started by the extension) doesn't run here.",
            )))
            .when_some(inst.as_ref().map(|i| i.folder.display().to_string()), |v, f| v.child(div().text_xs().font_family("Cascadia Code").text_color(c(theme::FG_DIM)).child(f)));

        let readme: AnyElement = match m.readme.get(&id) {
            None => h_flex()
                .gap_2()
                .text_xs()
                .text_color(c(theme::FG_DIM))
                .child(
                    Spinner::new()
                        .icon(Ico::LoaderCircle)
                        .xsmall()
                        .color(c(theme::FG_DIM)),
                )
                .child(tr("Loading the README…"))
                .into_any_element(),
            Some(Ok(t)) => div()
                .text_sm()
                .text_color(c(theme::FG))
                .child(TextView::markdown(
                    SharedString::from(format!("vsx-readme-{id}")),
                    t.clone(),
                ))
                .into_any_element(),
            Some(Err(e)) => div()
                .text_xs()
                .text_color(c(theme::FG_DIM))
                .child(e.clone())
                .into_any_element(),
        };

        let section = |t: &'static str| {
            div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(theme::FG_STRONG))
                .child(tr(t))
        };
        let body = v_flex()
            .w_full()
            .max_w(theme::u(760.))
            .gap(theme::u(28.))
            .child(head)
            .when_some(self.ext.note.clone(), |v, n| {
                v.child(div().text_xs().text_color(c(theme::FG_MUTED)).child(n))
            })
            .child(v_flex().gap_2().child(section("Details")).child(table))
            .child(v_flex().gap_2().child(section("In FluxCode")).child(here))
            .child(v_flex().gap_2().child(section("README")).child(readme));
        v_flex()
            .id("vsx-page")
            .key_context("ExtPage")
            .size_full()
            .items_center()
            .px(theme::u(24.))
            .pt(theme::u(28.))
            .pb(theme::u(40.))
            .overflow_y_scroll()
            .bg(c(theme::BG_EDITOR))
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                if e.keystroke.key == "escape" {
                    this.ext.page = None;
                    cx.notify();
                }
            }))
            .child(body)
            .into_any_element()
    }
}
