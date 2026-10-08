import QtQuick
import Quickshell
import Quickshell.Io

Item {
  id: root

  property var shell: null
  readonly property string socketPath: Quickshell.env("XDG_RUNTIME_DIR") + "/tidal.sock"
  readonly property string binaryPath: Qt.resolvedUrl("backend/target/release/tidal-daemon").toString().replace(/^file:\/\//, "")
  readonly property string buildScriptMessage: "Backend daemon not found. Run ~/.config/omarchy/plugins/jaj.tidal/scripts/build.sh to build."

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

  // Check daemon binary existence
  Process {
    id: binaryCheckProc
    command: ["test", "-x", root.binaryPath]
    running: false
    onExited: function(exitCode) {
      root.daemonBinaryExists = (exitCode === 0)
      if (exitCode !== 0 && !root.authenticated) {
        root.authError = root.buildScriptMessage
      } else if (root.authError === root.buildScriptMessage) {
        root.authError = ""
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
  }

  // Socket communication with daemon
  Socket {
    id: daemonSocket
    path: root.socketPath
    connected: false

    onConnectedChanged: {
      if (connected) {
        sendCommand({ "command": "get_status" })
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

  Timer {
    id: reconnectTimer
    interval: 3000
    repeat: true
    running: !daemonSocket.connected
    onTriggered: {
      checkDaemonBinary()
      if (!daemonSocket.connected && daemonBinaryExists) {
        daemonSocket.connected = true
      }
    }
  }

  Component.onCompleted: {
    checkDaemonBinary()
    daemonSocket.connected = true
  }

  function ensureDaemonRunning() {
    if (!daemonBinaryExists) {
      root.authError = root.buildScriptMessage
      return
    }
    if (!daemonSocket.connected && !daemonProcess.running) {
      daemonProcess.running = true
    }
  }

  function sendCommand(obj) {
    if (daemonSocket.connected) {
      daemonSocket.write(JSON.stringify(obj) + "\n")
    } else {
      ensureDaemonRunning()
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
    checkDaemonBinary()
    if (!daemonBinaryExists) {
      root.authError = root.buildScriptMessage
      return
    }
    ensureDaemonRunning()
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
