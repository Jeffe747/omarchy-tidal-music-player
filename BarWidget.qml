import QtQuick
import QtQuick.Controls
import Quickshell
import Quickshell.Services.Pipewire
import qs.Commons
import qs.Ui
import qs.Ui as Ui

Panel {
  id: root
  moduleName: "jaj.tidal"
  ipcTarget: "jaj.tidal.widget"

  // Service lookup
  readonly property var tidalService: {
    if (bar && bar.shell && typeof bar.shell.serviceFor === "function") {
      var s = bar.shell.serviceFor("jaj.tidal")
      if (s) return s
    }
    return null
  }

  readonly property bool isAuthenticated: tidalService ? tidalService.authenticated === true : false
  readonly property var defaultAudioSink: Pipewire.defaultAudioSink
  readonly property bool isSystemMuted: defaultAudioSink && defaultAudioSink.audio ? defaultAudioSink.audio.muted : false
  property string libraryTab: "favorites"
  property bool settingsOpen: false
  property string playlistFilter: ""
  property int keyboardIndex: 0

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  function formatTime(seconds) {
    if (isNaN(seconds) || seconds < 0) return "0:00"
    var mins = Math.floor(seconds / 60)
    var secs = Math.floor(seconds % 60)
    return mins + ":" + (secs < 10 ? "0" : "") + secs
  }

  function barLabelText() {
    if (tidalService && root.isAuthenticated && tidalService.trackTitle) {
      return (tidalService.isPlaying ? "󰐊 " : "󰏤 ") + tidalService.trackTitle + " • " + tidalService.trackArtist + (root.isSystemMuted ? " 󰖁" : "")
    }
    return root.isSystemMuted ? "󰖁" : ""
  }

  WidgetButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: root.barLabelText() || (root.isSystemMuted ? "󰖁" : "")
    fixedWidth: (root.barLabelText() === "" && !vertical) ? Style.space(32) : -1
    tooltipText: ((tidalService && tidalService.trackTitle) ? (tidalService.trackTitle + " - " + tidalService.trackArtist) : "Tidal Music") + (root.isSystemMuted ? " (System Muted)" : "")
    onPressed: function(b) {
      if (b === Qt.RightButton || b === Qt.MiddleButton) {
        if (tidalService) tidalService.togglePlay()
      } else {
        root.toggle()
      }
    }
  }

  TidalIcon {
    anchors.centerIn: button
    visible: root.barLabelText() === "" && !root.isSystemMuted
    color: button.active && button.useActiveColor ? button.activeColor : button.foreground
  }

  KeyboardPanel {
    id: panel
    anchorItem: button
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(Style.space(380))
    contentHeight: panel.fittedContentHeight(mainColumn.implicitHeight)
    onOpenChanged: {
      if (open && tidalService && typeof tidalService.checkDaemonBinary === "function") {
        if (typeof tidalService.initializeForWidget === "function") tidalService.initializeForWidget()
        else tidalService.checkDaemonBinary()
      }
      if (open && tidalService && tidalService.authenticated && tidalService.playlists.length === 0) {
        tidalService.loadPlaylists()
      }
    }

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }
      Keys.onPressed: function(event) {
        var editing = searchField.activeFocus || playlistFilterField.activeFocus
        if (event.key === Qt.Key_Slash) { searchField.forceActiveFocus(); event.accepted = true }
        else if (event.key === Qt.Key_Escape) { if (searchField.text !== "") { searchField.text = ""; if (tidalService) tidalService.clearSearch() } else root.close(); event.accepted = true }
        else if (!editing && event.key === Qt.Key_Space) { if (tidalService) tidalService.togglePlay(); event.accepted = true }
        else if (!editing && event.key === Qt.Key_Left) { if (tidalService) tidalService.seek(Math.max(0, tidalService.trackPosition - 5)); event.accepted = true }
        else if (!editing && event.key === Qt.Key_Right) { if (tidalService) tidalService.seek(Math.min(tidalService.trackDuration, tidalService.trackPosition + 5)); event.accepted = true }
        else if (!editing && event.key === Qt.Key_S) { if (tidalService) tidalService.toggleShuffle(); event.accepted = true }
        else if (!editing && event.key === Qt.Key_R) { if (tidalService) tidalService.cycleRepeat(); event.accepted = true }
        else if (!editing && event.key === Qt.Key_F) { if (tidalService) tidalService.toggleFavorite(); event.accepted = true }
        else if (!editing && event.key >= Qt.Key_1 && event.key <= Qt.Key_3) { root.libraryTab = ["queue", "favorites", "playlists"][event.key - Qt.Key_1]; event.accepted = true }
        else if (!editing && (event.key === Qt.Key_Plus || event.key === Qt.Key_Equal || event.key === Qt.Key_Minus)) { if (root.defaultAudioSink && root.defaultAudioSink.audio) root.defaultAudioSink.audio.volume = Math.max(0, Math.min(1.5, root.defaultAudioSink.audio.volume + (event.key === Qt.Key_Minus ? -0.05 : 0.05))); event.accepted = true }
        else if (!playlistFilterField.activeFocus && event.key === Qt.Key_Down) { root.keyboardIndex++; event.accepted = true }
        else if (!playlistFilterField.activeFocus && event.key === Qt.Key_Up) { root.keyboardIndex = Math.max(0, root.keyboardIndex - 1); event.accepted = true }
        else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) { var list = searchField.text.trim() !== "" ? (tidalService ? tidalService.searchResults : []) : (tidalService ? tidalService.favorites : []); if (list.length && list[root.keyboardIndex] && tidalService) tidalService.playTrack(list[root.keyboardIndex].id); event.accepted = true }
      }

      Column {
        id: mainColumn
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        spacing: Style.space(12)

        // Header: Tidal logo + Title + Quality Badge + Actions
        Item {
          width: parent.width
          implicitHeight: Math.max(headerLeft.implicitHeight, headerRight.implicitHeight)

          Row {
            id: headerLeft
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            spacing: Style.space(8)

            TidalIcon {
              iconSize: Style.font.display
              color: Color.accent
              anchors.verticalCenter: parent.verticalCenter
            }

            Column {
              anchors.verticalCenter: parent.verticalCenter
              spacing: Style.space(2)

              Text {
                text: "TIDAL"
                font.family: root.bar ? root.bar.fontFamily : Style.font.family
                font.pixelSize: Style.font.title
                font.bold: true
                color: Color.foreground
              }

              Text {
                text: root.isAuthenticated ? "Connected" : ((tidalService && tidalService.authPending) ? "Waiting for Login..." : "Not Logged In")
                font.family: root.bar ? root.bar.fontFamily : Style.font.family
                font.pixelSize: Style.font.caption
                color: root.isAuthenticated ? Color.accent : Color.muted
              }
            }
          }

          Row {
            id: headerRight
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            spacing: Style.space(8)

            // Audio Quality Badge
            Rectangle {
              visible: root.isAuthenticated && (tidalService && tidalService.trackTitle !== "")
              anchors.verticalCenter: parent.verticalCenter
              radius: Math.max(2, Style.space(4))
              color: Qt.rgba(Color.accent.r, Color.accent.g, Color.accent.b, 0.15)
              border.color: Color.accent
              border.width: 1
              implicitWidth: qualityText.implicitWidth + Style.space(12)
              implicitHeight: qualityText.implicitHeight + Style.space(6)

              Text {
                id: qualityText
                anchors.centerIn: parent
                text: (tidalService && tidalService.audioQuality === "HI_RES_LOSSLESS") ? "HI-RES FLAC" : ((tidalService && tidalService.audioQuality === "LOSSLESS") ? "LOSSLESS" : "HIGH AAC")
                font.pixelSize: Style.font.caption
                font.bold: true
                color: Color.accent
              }
            }

            Ui.Button {
              anchors.verticalCenter: parent.verticalCenter
              iconText: "󰒓"
              text: "Settings"
              tooltipText: "Settings"
              onClicked: root.settingsOpen = !root.settingsOpen
            }
          }
        }

        PanelSeparator { width: parent.width }

        Rectangle {
          visible: root.isSystemMuted
          width: parent.width
          radius: Style.cornerRadius
          color: Qt.rgba(Color.urgent.r, Color.urgent.g, Color.urgent.b, 0.12)
          border.color: Color.urgent
          border.width: 1
          implicitHeight: muteBannerRow.implicitHeight + Style.space(12)

          Row {
            id: muteBannerRow
            anchors.fill: parent
            anchors.margins: Style.space(6)
            spacing: Style.space(8)

            Text {
              text: "󰖁"
              color: Color.urgent
              font.pixelSize: Style.font.body
              anchors.verticalCenter: parent.verticalCenter
            }

            Text {
              text: "System audio is muted"
              color: Color.urgent
              font.pixelSize: Style.font.body
              anchors.verticalCenter: parent.verticalCenter
              width: Math.max(0, parent.width - Style.space(130))
              elide: Text.ElideRight
            }

            Ui.Button {
              text: "Unmute"
              onClicked: {
                if (root.defaultAudioSink && root.defaultAudioSink.audio)
                  root.defaultAudioSink.audio.muted = false
              }
            }
          }
        }

        // Unauthenticated State: Device Pairing
        Column {
          visible: !root.isAuthenticated
          width: parent.width
          spacing: Style.space(12)

          // Helpful banner when backend daemon binary is missing
          Rectangle {
            id: missingBinaryBanner
            visible: tidalService && (!tidalService.daemonBinaryExists || tidalService.authError === "Backend daemon not found. Run ~/.config/omarchy/plugins/jaj.tidal/scripts/build.sh to build.")
            width: parent.width
            radius: Style.cornerRadius
            color: Qt.rgba(Color.urgent.r, Color.urgent.g, Color.urgent.b, 0.12)
            border.color: Color.urgent
            border.width: 1
            implicitHeight: missingBinaryText.implicitHeight + Style.space(16)

            Text {
              id: missingBinaryText
              anchors.fill: parent
              anchors.margins: Style.space(8)
              text: "Backend daemon not found. Run ~/.config/omarchy/plugins/jaj.tidal/scripts/build.sh to build."
              wrapMode: Text.WordWrap
              color: Color.urgent
              font.pixelSize: Style.font.caption
            }
          }

          Text {
            text: "Sign in with Tidal to stream high-fidelity audio directly from Omarchy."
            wrapMode: Text.WordWrap
            width: parent.width
            color: Color.foreground
            font.pixelSize: Style.font.body
          }

          Text {
            visible: !tidalService
            text: "Tidal service is unavailable. Unlock the desktop and restart Omarchy shell."
            wrapMode: Text.WordWrap
            width: parent.width
            color: Color.urgent
            font.pixelSize: Style.font.caption
          }

          Column {
            visible: tidalService && tidalService.authPending
            width: parent.width
            spacing: Style.space(10)

            Text {
              text: "Pairing code (enter on link.tidal.com):"
              color: Color.muted
              font.pixelSize: Style.font.caption
            }

            Rectangle {
              width: parent.width
              height: Style.space(56)
              radius: Style.cornerRadius
              color: Qt.rgba(Color.accent.r, Color.accent.g, Color.accent.b, 0.1)
              border.color: Color.accent
              border.width: 1

              Text {
                anchors.centerIn: parent
                text: (tidalService && tidalService.authCode) ? tidalService.authCode : "FETCHING..."
                font.pixelSize: Style.space(26)
                font.bold: true
                color: Color.accent
              }
            }

            Text {
              text: "Enter the code in your browser to approve:"
              color: Color.muted
              font.pixelSize: Style.font.caption
              wrapMode: Text.WordWrap
              width: parent.width
            }

            Ui.Button {
              width: parent.width
              text: "Open link.tidal.com in Browser"
              iconText: "󰌹"
              onClicked: {
                if (tidalService && tidalService.authUrl) {
                  Quickshell.execDetached(["xdg-open", tidalService.authUrl])
                }
              }
            }
          }

          Text {
            visible: tidalService && tidalService.authError !== "" && !tidalService.authPending && (!missingBinaryBanner.visible || tidalService.authError !== "Backend daemon not found. Run ~/.config/omarchy/plugins/jaj.tidal/scripts/build.sh to build.")
            text: tidalService ? tidalService.authError : ""
            color: Color.urgent
            font.pixelSize: Style.font.caption
            wrapMode: Text.WordWrap
            width: parent.width
          }

          Ui.Button {
            visible: !tidalService || !tidalService.authPending
            enabled: tidalService !== null
            width: parent.width
            text: "Login with Tidal"
            iconText: "󰝚"
            onClicked: {
              if (tidalService) tidalService.startAuth()
            }
          }
        }

        // Authenticated State: Now Playing & Controls
        Column {
          visible: root.isAuthenticated
          width: parent.width
          spacing: Style.space(12)

          Rectangle {
            visible: root.settingsOpen
            width: parent.width
            radius: Style.cornerRadius
            color: Color.background
            border.color: Color.muted
            implicitHeight: settingsColumn.implicitHeight + Style.space(16)
            Column {
              id: settingsColumn
              anchors.fill: parent
              anchors.margins: Style.space(8)
              spacing: Style.space(6)
              PanelSectionHeader { text: "SETTINGS" }
              Text { text: "Audio quality preference"; color: Color.muted; font.pixelSize: Style.font.caption }
              Row {
                spacing: Style.space(4)
                Ui.Button { text: "Hi-Res FLAC"; selected: tidalService && tidalService.preferredAudioQuality === "HI_RES_LOSSLESS"; onClicked: tidalService.setAudioQuality("HI_RES_LOSSLESS") }
                Ui.Button { text: "Lossless"; selected: tidalService && tidalService.preferredAudioQuality === "LOSSLESS"; onClicked: tidalService.setAudioQuality("LOSSLESS") }
                Ui.Button { text: "High AAC"; selected: tidalService && tidalService.preferredAudioQuality === "HIGH"; onClicked: tidalService.setAudioQuality("HIGH") }
              }
              Text { text: root.isAuthenticated ? "Account connected" : "Not connected"; color: root.isAuthenticated ? Color.accent : Color.muted; font.pixelSize: Style.font.caption }
              Row {
                spacing: Style.space(6)
                Ui.Button { visible: root.isAuthenticated; text: "Logout"; iconText: "󰍃"; onClicked: { tidalService.logout(); root.settingsOpen = false } }
                Ui.Button { text: "Close"; onClicked: root.settingsOpen = false }
              }
            }
          }

          // Now Playing Card
          Row {
            width: parent.width; spacing: Style.space(6)
            visible: !tidalService || tidalService.explorationView === ""
            Ui.Button { text: "Queue"; selected: root.libraryTab === "queue"; onClicked: root.libraryTab = "queue" }
            Ui.Button { text: "Favorites"; selected: root.libraryTab === "favorites"; onClicked: root.libraryTab = "favorites" }
            Ui.Button { text: "Playlists"; selected: root.libraryTab === "playlists"; onClicked: { root.libraryTab = "playlists"; if (tidalService && tidalService.playlists.length === 0) tidalService.loadPlaylists() } }
          }

          Rectangle {
            visible: false
            width: parent.width
            radius: Style.cornerRadius
            color: Qt.rgba(Color.accent.r, Color.accent.g, Color.accent.b, 0.12)
            implicitHeight: playlistQualityText.implicitHeight + Style.space(10)
            Text {
              id: playlistQualityText
              anchors.centerIn: parent
              text: tidalService && tidalService.audioQuality === "HI_RES_LOSSLESS" ? "HI-RES FLAC" : (tidalService && tidalService.audioQuality === "HIGH" ? "HIGH AAC" : "LOSSLESS")
              color: Color.accent
              font.pixelSize: Style.font.caption
              font.bold: true
            }
          }

          Row {
            width: parent.width
            spacing: Style.space(12)

            // Album Artwork Placeholder / Image
            Rectangle {
              width: Style.space(96)
              height: Style.space(96)
              radius: Style.cornerRadius
              color: Qt.rgba(Color.foreground.r, Color.foreground.g, Color.foreground.b, 0.08)
              clip: true

              Image {
                objectName: "tidalNowPlayingArt"
                anchors.fill: parent
                source: (tidalService && tidalService.trackArtUrl) || ""
                fillMode: Image.PreserveAspectCrop
                visible: tidalService && tidalService.trackArtUrl !== ""
              }

              Text {
                anchors.centerIn: parent
                visible: !tidalService || tidalService.trackArtUrl === ""
                text: "󰝚"
                font.pixelSize: Style.space(32)
                color: Color.muted
              }
            }

            // Track Details
            Column {
              anchors.verticalCenter: parent.verticalCenter
              width: parent.width - Style.space(108)
              spacing: Style.space(3)

              Text {
                objectName: "tidalNowPlayingTitle"
                text: (tidalService && tidalService.trackTitle) || "No Track Playing"
                font.pixelSize: Style.font.title
                font.bold: true
                color: Color.foreground
                elide: Text.ElideRight
                width: parent.width
              }

              Row {
                spacing: Style.space(4)
                Ui.Button { text: (tidalService && tidalService.trackArtist) || "Select a favorite below"; onClicked: if (tidalService && tidalService.currentArtistId) tidalService.exploreArtist(tidalService.currentArtistId) }
                Ui.Button { text: tidalService && tidalService.currentTrackFavorite ? "󰋑" : "󰋕"; tooltipText: "Toggle favorite"; onClicked: if (tidalService) tidalService.toggleFavorite() }
              }

              Text {
                objectName: "tidalPlaybackError"
                width: parent.width
                visible: tidalService && tidalService.playbackError !== ""
                text: tidalService ? tidalService.playbackError : ""
                color: Color.urgent
                font.pixelSize: Style.font.caption
                wrapMode: Text.WordWrap
              }

              Ui.Button { text: (tidalService && tidalService.trackAlbum) || ""; visible: tidalService && tidalService.trackAlbum !== ""; onClicked: if (tidalService && tidalService.currentAlbumId) tidalService.exploreAlbum(tidalService.currentAlbumId) }
            }
          }

          // Seek Bar
          Column {
            width: parent.width
            spacing: Style.space(2)

            PanelSlider {
              width: parent.width
              bar: root.bar
              value: (tidalService && tidalService.trackDuration > 0) ? Math.max(0, Math.min(1, tidalService.trackPosition / tidalService.trackDuration)) : 0
              onMoved: function(val) {
                if (tidalService) {
                  tidalService.seek(val * tidalService.trackDuration)
                }
              }
            }

            Item {
              width: parent.width
              implicitHeight: timeCurrent.implicitHeight

              Text {
                id: timeCurrent
                anchors.left: parent.left
                text: root.formatTime(tidalService ? tidalService.trackPosition : 0)
                font.pixelSize: Style.font.caption
                color: Color.muted
              }

              Text {
                anchors.right: parent.right
                text: root.formatTime(tidalService ? tidalService.trackDuration : 0)
                font.pixelSize: Style.font.caption
                color: Color.muted
              }
            }
          }

          // Playback Controls
          Row {
            anchors.horizontalCenter: parent.horizontalCenter
            spacing: Style.space(8)

            Ui.Button {
              text: "⏮"
              tooltipText: "Previous track (or restart current track)"
              onClicked: {
                if (tidalService) tidalService.previous()
              }
            }

            Ui.Button {
              text: (tidalService && tidalService.isPlaying) ? "⏸" : "▶"
              selected: true
              tooltipText: (tidalService && tidalService.isPlaying) ? "Pause" : "Play"
              onClicked: {
                if (tidalService) tidalService.togglePlay()
              }
            }

            Ui.Button { text: "󰒝"; selected: tidalService && tidalService.shuffle; tooltipText: "Shuffle"; onClicked: if (tidalService) tidalService.toggleShuffle() }
            Ui.Button { text: "⏭"; tooltipText: "Next track"; onClicked: if (tidalService) tidalService.next() }
            Ui.Button { text: tidalService && tidalService.repeatMode === "one" ? "󰑘" : "󰑖"; selected: tidalService && tidalService.repeatMode !== "off"; tooltipText: "Repeat: " + (tidalService ? tidalService.repeatMode : "off"); onClicked: if (tidalService) tidalService.cycleRepeat() }
          }

          Row {
            width: parent.width; spacing: Style.space(6)
            Text { text: root.isSystemMuted ? "󰖁" : "󰕾"; color: Color.foreground; font.pixelSize: Style.font.body; anchors.verticalCenter: parent.verticalCenter
              TapHandler { onTapped: if (root.defaultAudioSink && root.defaultAudioSink.audio) root.defaultAudioSink.audio.muted = !root.defaultAudioSink.audio.muted }
            }
            Slider {
              id: volumeSlider; width: parent.width - volumePercent.implicitWidth - Style.space(44); anchors.verticalCenter: parent.verticalCenter
              from: 0; to: 1.5; value: root.defaultAudioSink && root.defaultAudioSink.audio ? root.defaultAudioSink.audio.volume : 0
              onMoved: if (root.defaultAudioSink && root.defaultAudioSink.audio) root.defaultAudioSink.audio.volume = value
              background: Rectangle { x: volumeSlider.leftPadding; y: volumeSlider.topPadding + volumeSlider.availableHeight / 2 - height / 2; width: volumeSlider.availableWidth; height: Style.space(4); radius: Style.cornerRadius; color: Color.muted }
              handle: Rectangle { x: volumeSlider.leftPadding + volumeSlider.visualPosition * (volumeSlider.availableWidth - width); y: volumeSlider.topPadding + volumeSlider.availableHeight / 2 - height / 2; width: Style.space(12); height: width; radius: Style.cornerRadius; color: Color.accent }
              WheelHandler { onWheel: function(event) { if (root.defaultAudioSink && root.defaultAudioSink.audio) root.defaultAudioSink.audio.volume = Math.max(0, Math.min(1.5, root.defaultAudioSink.audio.volume + (event.angleDelta.y > 0 ? 0.05 : -0.05))); event.accepted = true } }
            }
            Text { id: volumePercent; text: Math.round(volumeSlider.value * 100) + "%"; color: Color.muted; font.pixelSize: Style.font.caption; anchors.verticalCenter: parent.verticalCenter }
          }

          PanelSeparator { width: parent.width }

          Column {
            visible: tidalService && tidalService.explorationView !== ""
            width: parent.width
            spacing: Style.space(6)
            Ui.Button { text: "← Back"; onClicked: { tidalService.explorationView = ""; tidalService.explorationTracks = [] } }
            PanelSectionHeader { text: tidalService && tidalService.explorationView === "album" ? "ALBUM TRACKS" : "ARTIST TOP TRACKS" }
            Text { visible: tidalService && tidalService.explorationError !== ""; text: tidalService ? tidalService.explorationError : ""; color: Color.urgent; font.pixelSize: Style.font.caption }
            ListView {
              width: parent.width
              height: count > 0 ? Math.min(contentHeight, Style.space(280)) : 0
              clip: true
              ScrollBar.vertical: ScrollBar { policy: ScrollBar.AlwaysOn; width: Style.space(6); contentItem: Rectangle { radius: Style.cornerRadius; color: Color.muted } }
              model: tidalService ? tidalService.explorationTracks : []
              delegate: Rectangle {
                required property var modelData
                width: parent.width; height: Style.space(40); radius: Style.cornerRadius; color: Color.background
                Column { anchors.left: parent.left; anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter; anchors.leftMargin: Style.space(8); anchors.rightMargin: Style.space(8)
                  Text { width: parent.width; text: modelData.title || ""; color: Color.foreground; font.pixelSize: Style.font.body; elide: Text.ElideRight }
                  Row {
                    spacing: Style.space(3)
                    Text { text: modelData.artist || ""; color: Color.muted; font.pixelSize: Style.font.caption; elide: Text.ElideRight; TapHandler { onTapped: if (tidalService && modelData.artist_id) tidalService.exploreArtist(modelData.artist_id) } }
                    Text { visible: !!modelData.album; text: "•"; color: Color.muted; font.pixelSize: Style.font.caption }
                    Text { text: modelData.album || ""; color: Color.muted; font.pixelSize: Style.font.caption; elide: Text.ElideRight; TapHandler { onTapped: if (tidalService && modelData.album_id) tidalService.exploreAlbum(modelData.album_id) } }
                  }
                }
                MouseArea { z: -1; anchors.fill: parent; onClicked: if (tidalService) tidalService.playTrack(modelData.id) }
              }
            }
          }

          PanelSectionHeader { visible: !tidalService || tidalService.explorationView === ""; text: (searchField.text.trim() !== "" ? "SEARCH RESULTS" : (root.libraryTab === "playlists" ? "PLAYLISTS" : (root.libraryTab === "queue" ? "QUEUE" : "FAVORITES"))) }

          Timer {
            id: searchDebounce
            interval: 350
            repeat: false
            onTriggered: {
              if (tidalService && searchField.text.trim() !== "") tidalService.search(searchField.text.trim())
            }
          }

          Ui.TextField {
            id: searchField
            visible: !tidalService || tidalService.explorationView === ""
            width: parent.width
            placeholderText: "Search tracks, albums, artists, playlists...  (/ to focus)"
            onTextChanged: {
              root.keyboardIndex = 0
              if (tidalService) tidalService.clearSearch()
              searchDebounce.stop()
              if (text.trim() !== "") searchDebounce.start()
            }
          }

          Ui.Button {
            visible: (!tidalService || tidalService.explorationView === "") && searchField.text.trim() !== ""
            text: "← Back to favorites"
            onClicked: {
              searchDebounce.stop()
              searchField.text = ""
              if (tidalService) tidalService.clearSearch()
            }
          }

          Text {
            objectName: "tidalFavoritesState"
            width: parent.width
            visible: (!tidalService || tidalService.explorationView === "") && (root.libraryTab === "favorites" || root.libraryTab === "queue") && searchField.text.trim() === "" && tidalService && (tidalService.favoritesLoading || tidalService.favoritesError !== "" || tidalService.favorites.length === 0)
            text: !tidalService ? "" : (tidalService.favoritesLoading ? "Loading favorites..." : (tidalService.favoritesError || "No favorite tracks yet. Add favorites in Tidal."))
            color: tidalService && tidalService.favoritesError !== "" ? Color.urgent : Color.muted
            font.pixelSize: Style.font.caption
            wrapMode: Text.WordWrap
          }

          Ui.Button {
            visible: (root.libraryTab === "favorites" || root.libraryTab === "queue") && searchField.text.trim() === "" && tidalService && tidalService.favoritesError !== ""
            text: "Retry favorites"
            onClicked: if (tidalService) tidalService.loadFavorites()
          }

          ListView {
            id: favoritesList
            objectName: "tidalFavoritesList"
            width: parent.width
            height: Math.min(contentHeight, Style.space(280))
            visible: (!tidalService || tidalService.explorationView === "") && (root.libraryTab === "favorites" || root.libraryTab === "queue") && searchField.text.trim() === ""
            clip: true
            currentIndex: root.keyboardIndex
            ScrollBar.vertical: ScrollBar { policy: ScrollBar.AlwaysOn; width: Style.space(6); contentItem: Rectangle { radius: Style.cornerRadius; color: Color.muted } }
            spacing: Style.space(4)
            model: tidalService ? tidalService.favorites : []
            delegate: Rectangle {
              required property var modelData
              required property int index
              width: favoritesList.width
              height: Style.space(36)
              radius: Style.cornerRadius
              readonly property bool current: tidalService && tidalService.currentTrackId === modelData.id
              color: current || index === root.keyboardIndex ? Qt.rgba(Color.accent.r, Color.accent.g, Color.accent.b, 0.15) : Color.background
              border.color: current ? Color.accent : Color.background

              Row {
                anchors.fill: parent
                anchors.margins: Style.space(3)
                spacing: Style.space(5)

                Rectangle {
                  visible: false
                  width: Style.space(1)
                  height: Style.space(1)
                  radius: Style.cornerRadius
                  color: Qt.rgba(Color.foreground.r, Color.foreground.g, Color.foreground.b, 0.08)
                  clip: true
                  Image {
                    anchors.fill: parent
                    source: modelData.art_url || ""
                    fillMode: Image.PreserveAspectCrop
                    asynchronous: true
                  }
                  Text {
                    anchors.centerIn: parent
                    visible: !modelData.art_url
                    text: "󰝚"
                    color: Color.muted
                    font.pixelSize: Style.font.title
                  }
                }

                Column {
                  anchors.verticalCenter: parent.verticalCenter
                  width: parent.width - Style.space(64) - favoriteDuration.width
                  spacing: Style.space(3)
                  Text {
                    width: parent.width
                    text: modelData.title || ""
                    font.pixelSize: Style.font.body
                    color: current ? Color.accent : Color.foreground
                    elide: Text.ElideRight
                  }
                  Row {
                    spacing: Style.space(3)
                    Text {
                      text: modelData.artist || ""; font.pixelSize: Style.font.caption; color: Color.muted; elide: Text.ElideRight
                      TapHandler { onTapped: if (tidalService && modelData.artist_id) tidalService.exploreArtist(modelData.artist_id) }
                    }
                    Text { visible: !!modelData.album; text: "•"; font.pixelSize: Style.font.caption; color: Color.muted }
                    Text {
                      text: modelData.album || ""; font.pixelSize: Style.font.caption; color: Color.muted; elide: Text.ElideRight
                      TapHandler { onTapped: if (tidalService && modelData.album_id) tidalService.exploreAlbum(modelData.album_id) }
                    }
                  }
                }

                Text {
                  id: favoriteDuration
                  anchors.verticalCenter: parent.verticalCenter
                  text: root.formatTime(modelData.duration || 0)
                  color: Color.muted
                  font.pixelSize: Style.font.caption
                }
              }

              MouseArea {
                objectName: "tidalFavoriteClick"
                z: -1
                anchors.fill: parent
                cursorShape: Qt.PointingHandCursor
                onClicked: if (tidalService) tidalService.playTrack(modelData.id)
              }
            }
          }

          Column {
            width: parent.width
            spacing: Style.space(5)
            visible: (!tidalService || tidalService.explorationView === "") && root.libraryTab === "playlists"
            Ui.Button {
              visible: tidalService && tidalService.currentPlaylistId !== ""
              text: "← Back to playlists"
              onClicked: { tidalService.currentPlaylistId = ""; tidalService.playlistTracks = [] }
            }
            Text {
              visible: tidalService && (tidalService.playlistsLoading || tidalService.playlistsError !== "" || (tidalService.currentPlaylistId === "" && tidalService.playlists.length === 0) || tidalService.playlistTracksLoading || tidalService.playlistTracksError !== "")
              text: !tidalService ? "" : (tidalService.playlistsLoading ? "Loading playlists..." : (tidalService.playlistsError || (tidalService.playlistTracksLoading ? "Loading tracks..." : (tidalService.playlistTracksError || (tidalService.playlists.length === 0 ? "No playlists found" : "")))))
              color: tidalService && (tidalService.playlistsError !== "" || tidalService.playlistTracksError !== "") ? Color.urgent : Color.muted
              font.pixelSize: Style.font.caption
              wrapMode: Text.WordWrap
            }
            Ui.Button { visible: tidalService && tidalService.currentPlaylistId === "" && tidalService.playlistsError !== ""; text: "Retry playlists"; onClicked: tidalService.loadPlaylists() }
            ListView {
              id: playlistsList
              width: parent.width
              height: count > 0 ? Math.min(Math.max(contentHeight, Style.space(64)), Style.space(280)) : 0
              clip: true
              ScrollBar.vertical: ScrollBar { policy: ScrollBar.AlwaysOn; width: Style.space(6); contentItem: Rectangle { radius: Style.cornerRadius; color: Color.muted } }
              spacing: Style.space(4)
              model: tidalService && tidalService.currentPlaylistId === "" ? tidalService.playlists : []
              delegate: Rectangle {
                required property var modelData
                width: playlistsList.width
                height: Style.space(64)
                radius: Style.cornerRadius
                color: Color.background
                Row {
                  anchors.fill: parent; anchors.margins: Style.space(6); spacing: Style.space(8)
                  Rectangle {
                    width: Style.space(48); height: Style.space(48); radius: Style.cornerRadius
                    color: Qt.rgba(Color.foreground.r, Color.foreground.g, Color.foreground.b, 0.08)
                    clip: true
                    Image {
                      id: playlistArtwork
                      anchors.fill: parent
                      source: modelData.art_url || ""
                      fillMode: Image.PreserveAspectCrop
                      asynchronous: true
                    }
                    Text {
                      anchors.centerIn: parent
                      visible: playlistArtwork.status !== Image.Ready
                      text: "󰝚"
                      color: Color.muted
                      font.pixelSize: Style.font.title
                    }
                  }
                  Column {
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width - Style.space(64)
                    Text { width: parent.width; text: modelData.title || "Untitled playlist"; color: Color.foreground; font.pixelSize: Style.font.body; elide: Text.ElideRight }
                    Text { width: parent.width; text: (modelData.numberOfTracks || 0) + " tracks • " + root.formatTime(modelData.duration || 0); color: Color.muted; font.pixelSize: Style.font.caption }
                  }
                }
                MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: if (tidalService) tidalService.loadPlaylistTracks(modelData.uuid) }
              }
            }
          Ui.TextField {
              id: playlistFilterField
              visible: tidalService && tidalService.currentPlaylistId !== ""
              width: parent.width
              placeholderText: "Filter tracks by title or artist..."
              onTextChanged: root.playlistFilter = text.trim().toLowerCase()
            }
            ListView {
              id: playlistTracksList
              width: parent.width
              height: count > 0 ? Math.min(Math.max(contentHeight, Style.space(56)), Style.space(280)) : 0
              clip: true
              ScrollBar.vertical: ScrollBar { policy: ScrollBar.AlwaysOn; width: Style.space(6); contentItem: Rectangle { radius: Style.cornerRadius; color: Color.muted } }
              spacing: Style.space(4)
              model: tidalService && tidalService.currentPlaylistId !== "" ? tidalService.playlistTracks.filter(function(t) { return root.playlistFilter === "" || (t.title + " " + t.artist).toLowerCase().indexOf(root.playlistFilter) >= 0 }) : []
              delegate: Rectangle {
                required property var modelData
                width: playlistTracksList.width; height: Style.space(40); radius: Style.cornerRadius; color: Color.background
                Row {
                  anchors.fill: parent; anchors.margins: Style.space(6); spacing: Style.space(8)
                  Column {
                    anchors.verticalCenter: parent.verticalCenter; width: parent.width - playlistDuration.width
                    Text { width: parent.width; text: modelData.title || ""; color: Color.foreground; font.pixelSize: Style.font.body; elide: Text.ElideRight }
                    Row {
                      spacing: Style.space(3)
                      Text { text: modelData.artist || ""; color: Color.muted; font.pixelSize: Style.font.caption; elide: Text.ElideRight; TapHandler { onTapped: if (tidalService && modelData.artist_id) tidalService.exploreArtist(modelData.artist_id) } }
                      Text { visible: !!modelData.album; text: "•"; color: Color.muted; font.pixelSize: Style.font.caption }
                      Text { text: modelData.album || ""; color: Color.muted; font.pixelSize: Style.font.caption; elide: Text.ElideRight; TapHandler { onTapped: if (tidalService && modelData.album_id) tidalService.exploreAlbum(modelData.album_id) } }
                    }
                  }
                  Text { id: playlistDuration; anchors.verticalCenter: parent.verticalCenter; text: root.formatTime(modelData.duration || 0); color: Color.muted; font.pixelSize: Style.font.caption }
                }
                MouseArea { z: -1; anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: if (tidalService) tidalService.playTrack(modelData.id) }
              }
            }
          }

          // Search Results
          Column {
            width: parent.width
            spacing: Style.space(4)
            visible: searchField.text.trim() !== ""

            Text {
              visible: tidalService && (tidalService.searching || tidalService.searchError !== "" || (!tidalService.searching && tidalService.searchResults.length === 0))
              text: !tidalService ? "" : (tidalService.searching ? "Searching..." : (tidalService.searchError || (tidalService.searchResults.length === 0 ? "No tracks found" : "")))
              color: tidalService && tidalService.searchError !== "" ? Color.urgent : Color.muted
              font.pixelSize: Style.font.caption
            }

            ListView {
              id: searchResultsList
              width: parent.width
              height: count > 0 ? Math.min(contentHeight, Style.space(280)) : 0
              clip: true
              currentIndex: root.keyboardIndex
              ScrollBar.vertical: ScrollBar { policy: ScrollBar.AlwaysOn; width: Style.space(6); contentItem: Rectangle { radius: Style.cornerRadius; color: Color.muted } }
              model: (tidalService && tidalService.searchResults) ? tidalService.searchResults : []
              delegate: Rectangle {
                required property var modelData
                required property int index
                width: parent.width
                height: Style.space(40)
                radius: Style.cornerRadius
                color: index === root.keyboardIndex ? Qt.rgba(Color.accent.r, Color.accent.g, Color.accent.b, 0.15) : Color.background
                Row {
                  anchors.fill: parent
                  anchors.margins: Style.space(6)
                  spacing: Style.space(8)
                  Rectangle {
                    width: Style.space(48); height: Style.space(48); radius: Style.cornerRadius
                    color: Qt.rgba(Color.foreground.r, Color.foreground.g, Color.foreground.b, 0.08)
                    clip: true
                    Image { anchors.fill: parent; source: modelData.art_url || ""; fillMode: Image.PreserveAspectCrop; asynchronous: true }
                    Text { anchors.centerIn: parent; visible: !modelData.art_url; text: "󰝚"; color: Color.muted; font.pixelSize: Style.font.title }
                  }
                  Column {
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width - Style.space(64) - searchDuration.width
                    spacing: Style.space(3)
                    Text { width: parent.width; text: modelData.title || ""; font.pixelSize: Style.font.body; color: Color.foreground; elide: Text.ElideRight }
                    Row {
                      spacing: Style.space(3)
                      Text { text: modelData.artist || ""; font.pixelSize: Style.font.caption; color: Color.muted; elide: Text.ElideRight; TapHandler { onTapped: if (tidalService && modelData.artist_id) tidalService.exploreArtist(modelData.artist_id) } }
                      Text { visible: !!modelData.album; text: "•"; font.pixelSize: Style.font.caption; color: Color.muted }
                      Text { text: modelData.album || ""; font.pixelSize: Style.font.caption; color: Color.muted; elide: Text.ElideRight; TapHandler { onTapped: if (tidalService && modelData.album_id) tidalService.exploreAlbum(modelData.album_id) } }
                    }
                  }
                  Text { id: searchDuration; anchors.verticalCenter: parent.verticalCenter; text: root.formatTime(modelData.duration || 0); color: Color.muted; font.pixelSize: Style.font.caption }
                }
                MouseArea { z: -1; anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: if (tidalService) tidalService.playTrack(modelData.id) }
              }
            }
          }
        }
      }
    }
  }
}
