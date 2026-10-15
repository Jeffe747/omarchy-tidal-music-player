import QtQuick
import Quickshell
import Quickshell.Io

Item {
  id: root

  property var shell: null
  readonly property string channel: Quickshell.env("TIDAL_CHANNEL") || "stable"
  readonly property string socketPath: Quickshell.env("XDG_RUNTIME_DIR") + "/tidal.sock"
  readonly property string sessionPath: Quickshell.env("TIDAL_SESSION_PATH") || (Quickshell.env("HOME") + "/.local/state/tidal/session.json")
  readonly property string bundledBinaryPath: Qt.resolvedUrl("bin/tidal-daemon").toString().replace(/^file:\/\//, "")
  readonly property string developmentBinaryPath: Qt.resolvedUrl("backend/target/release/tidal-daemon").toString().replace(/^file:\/\//, "")
  readonly property string stateBinaryPath: Quickshell.env("HOME") + "/.local/state/omarchy/tidal/bin/tidal-daemon"
  property string selectedBinaryPath: ""
  property bool bootstrapping: false
  property string bootstrapStatus: "Setting up Tidal engine…"
  readonly property string binaryPath: selectedBinaryPath
  readonly property string buildScriptMessage: "Tidal daemon missing or not executable. Reinstall the plugin to restore bin/tidal-daemon, or run scripts/build.sh in a development checkout."

  // Reactive state properties exposed to BarWidget & Omarchy UI
  property bool authenticated: false
  property string authUrl: ""
  property string authCode: ""
  property bool authPending: false
  property string authError: ""

  property bool daemonBinaryExists: false
  property bool savedSessionExists: false

  property bool isPlaying: false
  property string trackTitle: ""
  property string trackArtist: ""
  property string trackAlbum: ""
  property string trackArtUrl: ""
  property real trackDuration: 0.0
  property real trackPosition: 0.0
  property string audioQuality: "LOSSLESS" // "HI_RES_LOSSLESS", "LOSSLESS", "HIGH"
  property var favorites: []
  property bool favoritesLoading: false
  property string favoritesError: ""
  property string playbackError: ""
  property var currentTrackId: null
  property bool favoritesRequested: false
  property var playlists: []
  property bool playlistsLoading: false
  property string playlistsError: ""
  property string currentPlaylistId: ""
  property var playlistTracks: []
  property bool playlistTracksLoading: false
  property string playlistTracksError: ""
  property string preferredAudioQuality: "LOSSLESS"
  property bool shuffle: false
  property string repeatMode: "off"
  property bool currentTrackFavorite: false
  property var currentArtistId: null
  property var currentAlbumId: null
  property var explorationTracks: []
  property string explorationView: ""
  property string explorationTitle: ""
  property string explorationError: ""
  property int explorationSearchRequestId: -1

  onAuthenticatedChanged: {
    if (authenticated) {
      if (!favoritesRequested) loadFavorites()
      loadPlaylists()
    } else {
      favorites = []
      playlists = []
      playlistTracks = []
      currentPlaylistId = ""
      favoritesLoading = false
      favoritesRequested = false
      favoritesError = ""
      playbackError = ""
      currentTrackId = null
    }
  }

  property var searchResults: []
  property bool searching: false
  property string searchError: ""
  property int searchRequestId: 0
  property bool searchCompleted: false
  property bool explorationLoading: false
  property var pendingCommands: []
  property int reconnectDelay: 3000
  readonly property var daemonSocket: socketLoader.item

  // Check daemon binary existence
  Process {
    id: binaryCheckProc
    command: ["sh", "-c",
      'sys_bin="$(command -v tidal-daemon 2>/dev/null || true)"; if [ "$4" = dev ]; then path="$3"; if test -f "$path" && test -x "$path"; then printf "%s\\n" "$path"; exit 0; fi; else for path in "$1" "$3" "$sys_bin" "/usr/bin/tidal-daemon" "$2"; do if test -n "$path" && test -f "$path" && test -x "$path"; then printf "%s\\n" "$path"; exit 0; fi; done; fi; exit 1',
      "tidal-binary-check", root.bundledBinaryPath, root.developmentBinaryPath, root.stateBinaryPath, root.channel]
    stdout: StdioCollector { id: binaryCheckOutput }
    running: false
    onExited: function(exitCode) {
      root.daemonBinaryExists = (exitCode === 0)
      if (root.daemonBinaryExists) root.bootstrapping = false
      if (root.daemonBinaryExists) root.selectedBinaryPath = binaryCheckOutput.text.trim()
      if (exitCode !== 0) {
        root.beginBootstrap()
      } else if (root.authError === root.buildScriptMessage || root.authError.indexOf("Tidal daemon missing.") === 0) {
        root.authError = ""
      }
      if (root.daemonBinaryExists && root.pendingCommands.length > 0) {
        root.ensureDaemonRunning()
      }
      if (root.daemonBinaryExists) root.checkSavedSession()
    }
  }

  function checkDaemonBinary() {
    if (!binaryCheckProc.running) {
      binaryCheckProc.running = true
    }
  }

  function beginBootstrap() {
    if (bootstrapProc.running || root.bootstrapping) return
    root.bootstrapping = true
    root.bootstrapStatus = "Setting up Tidal engine…"
    bootstrapProc.running = true
  }

  function stopDaemon() {
    if (daemonProcess.running) daemonProcess.running = false
  }

  function updateEngine() {
    if (bootstrapProc.running || root.bootstrapping) return
    stopDaemon()
    root.daemonBinaryExists = false
    root.beginBootstrap()
  }

  Process {
    id: bootstrapProc
    command: ["sh", "-c", 'set -eu; arch="$(uname -m)"; case "$arch" in x86_64|amd64) asset=tidal-daemon-x86_64; expected=d2667dc91bd99ab44e87001974e9552ceee8a24dd8c985478afba5b44ddc67ec ;; aarch64|arm64) asset=tidal-daemon-aarch64; expected=151eece893151952e94bab953d9ea0bfaa18d8d717c400c382210011982911cf ;; *) echo "Unsupported architecture: $arch" >&2; exit 1 ;; esac; channel="${2:-stable}"; tag="v1.0.0"; check_mode="hardcoded"; if [ "$channel" = "dev" ]; then tag="dev"; check_mode="remote"; fi; base_url="https://github.com/Jeffe747/omarchy-tidal-music-player/releases/download/${tag}"; if [ "$check_mode" = "remote" ]; then expected="$(curl -fsSL "${base_url}/SHA256SUMS" | awk -v a="$asset" \'$2 == a { print $1 }\')"; fi; dest="$1"; mkdir -p "$(dirname "$dest")"; tmp="${dest}.tmp.$$"; if command -v curl >/dev/null 2>&1; then curl -fL --retry 2 "${base_url}/${asset}" -o "$tmp" || { rm -f "$tmp"; exit 1; }; elif command -v wget >/dev/null 2>&1; then wget -O "$tmp" "${base_url}/${asset}" || { rm -f "$tmp"; exit 1; }; else echo "curl or wget is required" >&2; exit 1; fi; actual="$(sha256sum "$tmp" 2>/dev/null || shasum -a 256 "$tmp")"; actual="${actual%% *}"; if [ -z "$expected" ] || [ "$actual" != "$expected" ]; then echo "Checksum verification failed: expected $expected, got $actual" >&2; rm -f "$tmp"; exit 2; fi; chmod +x "$tmp"; mv -f "$tmp" "$dest"', "tidal-bootstrap", root.stateBinaryPath, root.channel]
    running: false
    onExited: function(exitCode) {
      root.bootstrapping = false
      if (exitCode === 0) {
        root.selectedBinaryPath = root.stateBinaryPath
        root.daemonBinaryExists = true
        root.authError = ""
        root.checkSavedSession()
        root.ensureDaemonRunning()
      } else {
        root.authError = "Tidal daemon missing. Failed to bootstrap release binary or find tidal-daemon on PATH."
        root.pendingCommands = []
        root.authPending = false
      }
    }
  }

  Process {
    id: sessionCheckProc
    command: ["sh", "-c", 'test -s "$1" || test -s "$2"', "tidal-session-check", root.sessionPath, Quickshell.env("HOME") + "/.local/state/omarchy/tidal/session.json"]
    running: false
    onExited: function(exitCode) {
      root.savedSessionExists = (exitCode === 0)
      if (root.savedSessionExists && root.daemonBinaryExists) {
        root.ensureDaemonRunning()
        if (root.daemonSocket && root.daemonSocket.connected) {
          root.sendCommand({ "command": "get_status" })
        }
      }
    }
  }

  function checkSavedSession() {
    if (!sessionCheckProc.running) sessionCheckProc.running = true
  }

  function initializeForWidget() {
    checkDaemonBinary()
    checkSavedSession()
  }

  // Start background daemon process if needed
  Process {
    id: daemonProcess
    command: [root.binaryPath]
    running: false
    onExited: function(exitCode) {
      if (exitCode !== 0) {
        root.authError = "Tidal daemon exited with code " + exitCode
        root.pendingCommands = []
        root.authPending = false
        root.searching = false
        root.favoritesLoading = false
        root.favoritesError = root.authError
        root.playlistsLoading = false
        root.playlistTracksLoading = false
        console.warn("Tidal Service:", root.authError)
      }
    }
  }

  // Socket communication with daemon
  Loader {
    id: socketLoader
    sourceComponent: socketComponent
  }

  Component {
    id: socketComponent

    Socket {
      path: root.socketPath
      connected: true

      onError: {
        if (root.authenticated) {
          root.playbackError = "Disconnected from Tidal daemon; reconnecting..."
          root.isPlaying = false
        }
        root.favoritesLoading = false
        root.favoritesRequested = false
        root.playlistsLoading = false
        root.playlistTracksLoading = false
        if (root.pendingCommands.length > 0) {
          root.authError = "Unable to connect to Tidal daemon; retrying..."
        }
        // Quickshell 0.3.1 retains failed sockets, so retries need a new instance.
        Qt.callLater(function() { socketLoader.active = false })
      }

      onConnectionStateChanged: {
        if (connected) {
          root.reconnectDelay = 3000
          if (root.playbackError === "Disconnected from Tidal daemon; reconnecting...") {
            root.playbackError = ""
          }
          if (root.authError === "Unable to connect to Tidal daemon; retrying...") {
            root.authError = ""
          }
          // Loader.item is not published until construction has finished.
          Qt.callLater(root.flushCommands)
        }
      }

      parser: SplitParser {
        onRead: function(line) {
          try {
            var msg = JSON.parse(line)
            root.handleDaemonMessage(msg)
          } catch (e) {
            console.warn("Tidal Service: Failed to parse message:", line)
          }
        }
      }
    }
  }

  Timer {
    id: reconnectTimer
    interval: root.reconnectDelay
    repeat: true
    running: (!root.daemonSocket || !root.daemonSocket.connected)
             && (root.savedSessionExists || root.pendingCommands.length > 0 || root.authenticated || root.authPending)
    onTriggered: {
      root.reconnectDelay = Math.min(30000, root.reconnectDelay * 2)
      checkDaemonBinary()
      checkSavedSession()
      if (daemonBinaryExists) {
        if (!socketLoader.active) socketLoader.active = true
        else if (root.daemonSocket) root.daemonSocket.connected = true
      }
    }
  }

  Component.onCompleted: {
    checkDaemonBinary()
  }

  function ensureDaemonRunning() {
    if (!daemonBinaryExists) {
      root.authError = root.buildScriptMessage
      return
    }
    if ((!daemonSocket || !daemonSocket.connected) && !daemonProcess.running) {
      daemonProcess.running = true
    }
  }

  function sendCommand(obj) {
    if (daemonSocket && daemonSocket.connected) {
      daemonSocket.write(JSON.stringify(obj) + "\n")
      daemonSocket.flush()
    } else {
      root.pendingCommands = root.pendingCommands.concat([obj])
      if (daemonBinaryExists) {
        ensureDaemonRunning()
      } else {
        checkDaemonBinary()
      }
    }
  }

  function flushCommands() {
    if (!daemonSocket || !daemonSocket.connected) return
    sendCommand({ "command": "get_status" })
    var commands = root.pendingCommands
    root.pendingCommands = []
    for (var i = 0; i < commands.length; i++) {
      sendCommand(commands[i])
    }
  }

  function handleDaemonMessage(msg) {
    if (!msg || typeof msg !== "object") return

    if (msg.type === "status" || msg.type === "state_change") {
      root.authenticated = msg.authenticated === true
      if (root.authenticated) {
        root.authPending = false
        root.authCode = ""
        root.authUrl = ""
        root.authError = ""
      }
      root.isPlaying = msg.is_playing === true
      root.currentTrackId = msg.track_id !== undefined ? msg.track_id : null
      root.trackTitle = msg.track_title || ""
      root.trackArtist = msg.track_artist || ""
      root.trackAlbum = msg.track_album || ""
      root.trackArtUrl = msg.track_art_url || ""
      root.trackDuration = msg.duration || 0.0
      if (msg.position !== undefined && msg.position !== null) {
        root.trackPosition = Math.max(0, Number(msg.position) || 0)
      }
      root.audioQuality = msg.audio_quality || "LOSSLESS"
      root.preferredAudioQuality = msg.preferred_audio_quality || root.preferredAudioQuality || "LOSSLESS"
      root.shuffle = msg.shuffle === true
      root.repeatMode = msg.repeat_mode || "off"
      root.currentTrackFavorite = msg.is_favorite === true
      root.currentArtistId = msg.artist_id !== undefined ? msg.artist_id : null
      root.currentAlbumId = msg.album_id !== undefined ? msg.album_id : null
    } else if (msg.type === "auth_code") {
      root.authPending = true
      root.authUrl = msg.verification_uri || "https://link.tidal.com"
      root.authCode = msg.user_code || ""
      root.authError = ""
    } else if (msg.type === "auth_success") {
      root.authPending = false
      root.authenticated = true
      root.authCode = ""
      root.authUrl = ""
      root.authError = ""
      if (!root.favoritesRequested) root.loadFavorites()
      sendCommand({ "command": "get_status" })
    } else if (msg.type === "auth_expired") {
      root.authPending = false
      root.authError = msg.error || "Device code expired"
    } else if (msg.type === "auth_error") {
      root.authPending = false
      root.authError = msg.error || "Authentication error"
    } else if (msg.type === "search_results") {
      if (msg.request_id !== root.searchRequestId) return
      root.searching = false
      root.searchCompleted = true
      root.searchError = ""
      root.searchResults = msg.results || []
      if (msg.request_id === root.explorationSearchRequestId && root.explorationView === "artist") {
        root.explorationLoading = false
        root.explorationTracks = root.searchResults
        root.explorationError = ""
      }
    } else if (msg.type === "search_error") {
      if (msg.request_id !== root.searchRequestId) return
      root.searching = false
      root.searchCompleted = true
      root.searchError = msg.error || "Search failed"
      root.searchResults = []
      if (msg.request_id === root.explorationSearchRequestId && root.explorationView === "artist") {
        root.explorationLoading = false
        root.explorationError = root.searchError
      }
    } else if (msg.type === "favorites_loaded") {
      root.favoritesLoading = false
      root.favoritesError = ""
      root.favorites = msg.tracks || []
    } else if (msg.type === "favorites_error") {
      root.favoritesLoading = false
      root.favoritesError = msg.error || "Unable to load favorites"
    } else if (msg.type === "playlists_loaded") {
      root.playlistsLoading = false
      root.playlistsError = ""
      root.playlists = msg.playlists || []
    } else if (msg.type === "playlists_error") {
      root.playlistsLoading = false
      root.playlistsError = msg.error || "Unable to load playlists"
    } else if (msg.type === "playlist_tracks_loaded") {
      root.playlistTracksLoading = false
      root.playlistTracksError = ""
      root.currentPlaylistId = msg.playlist_id || root.currentPlaylistId
      root.playlistTracks = msg.tracks || []
    } else if (msg.type === "playlist_tracks_error") {
      root.playlistTracksLoading = false
      root.playlistTracksError = msg.error || "Unable to load playlist tracks"
    } else if (msg.type === "exploration_loaded") {
      if (root.explorationView !== msg.view) return
      root.explorationLoading = false
      root.explorationTracks = msg.tracks || []
      root.explorationView = msg.view || ""
      root.explorationError = ""
      if (!root.explorationTitle && root.explorationTracks.length > 0)
        root.explorationTitle = (root.explorationView === "artist" ? root.explorationTracks[0].artist : root.explorationTracks[0].album) || ""
    } else if (msg.type === "exploration_error") {
      if (root.explorationView !== msg.view) return
      root.explorationLoading = false
      var error = msg.error || "Unable to load tracks"
      root.explorationError = msg.view === "radio" && (error.indexOf("404") !== -1 || error.toLowerCase().indexOf("unavailable") !== -1)
        ? "Song radio is not available for this track." : error
    } else if (msg.type === "playback_started") {
      root.currentTrackId = msg.track_id
      root.playbackError = ""
    } else if (msg.type === "position_changed") {
      root.trackPosition = Math.max(0, Number(msg.position) || 0)
      if (msg.duration !== undefined) root.trackDuration = Math.max(0, Number(msg.duration) || 0)
    } else if (msg.type === "playback_error") {
      root.playbackError = msg.error || "Unable to play track"
    }
    if (root.authenticated && !root.favoritesRequested) root.loadFavorites()
  }

  // Public control APIs
  function startAuth() {
    sendCommand({ "command": "start_auth" })
  }

  function logout() {
    sendCommand({ "command": "logout" })
  }

  function play() {
    sendCommand({ "command": "play" })
  }

  function pause() {
    sendCommand({ "command": "pause" })
  }

  function togglePlay() {
    sendCommand({ "command": "toggle_play" })
  }

  function next() {
    sendCommand({ "command": "next" })
  }

  function previous() {
    sendCommand({ "command": "previous" })
  }

  function seek(seconds) {
    sendCommand({ "command": "seek", "position": seconds })
  }

  function search(query) {
    root.searching = true
    root.searchError = ""
    root.searchCompleted = false
    root.searchResults = []
    root.searchRequestId++
    sendCommand({ "command": "search", "query": query, "request_id": root.searchRequestId })
  }

  function prepareSearch() {
    root.searching = true
    root.searchCompleted = false
    root.searchError = ""
    root.searchResults = []
    root.searchRequestId++
  }

  function clearSearch() {
    root.searchRequestId++
    root.searchResults = []
    root.searchError = ""
    root.searching = false
    root.searchCompleted = false
  }

  function playTrack(trackId) {
    root.playbackError = ""
    sendCommand({ "command": "play_track", "track_id": trackId })
  }

  function loadFavorites() {
    if (root.favoritesLoading) return
    root.favoritesRequested = true
    root.favoritesLoading = true
    root.favoritesError = ""
    sendCommand({ "command": "get_favorites" })
  }

  function loadPlaylists() {
    if (root.playlistsLoading) return
    root.playlistsLoading = true
    root.playlistsError = ""
    sendCommand({ "command": "get_playlists" })
  }

  function createPlaylist(title, description) {
    if (!title || !title.trim()) return
    root.playlistsLoading = true
    root.playlistsError = ""
    sendCommand({ "command": "create_playlist", "title": title.trim(), "description": description || "" })
  }

  function loadPlaylistTracks(playlistId) {
    root.currentPlaylistId = playlistId
    root.playlistTracks = []
    root.playlistTracksLoading = true
    root.playlistTracksError = ""
    sendCommand({ "command": "get_playlist_tracks", "playlist_id": playlistId })
  }

  function closePlaylist() {
    root.currentPlaylistId = ""
    root.playlistTracks = []
    root.playlistTracksLoading = false
    root.playlistTracksError = ""
  }

  function setAudioQuality(quality) {
    root.preferredAudioQuality = quality
    sendCommand({ "command": "set_audio_quality", "quality": quality })
  }

  function toggleShuffle() { sendCommand({ "command": "toggle_shuffle" }) }
  function cycleRepeat() { sendCommand({ "command": "cycle_repeat" }) }
  function toggleFavorite() { sendCommand({ "command": "toggle_favorite" }) }
  function addTrackToPlaylist(playlistId, trackId) { sendCommand({ "command": "add_playlist_track", "playlist_id": playlistId, "track_id": trackId }) }
  function toggleTrackFavorite(trackId) { sendCommand({ "command": "toggle_favorite", "track_id": trackId }) }
  function isTrackFavorite(trackId) {
    for (var i = 0; i < root.favorites.length; i++) if (root.favorites[i].id === trackId) return true
    return false
  }
  function closeExploration() {
    if (explorationSearchRequestId >= 0) clearSearch()
    explorationSearchRequestId = -1
    explorationView = ""
    explorationTitle = ""
    explorationTracks = []
    explorationLoading = false
    explorationError = ""
  }
  function exploreAlbum(id, title) {
    explorationSearchRequestId = -1
    explorationTitle = title || ""
    explorationView = "album"
    explorationTracks = []
    explorationLoading = true
    explorationError = ""
    sendCommand({ "command": "get_album_tracks", "album_id": id })
  }
  function exploreArtist(id, name) {
    explorationTitle = name || ""
    explorationView = "artist"
    explorationTracks = []
    explorationLoading = true
    explorationError = ""
    explorationSearchRequestId = -1
    if (!id && name) {
      search(name)
      explorationSearchRequestId = searchRequestId
    } else {
      sendCommand({ "command": "get_artist_tracks", "artist_id": id })
    }
  }
  function loadTrackRadio(trackId, trackTitle) {
    clearSearch()
    explorationView = "radio"
    explorationTitle = trackTitle ? trackTitle + " Radio" : "Track Radio"
    explorationTracks = []
    explorationError = ""
    explorationLoading = true
    sendCommand({ "command": "get_track_radio", "track_id": trackId })
  }
}
