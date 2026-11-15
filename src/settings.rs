//! Settings and session, stored as JSON in the user's config folder
//! (`%APPDATA%\FluxCode-GPUI` on Windows).

use crate::i18n::tr;
use crate::theme::{self, c, UiSize, Visibility, PALETTES};
use crate::workbench::Workbench;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex,
    input::Input,
    v_flex, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

const REPO_URL: &str = "https://github.com/MuaraAI/FluxCode";
const ISSUES_URL: &str = "https://github.com/MuaraAI/FluxCode/issues/new/choose";
pub(crate) const CHANGELOG_URL: &str = "https://github.com/MuaraAI/FluxCode/blob/main/CHANGELOG.md";

pub(crate) fn config_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("FluxCode-GPUI")
}

pub(crate) fn load_json<T: for<'a> Deserialize<'a> + Default>(name: &str) -> T {
    // A file saved by Notepad or PowerShell can start with a UTF-8 BOM, which serde_json rejects.
    std::fs::read_to_string(config_dir().join(name))
        .ok()
        .and_then(|s| serde_json::from_str(s.trim_start_matches('\u{feff}')).ok())
        .unwrap_or_default()
}

pub(crate) fn save_json<T: Serialize>(name: &str, value: &T) {
    let dir = config_dir();
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(s) = serde_json::to_string_pretty(value) {
        let _ = std::fs::write(dir.join(name), s);
    }
}

/// Image formats GPUI can decode for the background.
pub(crate) const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif", "bmp"];

const SETTINGS_REV: u32 = 1;

fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Settings {
    pub font_size: f32,
    pub font_family: SharedString,
    pub tab_size: usize,
    pub word_wrap: bool,
    /// Draw each space as a faint dot, so indentation reads as a level, not a gap.
    pub show_whitespace: bool,
    pub auto_save: bool,
    pub agent_auto_approve: bool,
    /// Palette id, see `theme::PALETTES`.
    pub theme: String,
    /// A picture of the user's choice behind the panels. Only the path is stored;
    /// the file stays where it is and is never copied.
    pub bg_image: Option<PathBuf>,
    pub bg_visibility: Visibility,
    /// Size of the menus, bars, lists and icons (not the editor font).
    pub ui_size: UiSize,
    /// OpenCode docks on the right of the editor (like a chat beside the code)
    /// instead of in the left side bar.
    pub agent_right: bool,
    /// When the agent edits a file, the editor opens it and shows the change being
    /// typed in, line by line, with the view following it.
    pub agent_follow_edits: bool,
    /// Extra Claude / OpenCode sign-ins (agent/worktree.rs), and the one each uses
    /// (family → account name; none: the default sign-in).
    pub agent_accounts: Vec<crate::agent::worktree::Account>,
    pub agent_account: std::collections::HashMap<String, String>,
    /// Which agent harness the panel runs: "auto" (Mura when installed), an agent id
    /// ("opencode-v2", "mura", ...).
    pub agent_cli: String,
    /// Turn the agent process off after this many idle minutes (0 = never). It
    /// resumes the same session on the next message.
    pub agent_idle_min: u32,
    /// Interface language: "auto" (Indonesian when Windows is), "en" or "id".
    pub language: String,
    /// Agent settings picked in the panel (ACP config options such as model,
    /// effort and mode), as "harness:option" → value. Applied to each new
    /// session when the harness still offers that value.
    pub agent_options: std::collections::HashMap<String, String>,
    /// What the agent may do without asking: "read" (read only), "write" (read and
    /// edit files; commands ask) or "full". Empty: from the old auto-approve switch.
    pub agent_access: String,
    /// The level Full access falls back to when it is switched off: "read" or "write".
    pub agent_access_off: String,
    /// A permission request flashes the taskbar button while FluxCode is in the background.
    pub agent_notify: bool,
    /// The browser the agents drive (`browser.rs`): "auto" (Chrome, else Brave, Edge or
    /// Chromium), a browser id, or "off".
    pub agent_browser: String,
    /// Run that browser without a window.
    pub agent_browser_headless: bool,
    /// Installed extensions (ids; `ext::catalog` has them all).
    pub extensions: Vec<String>,
    /// Launches that showed the Ctrl+L hint in the status bar (100: shortcut used).
    pub agent_hint: u8,
    /// Look for new development builds on GitHub at start and every 6 hours.
    pub update_check: bool,
    /// Read WSL folders through the helper inside the distro instead of the 9p
    /// share. Off by default: it installs and runs a program in the distro.
    pub wsl_helper: bool,
    /// The HTML preview notice: shown before the preview runs its WebView.
    /// The "Do not show again" box in the notice turns it off.
    #[cfg(feature = "embedded-preview")]
    #[serde(default = "default_true")]
    pub preview_webview_notice: bool,
    /// The editor's line numbers in the gutter.
    pub line_numbers: bool,
    /// Fold arrows in the gutter for blocks.
    pub folding: bool,
    /// Typing an opening bracket or quote adds the closing one.
    pub auto_close: bool,
    /// A new line keeps (and after `{` / `:` adds) indentation.
    pub smart_indent: bool,
    /// Lines kept visible above and below the cursor when it moves (VS Code's
    /// cursorSurroundingLines). 0: the editor's default.
    pub cursor_padding: u32,
    /// The editor scrolls past its last line.
    pub scroll_past_end: bool,
    /// GitHub Copilot ghost-text suggestions (the Copilot language server).
    /// Off means no server runs and no suggestion is ever asked for.
    #[serde(default = "default_true")]
    pub copilot: bool,
    /// The build that ran last, so a new one can open its release notes once.
    pub last_build: String,
    /// Settings revision, for one-time changes of defaults on existing installs.
    /// Missing in an older file means 0 (not the struct default, which is current).
    #[serde(default)]
    pub rev: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            font_size: 15.,
            font_family: "Cascadia Code".into(),
            tab_size: 4,
            word_wrap: false,
            show_whitespace: false,
            auto_save: true,
            agent_auto_approve: false,
            theme: PALETTES[0].id.into(),
            bg_image: None,
            bg_visibility: Visibility::default(),
            ui_size: UiSize::default(),
            agent_right: true,
            agent_follow_edits: true,
            agent_accounts: Vec::new(),
            agent_account: Default::default(),
            agent_cli: "auto".into(),
            agent_idle_min: 10,
            language: "auto".into(),
            agent_options: Default::default(),
            agent_access: String::new(),
            agent_access_off: "write".into(),
            agent_notify: true,
            agent_browser: "auto".into(),
            agent_browser_headless: false,
            extensions: crate::ext::default_installed(),
            agent_hint: 0,
            update_check: true,
            wsl_helper: false,
            #[cfg(feature = "embedded-preview")]
            preview_webview_notice: true,
            line_numbers: true,
            folding: true,
            auto_close: true,
            smart_indent: true,
            cursor_padding: 0,
            scroll_past_end: true,
            copilot: true,
            last_build: String::new(),
            rev: SETTINGS_REV,
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        EXISTED.get_or_init(|| config_dir().join("settings.json").is_file());
        let mut s: Self = load_json("settings.json");
        if s.rev < 1 {
            // 0.9: whitespace dots are off by default (they cluttered long lines);
            // Settings → Whitespace dots turns them back on.
            s.show_whitespace = false;
        }
        if s.agent_cli == "opencode" {
            // OpenCode became OpenCode 2 (the registry's native build): keep its choices.
            s.agent_cli = "opencode-v2".into();
            s.agent_options = std::mem::take(&mut s.agent_options)
                .into_iter()
                .map(|(k, v)| match k.strip_prefix("opencode:") {
                    Some(rest) => (format!("opencode-v2:{rest}"), v),
                    None => (k, v),
                })
                .collect();
            s.save();
        }
        if s.rev < SETTINGS_REV {
            s.rev = SETTINGS_REV;
            s.save();
        }
        s
    }
    pub fn save(&self) {
        save_json("settings.json", self);
    }
    /// The agent's access level: "read", "write" or "full".
    pub fn access(&self) -> &str {
        match self.agent_access.as_str() {
            "read" | "write" | "full" => &self.agent_access,
            _ if self.agent_auto_approve => "full",
            _ => "write",
        }
    }

    /// What Full access switches back to: read only, or edits with commands asking.
    pub fn access_off(&self) -> &'static str {
        if self.agent_access_off == "read" { "read" } else { "write" }
    }

    /// The background image, if one is set and the file is still there.
    pub fn background(&self) -> Option<&Path> {
        self.bg_image.as_deref().filter(|p| p.is_file())
    }
}

