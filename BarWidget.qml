import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui

Panel {
  id: root
  moduleName: "jaj.tidal"
  ipcTarget: "jaj.tidal.widget"

  // Service lookup
  readonly property var tidalService: bar?.shell?.firstPartyServiceFor ? bar.shell.firstPartyServiceFor("jaj.tidal") : localService
  Service { id: localService }

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  function formatTime(seconds) {
    if (isNaN(seconds) || seconds < 0) return "0:00"
    var mins = Math.floor(seconds / 60)
    var secs = Math.floor(seconds % 60)
    return mins + ":" + (secs < 10 ? "0" : "") + secs
  }

  function barLabelText() {
    if (!tidalService.authenticated) return "󰓇"
    if (tidalService.trackTitle) {
      var icon = tidalService.isPlaying ? "󰐊 " : "󰏤 "
      return icon + tidalService.trackTitle + " • " + tidalService.trackArtist
    }
    return "󰓇 Tidal"
  }

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: root.barLabelText()
    tooltipText: tidalService.trackTitle ? (tidalService.trackTitle + " - " + tidalService.trackArtist) : "Tidal Music"
    onPressed: function(b) {
      if (b === Qt.RightButton) {
        tidalService.togglePlay()
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

        // Header: Tidal logo + Title + Quality Badge
        Row {
          width: parent.width
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
              text: tidalService.authenticated ? "Connected" : "Not Logged In"
              font.family: root.bar ? root.bar.fontFamily : Style.font.family
              font.pixelSize: Style.font.caption
              color: Color.muted
            }
          }

          Item { width: 1; height: 1; Layout.fillWidth: true }

          // Audio Quality Badge
          Rectangle {
            visible: tidalService.authenticated && tidalService.trackTitle !== ""
            anchors.verticalCenter: parent.verticalCenter
            radius: Style.radiusSmall
            color: Qt.rgba(Color.accent.r, Color.accent.g, Color.accent.b, 0.15)
            border.color: Color.accent
            border.width: 1
            implicitWidth: qualityText.implicitWidth + Style.space(12)
            implicitHeight: qualityText.implicitHeight + Style.space(6)

            Text {
              id: qualityText
              anchors.centerIn: parent
              text: tidalService.audioQuality === "HI_RES_LOSSLESS" ? "HI-RES FLAC" : (tidalService.audioQuality === "LOSSLESS" ? "LOSSLESS" : "HIGH AAC")
              font.pixelSize: Style.font.caption
              font.bold: true
              color: Color.accent
            }
          }
        }

        PanelSeparator { width: parent.width }

        // Unauthenticated State: Device Pairing
        Column {
          visible: !tidalService.authenticated
          width: parent.width
          spacing: Style.space(10)

          Text {
            text: "Sign in with Tidal to stream high-fidelity audio."
            wrapMode: Text.WordWrap
            width: parent.width
            color: Color.foreground
            font.pixelSize: Style.font.body
          }

          Column {
            visible: tidalService.authPending
            width: parent.width
            spacing: Style.space(6)

            Text {
              text: "Enter this code on your device:"
              color: Color.muted
              font.pixelSize: Style.font.caption
            }

            Rectangle {
              width: parent.width
              height: Style.space(48)
              radius: Style.radiusMedium
              color: Qt.rgba(Color.foreground.r, Color.foreground.g, Color.foreground.b, 0.05)
              border.color: Color.accent
              border.width: 1

              Text {
                anchors.centerIn: parent
                text: tidalService.authCode || "FETCHING..."
                font.pixelSize: Style.space(22)
                font.bold: true
                color: Color.accent
              }
            }

            Button {
              width: parent.width
              text: "Open link.tidal.com in Browser"
              onClicked: Quickshell.execDetached(["xdg-open", tidalService.authUrl])
            }
          }

          Button {
            visible: !tidalService.authPending
            width: parent.width
            text: "Login with Tidal"
            onClicked: tidalService.startAuth()
          }
        }

        // Authenticated State: Now Playing & Controls
        Column {
          visible: tidalService.authenticated
          width: parent.width
          spacing: Style.space(12)

          // Now Playing Card
          Row {
            width: parent.width
            spacing: Style.space(12)

            // Album Artwork Placeholder / Image
            Rectangle {
              width: Style.space(72)
              height: Style.space(72)
              radius: Style.radiusMedium
              color: Qt.rgba(Color.foreground.r, Color.foreground.g, Color.foreground.b, 0.08)
              clip: true

              Image {
                anchors.fill: parent
                source: tidalService.trackArtUrl
                fillMode: Image.PreserveAspectCrop
                visible: tidalService.trackArtUrl !== ""
              }

              Text {
                anchors.centerIn: parent
                visible: tidalService.trackArtUrl === ""
                text: "󰝚"
                font.pixelSize: Style.space(32)
                color: Color.muted
              }
            }

            // Track Details
            Column {
              anchors.verticalCenter: parent.verticalCenter
              width: parent.width - Style.space(84)
              spacing: Style.space(3)

              Text {
                text: tidalService.trackTitle || "No Track Playing"
                font.pixelSize: Style.font.body
                font.bold: true
                color: Color.foreground
                elide: Text.ElideRight
                width: parent.width
              }

              Text {
                text: tidalService.trackArtist || "Select a song or search below"
                font.pixelSize: Style.font.caption
                color: Color.muted
                elide: Text.ElideRight
                width: parent.width
              }

              Text {
                text: tidalService.trackAlbum || ""
                font.pixelSize: Style.font.caption
                color: Color.muted
                elide: Text.ElideRight
                width: parent.width
                visible: tidalService.trackAlbum !== ""
              }
            }
          }

          // Seek Bar
          Column {
            width: parent.width
            spacing: Style.space(2)

            PanelSlider {
              width: parent.width
              value: tidalService.trackDuration > 0 ? (tidalService.trackPosition / tidalService.trackDuration) : 0
              onMoved: function(val) {
                tidalService.seek(val * tidalService.trackDuration)
              }
            }

            Row {
              width: parent.width
              Text {
                text: root.formatTime(tidalService.trackPosition)
                font.pixelSize: Style.font.caption
                color: Color.muted
              }
              Item { width: 1; height: 1; Layout.fillWidth: true }
              Text {
                text: root.formatTime(tidalService.trackDuration)
                font.pixelSize: Style.font.caption
                color: Color.muted
              }
            }
          }

          // Playback Controls
          Row {
            anchors.horizontalCenter: parent.horizontalCenter
            spacing: Style.space(16)

            Button {
              text: "⏮"
              onClicked: tidalService.previous()
            }

            Button {
              text: tidalService.isPlaying ? "⏸" : "▶"
              selected: true
              onClicked: tidalService.togglePlay()
            }

            Button {
              text: "⏭"
              onClicked: tidalService.next()
            }
          }

          PanelSeparator { width: parent.width }

          // Search Section
          PanelSectionHeader { text: "SEARCH TIDAL" }

          TextField {
            id: searchField
            width: parent.width
            placeholderText: "Search songs, albums, artists..."
            onAccepted: {
              if (text.trim() !== "") {
                tidalService.search(text.trim())
              }
            }
          }

          // Search Results
          Column {
            width: parent.width
            spacing: Style.space(4)
            visible: tidalService.searchResults && tidalService.searchResults.length > 0

            Repeater {
              model: tidalService.searchResults.slice(0, 5)
              delegate: WidgetButton {
                width: parent.width
                text: (modelData.title || "") + " • " + (modelData.artist || "")
                onClicked: tidalService.playTrack(modelData.id)
              }
            }
          }
        }
      }
    }
  }
}
