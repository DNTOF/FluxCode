# VS Code extensions in FluxCode

FluxCode installs extensions from the VS Code Marketplace (or a `.vsix` file) into
`~/.fluxcode/vscode-extensions/<publisher>.<name>-<version>/`. It has no
JavaScript extension host, so it applies what an extension **declares** in
`package.json` → `contributes` as data. This page covers what is applied today,
why grammars aren't, and a design for language-server extensions.

## Applied today

| Contribution | What FluxCode does | Code |
| --- | --- | --- |
| `themes` | Each color theme becomes a palette in the status bar's theme menu, Settings → Appearance and Ctrl+K Ctrl+T. It recolors the interface and the code. | `src/ext/native.rs`, `src/ext/vsx_theme.rs`, `src/theme.rs` |
| `languages` | File extensions (`.svelte`, `.blade.php`) and file names pick a language for files FluxCode doesn't already know. Endings with two dots win over FluxCode's own single-dot ones (`.blade.php` over `.php`). | `native::language`, `files::language_for` |

The extension's page ("In FluxCode") lists what was applied, and its theme names
are buttons that switch to that theme.

### How a theme is converted

1. The theme file is read as JSON with comments and trailing commas. `include`
   is followed first (base first, so the file's own values win), up to six levels,
   and never outside the extension folder. `tokenColors` may be an array or the
   path to another JSON file. `.tmTheme` (XML plist) files are not read.
2. Dark or light comes from `uiTheme` in the manifest, else the file's `type`,
   else the editor background's lightness.
3. FluxCode's 21 palette tokens take the closest VS Code key, with fallbacks:

   | FluxCode | VS Code keys, first found |
   | --- | --- |
   | editor surface | `editor.background` |
   | side bar | `sideBar.background` |
   | title bar | `titleBar.activeBackground`, `activityBar.background` |
   | raised (menus, popovers) | `editorWidget.background`, `menu.background`, `dropdown.background`, `quickInput.background` |
   | hover / selected row | `list.hoverBackground` / `list.activeSelectionBackground` |
   | borders | `sideBar.border`, `editorGroup.border`, `panel.border`, `tab.border` / `input.border`, `dropdown.border`, `contrastBorder` |
   | text, muted, dim | `editor.foreground`, `descriptionForeground`, `editorLineNumber.foreground` |
   | accent | the first *colorful* one of `activityBarBadge.background`, `button.background`, `statusBarItem.remoteBackground`, `progressBar.background`, `focusBorder`, `tab.activeBorder` |
   | accent text | `textLink.foreground` |
   | state colors | `terminal.ansi*`, `editorWarning.foreground`, `errorForeground` |
   | editor selection | `editor.selectionBackground` |

   Colors with alpha are blended onto the surface they sit on. A border equal to
   its background gets a faint hairline (FluxCode separates panels with them).
   The accent is the badge color rather than the button color because many themes
   keep buttons grey (Dracula: pink badge, grey buttons).
4. Text colors are moved toward white (dark) or black (light) until they reach
   4.5:1 on every surface, and the text on the accent 5.1:1: the same contract the
   built-in palettes are tested against. Dracula's line-number blue, for example,
   is lifted a little.
5. Code colors: syntect names what it matched with TextMate scopes, and gpui-kit
   maps those onto about 30 highlight names (`keyword`, `string`, `function`, ...).
   For each name, a few representative scopes (`keyword.control`, `storage.type`)
   are resolved against `tokenColors` the way VS Code does: the most specific
   selector wins, a later rule breaks a tie, foreground and font style are decided
   separately. Selectors with a parent (`source.python keyword`) are skipped,
   since one representative scope has no stack to match them against. Diagnostic
   and git colors come from `editorError.foreground`, `editorGutter.*Background`
   and similar keys.

Tested against the real Dracula Official (2 themes) and One Dark Pro (5 themes)
packages: `cargo test --bin fluxcode marketplace_themes -- --ignored --nocapture`
(needs the network; set `USERPROFILE` to a scratch folder so it installs there).

## Not applied

- **Grammars** (`contributes.grammars`, TextMate `.tmLanguage.json`): see below.
  A contributed language still gets colors when gpui-kit's syntect set (bat's
  ~200 grammars) knows its file extension: `.svelte`, `.tf`, `.vue` and most
  others do.
- **Snippets**: gpui-kit's editor has no tab stops yet, so a snippet would insert
  as plain text (the same limit Emmet has).
- **Icon themes** (`iconThemes`): FluxCode draws its own Lucide file icons; an icon
  theme is a font or SVG set keyed by file type, possible as a later step.
- **Anything with code** (`main` / `browser`): commands, debuggers, language
  servers. See the extension-host design below.

## Grammars: evaluation

Loading `.tmLanguage.json` into the highlighter needs two things, and neither is
small:

1. **A way in.** gpui-kit 0.9 builds its syntect `SyntaxSet` once, in a private
   `LazyLock` in `gpui-component/src/highlighter/syntect_backend.rs`, from
   `two_face::syntax::extra_newlines()`. There is no API to add a syntax. Options:
   - Upstream a hook (e.g. `register_syntaxes(Vec<SyntaxDefinition>)` before the
     first highlight, rebuilding the set through `SyntaxSetBuilder`). Cleanest,
     needs a gpui-kit release.
   - Vendor gpui-component like `vendor/gpui-fast-windows` and patch it with
     `[patch.crates-io]`. Works now, but every gpui-kit upgrade has to carry the
     patch.
