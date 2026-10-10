# Tidal flyout: implemented layout and remaining work

This document describes the current `jaj.tidal` UI after the review in `UI_UX_REVIEW_OPUS.md`. The flyout is a compact controller for playback, favorites, playlists, and catalog track search. It does not present recommendations or promotional content.

## Current layout

```text
┌─────────────────────────────────────┐
│ [art] Track title          LOSSLESS ♡ ⚙ │
│       Artist · Album                 │
│ 2:45 ━━━━━━━━━●━━━━━━━━━━━━━━ 9:22   │
│    󰒝    󰒮    󰏤    󰒭    󰑖            │
│ 󰕾 ━━━━━━━━━●━━━━━━━━━━━ 78%           │
├─────────────────────────────────────┤
│ Search Tidal tracks…                │
│ Favorites 312    Playlists 28       │
│ Track title · Artist · Album   9:22  │
└─────────────────────────────────────┘
```

The hero uses 60 px artwork and collapses to “Nothing playing” when idle. The quality label reports the negotiated stream tier. Artist and album labels open exploration when IDs are available. Settings opens over the panel with the preferred quality, current stream quality, connection status, binary path, and confirmed logout.

Search is a separate mode. Entering a query hides library tabs and rows until the query is cleared. The backend currently returns **tracks only**. Favorites and Playlists are the available library tabs. Playlist details support a local title/artist filter. Queue is hidden until the daemon exposes active queue contents.

## Keyboard and pointer behavior

- `/` focuses search. Typing while the panel owns focus starts a search.
- Escape clears search, then leaves exploration or playlist details, then closes settings, then closes the panel.
- Up/Down select a row within the active list. Enter opens a playlist or plays the selected track. Space toggles playback.
- Left/Right seek five seconds relative to the latest local seek target. Dragging the scrubber previews time and sends one seek when released.
- The bar defaults to a clickable Tidal icon. Middle click toggles playback, right click skips to the next track, and the wheel changes tracks.

## Component and service boundaries

`BarWidget.qml` owns panel navigation and the active view. `NowPlayingCard.qml` owns the hero, transport, seek, and PipeWire volume controls. `LibraryView.qml` owns the library/search viewport and keyboard selection. `TrackRow.qml` renders track metadata consistently in favorites, search, playlist tracks, and exploration. `Service.qml` owns daemon IPC state and close methods for nested views. Search commands carry request IDs, so older results do not overwrite the current query.

## Follow-up backend features

Catalog search categories (albums, artists, and playlists), active queue IPC and editing, and queue insertion shortcuts need backend support before their UI can be shown accurately. Sample rate and bit depth are not available in the current playback status, so the quality badge shows the negotiated tier rather than invented technical values. An icon-only bar widget avoids duplicating Tidal metadata already exposed through MPRIS to Omarchy's media widget.
