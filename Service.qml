import QtQuick
import Quickshell
import Quickshell.Io

Item {
  id: root

  property var shell: null
  readonly property string socketPath: Quickshell.env("XDG_RUNTIME_DIR") + "/tidal.sock"
  readonly property string bundledBinaryPath: Qt.resolvedUrl("bin/tidal-daemon").toString().replace(/^file:\/\//, "")
  readonly property string developmentBinaryPath: Qt.resolvedUrl("backend/target/release/tidal-daemon").toString().replace(/^file:\/\//, "")
  property string selectedBinaryPath: bundledBinaryPath
  readonly property string binaryPath: selectedBinaryPath
  readonly property string buildScriptMessage: "Tidal daemon missing or not executable. Reinstall the plugin to restore bin/tidal-daemon, or run scripts/build.sh in a development checkout."

  // Reactive state properties exposed to BarWidget & Omarchy UI
  property bool authenticated: false
  property string authUrl: ""
  property string authCode: ""
  property bool authPending: false
  property string authError: ""

  property bool daemonBinaryExists: false

  property bool isPlaying: false
  property string trackTitle: ""
  property string trackArtist: ""
  property string trackAlbum: ""
  property string trackArtUrl: ""
  property real trackDuration: 0.0
  property real trackPosition: 0.0
  property string audioQuality: "LOSSLESS" // "HI_RES_LOSSLESS", "LOSSLESS", "HIGH"

  property var searchResults: []
  property bool searching: false
  property var pendingCommands: []
  readonly property var daemonSocket: socketLoader.item

  // Check daemon binary existence
  Process {
    id: binaryCheckProc
    command: ["sh", "-c",
      'for path do if test -f "$path" && test -x "$path"; then printf "%s\\n" "$path"; exit 0; fi; done; exit 1',
      "tidal-binary-check", root.bundledBinaryPath, root.developmentBinaryPath]
    stdout: StdioCollector { id: binaryCheckOutput }
    running: false
    onExited: function(exitCode) {
      root.daemonBinaryExists = (exitCode === 0)
      if (root.daemonBinaryExists) root.selectedBinaryPath = binaryCheckOutput.text.trim()
      if (exitCode !== 0) {
        root.authError = root.buildScriptMessage
        root.pendingCommands = []
        root.authPending = false
        root.searching = false
      } else if (root.authError === root.buildScriptMessage) {
        root.authError = ""
      }
      if (root.daemonBinaryExists && root.pendingCommands.length > 0) {
        root.ensureDaemonRunning()
      }
    }
  }

  function checkDaemonBinary() {
    if (!binaryCheckProc.running) {
      binaryCheckProc.running = true
    }
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
        if (root.pendingCommands.length > 0) {
          root.authError = "Unable to connect to Tidal daemon; retrying..."
        }
        // Quickshell 0.3.1 retains failed sockets, so retries need a new instance.
        Qt.callLater(function() { socketLoader.active = false })
      }

      onConnectionStateChanged: {
        if (connected) {
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
    interval: 3000
    repeat: true
    running: !root.daemonSocket || !root.daemonSocket.connected
    onTriggered: {
      checkDaemonBinary()
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
      root.trackTitle = msg.track_title || ""
      root.trackArtist = msg.track_artist || ""
      root.trackAlbum = msg.track_album || ""
      root.trackArtUrl = msg.track_art_url || ""
      root.trackDuration = msg.duration || 0.0
      root.trackPosition = msg.position || 0.0
      root.audioQuality = msg.audio_quality || "LOSSLESS"
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
      sendCommand({ "command": "get_status" })
    } else if (msg.type === "auth_expired") {
      root.authPending = false
      root.authError = msg.error || "Device code expired"
    } else if (msg.type === "auth_error") {
      root.authPending = false
      root.authError = msg.error || "Authentication error"
    } else if (msg.type === "search_results") {
      root.searching = false
      root.searchResults = msg.results || []
    }
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
    sendCommand({ "command": "search", "query": query })
  }

  function playTrack(trackId) {
    sendCommand({ "command": "play_track", "track_id": trackId })
  }
}
