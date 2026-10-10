# `jaj.tidal`: UI/UX and QML Architecture Review

**Reviewer:** Antigravity (Claude Opus)
**Date:** 2026-10-10
**Commit reviewed:** `8d849c5` (`main`)
**Scope:** [BarWidget.qml](BarWidget.qml) (890 lines), [TidalIcon.qml](TidalIcon.qml), [Service.qml](Service.qml), [UI_PROPOSAL.md](UI_PROPOSAL.md)
**Reference baseline:** The Omarchy shell UI kit (`/usr/share/omarchy/shell/{Commons,Ui}`), the first-party `panels/audio` panel and the `services/media` bar widget.

---

## 0. Executive summary

The plugin works. It authenticates, plays lossless audio, exposes MPRIS, and stays within the Omarchy color rules (zero hex literals). What it does not yet deliver is the product its own design document describes. The current flyout is a **stack of every feature, laid out in a single 890-line `Column`**. Visual weight is spread evenly across brand chrome, status banners, three bordered chips in the hero, two different slider styles, and five list views that stay alive while hidden. The philosophy "the user knows what he wants" requires the opposite: one dominant surface (now playing) and one dominant input (search/library). Everything else should recede.

Several issues are **verified functional defects**, not matters of taste. The most important: **Escape always closes the panel, even when it should only clear the search.** I confirmed this with an isolated `qmltestrunner` experiment against the real `PanelKeyCatcher` (§3.1). Other defects: the seek slider floods the daemon with seek commands while dragging; search results flash "No tracks found" on every keystroke; the playlists list and the search results render **at the same time**; search-result artwork overflows its rows; and every favorites row downloads album art it never shows.

### Scorecard

| Area | Score | One-line verdict |
| :--- | :---: | :--- |
| Visual hierarchy & minimalism | **5 / 10** | The right ingredients in the wrong order. Brand chrome and status noise compete with playback. |
| Interaction design & usability | **5 / 10** | Core actions exist. Transport order, seek behavior, and search mode are flawed. |
| Keyboard navigation & accessibility | **4 / 10** | Generous shortcut map, broken Escape, unbounded cursor, no focus model, zero `Accessible` metadata. |
| Omarchy theming compliance | **6 / 10** | Passes the hex rule. Bypasses Style state/spacing tokens and the theme font family. |
| QML architecture & performance | **4 / 10** | Monolithic file, 4× duplicated delegates, hidden-but-live views, wasted image decoding, UI writes into service state. |
| **Overall** | **4.8 / 10** | **A functional foundation. Roughly two focused iterations away from the proposal's vision.** |

### Top 10 recommendations (in priority order)

1. **Fix Escape**: clear search → leave exploration → close, one level at a time (§3.1).
2. **Seek on release**, not on every `moved` event. Preview the time while dragging, and make arrow-key seeks relative (§2.2).
3. **Reorder the transport**: `Shuffle · Prev · ▶ · Next · Repeat`. Use Nerd Font glyphs and `PanelActionButton` (§2.1).
4. **Move the tabs directly above the list** they control. Remove the redundant section header. Hide the fake "Queue" tab until IPC exposes the queue (§1.2).
5. **Make search an exclusive mode**: hide the tabs and library lists while a query is active, keep stale results while typing, and stop the "No tracks found" flash (§2.3).
6. **Remove brand chrome** (logo + "TIDAL" + "Connected"). Fold quality and settings into the hero row (§1.1).
7. **Extract a single `TrackRow.qml` delegate** for favorites, playlist tracks, search, and exploration. This fixes the overflow and elision bugs and the inconsistent row heights in one place (§5.2).
8. **Adopt Style tokens**: `Style.selectedAccentFill`/`hoverFill`, `Style.spacing.*`, `Style.font.family`, and `Text.PlainText` everywhere (§4).
9. **Cap the bar label width** (marquee or elide, like the first-party media widget). Align the click/wheel conventions with `services/media` (§2.5).
10. **Load list views lazily** (`Loader`/`StackLayout`), set `sourceSize` on all images, and delete the hidden image and dead code (§5).

---

## 1. Visual hierarchy & minimalism

### 1.1 What the flyout looks like today (top → bottom)

```
┌──────────────────────────────────────────────┐
│ ◆◆◆ TIDAL               [LOSSLESS] [󰒓 Settings]│  ← brand row (≈ 40 px)
│     Connected                                  │
│ ───────────────────────────────────────────── │
│ [󰖁 System audio is muted        ][Unmute]     │  ← conditional banner
│ [Queue] [Favorites] [Playlists]                │  ← tabs (far from their list!)
│ ┌──────┐ Title                                 │
│ │ ART  │ [Artist chip] [♡ chip]                │  ← 3 bordered chips in hero
│ │ 96px │ [Album chip]                          │
│ └──────┘                                       │
│ ━━━━━━━━━━━━━━●──────────────────              │
│ 1:23                                    4:56   │
│      [⏮] [▶] [󰒝] [⏭] [󰑖]                       │  ← play is not centered
│ 󰕾 ────────────●──────────────── 78%            │  ← second slider style
│ ───────────────────────────────────────────── │
│ FAVORITES                                      │  ← duplicates the selected tab
│ [ Search tracks, albums, artists, playlists… ] │
│ list (≤ 280 px)                                │
└──────────────────────────────────────────────┘
```

**Findings**