static EXISTED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

/// settings.json was there when FluxCode started (so this is not a first run).
pub fn existed_before_start() -> bool {
    EXISTED.get().copied().unwrap_or(false)
}

/// The editor options from Settings, applied to each open editor and every new one.
#[derive(Clone, Copy)]
pub struct EditorOpts {
    pub line_numbers: bool,
    folding: bool,
    auto_close: bool,
    smart_indent: bool,
    cursor_padding: u32,
    scroll_past_end: bool,
}

impl EditorOpts {
    pub fn of(s: &Settings) -> Self {
        Self {
            line_numbers: s.line_numbers,
            folding: s.folding,
            auto_close: s.auto_close,
            smart_indent: s.smart_indent,
            cursor_padding: s.cursor_padding,
            scroll_past_end: s.scroll_past_end,
        }
    }

    pub fn apply(self, e: &mut gpui_kit::component::input::EditorState, window: &mut Window, cx: &mut Context<gpui_kit::component::input::EditorState>) {
        e.set_line_number(self.line_numbers, window, cx);
        e.set_folding(self.folding, window, cx);
        e.set_auto_close(self.auto_close, window, cx);
        e.set_smart_indent(self.smart_indent, window, cx);
        e.set_cursor_surrounding_lines((self.cursor_padding > 0).then_some(self.cursor_padding as usize), window, cx);
        e.set_scroll_beyond_last_line(if self.scroll_past_end { None } else { Some(0) }, window, cx);
    }
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Session {
    pub root: Option<PathBuf>,
    /// Folders added next to `root` (a multi-folder workspace).
    pub extra: Vec<PathBuf>,
    pub files: Vec<PathBuf>,
    pub active: Option<usize>,
}

impl Session {
    pub fn load() -> Self {
        load_json("session.json")
    }
    pub fn save(&self) {
        save_json("session.json", self);
    }
}

/// One page of the Settings UI. The left navigation lists these, and the
/// main area shows one at a time.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingsSection {
    Appearance,
    Editor,
    Wsl,
    Agents,
    SoulFiles,
    Shortcuts,
    Updates,
    About,
}

impl SettingsSection {
    pub(crate) const ALL: [SettingsSection; 8] = [
        SettingsSection::Appearance,
        SettingsSection::Editor,
        SettingsSection::Wsl,
        SettingsSection::Agents,
        SettingsSection::SoulFiles,
        SettingsSection::Shortcuts,
        SettingsSection::Updates,
        SettingsSection::About,
    ];

