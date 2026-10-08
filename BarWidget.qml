import QtQuick
import Quickshell
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
      return (tidalService.isPlaying ? "󰐊 " : "󰏤 ") + tidalService.trackTitle + " • " + tidalService.trackArtist
    }
    return ""
  }

  WidgetButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: root.barLabelText() || "󰓇"
    tooltipText: (tidalService && tidalService.trackTitle) ? (tidalService.trackTitle + " - " + tidalService.trackArtist) : "Tidal Music"
    onPressed: function(b) {
      if (b === Qt.RightButton || b === Qt.MiddleButton) {
        if (tidalService) tidalService.togglePlay()
      } else {
        root.toggle()
      }
    }
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
        tidalService.checkDaemonBinary()
      }
    }

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }

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

            Text {
              text: "󰓇"
              font.family: root.bar ? root.bar.fontFamily : Style.font.family
              font.pixelSize: Style.font.display
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

            // Disconnect / Logout Button
            Ui.Button {
              visible: root.isAuthenticated
              anchors.verticalCenter: parent.verticalCenter
              text: "Logout"
              iconText: "󰍃"
              onClicked: {
                if (tidalService) tidalService.logout()
              }
            }
          }
        }

        PanelSeparator { width: parent.width }

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
            iconText: "󰓇"
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

          // Now Playing Card
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

              Text {
                text: (tidalService && tidalService.trackArtist) || "Select a favorite below"
                font.pixelSize: Style.font.caption
                color: Color.muted
                elide: Text.ElideRight
                width: parent.width
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

              Text {
                text: (tidalService && tidalService.trackAlbum) || ""
                font.pixelSize: Style.font.caption
                color: Color.muted
                elide: Text.ElideRight
                width: parent.width
                visible: tidalService && tidalService.trackAlbum !== ""
              }
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
            spacing: Style.space(16)

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

            Ui.Button {
              text: "⏭"
              tooltipText: "Next track"
              onClicked: {
                if (tidalService) tidalService.next()
              }
            }
          }

          PanelSeparator { width: parent.width }

          PanelSectionHeader { text: "FAVORITES" }

          Text {
            objectName: "tidalFavoritesState"
            width: parent.width
            visible: tidalService && (tidalService.favoritesLoading || tidalService.favoritesError !== "" || tidalService.favorites.length === 0)
            text: !tidalService ? "" : (tidalService.favoritesLoading ? "Loading favorites..." : (tidalService.favoritesError || "No favorite tracks yet. Add favorites in Tidal."))
            color: tidalService && tidalService.favoritesError !== "" ? Color.urgent : Color.muted
            font.pixelSize: Style.font.caption
            wrapMode: Text.WordWrap
          }

          Ui.Button {
            visible: tidalService && tidalService.favoritesError !== ""
            text: "Retry favorites"
            onClicked: if (tidalService) tidalService.loadFavorites()
          }

          ListView {
            id: favoritesList
            objectName: "tidalFavoritesList"
            width: parent.width
            height: Math.min(contentHeight, Style.space(280))
            clip: true
            spacing: Style.space(4)
            model: tidalService ? tidalService.favorites : []
            delegate: Rectangle {
              required property var modelData
              width: favoritesList.width
              height: Style.space(60)
              radius: Style.cornerRadius
              readonly property bool current: tidalService && tidalService.currentTrackId === modelData.id
              color: current ? Qt.rgba(Color.accent.r, Color.accent.g, Color.accent.b, 0.15) : Color.background
              border.color: current ? Color.accent : Color.background

              Row {
                anchors.fill: parent
                anchors.margins: Style.space(6)
                spacing: Style.space(8)

                Rectangle {
                  width: Style.space(48)
                  height: Style.space(48)
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
                  Text {
                    width: parent.width
                    text: (modelData.artist || "") + (modelData.album ? " • " + modelData.album : "")
                    font.pixelSize: Style.font.caption
                    color: Color.muted
                    elide: Text.ElideRight
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
                anchors.fill: parent
                cursorShape: Qt.PointingHandCursor
                onClicked: if (tidalService) tidalService.playTrack(modelData.id)
              }
            }
          }

          PanelSeparator { width: parent.width }

          // Search Section
          PanelSectionHeader { text: "SEARCH TIDAL" }

          Ui.TextField {
            id: searchField
            width: parent.width
            placeholderText: "Search songs, albums, artists..."
            onAccepted: {
              if (text.trim() !== "" && tidalService) {
                tidalService.search(text.trim())
              }
            }
          }

          // Search Results
          Column {
            width: parent.width
            spacing: Style.space(4)
            visible: tidalService && tidalService.searchResults && tidalService.searchResults.length > 0

            Repeater {
              model: (tidalService && tidalService.searchResults) ? tidalService.searchResults.slice(0, 5) : []
              delegate: Ui.Button {
                width: parent.width
                leftAlign: true
                text: (modelData.title || "") + " • " + (modelData.artist || "")
                onClicked: {
                  if (tidalService) tidalService.playTrack(modelData.id)
                }
              }
            }
          }
        }
      }
    }
  }
}