| # | Finding | Evidence | Severity |
| :-- | :--- | :--- | :---: |
| V1 | **The brand row is the most visually prominent element** (24 px logo + 14 px bold "TIDAL"). The hero track title is *also* 14 px bold (`Style.font.title`), so the brand ties with the music. "Connected" is status noise: if the hero renders, the user is connected. | [BarWidget.qml#L119-L192](BarWidget.qml#L119-L192), [#L446-L454](BarWidget.qml#L446-L454) | High |
| V2 | **The tabs are separated from the list they control** by the hero, scrubber, transport, volume, a separator, a section header, and the search field. This violates the Gestalt principle of proximity. The `// Now Playing Card` comment sits above the *tabs*, which suggests an accidental reorder. | [#L386-L393](BarWidget.qml#L386-L393) | High |
| V3 | **Three bordered `Ui.Button` chips in the hero** (artist, heart, album) give metadata the same visual weight as transport actions. Metadata should read as text and reveal clickability on hover. | [#L456-L472](BarWidget.qml#L456-L472) | Medium |
| V4 | **The "FAVORITES / PLAYLISTS / QUEUE" section header repeats the selected tab.** It is redundant ink. | [#L588](BarWidget.qml#L588) | Low |
| V5 | **The idle state is noisy.** With nothing playing, the panel still shows "No Track Playing", an artist chip reading "Select a favorite below" (clickable, does nothing), a heart button (toggles the favorite on *nothing*), a 0:00/0:00 scrubber, full transport, and volume. Pillar 1 of the proposal ("No Idle Noise") is not met. | [#L448-L459](BarWidget.qml#L448-L459) | Medium |
| V6 | **System mute is shown in three places**: the bar label glyph, a full-width urgent banner, and the volume icon. System audio state belongs to Omarchy's audio panel. The banner is useful, but the bar glyph replaces the Tidal identity when idle (see B3). | [#L43-L45](BarWidget.qml#L43-L45), [#L196-L235](BarWidget.qml#L196-L235) | Low |
| V7 | **The quality badge maps every unknown tier to "HIGH AAC"** (e.g. `LOW`, `HI_RES`, empty), so it can show false information. | [#L177](BarWidget.qml#L177) | Low |

### 1.2 Information density & proportions

* Panel width is `Style.space(380)`, which matches the audio panel. Good. However, `contentHeight` is **uncapped** ([#L79](BarWidget.qml#L79)) and there is **no outer `ScrollView`**. The first-party audio panel caps at `Style.space(560)` and wraps its content in a `ScrollView`. With the settings card + mute banner + error text + a 280 px list, the tail of the panel can be clipped by `fittedContentHeight` on short or scaled screens.
* **Visible rows per list (280 px viewport):** favorites ≈ 7 (36 px + 4 spacing), search ≈ 7, playlist tracks ≈ 6, playlists ≈ 4. The proposal targets 8–10.
* **Row heights are inconsistent across the four track lists:** 36 / 40 / 40 / 40 px, with spacing of 4 / 0 / 4 / 0. Switching contexts makes the list visibly "jump".
* **Layout instability:** the panel height changes on every tab switch, every search keystroke (the list collapses, see S2), when settings open, and as `contentHeight` estimates settle. A **fixed list viewport** (e.g. `Style.space(280)`) would keep the panel geometry stable, which matters for a popover anchored to the bar.

### 1.3 Typography

| Role | Current token | Recommended |
| :--- | :--- | :--- |
| Hero title | `font.title` (14) bold | `font.heading` (16) bold. Must outrank everything else. |
| Hero artist/album | Button text `font.body` | `font.body` / `font.bodySmall`, `Color.muted`, underline on hover |
| Brand "TIDAL" | `font.title` bold | **Remove** |
| Row title | `font.body` | `font.body` ✓ |
| Row meta (artist • album, duration) | `font.caption` (10) muted | `font.bodySmall` (11). 10 px muted text fails comfortable contrast on several themes. |
| Pairing code | `font.pixelSize: Style.space(26)` | `Style.font.displayLarge`. Spacing tokens must not size fonts. |
| Placeholder glyphs | `Style.space(32)` | `Style.font.display` |

**Font family:** of 50 `Text` elements, **only 2 set `font.family`** ([#L142](BarWidget.qml#L142), [#L150](BarWidget.qml#L150)). Plain QtQuick `Text` does not inherit fonts, and the shell does not set an application font. The rest of the flyout therefore renders in the fontconfig default (typically a sans-serif) instead of the theme's `Style.font.family` (monospace by default). Next to first-party panels, this is the most visible theming inconsistency.

---

## 2. Interaction design & usability

### 2.1 Transport controls

| # | Finding | Evidence | Severity |
| :-- | :--- | :--- | :---: |
| T1 | **The button order is `Prev · Play · Shuffle · Next · Repeat`.** Play is off-center, and shuffle separates play from next. Every mainstream player (and the proposal's own wireframe) uses `Shuffle · Prev · Play · Next · Repeat`, symmetric around play. | [#L514-L538](BarWidget.qml#L514-L538) | **High** |
| T2 | **Glyph sets are mixed.** `⏮ ⏸ ▶ ⏭` are Unicode symbols, while `󰒝 󰑖 󰑘` are Nerd Font MDI glyphs. Their baselines and advance widths differ. `Noto Color Emoji` is installed on this system, so fontconfig fallback can render the Unicode ones as color emoji on themes whose font lacks them. Use `󰒮 󰐊/󰏤 󰒭` throughout. | [#L519-L537](BarWidget.qml#L519-L537) | Medium |
| T3 | **The play button uses `selected: true` to mean "primary".** In the Omarchy state vocabulary, `selected` means a *persistent chosen state*. The shuffle and repeat buttons also use `selected` to mean "on". The result: play looks identical to "shuffle on", so the user cannot tell at a glance whether shuffle is active. Give play a distinct size or fill (`PanelActionButton` with a larger `size`, or an accent-filled circle) and keep `selected` for toggles. | [#L528](BarWidget.qml#L528), [#L535-L537](BarWidget.qml#L535-L537) | Medium |
| T4 | **The mute toggle is a `Text` with a `TapHandler`.** It has no hover state, no pointer cursor, no tooltip, cannot take focus, and has a ~12 px hit target. | [#L542-L544](BarWidget.qml#L542-L544) | Medium |
| T5 | **The volume slider is a custom `QtQuick.Controls.Slider`** with a single-color track (no fill), so you cannot read the level without finding the knob. It also looks different from the seek `PanelSlider` directly above it. `PanelSlider` already supports `maximum`, `rightClicked` (the mute convention used by the audio panel), wheel input, and `tickCount` (useful to mark 100 % on a 0–150 % range). | [#L545-L552](BarWidget.qml#L545-L552) | Medium |
| T6 | **The volume slider width depends on the percent label's implicit width** (`"5%"` vs `"150%"`), so the slider resizes as you drag it. Give the label a fixed width with right alignment. | [#L546](BarWidget.qml#L546), [#L553](BarWidget.qml#L553) | Low |
| T7 | **Wheel steps are per event, not per notch.** Both the custom `WheelHandler` and `PanelSlider.onWheel` apply ±5 % per event. Touchpads and high-resolution wheels emit many small `angleDelta` events, so a gentle two-finger scroll jumps volume by 30–50 %. Accumulate `angleDelta.y` and step per 120 units. | [#L551](BarWidget.qml#L551) | Medium |
| T8 | **Volume can exceed 100 % (up to 150 %)** with no visual threshold. Over-amplification should be opt-in or at least marked. | [#L547](BarWidget.qml#L547) | Low |

### 2.2 Seek scrubber

| # | Finding | Evidence | Severity |
| :-- | :--- | :--- | :---: |
| SK1 | **`onMoved` sends a seek IPC command on every mouse-move during a drag.** One drag produces dozens of mpv seeks, which causes audio stutter and daemon load. `PanelSlider` emits `released(value)`, so seek on that. | [#L485-L489](BarWidget.qml#L485-L489) | **High** |
| SK2 | **No time preview while dragging.** The time labels bind to `trackPosition`, not to the slider's `liveValue`, so the user can't see where they will land. | [#L499](BarWidget.qml#L499) | Medium |
| SK3 | **The mouse wheel over the scrubber seeks 5 % of the track per event** (and fires both `moved` and `released`, so two IPC commands). Wheel-scrolling the panel near the scrubber can accidentally jump the song. | `PanelSlider.onWheel` | Medium |
| SK4 | **Keyboard seek is absolute and computed from a stale position.** `seek(trackPosition ± 5)` only advances once per daemon `position_changed` tick, so pressing → three times quickly seeks +5 s, not +15 s. Add a relative `seek_relative` IPC or an optimistic local position. | [#L100-L101](BarWidget.qml#L100-L101) | Medium |

### 2.3 Search & library browsing

| # | Finding | Evidence | Severity |
| :-- | :--- | :--- | :---: |
| S1 | **The playlists tab and search results render at the same time.** The playlists `Column` checks only `libraryTab === "playlists"`, not whether a search is active. On the Playlists tab, typing a query stacks two lists (up to 560 px). | [#L733](BarWidget.qml#L733) vs [#L833](BarWidget.qml#L833) | **High** |
| S2 | **Every keystroke flashes "No tracks found".** `onTextChanged` calls `clearSearch()`, which sets `searching = false` and `results = []`. During the 350 ms debounce, the status text evaluates to "No tracks found", and the list collapses to 0 px, so the panel shrinks and grows on every key. Keep stale results and set `searching = true` when the debounce starts. | [#L604-L609](BarWidget.qml#L604-L609), [Service.qml#L401-L405](Service.qml#L401-L405) | **High** |
| S3 | **Search results stay visible during exploration.** The search field hides during exploration, but its text and the results `Column` stay visible below the album/artist list. | [#L833](BarWidget.qml#L833) | Medium |
| S4 | **False affordance:** the placeholder promises "tracks, albums, artists, playlists", but the backend returns tracks only, and the empty state says "No tracks found". | [#L603](BarWidget.qml#L603), [#L837](BarWidget.qml#L837) | Medium |
| S5 | **Mode confusion:** while searching, the tabs stay visible with one still highlighted, the header says "SEARCH RESULTS", and the escape hatch is labelled "← Back to favorites" even when the user came from Playlists. | [#L612-L620](BarWidget.qml#L612-L620) | Medium |
| S6 | **No guard against stale responses.** Results carry no query or request id, so a slow response for `"mile"` can overwrite results for `"miles davis"`. | [Service.qml#L314-L317](Service.qml#L314-L317) | Medium |
| S7 | **The "Queue" tab shows favorites** (the AGENTS.md M9 section acknowledges this). Labelling favorites "QUEUE" is misleading. Hide the tab until queue IPC exists. | [#L625](BarWidget.qml#L625), [#L643](BarWidget.qml#L643) | Medium |
| S8 | **Artist/album sub-links are invisible hit zones.** Clicking the artist/album caption explores. Clicking anywhere else in the row plays. The links have no hover state or cursor change, so the user hits them by accident. | [#L697-L708](BarWidget.qml#L697-L708) | Medium |
| S9 | **Exploration has no loading state** (the list is empty while the request runs). Its header says "ARTIST TOP TRACKS" without naming the artist or album. | [Service.qml#L443-L444](Service.qml#L443-L444), [#L563](BarWidget.qml#L563) | Low |
| S10 | **Only favorites highlight the playing track.** Search, playlist, and exploration rows have no now-playing indicator and no hover state. | [#L655-L657](BarWidget.qml#L655-L657) | Low |
| S11 | **The playlist filter persists** after leaving a playlist (`playlistFilter` and the field text are never reset). `(t.title + " " + t.artist)` yields `"… undefined"` for missing artists. | [#L797](BarWidget.qml#L797), [#L806](BarWidget.qml#L806) | Low |

### 2.4 Settings popup

| # | Finding | Severity |
| :-- | :--- | :---: |
| ST1 | **It is not a popup.** The card is inlined at the top of the authenticated column and pushes the hero down, so the layout jumps. It is only reachable when authenticated, which is exactly when diagnostics are least needed. | Medium |
| ST2 | **Logout is one click away with no confirmation.** `qs.Ui.ConfirmDialog` exists for exactly this case. | Medium |
| ST3 | **No daemon diagnostics at all:** no daemon version, binary path (bundled vs development), socket state, reconnect status, last error, or "restart daemon" action. The service already holds most of this (`binaryPath`, `daemonBinaryExists`, socket state). | Medium |
| ST4 | **Quality selection is three loose `Ui.Button`s.** `qs.Ui.ButtonGroup` is the kit's segmented control, with keyboard support. There is no hint that the change applies from the next track, and no display of requested vs. negotiated quality (e.g. "Requested Hi-Res → playing Lossless"), which is the most useful piece of information given the HI_RES negotiation finding in M6. | Medium |
| ST5 | **Redundant copy and controls:** "Account connected" repeats the header, and the "Close" button duplicates the toggle. | Low |

### 2.5 Bar widget

| # | Finding | Evidence | Severity |
| :-- | :--- | :--- | :---: |
| B1 | **Label width is unbounded.** `"󰐊 " + title + " • " + artist` grows with the title, so long classical titles can take over the bar. The first-party media widget caps at `maxLabelWidth: 180` with a marquee. | [#L41-L46](BarWidget.qml#L41-L46) | **High** |
| B2 | **Duplicated now-playing info:** because M4 exposes MPRIS, Omarchy's own `services/media` widget already shows the Tidal track. With both widgets on the bar, the title appears twice. Offer an **icon-only mode** (a manifest/setting), or default to icon-only and let the native media widget own the now-playing text. | — | Medium |
| B3 | **When idle and muted, the Tidal icon is replaced by `󰖁`**, which is indistinguishable from a system volume indicator. The plugin loses its identity in the bar. | [#L53](BarWidget.qml#L53), [#L59](BarWidget.qml#L59) | Medium |
| B4 | **Click conventions conflict with the native media widget.** Native: left = play/pause, middle = next, right = popup, wheel = prev/next. Tidal: left = panel, middle **and** right = play/pause, wheel = nothing. Two adjacent media widgets with different mouse grammar is a usability trap, and binding two buttons to one action wastes a gesture. Suggest: left = panel, middle = play/pause, right = next, wheel = volume or prev/next (`WidgetButton.wheelMoved` is already available). | [#L62-L68](BarWidget.qml#L62-L68) | Medium |
| B5 | **The play icon has the opposite meaning from the native widget.** Tidal shows `󰐊` while *playing* (state semantics). The native widget shows `󰏤` while playing (action semantics). Users will see opposite icons for the same state side by side. Pick one; matching the shell is preferable. | [#L43](BarWidget.qml#L43) | Low |
| B6 | **Tooltip and label separators are inconsistent:** `" - "` (tooltip), `" • "` (label), `" — "` (native widget and proposal). | [#L55](BarWidget.qml#L55) | Low |

### 2.6 Onboarding (unauthenticated)

* The **missing-daemon banner tells users to run `build.sh`**. That contradicts the "installs without Rust/Cargo" contract in AGENTS.md. It also compares `authError` against a *different* string from `Service.buildScriptMessage` ([Service.qml#L15](Service.qml#L15) vs [BarWidget.qml#L246](BarWidget.qml#L246), [#L331](BarWidget.qml#L331)). The comparison never matches, so **both** the stale banner and the real error render. The banner also flashes on shell start, because `daemonBinaryExists` defaults to `false` before the first check completes.
* The pairing card repeats its instruction ("enter on link.tidal.com" twice). It is missing **copy code**, **auto-open on start**, and an **expiry countdown**. The code is sized with `Style.space(26)` instead of a font token.

---

## 3. Keyboard navigation & accessibility

### 3.1 Escape always closes the panel (verified)

`PanelKeyCatcher` defines its own `Keys.onPressed` that emits `closeRequested()` on Escape. QML signal handlers are **additive**: the instance's `Keys.onPressed` in [BarWidget.qml#L95-L110](BarWidget.qml#L95-L110) runs **in addition to** the base handler, not instead of it. Setting `event.accepted = true` does not stop the second handler, and `onCloseRequested: root.close()` is wired.

I ran an isolated `qmltestrunner` experiment against a copy of the real `/usr/share/omarchy/shell/Ui/PanelKeyCatcher.qml` with an instance-level Escape handler that clears a text field:

```
ESC(catcher focus): closes=1 instanceRuns=1 text=''   ← cleared AND closed
TYPE(field focus):  text='j xa' instanceRuns=0        ← printable keys reach the field ✓
ESC(field focus):   closes=1 text=''                  ← cleared AND closed
TAB(field focus):   tabs=1                            ← Tab leaves the panel
```

**Effect:** Escape never performs "clear search" on its own; it always closes the flyout. Escape inside exploration or a playlist also closes instead of going back.

**Fix:** don't attach a second `Keys.onPressed`. Use the semantic signals, and route Escape through a back-stack:

```qml
PanelKeyCatcher {
  id: keyCatcher
  blocked: playlistFilterField.activeFocus          // let the editor own its keys
  onCloseRequested: root.goBack()                    // not root.close()
  onMoveRequested: function(dx, dy) { dy !== 0 ? root.moveCursor(dy) : root.seekRelative(dx * 5) }
  onActivateRequested: root.activateCursor()         // Enter + Space → see note below
  onTextKey: function(t) { root.handleShortcut(t) }
  onTabRequested: function(d) { root.switchPanel(d) }
}

function goBack() {
  if (searchField.text !== "")                  { searchField.text = ""; keyCatcher.forceActiveFocus() }
  else if (tidalService.explorationView !== "") { tidalService.closeExploration() }
  else if (tidalService.currentPlaylistId !== "") { tidalService.closePlaylist() }
  else if (root.settingsOpen)                   { root.settingsOpen = false }
  else root.close()
}
```

> [!NOTE]
> `PanelKeyCatcher` maps both Return and Space to `activateRequested`. To keep "Space = play/pause" while the cursor is on a list, intercept Space in a small `Keys.onSpacePressed` on a child item, or treat activate as "play cursor row" and give play/pause a different key (e.g. `p`). Either way, keep a single source of truth.

### 3.2 Other keyboard findings

| # | Finding | Evidence | Severity |
| :-- | :--- | :--- | :---: |
| K1 | **`keyboardIndex` is unbounded.** `Down` increments forever. After overshooting, `Up` needs N presses before anything visible happens. | [#L107](BarWidget.qml#L107) | High |
| K2 | **Enter plays from favorites (or search), regardless of the active tab.** On Playlists, a playlist's tracks, or exploration, Enter plays the *n*-th **favorite**. | [#L109](BarWidget.qml#L109) | High |
| K3 | **Bare letter shortcuts are a hazard.** On open, focus is on the catcher, so typing an artist name runs commands: `f` toggles a favorite on the user's remote library, `r` cycles repeat, `s` toggles shuffle, and Space toggles play. The proposal's "start typing to search" is not implemented, and it conflicts with single-letter shortcuts anyway. **Recommendation:** route printable keys to search (type-to-search) and move actions to modifier chords (`Ctrl+S`, `Ctrl+R`, `Ctrl+L` for "like"), or require the list cursor to be active first, as the audio panel does with `cursorActive`. | [#L102-L105](BarWidget.qml#L102-L105) | High |
| K4 | **Shortcuts ignore modifiers.** `event.key === Qt.Key_S` also matches `Ctrl+S`, `Alt+S`, and `Shift+S`. | [#L102-L106](BarWidget.qml#L102-L106) | Low |
| K5 | **`j/k/h/l/x` are swallowed with no effect.** The base catcher accepts them and emits `moveRequested`/`deleteRequested`, but nothing is connected, so the vim keys promised by the proposal do nothing. | `PanelKeyCatcher` | Medium |
| K6 | **No cursor model for controls.** `Ui.Button.hasCursor`/`focusable` are never used, so transport, tabs, and settings can't be reached by keyboard. Tab is reserved by Omarchy for panel switching (a correct convention), so the panel needs an internal "focus section" model (header → transport → tabs → list), as the audio panel implements. | — | Medium |
| K7 | **Keyboard and hover selection are conflated.** Row 0 is always highlighted (`index === keyboardIndex`, default 0) even for mouse users, and the highlight looks identical to "now playing". Only show the cursor once a navigation key is pressed, and use `Style.hoverFill`/`focusFillColor` for the cursor and `selectedAccentFill` for the playing track. | [#L656](BarWidget.qml#L656) | Medium |
| K8 | **No way back from search focus except Escape** (which closes the panel, §3.1). Clicking a row leaves focus in the search field, which disables all shortcuts. | — | Low |
| K9 | **No `Accessible.*` metadata** on any custom control (0 occurrences). Text+TapHandler controls are invisible to assistive tech. Track text uses the default `AutoText` format. Use `Text.PlainText`, as every first-party panel does, so titles containing `<` are never parsed as rich text. | — | Medium |

---

## 4. Omarchy theming & design-system compliance

| Rule | Status | Notes |
| :--- | :---: | :--- |
| Zero hex literals | ✅ | The `verify-omarchy-compliance.sh` hex grep passes. |
| Colors only from `qs.Commons.Color` | ⚠️ | **11 `Qt.rgba(Color.x.r, …, α)` literals** with hard-coded alphas (0.08, 0.10, 0.12, 0.15). These bypass the theme-tunable state tokens (`Style.selectedAccentFill`, `Style.hoverFill`, `Style.selectedFill`, `Util.alpha`). A theme that sets `selected-fill-alpha` in `shell.toml` changes every first-party panel but not this one. |
| Surface roles | ⚠️ | Rows and the settings card fill with `Color.background`. Popups render on `Color.popups.background`, which themes may tint or make translucent (`popups.background-alpha`). The rows then show as opaque blocks. Use `"transparent"` plus state fills. Non-current favorite rows also draw a 1 px `Color.background` border. |
| Spacing tokens | ⚠️ | **88 `Style.space(N)` calls with magic numbers, and 0 uses of `Style.spacing.*`.** These scale correctly, but they are "arbitrary margins" in the sense of AGENTS.md rule 4. Map them: 4→`spacing.sm`, 6→`md`, 8→`lg`/`controlGap`, 12→`xxl`, 14→`panelGap`, row heights→`spacing.popupRowHeight`/`controlHeight`. |
| Border widths | ⚠️ | `border.width: 1` literals. Use `Style.normalBorderWidth`/`Border.controlSpec(...)` (respects `*-border-width: 0` themes). |
| Typography tokens | ⚠️ | Font sizes mostly use tokens. The exceptions use `Style.space()` for font sizes. **The font family is missing on 48 of 50 `Text` elements** (§1.3). |
| Live theme switching | ✅ | All colors are bindings, so `omarchy theme set` restyles without a restart. `TidalIcon` repaints on `colorChanged`. |
| Dark/light adaptability | ✅/⚠️ | Foreground-alpha fills work in both modes. `Color.muted` captions at 10 px are borderline on light themes (e.g. `catppuccin-latte`, `flexoki-light`, `rose-pine`, `white`). Accent text on a 15 % accent tint is fine on most palettes, but low on themes where accent ≈ foreground (the default palette has `accent == foreground`). |

---

## 5. QML architecture & performance

### 5.1 Structure

* **[BarWidget.qml](BarWidget.qml) is an 890-line monolith** containing ~25 `Ui.Button`s, 5 `ListView`s, 4 near-identical track delegates, inline 400-character one-liners (e.g. [#L106](BarWidget.qml#L106), [#L109](BarWidget.qml#L109)), and dead code. The proposal's §5.1 decomposition (`MiniPlayerCard`, `DenseTrackList`, `KeyboardRouter`, `SearchEngine`) is the right direction and **has not been started**.
* **Dead code ships:** the `visible: false` quality rectangle ([#L395-L409](BarWidget.qml#L395-L409)) and the `visible: false` 1×1 artwork tile in every favorites row ([#L664-L684](BarWidget.qml#L664-L684)).
* **The UI writes service state directly**: `tidalService.explorationView = ""`, `explorationTracks = []`, `currentPlaylistId = ""`, `playlistTracks = []` ([#L562](BarWidget.qml#L562), [#L737](BarWidget.qml#L737)). Expose `closeExploration()`/`closePlaylist()` so the service owns its invariants. This also breaks bindings if those properties are ever made `readonly`/derived.
* **Visibility is predicated on `(!tidalService || tidalService.explorationView === "")` in six places**, plus `searchField.text.trim() !== ""` in seven. Derive one `readonly property string view: settingsOpen ? "settings" : exploring ? "explore" : searching ? "search" : libraryTab` and drive a `StackLayout`/`Loader` from it. This removes the S1/S3 class of bugs by construction.

### 5.2 Delegates

Four delegates duplicate ~30 lines each and have each drifted:

| Delegate | Height | Spacing | Art | Now-playing | Keyboard | Meta elision |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| Favorites | 36 | 4 | hidden but **loading** | ✅ | ✅ | ❌ |
| Playlist tracks | 40 | 4 | — | ❌ | ❌ | ❌ |
| Search | 40 | 0 | **48 px in a 28 px slot → overflows into neighbouring rows** | ❌ | ✅ | ❌ |
| Exploration | 40 | 0 | — | ❌ | ❌ | ❌ |

**The meta line doesn't elide:** the artist • album `Row` children have no width, so `elide` does nothing, and long album names overlap the duration column (no clip). One `TrackRow.qml` (`title`, `artist`, `album`, `duration`, `current`, `hasCursor`, signals `play/exploreArtist/exploreAlbum`) fixes all of this at once.

Also: use `ListView.view.width`, not `parent.width`, in delegates ([#L573](BarWidget.qml#L573), [#L853](BarWidget.qml#L853)).

### 5.3 Performance & memory

| # | Finding | Impact |
| :-- | :--- | :--- |
| P1 | **Every favorites delegate loads album art into an invisible 1×1 `Image`.** `visible: false` does not stop loading, and no `sourceSize` is set, so each row downloads and decodes a full-resolution cover for nothing. This repeats as delegates are recycled during scrolling. | Network, decode CPU, texture memory |
| P2 | **No `sourceSize` on any `Image`.** The 96 px hero and the 48 px thumbnails decode Tidal art at native resolution (up to 1280²). Set `sourceSize: Qt.size(w * Screen.devicePixelRatio, …)`. | Memory |
| P3 | **All 5 `ListView`s are instantiated at all times.** Hidden lists still create delegates for their viewport, and the favorites list stays populated while viewing playlists. Use `Loader { active: view === "…" }` or a `StackLayout` with lazy children. | Delegate count, binding evaluations |
| P4 | **The playlist filter model is `playlistTracks.filter(...)` inline**, so each keystroke builds a new JS array, fully resets the model, and loses the scroll position. That's acceptable at 100 tracks but degrades on large playlists once pagination lands. Debounce it, or use a `DelegateModel` filter. | Model churn |
| P5 | **The hero art placeholder keys off the URL, not `Image.status`**, so every track change flashes the placeholder glyph until the new art loads. (The playlist delegate already does this correctly with `status !== Image.Ready`.) | Visual flicker |
| P6 | **List heights are bound to `contentHeight`**, and the panel height is bound to the column's implicit height. Lazy `contentHeight` estimates ripple into window geometry recalculation on the layer-shell popup. A fixed viewport removes this. | Relayout |
| P7 | **[Service.qml#L216-L229](Service.qml#L216-L229): while disconnected, the reconnect timer spawns two `sh` processes every 3 s, forever.** Users who installed but haven't logged in (no daemon running) incur ~40 process spawns per minute for the life of the shell. Back off exponentially, and stop polling when there is no session and no pending command. | Battery/CPU |
| P8 | **No PipeWire binding tracker.** The plugin reads and writes `Pipewire.defaultAudioSink.audio.volume/muted` without a `PwObjectTracker`. Quickshell only guarantees valid `audio` properties for bound nodes. This works today only because the first-party audio panel tracks the same sink. Add `PwObjectTracker { objects: [Pipewire.defaultAudioSink] }`. | Fragile correctness |
| P9 | **`barLabelText()` is evaluated in 3 separate bindings.** That's cheap, but turn it into a `readonly property string barLabel` for clarity and single evaluation. | Minor |

### 5.4 Service ↔ UI contract

* **Optimistic and authoritative state are mixed.** `setAudioQuality` sets `preferredAudioQuality` optimistically, and then every `status` message overwrites it. A status that arrives in between can make the selection "bounce".
* **Missing loading flags:** `explorationLoading` and a `searching` state set at debounce time.
* **Missing request correlation:** no request id for search/exploration responses (S6).
* **Missing derived quality display:** `negotiatedQuality` vs `preferredAudioQuality` is the most valuable audiophile signal and is not surfaced (ST4).
* `Service` is an `Item` (a visual type) but only hosts non-visual children. A `QtObject`/`Scope` is lighter. (Minor; this may follow host conventions.)

---

## 6. TidalIcon.qml

| # | Finding | Recommendation |
| :-- | :--- | :--- |
| I1 | **The diamonds touch tip-to-tip** (centers 8 units apart, radius 4). The real Tidal mark has visible gaps. At bar size (~14 px tall, ~3.5 px diamonds) antialiasing fuses them into a zig-zag blob. | Shrink the radius to ~3.4 units, or space the centers at ~8.8 units. Optically test at `Style.bar.iconCanvas` (16 px). |
| I2 | **`Canvas` repaints in JavaScript** on every size or color change into a raster target, with no explicit HiDPI handling and no `antialiasing`/`renderStrategy` tuning. | Use `QtQuick.Shapes` (`Shape` + four `ShapePath`s, `fillColor: root.color`). It's GPU-rendered, resolution-independent, and has no JS paint path. Alternatively, ship an SVG and render it with `Image` + `MultiEffect` colorization. |
| I3 | **`implicitWidth = 1.5 × iconSize`** makes the bar slot wider than sibling glyph icons. | Size the icon against `Style.bar.iconCanvas` and center it in `Style.bar.iconSlot` for parity with other widgets. |
| I4 | **No `onIconSizeChanged` repaint.** It currently works indirectly through width/height. | Fine. Note it if the icon is ever given explicit dimensions. |

---

## 7. UI_PROPOSAL.md review

### 7.1 Strengths

The philosophy is sharp and correct for this audience. The three pillars (no promotional clutter, search-first, compact hero) are right. The keyboard map is ambitious in a good way, and the backend extensions (categorized search, queue IPC) are the correct enablers.

### 7.2 Internal contradictions and errors

| # | Issue | Location |
| :-- | :--- | :--- |
| D1 | **It still uses the Spotify glyph `󰓇` (U+F04C7, `nf-md-spotify`)** in three wireframes, after commit `a6e41b8` removed it from the code. | [UI_PROPOSAL.md#L21](UI_PROPOSAL.md#L21), [#L75](UI_PROPOSAL.md#L75), [#L110](UI_PROPOSAL.md#L110) |
| D2 | **Queue shortcuts contradict each other:** Shift+Enter = "appends to the queue" (§2), Ctrl+Enter = "Queue next" (§3.2), and the table says Shift+Enter = queue next, Ctrl+Enter = append. | [#L55](UI_PROPOSAL.md#L55), [#L117](UI_PROPOSAL.md#L117), [#L159-L160](UI_PROPOSAL.md#L159-L160) |
| D3 | **Volume step:** ±2 % per notch (§3.1) vs ±5 % (§4). | [#L86](UI_PROPOSAL.md#L86), [#L167](UI_PROPOSAL.md#L167) |
| D4 | **"Starting to type immediately focuses search"** conflicts with the bare `s`/`r`/`f`/`1-3` shortcuts. | [#L48](UI_PROPOSAL.md#L48), [#L163-L166](UI_PROPOSAL.md#L163-L166) |
| D5 | **Tab/Shift+Tab is proposed for result navigation**, but Omarchy reserves Tab for panel switching (`PanelKeyCatcher.tabRequested`). | [#L115](UI_PROPOSAL.md#L115) |
| D6 | **Tab sets differ:** `[Library] [Queue] [Playlists]` (§1) vs `[Queue] [Favorites] [Playlists]` (§3.3). | [#L27](UI_PROPOSAL.md#L27), [#L126](UI_PROPOSAL.md#L126) |
| D7 | **The 4-column grid (#, Title, Artist, Album, Time) is drawn ~72 characters wide**, but the panel is `Style.space(380)` ≈ 45 monospace characters at 12 px. As drawn, the grid doesn't fit. Either widen to ~560 px or use a two-line row (title / artist • album) with a duration column, which is what the implementation does. | §3.3 |
| D8 | **Prefixes `a:` and `art:` are ambiguous** while typing (`a` is a prefix of `art`). Use `al:`/`ar:`, or the sigils `@artist` and `#album`. | [#L49-L54](UI_PROPOSAL.md#L49-L54) |
| D9 | **The "24-bit / 192 kHz" HUD needs stream sample-rate/bit-depth data** that the IPC doesn't expose (mpv `audio-params` could provide it). It is listed as a feature without a backend task. | §2 Pillar 3 |
| D10 | **The §6 "Current UI" column is stale** (it claims there is no volume slider and no queue tab; both now exist in some form). | [#L200-L209](UI_PROPOSAL.md#L200-L209) |
| D11 | **The document is untracked in git** (`?? UI_PROPOSAL.md`). | `git status` |

### 7.3 Implementation vs. proposal gap

| Proposal item | Status |
| :--- | :---: |
| Ultra-compact header (no brand row) | ❌ Brand row present |
| Transport `Shuffle · Prev · Play · Next · Repeat` | ❌ Misordered |
| Integrated volume + wheel | ✅ (issues T5–T8) |
| Omnibox, categorized results, prefixes | ❌ Tracks only |
| Type-to-search | ❌ |
| Esc clears, then returns | ❌ Broken (§3.1) |
| Real Queue tab with counts | ❌ Shows favorites, no counts |
| Dense 36 px rows | ◐ Favorites only |
| Row hover actions (play / ♥ / +queue) | ❌ |
| Up/Down + Enter navigation | ◐ Favorites/search only, unbounded |
| j/k, Shift/Ctrl+Enter | ❌ |
| Component decomposition (§5.1) | ❌ |

---

## 8. Recommended target layout

This keeps the 380 px panel width, removes brand chrome, and puts tabs next to the list:

```
┌────────────────────────────────────────────┐
│ ┌────┐ So What                    LOSSLESS │  ← heading/bold, quality pill
│ │ART │ Miles Davis · Kind of Blue   ♡   󰒓  │  ← muted links, heart, settings
│ └────┘                                     │    (art 56–64 px, not 96)
│ 2:45 ━━━━━━━━━━━━━━●──────────────── 9:22  │  ← times inline with scrubber
│   󰒝    󰒮    ( 󰏤 )    󰒭    󰑖     󰕾 ━━━●── │  ← symmetric transport + volume
├────────────────────────────────────────────┤
│ 󰍉 Search Tidal…                       /    │  ← type-to-search
│ Favorites 312   Playlists 28   (Queue 14)  │  ← tabs directly above list
│ ▸ So What           Miles Davis       9:22 │  ← fixed-height viewport,
│   Freddie Freeloader Miles Davis      9:49 │    TrackRow delegate
│   …                                        │
└────────────────────────────────────────────┘
Idle: hero collapses to a single muted line ("Nothing playing · / to search").
Search active: tabs hide; list shows results; Esc restores the previous tab.
```

---

## 9. Prioritized remediation plan

### P0: Correctness (this week)

1. Escape back-stack via semantic `PanelKeyCatcher` signals; drop the duplicate `Keys.onPressed` (§3.1).
2. Seek on `released`, preview while dragging, disable the scrubber wheel, relative keyboard seek (SK1–SK4).
3. Make search an exclusive view; stop calling `clearSearch()` on each keystroke; add request ids (S1–S3, S6).
4. Clamp `keyboardIndex`; make Enter act on the *active* list (K1, K2).
5. Fix the search-row art overflow; delete the hidden favorites image and the dead quality rectangle (P1, §5.2).
6. Remove the stale `build.sh` banner and its broken string comparison (§2.6).

### P1: Hierarchy & ergonomics

7. Remove the brand row; reorder the transport; use Nerd Font glyphs and `PanelActionButton` (V1, T1–T3).
8. Move the tabs above the list; remove the section header; hide the Queue tab until IPC exists (V2, V4, S7).
9. Turn the artist/album chips into hover-underlined text links (V3, S8).
10. Replace the volume `Slider` with `PanelSlider` (fill, right-click mute, 100 % tick); accumulate wheel deltas (T4–T8).
11. Cap the bar label width; add an icon-only option; align click/wheel grammar with `services/media` (B1–B5).
12. Turn settings into an overlay: `ButtonGroup` for quality, `ConfirmDialog` for logout, a diagnostics section (ST1–ST4).

### P2: System hygiene

13. Extract `TrackRow.qml`, `NowPlayingCard.qml`, `LibraryView.qml`; add a derived `view` state with `Loader`/`StackLayout` (§5.1–5.3).
14. Token pass: `Style.spacing.*`, `Style.*Fill`, `Style.font.family`, `Text.PlainText`, `Style.normalBorderWidth` (§4).
15. Add `PwObjectTracker`; reconnect backoff; `sourceSize` on images (P2, P7, P8).
16. Rebuild `TidalIcon` with `Shape` and visible diamond gaps (§6).
17. Add `Accessible.name`/`role` to all interactive elements (K9).
18. Reconcile `UI_PROPOSAL.md` contradictions (D1–D10) and commit it.

### Suggested regression tests (extend `tests/qml/shell.qml`)

* Escape with search text → panel stays open, text is empty; a second Escape → panel closes.
* Playlists tab + search text → `playlistsList.visible === false`.
* Drag simulation on the scrubber → exactly one `seek` command reaches the fake daemon.
* Pressing `Down` 50× on a 3-item list → `keyboardIndex === 2`.
* Favorites delegate contains no `Image` when the art tile is not shown.

---

## Appendix A: Verification artefacts

* Key-dispatch experiment: `qmltestrunner` (Qt 6.11.2, offscreen) using a copy of `/usr/share/omarchy/shell/Ui/PanelKeyCatcher.qml`. Scratch files are stored outside the repository in the agent scratch directory (`…/brain/<conversation>/scratch/keytest/`). No repository files were modified other than adding this report.
* Static metrics from `BarWidget.qml`: 11 × `Qt.rgba`, 88 × `Style.space(`, 0 × `Style.spacing.`, 50 × `Text {`, 2 × `font.family`, 0 × `PlainText`, 5 × `ListView`, 25 × `Ui.Button`, 0 × `Accessible`.