    /// The full title shown above the section in the main area.
    pub(crate) fn title(self) -> &'static str {
        match self {
            SettingsSection::Appearance => "Appearance",
            SettingsSection::Editor => "Editor",
            SettingsSection::Wsl => "WSL",
            SettingsSection::Agents => "AI agents and GitHub",
            SettingsSection::SoulFiles => "Agent instructions (soul files)",
            SettingsSection::Shortcuts => "Keyboard shortcuts",
            SettingsSection::Updates => "Updates",
            SettingsSection::About => "About FluxCode",
        }
    }

    /// The short label shown in the left navigation.
    pub(crate) fn nav_label(self) -> &'static str {
        match self {
            SettingsSection::SoulFiles => "Agent instructions",
            s => s.title(),
        }
    }

    /// One line saying what the section controls.
    pub(crate) fn description(self) -> &'static str {
        match self {
            SettingsSection::Appearance => "Theme, language, interface size, and the picture behind the panels.",
            SettingsSection::Editor => "Font, wrapping, line numbers, and the Copilot suggestions the editor shows as you type.",
            SettingsSection::Wsl => "Read folders inside a WSL distro through a helper, instead of the slow Windows share.",
            SettingsSection::Agents => "What the agent may do, how it docks, and the GitHub and Copilot sign-ins.",
            SettingsSection::SoulFiles => "The files each installed agent reads for its standing instructions and character. Open one to read or edit it; a missing one can be created empty.",
            SettingsSection::Shortcuts => "VS Code's keys. Ctrl+Shift+P (or F1) opens every command with its keys; type to filter.",
            SettingsSection::Updates => "Check for new development builds and whether FluxCode looks for them.",
            SettingsSection::About => "What FluxCode is, where its code lives, and how to report a problem.",
        }
    }

    /// True when any setting in this section differs from its default, for
    /// the dot on the nav row.
    pub(crate) fn modified(self, w: &Workbench) -> bool {
        let s = &w.settings;
        match self {
            SettingsSection::Appearance => s.theme != PALETTES[0].id || s.language != "auto" || s.ui_size != UiSize::default() || s.bg_image.is_some(),
            SettingsSection::Editor => {
                s.font_size != 15.
                    || s.tab_size != 4
                    || s.word_wrap
                    || s.show_whitespace
                    || !s.auto_save
                    || &*s.font_family != "Cascadia Code"
                    || !s.line_numbers
                    || !s.folding
                    || !s.auto_close
                    || !s.smart_indent
                    || s.cursor_padding != 0
                    || !s.scroll_past_end
                    || !s.copilot
            }
            SettingsSection::Wsl => s.wsl_helper,
            SettingsSection::Agents => {
                !s.agent_access.is_empty()
                    || s.agent_auto_approve
                    || s.agent_access_off != "write"
                    || s.agent_idle_min != 10
                    || !s.agent_right
                    || !s.agent_follow_edits
                    || w.github.is_some()
            }
            SettingsSection::SoulFiles | SettingsSection::Shortcuts | SettingsSection::About => false,
            SettingsSection::Updates => !s.update_check,
        }
    }
}

/// One setting inside a card: title and short description on the left, the
/// control at the right edge. Rows share the card's hairline dividers, so
/// this helper draws none of its own.
fn settings_row(title: &'static str, desc: SharedString, control: AnyElement) -> AnyElement {
    h_flex()
        .w_full()
        .py_3()
        .px_4()
        .gap_4()
        .child(
            // min_w_0: lets the description wrap instead of pushing the control off screen.
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_1()
                .child(div().text_sm().text_color(c(theme::FG_STRONG)).child(title))
                .child(div().text_xs().text_color(c(theme::FG_MUTED)).child(desc)),
        )
        .child(div().flex_none().child(control))
        .into_any_element()
}

/// A bordered card holding one setting row per child, with a hairline
/// between rows.
fn settings_card(rows: Vec<AnyElement>) -> AnyElement {
    let n = rows.len();
    v_flex()
        .w_full()
        .max_w(theme::u(760.))
        .rounded(theme::u(8.))
        .border_1()
        .border_color(c(theme::BORDER))
        .bg(c(theme::BG_ELEVATED))
        .children(rows.into_iter().enumerate().map(|(i, row)| {
            div()
                .when(i + 1 < n, |d| d.border_b_1().border_color(c(theme::BORDER)))
                .child(row)
        }))
        .into_any_element()
}

/// The six theme palettes as a grid of swatch cards. Extracted unchanged
/// from the old flat settings page.
fn theme_cards(cx: &mut Context<Workbench>) -> Div {
    // Theme cards: a strip of the palette's own colors, its name and one line.
    let active_theme = theme::palette().id;
    h_flex().w_full().max_w(theme::u(760.)).flex_wrap().gap_3().py_3().children(PALETTES.iter().map(|p| {
        let on = p.id == active_theme;
        let id = p.id;
        let [surface, side, accent, text] = p.swatches();
        v_flex()
            .id(SharedString::from(format!("theme-{id}")))
            .w(theme::u(176.))
            .rounded(px(6.))
            .overflow_hidden()
            .cursor_pointer()
            .border_2()
            .border_color(if on { c(theme::ACCENT) } else { c(theme::BORDER) })
            // One hover style (gpui allows only one): the active card keeps its accent.
            .hover(move |s| s.border_color(c(if on { theme::ACCENT } else { theme::BORDER_STRONG })))
            .on_click(cx.listener(move |this, _, w, cx| this.set_theme(id, w, cx)))
            .child(
                h_flex()
                    .h(theme::u(56.))
                    .child(div().w(px(36.)).h_full().bg(c(side)))
                    .child(
                        v_flex()
                            .flex_1()
                            .h_full()
                            .p_2()
                            .gap_1()
                            .bg(c(surface))
                            .child(div().w(px(70.)).h(px(4.)).rounded(px(2.)).bg(c(text)))
                            .child(div().w(px(46.)).h(px(4.)).rounded(px(2.)).bg(c(accent)))
                            .child(div().w(px(92.)).h(px(4.)).rounded(px(2.)).bg(c(text)).opacity(0.6)),
                    ),
            )
            .child(
                v_flex()
                    .px_2()
                    .py_2()
                    .gap(px(2.))
                    .bg(c(theme::BG_ELEVATED))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(div().flex_1().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(p.name))
                            .when(on, |h| h.child(div().text_xs().text_color(c(theme::ACCENT_TEXT)).child(tr("Active")))),
                    )
                    .child(div().text_xs().truncate().text_color(c(theme::FG_MUTED)).child(p.note)),
            )
    }))
}