2. **A converter.** syntect reads Sublime's `.sublime-syntax` (YAML), not
   TextMate JSON. A converter has to map `patterns` / `repository` / `include`
   (`#name`, `$self`, `$base`, `source.other`), `begin`/`end` with
   `beginCaptures`/`endCaptures`/`contentName` onto contexts with push/pop, and
   handle what has no direct equivalent: `while` rules (Markdown-style blocks),
   `injections` / `injectionSelector`, `\G` anchors, back-references from `begin`
   into `end`. The regexes are Oniguruma; syntect here uses fancy-regex
   (`syntect-fancy`), which lacks some Oniguruma syntax (`\h`, some possessive and
   conditional forms) and would need rewriting or a fallback to plain text.

Estimate: the hook is a day; a converter that handles real grammars (Svelte,
Astro, Prisma, Terraform) well enough is one to two weeks with a corpus test of
popular grammars. Value is moderate, since bat's set already covers most languages
people install grammars for. Recommendation: do the upstream hook when there is a
concrete language users miss, convert only the subset that grammar needs, and fall
back to plain text for a grammar that fails to convert.

## Extension host for language servers: design

Most extensions people want "to work" are language servers wrapped in a little
JavaScript: the extension's `activate()` builds a `LanguageClient` from
`vscode-languageclient` with `serverOptions` (a command or a Node module to run)
and `clientOptions` (document selector, settings section, initialization options),
and the client spawns the server over stdio. The server is what does the work.

FluxCode already speaks LSP to Copilot (`src/copilot.rs`, inline completions
only) and gpui-kit's editor has provider hooks for completion, hover,
go-to-definition, code actions, document colors, semantic tokens and diagnostics.
So the plan is to let Node run just enough of the extension to learn **how to start
its server**, then have FluxCode run the server itself.

### Shape

```
FluxCode ──spawn──▶ node fluxcode-host.js <extension folder>
                      │ fake `vscode` module (require hook)
                      │ hooks child_process.spawn / fork
                      │ calls the extension's activate(context)
                      │   └─ LanguageClient.start() → spawn(server)   ◀── captured
                      │ writes {command, args, cwd, env, transport,
                      │         documentSelector, initializationOptions,
                      │         settings section} as JSON to stdout, exits
FluxCode ──spawn──▶ the language server (stdio), its own LSP client
```

- **The shim** (`fluxcode-host.js`, about 600–1,000 lines, shipped with FluxCode):
  a `vscode` module with the activation-time surface `vscode-languageclient`
  touches: `workspace.getConfiguration` / `workspaceFolders` / `onDidChange*`
  events, `window.createOutputChannel` / `showInformationMessage` (logged),
  `languages.createDiagnosticCollection`, `commands.registerCommand` (recorded),
  `Uri`, `EventEmitter`, `Disposable`, `Position` / `Range` / `Location`, and an
  `ExtensionContext` (`extensionPath`, `asAbsolutePath`, `globalState`,
  `workspaceState`, `secrets`, `subscriptions`). Extensions are usually bundled
  (esbuild/webpack inline `vscode-languageclient`), but `require('vscode')` is
  always external, so the fake module is the reliable seam; spawning is caught at
  `child_process`, which bundling doesn't hide. The first message the client
  writes to the server's stdin is `initialize`; the shim keeps its
  `initializationOptions` and kills the child.
- **FluxCode's side**: a general LSP client (`src/lsp.rs`, generalizing what
  `copilot.rs` does) with document sync, `publishDiagnostics` into the editor's
  `DiagnosticSet`, completion / hover / definition / code-action providers, and
  answers to `workspace/configuration` from the extension's
  `contributes.configuration` defaults plus a per-extension section in
  settings.json. One server per (extension, workspace folder), started when a file
  matching the document selector opens, stopped with the folder.
- **Caching**: the captured launch is stored next to the extension
  (`.fluxcode-launch.json`, keyed by version), so Node runs once per install, not
  on every start.

### What it would and wouldn't cover

Works: servers that ship in the VSIX or are found on PATH, started by the client:
rust-analyzer, YAML (Red Hat), Svelte, Tailwind CSS IntelliSense, Prisma, Astro,
ESLint's server, TOML (Even Better TOML), clangd (once its binary is downloaded).

Doesn't: client-side middleware and custom requests (rust-analyzer's "run test"
lenses, Expand Macro), webviews, tree views, debuggers (DAP is another protocol and
another project), extensions whose value is the JavaScript itself (GitLens),
and Microsoft's licensed servers (Pylance, C# Dev Kit, the C/C++ IntelliSense
engine): their licenses allow use with Microsoft products only.

### Costs

| | |
| --- | --- |
| Requires | Node.js on PATH (FluxCode already detects it for the Copilot server install). Without Node, an extension with a server says so on its page. |
| Memory | Node only during capture (~40–60 MB for a second or two, once per install). Each server costs what it costs in VS Code (rust-analyzer: hundreds of MB on a large crate). |
| Startup | None at FluxCode start; a server starts when its first file opens. |
| Exe size | The shim is a JS file of a few tens of KB; the LSP client is Rust FluxCode mostly has. |
| Effort | Shim with a test corpus of 10 popular extensions: ~1 week. General LSP client (sync, diagnostics, completion, hover, definition, code actions, configuration): ~2 weeks. Settings UI for per-extension configuration: a few days. |
| Risk | Extensions do unexpected things in `activate()` (download a server, read VS Code-only APIs). The shim logs every unimplemented API it is asked for, and capture fails cleanly ("this extension needs VS Code's host") instead of half-working. |

A cheaper first step with most of the value: hand-written launch recipes for the
five or six most installed server extensions (rust-analyzer, YAML, Svelte,
Tailwind, Prisma, TOML) and the general LSP client, no Node shim. The shim then
generalizes the recipes to the long tail.
