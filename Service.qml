import QtQuick
import Quickshell
import Quickshell.Io

Item {
  id: root

  property var shell: null
  readonly property string pluginVersion: "1.0.76"
  property string daemonVersion: ""
  readonly property string socketPath: Quickshell.env("XDG_RUNTIME_DIR") + "/tidal.sock"
  readonly property string sessionPath: Quickshell.env("TIDAL_SESSION_PATH") || (Quickshell.env("HOME") + "/.local/state/tidal/session.json")
  readonly property string bundledBinaryPath: Qt.resolvedUrl("bin/tidal-daemon").toString().replace(/^file:\/\//, "")
  readonly property string developmentBinaryPath: Qt.resolvedUrl("backend/target/release/tidal-daemon").toString().replace(/^file:\/\//, "")
  readonly property string stateBinaryPath: Quickshell.env("HOME") + "/.local/state/omarchy/tidal/bin/tidal-daemon"
  readonly property string uiSettingsPath: Quickshell.env("HOME") + "/.local/state/omarchy/tidal/ui_settings.json"
  property string playerStyle: "standard"
  property bool libraryOpen: true
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
  property bool trackLoading: false
  property string trackTitle: ""
  property string trackArtist: ""
  property string trackAlbum: ""
  property string trackArtUrl: ""
  property real trackDuration: 0.0
  property real trackPosition: 0.0
  property string audioQuality: "LOSSLESS" // "HI_RES_LOSSLESS", "LOSSLESS", "HIGH"
  property var favorites: []
  property int favoritesTotal: 0
  property bool favoritesLoading: false
  property bool favoritesInProgress: false
  property var favoritesMap: ({})
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

  // Chromecast streaming state
  property bool castActive: false
  property string castDeviceName: ""
  property string castState: "idle" // "idle", "connecting", "playing", "buffering", "paused"
  property real castVolume: 1.0
  property real lastVolumeSetTime: 0
  property var castDevices: []
  property bool castDiscovering: false
  property string castError: ""
  property bool castFirewallBlocked: false

  onAuthenticatedChanged: {
    if (authenticated) {
      if (!favoritesRequested) loadFavorites()
      loadPlaylists()
    } else {
      favorites = []
      favoritesTotal = 0
      favoritesInProgress = false
      favoritesMap = ({})
      playlists = []
      playlistTracks = []
      currentPlaylistId = ""
      favoritesLoading = false
      favoritesRequested = false
      favoritesError = ""
      playbackError = ""
      currentTrackId = null
      trackLoading = false
      castActive = false
      castDeviceName = ""
      castState = "idle"
      castVolume = 1.0
      lastVolumeSetTime = 0
      castDevices = []
      castDiscoverTimeout.stop()
      castDiscovering = false
      castError = ""
    }
  }

  property var searchResults: []
  property bool searching: false
  property string searchError: ""
  property int searchRequestId: 0
  property bool searchCompleted: false
  property bool explorationLoading: false
  property var artworkCache: ({})
  property var artworkRequested: ({})
  property var pendingCommands: []
  property int reconnectDelay: 3000
  readonly property var daemonSocket: socketLoader.item

  function sanitizeLocalArtUrl(url) {
    if (url && typeof url === "string" && url.indexOf("file://") === 0) return url
    return ""
  }

  function requestArtwork(cover) {
    if (!cover || typeof cover !== "string") return
    if (root.artworkCache[cover] || root.artworkRequested[cover]) return
    var req = Object.assign({}, root.artworkRequested)
    req[cover] = true
    root.artworkRequested = req
    sendCommand({ "command": "fetch_artwork", "cover": cover })
  }

  // Check daemon binary existence
  Process {
    id: binaryCheckProc
    command: ["sh", "-c",
      'if [ -n "${TIDAL_TEST_MODE:-}" ]; then for path in "$1" "$2"; do if test -n "$path" && test -f "$path" && test -x "$path"; then printf "%s\\n" "$path"; exit 0; fi; done; exit 1; fi; arch="$(uname -m)"; case "$arch" in x86_64|amd64) expected="eb0870a33c917673f2a6d61e55f3fd8a3abb88959482aa3f06e3f14ab0b3acc1" ;; aarch64|arm64) expected="ebe23945a290ee24e7fc2d24b7fd50bc031a0a6a3c4929db76a729cf4843663d" ;; *) expected="" ;; esac; for path in "$1" "$3"; do if test -n "$path" && test -f "$path" && test -x "$path"; then actual="$(sha256sum "$path" 2>/dev/null || shasum -a 256 "$path" 2>/dev/null || true)"; actual="${actual%% *}"; if [ -n "$expected" ] && [ "$actual" = "$expected" ]; then printf "%s\\n" "$path"; exit 0; fi; fi; done; exit 1',
      "tidal-binary-check", root.bundledBinaryPath, root.developmentBinaryPath, root.stateBinaryPath]
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

  Process {
    id: bootstrapProc
    command: ["sh", "-c", 'set -eu; arch="$(uname -m)"; case "$arch" in x86_64|amd64) asset=tidal-daemon-x86_64; expected=eb0870a33c917673f2a6d61e55f3fd8a3abb88959482aa3f06e3f14ab0b3acc1 ;; aarch64|arm64) asset=tidal-daemon-aarch64; expected=ebe23945a290ee24e7fc2d24b7fd50bc031a0a6a3c4929db76a729cf4843663d ;; *) echo "Unsupported architecture: $arch" >&2; exit 1 ;; esac; tag="v1.0.76"; base_url="https://github.com/Jeffe747/omarchy-tidal-music-player/releases/download/${tag}"; dest="$1"; mkdir -p "$(dirname "$dest")"; tmp="${dest}.tmp.$$"; if command -v curl >/dev/null 2>&1; then curl -fL --retry 2 "${base_url}/${asset}" -o "$tmp" || { rm -f "$tmp"; exit 1; }; elif command -v wget >/dev/null 2>&1; then wget -O "$tmp" "${base_url}/${asset}" || { rm -f "$tmp"; exit 1; }; else echo "curl or wget is required" >&2; exit 1; fi; actual="$(sha256sum "$tmp" 2>/dev/null || shasum -a 256 "$tmp")"; actual="${actual%% *}"; if [ -z "$expected" ] || [ "$actual" != "$expected" ]; then echo "Checksum verification failed: expected $expected, got $actual" >&2; rm -f "$tmp"; exit 2; fi; chmod +x "$tmp"; mv -f "$tmp" "$dest"', "tidal-bootstrap", root.stateBinaryPath]
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
    if (root.daemonSocket && root.daemonSocket.connected && root.authenticated) return
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
        playlistsWatchdog.stop()
        root.playlistsLoading = false
        root.playlistTracksLoading = false
        root.castDiscovering = false
        root.castActive = false
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
        playlistsWatchdog.stop()
        root.playlistsLoading = false
        root.playlistTracksLoading = false
        root.castDiscovering = false
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

  Timer {
    id: castDiscoverTimeout
    interval: 6000
    repeat: false
    running: false
    onTriggered: {
      root.castDiscovering = false
    }
  }

  Timer {
    id: playlistsWatchdog
    interval: 8000
    repeat: false
    running: false
    onTriggered: {
      if (root.playlistsLoading) {
        root.playlistsLoading = false
        if (root.playlists.length === 0) {
          root.playlistsError = "Playlist loading timed out"
        }
      }
    }
  }

  FileView {
    id: uiSettingsFile
    path: root.uiSettingsPath
    watchChanges: false
    atomicWrites: true
    printErrors: false
    onLoaded: function() {
      try {
        var raw = text()
        if (raw && raw.trim().length > 0) {
          var parsed = JSON.parse(raw)
          if (parsed) {
            if (parsed.player_style) {
              var valid = ["standard", "turntable", "showcase"]
              if (valid.indexOf(parsed.player_style) !== -1) {
                root.playerStyle = parsed.player_style
              }
            }
            if (parsed.library_open !== undefined) {
              root.libraryOpen = parsed.library_open === true
            }
          }
        }
      } catch (e) {
        console.warn("Tidal Service: Failed to parse ui_settings.json:", e)
      }
    }
    onLoadFailed: function() {}
  }

  Timer {
    id: uiSettingsSaveTimer
    interval: 200
    repeat: false
    onTriggered: {
      ensureStateDirProc.running = false
      ensureStateDirProc.running = true
    }
  }

  function scheduleUiSettingsSave() {
    uiSettingsSaveTimer.restart()
  }

  Process {
    id: ensureStateDirProc
    command: ["sh", "-c", 'mkdir -p "$(dirname "$1")"', "tidal-mkdir", root.uiSettingsPath]
    running: false
    onExited: function(exitCode) {
      if (exitCode === 0) {
        uiSettingsFile.setText(JSON.stringify({
          "player_style": root.playerStyle,
          "library_open": root.libraryOpen
        }, null, 2) + "\n")
      }
    }
  }

  function setPlayerStyle(style) {
    var valid = ["standard", "turntable", "showcase"]
    if (valid.indexOf(style) === -1) return
    root.playerStyle = style
    scheduleUiSettingsSave()
  }

  function setLibraryOpen(open) {
    root.libraryOpen = open === true
    scheduleUiSettingsSave()
  }

  Component.onCompleted: {
    checkDaemonBinary()
    uiSettingsFile.reload()
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
      root.daemonVersion = msg.daemon_version || ""
      if (root.authenticated) {
        root.authPending = false
        root.authCode = ""
        root.authUrl = ""
        root.authError = ""
        if (root.playlists.length === 0 && !root.playlistsLoading) {
          root.loadPlaylists()
        }
      }
      root.isPlaying = msg.is_playing === true
      if (root.trackLoading && (msg.is_playing || msg.track_id === root.currentTrackId)) {
        root.trackLoading = false
      }
      if (!root.trackLoading || msg.track_id === root.currentTrackId || msg.track_id === undefined) {
        root.currentTrackId = msg.track_id !== undefined ? msg.track_id : null
        root.trackTitle = msg.track_title || ""
        root.trackArtist = msg.track_artist || ""
        root.trackAlbum = msg.track_album || ""
        root.trackArtUrl = root.sanitizeLocalArtUrl(msg.track_art_url)
        root.trackDuration = msg.duration || 0.0
      }
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
      root.castActive = msg.cast_active === true
      root.castDeviceName = msg.cast_device_name || ""
      root.castState = msg.cast_state || "idle"
      if (msg.cast_firewall_blocked !== undefined) {
        root.castFirewallBlocked = msg.cast_firewall_blocked === true
      }
      if (msg.cast_volume !== undefined && msg.cast_volume !== null && (Date.now() - root.lastVolumeSetTime > 1200)) {
        var cv = Number(msg.cast_volume)
        if (!isNaN(cv)) root.castVolume = Math.max(0.0, Math.min(1.0, cv))
      }
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
      var results = msg.results || []
      for (var s = 0; s < results.length && s < 20; s++) {
        var r = results[s]
        if (r && r.cover && !root.sanitizeLocalArtUrl(r.art_url)) {
          root.requestArtwork(r.cover)
        }
      }
      root.searchResults = results
      if (msg.request_id === root.explorationSearchRequestId && root.explorationView === "artist") {
        root.explorationLoading = false
        root.explorationTracks = root.searchResults
        root.explorationError = ""
      }
    } else if (msg.type === "artwork_loaded") {
      var localPath = root.sanitizeLocalArtUrl(msg.path)
      if (msg.cover && localPath) {
        var nextCache = Object.assign({}, root.artworkCache)
        nextCache[msg.cover] = localPath
        root.artworkCache = nextCache
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
      root.favoritesTotal = (msg.total !== undefined && msg.total !== null) ? Number(msg.total) : root.favorites.length
      root.favoritesInProgress = msg.has_more === true
      var initialMap = {}
      for (var i = 0; i < root.favorites.length; i++) {
        initialMap[root.favorites[i].id] = true
      }
      root.favoritesMap = initialMap
    } else if (msg.type === "favorites_progress") {
      if (!root.authenticated) return;
      var moreTracks = msg.tracks || []
      if (moreTracks.length > 0) {
        root.favorites = root.favorites.concat(moreTracks)
        var updatedMap = Object.assign({}, root.favoritesMap)
        for (var j = 0; j < moreTracks.length; j++) {
          updatedMap[moreTracks[j].id] = true
        }
        root.favoritesMap = updatedMap
      }
      if (msg.total !== undefined && msg.total !== null) root.favoritesTotal = Number(msg.total)
      root.favoritesInProgress = true
    } else if (msg.type === "favorites_complete") {
      if (!root.authenticated) return;
      root.favoritesInProgress = false
      if (msg.total !== undefined && msg.total !== null) root.favoritesTotal = Number(msg.total)
    } else if (msg.type === "favorite_updated") {
      var favTrackId = msg.track_id
      var isFav = msg.is_favorite === true
      if (root.currentTrackId === favTrackId) root.currentTrackFavorite = isFav
      var mapCopy = Object.assign({}, root.favoritesMap)
      if (isFav) {
        var alreadyInList = false
        for (var k = 0; k < root.favorites.length; k++) {
          if (root.favorites[k].id === favTrackId) {
            alreadyInList = true
            break
          }
        }
        if (!alreadyInList) {
          if (msg.track) {
            root.favorites = root.favorites.concat([msg.track])
          }
          root.favoritesTotal++
        }
        mapCopy[favTrackId] = true
      } else {
        var prevLen = root.favorites.length
        root.favorites = root.favorites.filter(function(t) { return t.id !== favTrackId })
        if (mapCopy[favTrackId] || root.favorites.length < prevLen) {
          root.favoritesTotal = Math.max(0, root.favoritesTotal - 1)
        }
        delete mapCopy[favTrackId]
      }
      root.favoritesMap = mapCopy
    } else if (msg.type === "favorites_error") {
      root.favoritesLoading = false
      root.favoritesInProgress = false
      root.favoritesError = msg.error || "Unable to load favorites"
    } else if (msg.type === "playlists_loaded") {
      playlistsWatchdog.stop()
      root.playlistsLoading = false
      root.playlistsError = ""
      root.playlists = msg.playlists || []
    } else if (msg.type === "playlists_error") {
      playlistsWatchdog.stop()
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
      root.trackLoading = false
      root.playbackError = ""
    } else if (msg.type === "position_changed") {
      root.trackPosition = Math.max(0, Number(msg.position) || 0)
      if (msg.duration !== undefined) root.trackDuration = Math.max(0, Number(msg.duration) || 0)
      if (root.trackLoading && root.trackPosition > 0.5) root.trackLoading = false
    } else if (msg.type === "playback_error") {
      root.trackLoading = false
      root.playbackError = msg.error || "Unable to play track"
    } else if (msg.type === "cast_devices_discovered") {
      castDiscoverTimeout.stop()
      root.castDiscovering = false
      root.castDevices = msg.devices || []
    } else if (msg.type === "cast_error") {
      castDiscoverTimeout.stop()
      root.castDiscovering = false
      root.castError = msg.error || "Chromecast error"
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
    if (!root.isPlaying) {
      root.trackLoading = true
    }
    sendCommand({ "command": "toggle_play" })
  }

  function next() {
    root.trackLoading = true
    sendCommand({ "command": "next" })
  }

  function previous() {
    root.trackLoading = true
    sendCommand({ "command": "previous" })
  }

  function seek(seconds) {
    root.trackLoading = true
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

  function playTrack(trackId, trackData, context) {
    root.playbackError = ""
    root.trackLoading = true
    if (!trackData && root.favorites && root.favorites.length > 0) {
      for (var i = 0; i < root.favorites.length; i++) {
        if (root.favorites[i].id === trackId) {
          trackData = root.favorites[i]
          break
        }
      }
    }
    if (trackData && typeof trackData === "object") {
      root.currentTrackId = trackId
      if (trackData.title) root.trackTitle = trackData.title
      if (trackData.artist) root.trackArtist = trackData.artist
      if (trackData.album) root.trackAlbum = trackData.album
      var candidateArt = root.sanitizeLocalArtUrl(trackData.art_url)
      if (!candidateArt && trackData.cover && root.artworkCache[trackData.cover]) {
        candidateArt = root.artworkCache[trackData.cover]
      }
      root.trackArtUrl = candidateArt
      if (trackData.duration) root.trackDuration = Math.max(0, Number(trackData.duration) || 0)
      root.trackPosition = 0
    }
    var msg = { "command": "play_track", "track_id": trackId }
    if (context) msg.context = context
    sendCommand(msg)
  }

  function loadFavorites() {
    if (root.favoritesLoading) return
    root.favoritesRequested = true
    root.favoritesLoading = true
    root.favoritesError = ""
    sendCommand({ "command": "get_favorites" })
  }

  function loadPlaylists() {
    root.playlistsLoading = true
    root.playlistsError = ""
    playlistsWatchdog.restart()
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
    if (!trackId) return false
    if (root.favoritesMap && root.favoritesMap[trackId] !== undefined) {
      return root.favoritesMap[trackId] === true
    }
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

  function discoverCastDevices() {
    root.castDiscovering = true
    root.castError = ""
    castDiscoverTimeout.restart()
    sendCommand({ "command": "discover_cast_devices" })
  }

  function refreshCastDevices() {
    root.castDiscovering = true
    root.castError = ""
    castDiscoverTimeout.restart()
    sendCommand({ "command": "refresh_cast_devices" })
  }

  function connectCast(deviceId) {
    root.castError = ""
    sendCommand({ "command": "connect_cast", "device_id": deviceId })
  }

  function disconnectCast() {
    root.castError = ""
    sendCommand({ "command": "disconnect_cast" })
  }

  function setCastVolume(volume) {
    var clamped = Math.max(0.0, Math.min(1.0, Number(volume) || 0.0))
    root.lastVolumeSetTime = Date.now()
    root.castVolume = clamped
    sendCommand({ "command": "set_cast_volume", "volume": clamped })
  }
}