impl Workbench {
    pub(crate) fn apply_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.save();
        self.copilot.set_enabled(self.settings.copilot);
        crate::remote::set_helper_enabled(self.settings.wsl_helper);
        if self.settings.agent_right && self.view == crate::workbench::SideView::Agent {
            self.view = crate::workbench::SideView::Explorer;
            self.agent_open = true;
        }
        let (wrap, dots) = (self.settings.word_wrap, self.settings.show_whitespace);
        let opts = EditorOpts::of(&self.settings);
        for d in &self.docs {
            d.editor.update(cx, |s, cx| {
                s.set_soft_wrap(wrap, window, cx);
                s.set_show_whitespaces(dots, window, cx);
                opts.apply(s, window, cx);
            });
        }
        cx.notify();
    }

    /// Re-apply the palette and image state everywhere (after a theme or background change).
    pub(crate) fn apply_appearance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.save();
        theme::set_ui_size(self.settings.ui_size);
        theme::set(&self.settings.theme, self.settings.background().map(|_| self.settings.bg_visibility));
        theme::apply(cx);
        window.refresh();
        cx.notify();
    }

    pub(crate) fn set_theme(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.theme = id.into();
        self.apply_appearance(window, cx);
        let name = PALETTES.iter().find(|p| p.id == id).map_or("", |p| p.name);
        self.status = crate::trf!("Color theme: {name}"; name => name).into();
    }

    pub(crate) fn choose_background(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose Background Image".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let path = paths.await.ok()?.ok()??.into_iter().next()?;
            this.update_in(cx, |this, window, cx| {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
                if !IMAGE_EXTENSIONS.contains(&ext.as_str()) {
                    this.status = crate::trf!("{0} is not an image FluxCode can show (PNG, JPEG, WebP, GIF or BMP)."; 0 => path.display()).into();
                    cx.notify();
                    return;
                }
                this.settings.bg_image = Some(path);
                this.apply_appearance(window, cx);
                this.status = tr("Background image set").into();
            })
            .ok()
        })
        .detach();
    }

    pub(crate) fn render_settings(&self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let close_btn = self.action_button("settings-close", tr("Close"), crate::ui::Kind::Plain, cx, |this, _, cx| {
            this.settings_open = false;
            cx.notify();
        });
        let section = self.settings_section;
        let content = self.settings_section_content(section, cx);
        let key = SharedString::from(format!("settings-section-{}", section as u8));
        // Switching sections fades the new one in over 160 ms, opacity only.
        // The feel follows PandaCode (aicodeking/pandacode, src/motion.css and
        // src/motion.ts): its content appears with a 160 ms ease-out fade from
        // opacity 0, and it never animates a layout property such as top or
        // left, so no layout pass runs per frame. The fade uses GPUI's cubic
        // ease-out, the same curve as CSS ease-out, and only the active
        // section is built. Under the system's reduce-motion setting there is
        // no motion at all.
        let body = if cx.reduce_motion() {
            div().child(content).into_any_element()
        } else {
            div()
                .child(content)
                .with_animation(key, Animation::new(Duration::from_millis(160)).with_easing(gpui_kit::base::animation::ease_out_cubic), |el, t| {
                    el.opacity(t)
                })
                .into_any_element()
        };
        h_flex()
            .id("settings")
            .size_full()
            .bg(c(theme::BG_EDITOR))
            .child(self.render_settings_nav(cx))
            .child(
                v_flex()
                    .id("settings-content")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_y_scroll()
                    .px_8()
                    .py_6()
                    .child(
                        h_flex()
                            .w_full()
                            .max_w(theme::u(760.))
                            .items_start()
                            .gap_4()
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_1()
                                    .child(div().text_xl().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(tr(section.title())))
                                    .child(div().text_xs().text_color(c(theme::FG_MUTED)).child(tr(section.description()))),
                            )
                            .child(close_btn),
                    )
                    .child(div().h(theme::u(16.)))
                    .child(body),
            )
            .into_any_element()
    }

    /// The left navigation: one row per section, an accent bar on the active
    /// row, and a dot on rows whose section differs from the defaults.
    fn render_settings_nav(&self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .w(theme::u(208.))
            .h_full()
            .flex_shrink_0()
            .bg(c(theme::BG_SIDE))
            .border_r_1()
            .border_color(c(theme::BORDER))
            .py_3()
            .px_2()
            .child(
                div()
                    .px_3()
                    .pb_2()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(c(theme::FG_STRONG))
                    .child(tr("Settings")),
            )
            .children(SettingsSection::ALL.iter().enumerate().map(|(ix, sec)| {
                let sec = *sec;
                let active = sec == self.settings_section;
                let modified = sec.modified(self);
                div()
                    .relative()
                    .id(SharedString::from(format!("settings-nav-{ix}")))
                    .h(theme::u(30.))
                    .px_3()
                    .rounded(theme::u(6.))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .gap_2()
                    .when(active, |d| d.bg(c(theme::SELECT)))
                    .when(!active, |d| d.hover(|s| s.bg(c(theme::BG_HOVER))))
                    .when(active, |d| {
                        d.child(
                            div()
                                .absolute()
                                .left(px(0.))
                                .top(theme::u(7.))
                                .bottom(theme::u(7.))
                                .w(px(2.))
                                .rounded_full()
                                .bg(c(theme::ACCENT)),
                        )
                    })
                    .child(div().flex_1().min_w_0().truncate().text_sm().text_color(c(if active { theme::FG_STRONG } else { theme::FG })).child(tr(sec.nav_label())))
                    .when(modified, |d| d.child(div().size(px(6.)).rounded_full().bg(c(theme::WARN))))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.settings_section = sec;
                        cx.notify();
                    }))
            }))
            .into_any_element()
    }

    /// Builds the content of one section. Only the active section is built,
    /// so opening Settings no longer reads the soul files or lists the
    /// shortcuts of the rest.
    fn settings_section_content(&self, section: SettingsSection, cx: &mut Context<Self>) -> AnyElement {
        let s = &self.settings;
        // Same behaviour as the old flat page: each control applies the
        // setting and saves it at once.
        let stepper = |id: &'static str, value: String, f: fn(&mut Workbench, i32)| {
            h_flex()
                .gap_1()
                .child(Button::new((id, 0usize)).label("−").xsmall().on_click(cx.listener(move |this, _, w, cx| {
                    f(this, -1);
                    this.apply_settings(w, cx);
                })))
                .child(div().w(px(48.)).text_center().text_sm().child(value))
                .child(Button::new((id, 1usize)).label("+").xsmall().on_click(cx.listener(move |this, _, w, cx| {
                    f(this, 1);
                    this.apply_settings(w, cx);
                })))
                .into_any_element()
        };
        let toggle = |id: &'static str, on: bool, f: fn(&mut Workbench)| {
            Button::new(id)
                .label(if on { tr("On") } else { tr("Off") })
                .xsmall()
                .when(on, |b| b.primary())
                .on_click(cx.listener(move |this, _, w, cx| {
                    f(this);
                    this.apply_settings(w, cx);
                }))
                .into_any_element()
        };
        match section {
            SettingsSection::Appearance => {
                // Language names stay in their own language, so they can be found in either.
                let lang = s.language.clone();
                let lang_controls = h_flex()
                    .gap_1()
                    .children([("auto", tr("Automatic")), ("en", "English"), ("id", "Bahasa Indonesia")].into_iter().map(|(id, name)| {
                        Button::new(SharedString::from(format!("lang-{id}")))
                            .label(name)
                            .xsmall()
                            .when(lang == id, |b| b.primary())
                            .on_click(cx.listener(move |this, _, w, cx| {
                                this.settings.language = id.into();
                                crate::i18n::set(id);
                                this.apply_language(w, cx);
                                this.apply_settings(w, cx);
                            }))
                    }))
                    .into_any_element();
                let size = s.ui_size;
                let size_controls = h_flex()
                    .gap_1()
                    .children(UiSize::ALL.iter().map(|z| {
                        let z = *z;
                        Button::new(SharedString::from(format!("ui-size-{}", z.label())))
                            .label(tr(z.label()))
                            .xsmall()
                            .when(z == size, |b| b.primary())
                            .on_click(cx.listener(move |this, _, w, cx| {
                                this.settings.ui_size = z;
                                this.apply_appearance(w, cx);
                            }))
                    }))
                    .into_any_element();
                // Background image: choose / remove, and how strongly it shows.
                let bg_state: (SharedString, bool) = match (&s.bg_image, s.background()) {
                    (None, _) => ("No image. Panels use the theme's solid colors.".into(), false),
                    (Some(p), None) => (crate::trf!("{0} was not found. Choose another image or remove it."; 0 => p.display()).into(), true),
                    (Some(p), Some(_)) => (p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default().into(), false),
                };
                let bg_controls = h_flex()
                    .gap_2()
                    .child(Button::new("bg-choose").label(if s.bg_image.is_some() { tr("Change image…") } else { tr("Choose image…") }).xsmall().on_click(cx.listener(|this, _, w, cx| this.choose_background(w, cx))))
                    .when(s.bg_image.is_some(), |h| {
                        h.child(Button::new("bg-remove").label(tr("Remove")).xsmall().on_click(cx.listener(|this, _, w, cx| {
                            this.settings.bg_image = None;
                            this.apply_appearance(w, cx);
                            this.status = tr("Background image removed").into();
                        })))
                    })
                    .into_any_element();
                let vis = s.bg_visibility;
                let vis_controls = h_flex()
                    .gap_1()
                    .children(Visibility::ALL.iter().map(|v| {
                        let v = *v;
                        Button::new(SharedString::from(format!("bg-vis-{}", v.label())))
                            .label(tr(v.label()))
                            .xsmall()
                            .when(v == vis, |b| b.primary())
                            .on_click(cx.listener(move |this, _, w, cx| {
                                this.settings.bg_visibility = v;
                                this.apply_appearance(w, cx);
                            }))
                    }))
                    .into_any_element();
                let vis_desc: SharedString = match vis {
                    Visibility::Strong => tr("Strong: the picture shows clearly. Over very bright pictures, dim text (line numbers, hints) can drop below comfortable contrast.").into(),
                    _ => tr("How much of the picture shows through the panels. The code side of the editor always gets less of it than the right edge.").into(),
                };
                let mut rows = vec![
                    div().p_4().child(theme_cards(cx)).into_any_element(),
                    settings_row(tr("Language"), tr("Menus, panels and messages. Automatic follows Windows.").into(), lang_controls),
                    settings_row(
                        tr("Interface size"),
                        tr("Menus, tabs, the explorer, panels and icons. The editor font has its own size below.").into(),
                        size_controls,
                    ),
                    settings_row(
                        tr("Background image"),
                        if bg_state.1 { bg_state.0.clone() } else { crate::trf!("A picture of your own behind the panels. Stays on your PC. Current: {0}"; 0 => bg_state.0).into() },
                        bg_controls,
                    ),
                ];
                if s.background().is_some() {
                    rows.push(settings_row(tr("Background visibility"), vis_desc, vis_controls));
                }
                settings_card(rows)
            }
            SettingsSection::Editor => {
                let fonts: &[&'static str] = if cfg!(windows) {
                    &["Cascadia Code", "Cascadia Mono", "Consolas", "Courier New"]
                } else {
                    &["DejaVu Sans Mono", "Liberation Mono", "Noto Sans Mono", "Ubuntu Mono"]
                };
                let current_font = s.font_family.to_string();
                let font_controls = h_flex()
                    .gap_1()
                    .flex_wrap()
                    .justify_end()
                    .max_w(theme::u(360.))
                    .children(fonts.iter().map(|f| {
                        let f = *f;
                        Button::new(SharedString::from(format!("font-{f}")))
                            .label(f)
                            .xsmall()
                            .when(current_font == f, |b| b.primary())
                            .on_click(cx.listener(move |this, _, w, cx| {
                                this.settings.font_family = f.into();
                                this.apply_settings(w, cx);
                            }))
                    }))
                    .into_any_element();
                settings_card(vec![
                    settings_row(tr("Font size"), tr("Font size of the editor and terminal, in pixels.").into(), stepper("fs", format!("{}", s.font_size), |w, d| {
                        w.settings.font_size = (w.settings.font_size + d as f32).clamp(8., 32.)
                    })),
                    settings_row(tr("Tab size"), tr("Spaces per indentation level (applies to newly opened files).").into(), stepper("ts", s.tab_size.to_string(), |w, d| {
                        w.settings.tab_size = (w.settings.tab_size as i32 + d).clamp(1, 8) as usize
                    })),
                    settings_row(tr("Word wrap"), tr("Wrap long lines at the edge of the editor.").into(), toggle("ww", s.word_wrap, |w| w.settings.word_wrap = !w.settings.word_wrap)),
                    settings_row(
                        tr("Whitespace dots"),
                        tr("Show each space as a faint dot, next to the guide lines that mark how far each block reaches.").into(),
                        toggle("wsd", s.show_whitespace, |w| w.settings.show_whitespace = !w.settings.show_whitespace),
                    ),
                    settings_row(tr("Auto save"), tr("Save a file shortly after you stop typing.").into(), toggle("as", s.auto_save, |w| w.settings.auto_save = !w.settings.auto_save)),
                    settings_row(tr("Font"), tr("The editor and terminal font. Fonts that are not installed fall back to the system's monospace font.").into(), font_controls),
                    settings_row(tr("Line numbers"), tr("Numbers in the gutter left of the code.").into(), toggle("ln", s.line_numbers, |w| w.settings.line_numbers = !w.settings.line_numbers)),
                    settings_row(tr("Code folding"), tr("Arrows in the gutter that fold a block away.").into(), toggle("fold", s.folding, |w| w.settings.folding = !w.settings.folding)),
                    settings_row(tr("Close brackets and quotes"), tr("Typing ( [ { or a quote adds its closing pair.").into(), toggle("acl", s.auto_close, |w| w.settings.auto_close = !w.settings.auto_close)),
                    settings_row(tr("Smart indent"), tr("A new line keeps the indentation, and indents one more level after { or :.").into(), toggle("si", s.smart_indent, |w| w.settings.smart_indent = !w.settings.smart_indent)),
                    settings_row(tr("Cursor padding"), tr("Lines kept visible above and below the cursor as it moves (0: the editor's default).").into(), stepper("cp", s.cursor_padding.to_string(), |w, d| {
                        w.settings.cursor_padding = (w.settings.cursor_padding as i32 + d).clamp(0, 12) as u32
                    })),
                    settings_row(tr("Scroll past the last line"), tr("Leave room below the end of a file, so its last lines can sit mid-screen.").into(), toggle("spe", s.scroll_past_end, |w| w.settings.scroll_past_end = !w.settings.scroll_past_end)),
                    settings_row(
                        tr("Copilot suggestions"),
                        tr("Ghost-text completions from GitHub Copilot, a moment after you stop typing. Tab accepts, Esc dismisses. The Copilot language server does the work in its own process, so the editor stays light.").into(),
                        toggle("cpl", s.copilot, |w| w.settings.copilot = !w.settings.copilot),
                    ),
                ])
            }
            SettingsSection::Wsl => settings_card(vec![settings_row(
                tr("Read WSL folders through a helper in the distro"),
                tr("Faster: the helper reads the distro's own disk instead of Windows' 9p share. It is FluxCode itself, built for Linux, taken from this build's release and installed once in ~/.fluxcode/helper. Off: folders are read through \\\\wsl.localhost, which works but is slower.").into(),
                toggle("wslh", s.wsl_helper, |w| w.settings.wsl_helper = !w.settings.wsl_helper),
            )]),
            SettingsSection::Agents => {
                let access = s.access().to_string();
                let access_controls = h_flex()
                    .gap_1()
                    .children([("read", tr("Read only")), ("write", tr("Read & write")), ("full", tr("Full access"))].into_iter().map(|(id, name)| {
                        Button::new(SharedString::from(format!("access-{id}")))
                            .label(name)
                            .xsmall()
                            .when(access == id, |b| b.primary())
                            .on_click(cx.listener(move |this, _, w, cx| {
                                this.settings.agent_access = id.into();
                                this.apply_settings(w, cx);
                            }))
                    }))
                    .into_any_element();
                settings_card(vec![
                    settings_row(
                        tr("AI agent: access"),
                        tr("Read only: it reads and explains, edits and commands are refused. Read & write: it edits files (Undo is under the chat), commands ask first. Full access: nothing asks.").into(),
                        access_controls,
                    ),
                    settings_row(
                        tr("AI agent: turn off when idle"),
                        tr("After 10 idle minutes the agent process stops and frees its memory (OpenCode uses several hundred MB). Your next message starts it again and continues the same chat.").into(),
                        toggle("idle", s.agent_idle_min > 0, |w| w.settings.agent_idle_min = if w.settings.agent_idle_min == 0 { 10 } else { 0 }),
                    ),
                    settings_row(
                        tr("Agent panel on the right"),
                        tr("Dock the agent chat beside the editor instead of in the side bar. The arrow button in its header does the same.").into(),
                        toggle("ar", s.agent_right, |w| w.settings.agent_right = !w.settings.agent_right),
                    ),
                    settings_row(
                        tr("Follow the agent's edits"),
                        tr("When the agent changes a file, it opens in the editor and the change is typed in line by line while the view follows it. Takes under a second and never slows the agent.").into(),
                        toggle("follow", s.agent_follow_edits, |w| w.settings.agent_follow_edits = !w.settings.agent_follow_edits),
                    ),
                    settings_row(tr("GitHub"), tr("Sign in so gh, git and OpenCode inside FluxCode can use your account (GH_TOKEN).").into(), self.render_github(cx)),
                    settings_row(
                        tr("GitHub Copilot"),
                        tr("Sign in so the editor can suggest the next lines of code. Needs the Copilot language server installed once, and a Copilot plan (a free plan exists).").into(),
                        self.render_copilot(cx),
                    ),
                ])
            }
            SettingsSection::SoulFiles => settings_card(vec![div().p_4().child(self.render_soul_files(cx)).into_any_element()]),
            SettingsSection::Shortcuts => {
                let shortcuts = v_flex().w_full().children(crate::commands::commands().into_iter().filter(|k| !k.keys.is_empty()).map(|k| {
                    h_flex()
                        .w_full()
                        .py_1()
                        .gap_3()
                        .border_b_1()
                        .border_color(c(theme::BORDER))
                        .child(div().flex_1().min_w_0().truncate().text_sm().text_color(c(theme::FG)).child(tr(k.label)))
                        .child(crate::quick::keys_chips(k.keys))
                }));
                settings_card(vec![div().p_4().child(shortcuts).into_any_element()])
            }
            SettingsSection::Updates => {
                let update_controls = h_flex()
                    .gap_2()
                    .child(Button::new("upd-now").label(tr("Check now")).xsmall().ghost().on_click(cx.listener(|this, _, _, cx| this.check_updates_now(cx))))
                    .child(toggle("upd", s.update_check, |w| w.settings.update_check = !w.settings.update_check))
                    .into_any_element();
                settings_card(vec![settings_row(
                    tr("Check for new builds"),
                    crate::trf!("FluxCode asks github.com for its list of releases at start and every 6 hours. Nothing about you, this PC or your files is sent. This is {0}."; 0 => crate::update::BUILD).into(),
                    update_controls,
                )])
            }
            SettingsSection::About => {
                v_flex()
                    .w_full()
                    .max_w(theme::u(760.))
                    .gap_4()
                    .child(settings_card(vec![div()
                        .p_4()
                        .child(
                            v_flex()
                                .w_full()
                                .gap_2()
                                .text_xs()
                                .child(div().text_color(c(theme::FG)).child(tr("FluxCode is open source under the GPL-3.0 license and still in development, so expect rough edges and frequent builds. The code, every build and the issue tracker are on GitHub.")))
                                .child(div().text_color(c(theme::FG_MUTED)).child(tr("A MuaraAI project. Copyright (C) 2026 MuaraAI and contributors.")))
                                .child(
                                    h_flex()
                                        .pt_1()
                                        .gap_2()
                                        .flex_wrap()
                                        .child(Button::new("about-repo").label(tr("Open the repository")).xsmall().on_click(|_, _, cx| cx.open_url(REPO_URL)))
                                        .child(Button::new("about-issue").label(tr("Report a problem")).xsmall().ghost().on_click(|_, _, cx| cx.open_url(ISSUES_URL)))
                                        .child(Button::new("about-changes").label(tr("What changed")).xsmall().ghost().on_click(|_, w, cx| w.dispatch_action(Box::new(crate::workbench::ShowWhatsNew), cx))),
                                )
                                .child(div().text_color(c(theme::FG_DIM)).child(format!("github.com/MuaraAI/FluxCode · {}", crate::update::BUILD))),
                        )
                        .into_any_element()]))
                    .child(
                        div()
                            .text_xs()
                            .text_color(c(theme::FG_DIM))
                            .child(crate::trf!("Stored in {0}"; 0 => config_dir().join("settings.json").display())),
                    )
                    .into_any_element()
            }
        }
    }

}

impl Workbench {
    /// Every installed agent with its instruction files: where each one is, and Open
    /// (or Create, when it isn't there yet).
    fn render_soul_files(&self, cx: &mut Context<Self>) -> AnyElement {
        let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from);
        let installed = crate::acp::installed();
        if installed.is_empty() {
            return div().w_full().max_w(theme::u(760.)).py_2().text_xs().text_color(c(theme::FG_DIM)).child(tr("No agent installed yet. The agent panel (Ctrl+L) lists agents to download.")).into_any_element();
        }
        let mut list = v_flex().w_full().max_w(theme::u(760.));
        for id in installed {
            let mut files = v_flex().gap_1();
            if id == "mura" {
                files = files.child(div().text_xs().text_color(c(theme::FG_DIM)).child(tr("Mura's character is built into the program. It follows these project files:")));
            }
            for (k, place) in crate::acp::instruction_files(&id).iter().enumerate() {
                let (label, path) = match *place {
                    crate::acp::Place::Home(rel) => (format!("~/{rel}"), home.as_ref().map(|h| rel.split('/').fold(h.clone(), |p, s| p.join(s)))),
                    crate::acp::Place::Project(rel) => (format!("{}/{rel}", tr("project")), self.root.as_ref().map(|r| rel.split('/').fold(r.clone(), |p, s| p.join(s)))),
                };
                let exists = path.as_ref().is_some_and(|p| p.is_file());
                let key = SharedString::from(format!("soul-{id}-{k}"));
                files = files.child(
                    h_flex()
                        .gap_2()
                        .child(gpui_kit::component::Icon::new(gpui_kit::assets::IconName::ScrollText).size(theme::u(13.)).text_color(c(if exists { theme::FG_MUTED } else { theme::FG_DIM })))
                        .child(div().flex_1().min_w_0().truncate().text_xs().text_color(c(if exists { theme::FG } else { theme::FG_DIM })).child(label))
                        .when_some(path, |h, p| {
                            h.child(
                                Button::new(key)
                                    .label(if exists { tr("Open") } else { tr("Create") })
                                    .xsmall()
                                    .ghost()
                                    .on_click(cx.listener(move |this, _, w, cx| {
                                        if !p.is_file() {
                                            if let Some(d) = p.parent() {
                                                let _ = std::fs::create_dir_all(d);
                                            }
                                            let _ = std::fs::write(&p, "");
                                        }
                                        this.settings_open = false;
                                        this.open_file(p.clone(), w, cx);
                                    })),
                            )
                        }),
                );
            }
            list = list.child(
                h_flex()
                    .w_full()
                    .py_2()
                    .gap_4()
                    .items_start()
                    .border_b_1()
                    .border_color(c(theme::BORDER))
                    .child(div().w(theme::u(150.)).flex_none().text_sm().text_color(c(theme::FG_STRONG)).child(crate::acp::display_name(&id)))
                    .child(div().flex_1().min_w_0().child(files)),
            );
        }
        list.into_any_element()
    }

    fn github_result(&mut self, r: Result<crate::github::Account, String>, cx: &mut Context<Self>) {
        match r {
            Ok(a) => {
                self.github = Some(a);
                self.status = tr("Signed in to GitHub").into();
            }
            Err(e) => self.status = e.into(),
        }
        cx.notify();
    }

    fn copilot_signin_result(&mut self, r: Result<String, String>, cx: &mut Context<Self>) {
        match r {
            Ok(_) => self.status = tr("Signed in to Copilot").into(),
            Err(e) => self.status = e.into(),
        }
        cx.notify();
    }

    fn render_copilot(&self, cx: &mut Context<Self>) -> AnyElement {
        use crate::copilot::Status;
        let (text, show_signin, show_signout): (SharedString, bool, bool) = match self.copilot.status() {
            Status::Off => (tr("Off. Turn Copilot suggestions on in the Editor section.").into(), false, false),
            Status::Starting => (tr("Starting the Copilot server...").into(), false, false),
            Status::NotInstalled => (
                tr("Not installed. Install it once with: npm install -g @github/copilot-language-server (Node.js 20.8 or later).").into(),
                false,
                false,
            ),
            Status::Ready => (
                tr("Ready. Suggestions appear as ghost text a moment after you stop typing.").into(),
                false,
                true,
            ),
            Status::NotSignedIn(m) => (
                if m.is_empty() { tr("Not signed in to Copilot yet.").into() } else { m.into() },
                true,
                false,
            ),
            Status::Warning(m) => (
                if m.is_empty() { tr("Copilot has a temporary problem.").into() } else { m.into() },
                false,
                false,
            ),
            Status::Inactive => (tr("Copilot ignores this file.").into(), false, false),
            Status::Error(e) => (e.into(), true, false),
        };
        let code = self.copilot.user_code();
        let mut col = v_flex()
            .w(px(300.))
            .gap_1()
            .text_sm()
            .child(div().child(text));
        if matches!(self.copilot.status(), Status::NotInstalled) {
            if crate::copilot::node_present() {
                col = col.child(
                    Button::new("cpl-install")
                        .label(tr("Install server"))
                        .xsmall()
                        .loading(self.copilot_installing)
                        .on_click(cx.listener(|this, _, _, cx| {
                            if this.copilot_installing {
                                return;
                            }
                            this.copilot_installing = true;
                            cx.notify();
                            cx.spawn(async move |this, cx| {
                                let r = cx
                                    .background_spawn(async move {
                                        #[cfg(windows)]
                                        let out = crate::proc::shell_command("npm install -g @github/copilot-language-server").output();
                                        #[cfg(not(windows))]
                                        let out = std::process::Command::new("npm")
                                            .args(["install", "-g", "@github/copilot-language-server"])
                                            .output();
                                        match out {
                                            Ok(o) if o.status.success() => Ok(()),
                                            Ok(o) => Err(if o.stderr.is_empty() { format!("npm exited with {}", o.status) } else { String::from_utf8_lossy(&o.stderr).trim().to_string() }),
                                            Err(e) => Err(e.to_string()),
                                        }
                                    })
                                    .await;
                                _ = this.update(cx, |this, cx| {
                                    this.copilot_installing = false;
                                    match r {
                                        Ok(()) => {
                                            this.copilot.retry();
                                            this.status = tr("Copilot server installed.").into();
                                        }
                                        Err(e) => this.status = crate::trf!("Install failed: {e}"; e => e).into(),
                                    }
                                    cx.notify();
                                });
                            })
                            .detach();
                        })),
                );
            } else {
                col = col.child(
                    Button::new("cpl-node")
                        .label(tr("Get Node.js"))
                        .xsmall()
                        .on_click(|_, _, cx| cx.open_url("https://nodejs.org")),
                );
            }
        }
        if let Some(notice) = self.copilot.notice() {
            col = col.child(div().text_xs().text_color(c(theme::FG_MUTED)).child(notice));
        }
        if let Some(code) = code {
            let copy = code.clone();
            col = col.child(
                h_flex()
                    .gap_1()
                    .child(div().child(crate::trf!("Your sign-in code: {code}"; code => code)))
                    .child(
                        Button::new("cpl-copy")
                            .label(tr("Copy code"))
                            .xsmall()
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()));
                            }),
                    ),
            );
        }
        col = col.child(
            h_flex()
                .gap_1()
                .when(show_signin, |row| {
                    row.child(
                        Button::new("cpl-in")
                            .label(tr("Sign in"))
                            .xsmall()
                            .primary()
                            .on_click(cx.listener(|this, _, _, cx| {
                                let copilot = this.copilot.clone();
                                cx.spawn(async move |this, cx| {
                                    let r =
                                        cx.background_spawn(async move { copilot.sign_in() }).await;
                                    _ = this
                                        .update(cx, |this, cx| this.copilot_signin_result(r, cx));
                                })
                                .detach();
                            })),
                    )
                })
                .when(show_signout, |row| {
                    row.child(
                        Button::new("cpl-out")
                            .label(tr("Sign out"))
                            .xsmall()
                            .on_click(cx.listener(|this, _, _, cx| {
                                let copilot = this.copilot.clone();
                                cx.spawn(async move |this, cx| {
                                    cx.background_spawn(async move { copilot.sign_out() }).await;
                                    _ = this.update(cx, |this, cx| {
                                        this.status = tr("Signed out of Copilot").into();
                                        cx.notify();
                                    });
                                })
                                .detach();
                            })),
                    )
                }),
        );
        col.into_any_element()
    }

    fn render_github(&self, cx: &mut Context<Self>) -> AnyElement {
        if let Some(a) = &self.github {
            return h_flex()
                .gap_2()
                .text_sm()
                .child(format!("{} ({})", a.login, if a.source == "gh" { "GitHub CLI" } else { "token" }))
                .child(Button::new("gh-out").label(tr("Sign out")).xsmall().on_click(cx.listener(|this, _, _, cx| {
                    let _ = crate::github::logout();
                    this.github = None;
                    cx.notify();
                })))
                .into_any_element();
        }
        v_flex()
            .w(px(300.))
            .gap_1()
            .child(Input::new(&self.github_input).small())
            .child(
                h_flex()
                    .gap_1()
                    .child(Button::new("gh-token").label(tr("Sign in with token")).xsmall().primary().on_click(cx.listener(|this, _, _, cx| {
                        let t = this.github_input.read(cx).value().to_string();
                        cx.spawn(async move |this, cx| {
                            let r = cx.background_spawn(async move { crate::github::login_token(&t) }).await;
                            _ = this.update(cx, |this, cx| this.github_result(r, cx));
                        })
                        .detach();
                    })))
                    .child(Button::new("gh-cli").label(tr("Use GitHub CLI login")).xsmall().on_click(cx.listener(|_, _, _, cx| {
                        cx.spawn(async move |this, cx| {
                            let r = cx.background_spawn(async move { crate::github::login_cli() }).await;
                            _ = this.update(cx, |this, cx| this.github_result(r, cx));
                        })
                        .detach();
                    }))),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::Settings;

    /// The WebView notice is on unless the user turned it off: a fresh
    /// install shows it, and so does a settings file written before it existed.
    #[test]
    #[cfg(feature = "embedded-preview")]
    fn webview_notice_is_on_by_default() {
        assert!(Settings::default().preview_webview_notice);
        let old: Settings = serde_json::from_str("{}").unwrap();
        assert!(old.preview_webview_notice);
        let off: Settings = serde_json::from_str("{\"preview_webview_notice\":false}").unwrap();
        assert!(!off.preview_webview_notice);
    }
}
