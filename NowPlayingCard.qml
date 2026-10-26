import QtQuick
import Quickshell.Services.Pipewire
import qs.Commons
import qs.Ui as Ui

Column {
  id: root
  property var service: null
  property var bar: null
  property var sink: Pipewire.defaultAudioSink
  property bool settingsOpen: false
  property real previewSeconds: 0
  property real pendingSeek: -1
  property real wheelRemainder: 0
  signal settingsRequested()
  signal chromecastRequested()
  signal exploreArtist(var artistId, string artistName)
  signal exploreAlbum(var albumId, string albumTitle)

  readonly property string currentStyle: root.service && root.service.playerStyle ? root.service.playerStyle : "standard"
  readonly property string localArtSource: root.service && root.service.trackArtUrl && root.service.trackArtUrl.indexOf("file://") === 0 ? root.service.trackArtUrl : ""
  readonly property bool hasTrack: !!root.service && (!!root.service.trackTitle || !!root.service.trackLoading)
  readonly property bool isPlaying: !!root.service && root.service.isPlaying
  readonly property bool isFav: !!root.service && root.service.currentTrackFavorite
  readonly property real currentPos: !root.service || !root.service.trackTitle ? 0 : (root.pendingSeek >= 0 ? root.pendingSeek : root.service.trackPosition)
  readonly property real duration: root.service && root.service.trackTitle ? root.service.trackDuration : 0
  readonly property real progressValue: duration > 0 ? Math.max(0, Math.min(1, currentPos / duration)) : 0
  readonly property string qualityText: {
    var q = root.service ? root.service.audioQuality : "LOSSLESS"
    if (q === "HI_RES_LOSSLESS") return "24-BIT / 192 kHz FLAC · HI-RES"
    if (q === "LOSSLESS") return "16-BIT / 44.1 kHz FLAC · LOSSLESS"
    return "320 KBPS AAC · HIGH"
  }
  readonly property string provenanceShortBadge: {
    var q = root.service ? root.service.audioQuality : "LOSSLESS"
    if (q === "HI_RES_LOSSLESS") return "HI-RES"
    if (q === "LOSSLESS") return "LOSSLESS"
    return "HIGH AAC"
  }

  width: parent ? parent.width : implicitWidth
  spacing: Style.spacing.md
  PwObjectTracker { objects: root.sink ? [root.sink] : [] }

  function formatTime(seconds) {
    var safe = Math.max(0, Number(seconds) || 0)
    return Math.floor(safe / 60) + ":" + ("0" + Math.floor(safe % 60)).slice(-2)
  }
  function seekRelative(delta) {
    if (!service || service.trackDuration <= 0) return
    var base = pendingSeek >= 0 ? pendingSeek : service.trackPosition
    pendingSeek = Math.max(0, Math.min(service.trackDuration, base + delta))
    service.seek(pendingSeek)
  }
  Timer { interval: 750; running: root.pendingSeek >= 0; onTriggered: root.pendingSeek = -1 }

  function setVolume(v) {
    if (root.service && root.service.castActive) {
      root.service.setCastVolume(Math.min(1.0, v))
    } else if (root.sink && root.sink.audio) {
      root.sink.audio.volume = v
    }
  }

  function handleWheelVolume(deltaY) {
    root.wheelRemainder += deltaY
    var steps = Math.trunc(root.wheelRemainder / 120)
    if (steps) {
      root.wheelRemainder -= steps * 120
      if (root.service && root.service.castActive) {
        var castVol = Math.max(0, Math.min(1.0, root.service.castVolume + steps * 0.05))
        root.service.setCastVolume(castVol)
      } else if (root.sink && root.sink.audio) {
        var newVol = Math.max(0, Math.min(1.5, root.sink.audio.volume + steps * 0.05))
        root.sink.audio.volume = newVol
      }
    }
  }

  // =========================================================================
  // Style 1: Standard (Balanced Classic)
  // =========================================================================
  Column {
    id: standardCard
    visible: root.currentStyle === "standard"
    width: parent.width
    height: visible ? implicitHeight : 0
    spacing: Style.spacing.lg

    Row {
      width: parent.width
      spacing: Style.spacing.lg
      Rectangle {
        width: Style.space(60)
        height: width
        radius: Style.cornerRadius
        color: Style.hoverFill
        clip: true
        Image {
          id: standardCover
          objectName: "tidalNowPlayingArt"
          anchors.fill: parent
          visible: root.localArtSource !== ""
          source: root.localArtSource
          sourceSize: Qt.size(width * Screen.devicePixelRatio, height * Screen.devicePixelRatio)
          fillMode: Image.PreserveAspectCrop
          asynchronous: true
        }
        Text { anchors.centerIn: parent; visible: !standardCover.visible || standardCover.status !== Image.Ready; textFormat: Text.PlainText; text: "󰝚"; color: Color.muted; font.family: Style.font.family; font.pixelSize: Style.font.title }
      }
      Column {
        width: Math.max(0, parent.width - Style.space(60) - Style.spacing.lg)
        anchors.verticalCenter: parent.verticalCenter
        spacing: Style.spacing.xs
        Row {
          width: parent.width
          spacing: Style.spacing.sm
          Text {
            id: standardTitle
            objectName: "tidalNowPlayingTitle"
            width: Math.max(0, parent.width - standardSettings.implicitWidth - (standardChromecast.visible ? standardChromecast.implicitWidth + Style.spacing.sm : 0) - (standardFavorite.visible ? standardFavorite.implicitWidth + Style.spacing.sm : 0) - Style.spacing.sm)
            textFormat: Text.PlainText
            text: root.service && root.service.trackTitle ? root.service.trackTitle : (root.service && root.service.trackLoading ? "Loading track…" : "Nothing playing")
            color: root.service && (root.service.trackTitle || root.service.trackLoading) ? Color.foreground : Color.muted
            font.family: Style.font.family
            font.pixelSize: Style.font.title
            font.bold: true
            elide: Text.ElideRight
          }
          Ui.PanelActionButton {
            id: standardFavorite
            visible: true
            enabled: !!root.service && !!root.service.trackTitle
            size: Style.spacing.controlHeight
            iconText: root.isFav ? "󰋑" : "󰋕"
            foreground: enabled ? (root.isFav ? Color.accent : Color.foreground) : Color.muted
            opacity: enabled ? 1.0 : 0.4
            tooltipText: enabled ? "Toggle favorite" : ""
            Accessible.role: Accessible.Button
            Accessible.name: "Toggle favorite"
            onClicked: if (root.service && enabled) root.service.toggleFavorite()
          }
          Ui.PanelActionButton {
            id: standardChromecast
            objectName: "tidalChromecastButton"
            visible: true
            size: Style.spacing.controlHeight
            iconText: "󰄘"
            tooltipText: root.service && root.service.castActive ? ("Casting to " + (root.service.castDeviceName || "Chromecast")) : "Cast Audio"
            foreground: root.service && root.service.castActive ? Color.accent : Color.foreground
            Accessible.role: Accessible.Button
            Accessible.name: tooltipText
            onClicked: root.chromecastRequested()
          }
          Ui.PanelActionButton {
            id: standardSettings
            size: Style.spacing.controlHeight
            iconText: "󰒓"
            tooltipText: "Settings"
            Accessible.role: Accessible.Button
            Accessible.name: tooltipText
            onClicked: root.settingsRequested()
          }
        }
        Row {
          width: parent.width
          spacing: Style.spacing.xs
          height: Math.max(standardArtist.implicitHeight, standardNullSubtitle.implicitHeight)
          Text {
            id: standardNullSubtitle
            visible: !root.service || (!root.service.trackTitle && !root.service.trackLoading)
            textFormat: Text.PlainText
            text: "Select a track to start listening"
            color: Color.muted
            font.family: Style.font.family
            font.pixelSize: Style.font.caption
            elide: Text.ElideRight
            width: parent.width
          }
          Text {
            id: standardArtist
            visible: !standardNullSubtitle.visible
            textFormat: Text.PlainText
            text: root.service ? root.service.trackArtist : ""
            width: Math.min(implicitWidth, parent.width * 0.45)
            elide: Text.ElideRight
            color: Color.muted
            font.family: Style.font.family
            font.pixelSize: Style.font.caption
            font.underline: standardArtistMouse.containsMouse && !!root.service && !!root.service.trackArtist
            MouseArea { id: standardArtistMouse; Accessible.role: Accessible.Link; Accessible.name: root.service ? root.service.trackArtist : ""; anchors.fill: parent; hoverEnabled: true; cursorShape: root.service && root.service.trackArtist ? Qt.PointingHandCursor : Qt.ArrowCursor; onClicked: if (root.service && root.service.trackArtist) root.exploreArtist(root.service.currentArtistId || 0, root.service.trackArtist) }
          }
          Text {
            visible: !standardNullSubtitle.visible && !!root.service && (!!root.service.trackArtist || !!root.service.trackAlbum)
            textFormat: Text.PlainText
            text: "·"
            color: Color.muted
            font.family: Style.font.family
            font.pixelSize: Style.font.caption
          }
          Text {
            id: standardAlbum
            visible: !standardNullSubtitle.visible
            textFormat: Text.PlainText
            text: root.service ? root.service.trackAlbum : ""
            width: Math.max(0, parent.width - standardArtist.width - Style.spacing.xs * 2 - Style.spacing.sm)
            elide: Text.ElideRight
            color: Color.muted
            font.family: Style.font.family
            font.pixelSize: Style.font.caption
            font.underline: standardAlbumMouse.containsMouse && !!root.service && !!root.service.trackAlbum
            MouseArea { id: standardAlbumMouse; Accessible.role: Accessible.Link; Accessible.name: root.service ? root.service.trackAlbum : ""; anchors.fill: parent; hoverEnabled: true; cursorShape: root.service && root.service.trackAlbum ? Qt.PointingHandCursor : Qt.ArrowCursor; onClicked: if (root.service && root.service.trackAlbum) root.exploreAlbum(root.service.currentAlbumId || 0, root.service.trackAlbum) }
          }
        }
      }
    }

    Text {
      objectName: "tidalPlaybackError"
      visible: !!root.service && root.service.playbackError !== ""
      width: parent.width
      textFormat: Text.PlainText
      text: root.service ? root.service.playbackError : ""
      wrapMode: Text.WordWrap
      color: Color.urgent
      font.family: Style.font.family
      font.pixelSize: Style.font.caption
    }

    Row {
      width: parent.width
      spacing: Style.spacing.md
      opacity: root.service && (root.service.trackTitle || root.service.trackLoading) ? 1.0 : 0.4
      Text { anchors.verticalCenter: parent.verticalCenter; textFormat: Text.PlainText; text: root.formatTime(!root.service || !root.service.trackTitle ? 0 : (standardSeek.dragging ? standardSeek.liveValue * root.service.trackDuration : (root.pendingSeek >= 0 ? root.pendingSeek : root.service.trackPosition))); color: Color.muted; font.family: Style.font.family; font.pixelSize: Style.font.caption }
      Ui.PanelSlider {
        id: standardSeek
        Accessible.role: Accessible.Slider
        Accessible.name: "Seek position"
        objectName: "tidalSeekSlider"
        bar: root.bar
        enabled: !!root.service && root.service.trackDuration > 0
        width: Math.max(0, parent.width - parent.children[0].implicitWidth - parent.children[2].implicitWidth - Style.spacing.md * 2)
        value: root.progressValue
        onReleased: function(v) { if (root.service && root.service.trackDuration > 0) { root.pendingSeek = v * root.service.trackDuration; root.service.seek(root.pendingSeek) } }
        MouseArea { anchors.fill: parent; acceptedButtons: Qt.NoButton; onWheel: function(wheel) { wheel.accepted = true } }
        Rectangle {
          id: standardPulseHalo
          visible: !!root.service && root.service.trackLoading
          width: standardSeek.knobSize + Style.space(12)
          height: width
          radius: width / 2
          anchors.verticalCenter: parent.verticalCenter
          readonly property real knobCenterX: Math.max(0, Math.min(standardSeek.width - standardSeek.knobSize, standardSeek.width * standardSeek.progress - standardSeek.knobSize / 2)) + standardSeek.knobSize / 2
          x: knobCenterX - width / 2
          color: Color.accent
          opacity: 0.35
          z: 2
          SequentialAnimation on scale {
            running: standardPulseHalo.visible
            loops: Animation.Infinite
            NumberAnimation { from: 0.85; to: 1.45; duration: 650; easing.type: Easing.InOutQuad }
            NumberAnimation { from: 1.45; to: 0.85; duration: 650; easing.type: Easing.InOutQuad }
          }
          SequentialAnimation on opacity {
            running: standardPulseHalo.visible
            loops: Animation.Infinite
            NumberAnimation { from: 0.45; to: 0.1; duration: 650; easing.type: Easing.InOutQuad }
            NumberAnimation { from: 0.1; to: 0.45; duration: 650; easing.type: Easing.InOutQuad }
          }
        }
        Rectangle {
          id: standardPulseRing
          visible: standardPulseHalo.visible
          width: standardSeek.knobSize + Style.space(4)
          height: width
          radius: width / 2
          anchors.verticalCenter: parent.verticalCenter
          x: standardPulseHalo.knobCenterX - width / 2
          color: "transparent"
          border.color: Color.accent
          border.width: Style.space(2)
          z: 3
          SequentialAnimation on opacity {
            running: standardPulseRing.visible
            loops: Animation.Infinite
            NumberAnimation { from: 0.9; to: 0.3; duration: 450; easing.type: Easing.InOutSine }
            NumberAnimation { from: 0.3; to: 0.9; duration: 450; easing.type: Easing.InOutSine }
          }
        }
      }
      Text { anchors.verticalCenter: parent.verticalCenter; textFormat: Text.PlainText; text: root.formatTime(root.service && root.service.trackTitle ? root.service.trackDuration : 0); color: Color.muted; font.family: Style.font.family; font.pixelSize: Style.font.caption }
    }

    Row {
      visible: !!root.service
      anchors.horizontalCenter: parent.horizontalCenter
      spacing: Style.spacing.lg
      readonly property real controlButtonSize: Style.spacing.controlHeight + Style.spacing.md
      Ui.PanelActionButton { objectName: "tidalShuffleButton"; Accessible.role: Accessible.Button; Accessible.name: "Shuffle"; iconText: "󰒝"; tooltipText: "Shuffle"; size: parent.controlButtonSize; foreground: root.service && root.service.shuffle ? Color.accent : Color.foreground; onClicked: if (root.service) root.service.toggleShuffle() }
      Ui.PanelActionButton { objectName: "tidalPrevButton"; Accessible.role: Accessible.Button; Accessible.name: "Previous track"; iconText: "󰒮"; tooltipText: "Previous"; size: parent.controlButtonSize; onClicked: if (root.service) root.service.previous() }
      Ui.PanelActionButton { objectName: "tidalPlayButton"; Accessible.role: Accessible.Button; Accessible.name: root.isPlaying ? "Pause" : "Play"; iconText: root.isPlaying ? "󰏤" : "󰐊"; tooltipText: root.isPlaying ? "Pause" : "Play"; size: parent.controlButtonSize; foreground: Color.accent; bordered: true; onClicked: if (root.service) root.service.togglePlay() }
      Ui.PanelActionButton { objectName: "tidalNextButton"; Accessible.role: Accessible.Button; Accessible.name: "Next track"; iconText: "󰒭"; tooltipText: "Next"; size: parent.controlButtonSize; onClicked: if (root.service) root.service.next() }
      Ui.PanelActionButton { objectName: "tidalRepeatButton"; Accessible.role: Accessible.Button; Accessible.name: "Repeat"; iconText: root.service && root.service.repeatMode === "one" ? "󰑘" : "󰑖"; tooltipText: "Repeat"; size: parent.controlButtonSize; foreground: root.service && root.service.repeatMode !== "off" ? Color.accent : Color.foreground; onClicked: if (root.service) root.service.cycleRepeat() }
    }

    Row {
      width: parent.width
      spacing: Style.spacing.md
      visible: (!!root.sink && !!root.sink.audio) || (!!root.service && root.service.castActive)
      Ui.PanelActionButton { Accessible.role: Accessible.Button; Accessible.name: "Mute audio"; iconText: root.sink && root.sink.audio && root.sink.audio.muted ? "󰖁" : "󰕾"; tooltipText: "Mute audio"; onClicked: root.toggleMute() }
      Ui.PanelSlider {
        id: standardVolume
        Accessible.role: Accessible.Slider
        Accessible.name: "Volume"
        bar: root.bar
        width: Math.max(0, parent.width - parent.children[0].implicitWidth - standardVolumeLabel.implicitWidth - Style.spacing.md * 2)
        maximum: root.service && root.service.castActive ? 1.0 : 1.5
        value: root.service && root.service.castActive ? root.service.castVolume : (root.sink && root.sink.audio ? root.sink.audio.volume : 0)
        tickCount: 4
        onMoved: function(v) { root.setVolume(v) }
        onReleased: function(v) { root.setVolume(v) }
        onRightClicked: root.toggleMute()
        WheelHandler {
          acceptedDevices: PointerDevice.Mouse
          onWheel: function(event) { root.handleWheelVolume(event.angleDelta.y); event.accepted = true }
        }
      }
      Text { id: standardVolumeLabel; anchors.verticalCenter: parent.verticalCenter; textFormat: Text.PlainText; text: Math.round((standardVolume.dragging ? standardVolume.liveValue : standardVolume.value) * 100) + "%"; color: Color.muted; font.family: Style.font.family; font.pixelSize: Style.font.caption }
    }
  }

  // =========================================================================
  // Style 3: Audiophile Turntable (Spinning Vinyl & Provenance Badge)
  // =========================================================================
  Column {
    id: turntableCard
    visible: root.currentStyle === "turntable"
    width: parent.width
    height: visible ? implicitHeight : 0
    spacing: Style.spacing.md

    Row {
      width: parent.width
      spacing: Style.spacing.lg

      // Spinning Vinyl Record
      Item {
        width: Style.space(110)
        height: width
        anchors.verticalCenter: parent.verticalCenter

        Rectangle {
          id: vinylDisc
          anchors.fill: parent
          radius: width / 2
          color: Style.hoverFill
          border.color: Color.muted
          border.width: 1

          RotationAnimation on rotation {
            from: 0
            to: 360
            duration: 3300
            loops: Animation.Infinite
            running: root.isPlaying
          }

          // Concentric groove rings
          Rectangle {
            width: parent.width * 0.82
            height: width
            radius: width / 2
            anchors.centerIn: parent
            color: "transparent"
            border.color: Color.muted
            border.width: 1
            opacity: 0.35
          }
          Rectangle {
            width: parent.width * 0.66
            height: width
            radius: width / 2
            anchors.centerIn: parent
            color: "transparent"
            border.color: Color.muted
            border.width: 1
            opacity: 0.25
          }

          // Center Label Artwork
          Rectangle {
            width: parent.width * 0.42
            height: width
            radius: width / 2
            anchors.centerIn: parent
            color: Color.accent
            clip: true

            Image {
              anchors.fill: parent
              visible: root.localArtSource !== ""
              source: root.localArtSource
              sourceSize: Qt.size(width * Screen.devicePixelRatio, height * Screen.devicePixelRatio)
              fillMode: Image.PreserveAspectCrop
              asynchronous: true
            }
            Text {
              anchors.centerIn: parent
              visible: root.localArtSource === ""
              textFormat: Text.PlainText
              text: "󰝚"
              color: Color.popups.background
              font.family: Style.font.family
              font.pixelSize: Style.font.body
            }
          }

          // Center Spindle Hole
          Rectangle {
            width: Style.space(8)
            height: width
            radius: width / 2
            anchors.centerIn: parent
            color: Color.popups.background
            border.color: Color.accent
            border.width: 1
          }
        }
      }

      // Deck Info Column
      Column {
        width: Math.max(0, parent.width - Style.space(110) - Style.spacing.lg)
        anchors.verticalCenter: parent.verticalCenter
        spacing: Style.spacing.xs

        Row {
          width: parent.width
          Text {
            textFormat: Text.PlainText
            text: "󰓃 TURNTABLE DECK"
            color: Color.accent
            font.family: Style.font.family
            font.pixelSize: Style.font.caption
            font.bold: true
            anchors.verticalCenter: parent.verticalCenter
          }
          Item { width: Math.max(0, parent.width - parent.children[0].implicitWidth - deckActions.implicitWidth); height: 1 }
          Row {
            id: deckActions
            spacing: Style.spacing.xs
            anchors.verticalCenter: parent.verticalCenter
            Ui.PanelActionButton { size: Style.spacing.controlHeight; iconText: root.isFav ? "󰋑" : "󰋕"; foreground: root.isFav ? Color.accent : Color.foreground; tooltipText: "Favorite"; onClicked: if (root.service) root.service.toggleFavorite() }
            Ui.PanelActionButton { size: Style.spacing.controlHeight; iconText: "󰄘"; foreground: root.service && root.service.castActive ? Color.accent : Color.foreground; tooltipText: "Cast"; onClicked: root.chromecastRequested() }
            Ui.PanelActionButton { size: Style.spacing.controlHeight; iconText: "󰒓"; tooltipText: "Settings"; onClicked: root.settingsRequested() }
          }
        }

        Text {
          width: parent.width
          textFormat: Text.PlainText
          text: root.service && root.service.trackTitle ? root.service.trackTitle : "Nothing playing"
          color: root.service && root.service.trackTitle ? Color.foreground : Color.muted
          font.family: Style.font.family
          font.pixelSize: Style.font.title
          font.bold: true
          elide: Text.ElideRight
        }

        Row {
          width: parent.width
          spacing: Style.spacing.xs
          Text {
            textFormat: Text.PlainText
            text: root.service ? (root.service.trackArtist || "Unknown artist") : ""
            font.family: Style.font.family
            font.pixelSize: Style.font.caption
            color: Color.muted
            elide: Text.ElideRight
            font.underline: ttArtistMouse.containsMouse
            MouseArea { id: ttArtistMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: if (root.service && root.service.trackArtist) root.exploreArtist(root.service.currentArtistId || 0, root.service.trackArtist) }
          }
          Text { visible: !!root.service && !!root.service.trackAlbum; textFormat: Text.PlainText; text: "·"; color: Color.muted; font.pixelSize: Style.font.caption }
          Text {
            visible: !!root.service && !!root.service.trackAlbum
            textFormat: Text.PlainText
            text: root.service ? root.service.trackAlbum : ""
            font.family: Style.font.family
            font.pixelSize: Style.font.caption
            color: Color.muted
            elide: Text.ElideRight
            font.underline: ttAlbumMouse.containsMouse
            MouseArea { id: ttAlbumMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: if (root.service && root.service.trackAlbum) root.exploreAlbum(root.service.currentAlbumId || 0, root.service.trackAlbum) }
          }
        }

        // Fidelity & Provenance Badge
        Rectangle {
          width: parent.width
          height: Style.space(22)
          radius: Style.cornerRadius
          color: Style.hoverFill
          border.color: Color.accent
          border.width: Style.normalBorderWidth

          Text {
            anchors.centerIn: parent
            textFormat: Text.PlainText
            text: root.qualityText
            color: Color.accent
            font.family: Style.font.family
            font.pixelSize: Style.font.caption
            font.bold: true
          }
        }
      }
    }

    // Scrubber
    Row {
      width: parent.width
      spacing: Style.spacing.md
      Text { anchors.verticalCenter: parent.verticalCenter; textFormat: Text.PlainText; text: root.formatTime(ttSeek.dragging ? ttSeek.liveValue * root.duration : root.currentPos); color: Color.muted; font.family: Style.font.family; font.pixelSize: Style.font.caption }
      Ui.PanelSlider {
        id: ttSeek
        bar: root.bar
        enabled: !!root.service && root.duration > 0
        width: Math.max(0, parent.width - parent.children[0].implicitWidth - parent.children[2].implicitWidth - Style.spacing.md * 2)
        value: root.progressValue
        onReleased: function(v) { if (root.service && root.duration > 0) { root.pendingSeek = v * root.duration; root.service.seek(root.pendingSeek) } }
        MouseArea { anchors.fill: parent; acceptedButtons: Qt.NoButton; onWheel: function(wheel) { wheel.accepted = true } }
      }
      Text { anchors.verticalCenter: parent.verticalCenter; textFormat: Text.PlainText; text: root.formatTime(root.duration); color: Color.muted; font.family: Style.font.family; font.pixelSize: Style.font.caption }
    }

    // Tactile Transport
    Row {
      anchors.horizontalCenter: parent.horizontalCenter
      spacing: Style.spacing.lg
      readonly property real controlButtonSize: Style.spacing.controlHeight + Style.spacing.md
      Ui.PanelActionButton { iconText: "󰒝"; tooltipText: "Shuffle"; size: parent.controlButtonSize; foreground: root.service && root.service.shuffle ? Color.accent : Color.foreground; onClicked: if (root.service) root.service.toggleShuffle() }
      Ui.PanelActionButton { iconText: "󰒮"; tooltipText: "Previous"; size: parent.controlButtonSize; onClicked: if (root.service) root.service.previous() }
      Ui.PanelActionButton { iconText: root.isPlaying ? "󰏤" : "󰐊"; tooltipText: root.isPlaying ? "Pause" : "Play"; size: parent.controlButtonSize; foreground: Color.accent; bordered: true; onClicked: if (root.service) root.service.togglePlay() }
      Ui.PanelActionButton { iconText: "󰒭"; tooltipText: "Next"; size: parent.controlButtonSize; onClicked: if (root.service) root.service.next() }
      Ui.PanelActionButton { iconText: root.service && root.service.repeatMode === "one" ? "󰑘" : "󰑖"; tooltipText: "Repeat"; size: parent.controlButtonSize; foreground: root.service && root.service.repeatMode !== "off" ? Color.accent : Color.foreground; onClicked: if (root.service) root.service.cycleRepeat() }
    }

    // Volume
    Row {
      width: parent.width
      spacing: Style.spacing.md
      Ui.PanelActionButton { iconText: root.sink && root.sink.audio && root.sink.audio.muted ? "󰖁" : "󰕾"; tooltipText: "Mute"; onClicked: root.toggleMute() }
      Ui.PanelSlider {
        id: ttVolume
        bar: root.bar
        width: Math.max(0, parent.width - parent.children[0].implicitWidth - ttVolumeLabel.implicitWidth - Style.spacing.md * 2)
        maximum: root.service && root.service.castActive ? 1.0 : 1.5
        value: root.service && root.service.castActive ? root.service.castVolume : (root.sink && root.sink.audio ? root.sink.audio.volume : 0)
        tickCount: 4
        onMoved: function(v) { root.setVolume(v) }
        onReleased: function(v) { root.setVolume(v) }
        onRightClicked: root.toggleMute()
        WheelHandler { acceptedDevices: PointerDevice.Mouse; onWheel: function(e) { root.handleWheelVolume(e.angleDelta.y); e.accepted = true } }
      }
      Text { id: ttVolumeLabel; anchors.verticalCenter: parent.verticalCenter; textFormat: Text.PlainText; text: Math.round((ttVolume.dragging ? ttVolume.liveValue : ttVolume.value) * 100) + "%"; color: Color.muted; font.family: Style.font.family; font.pixelSize: Style.font.caption }
    }
  }

  // =========================================================================
  // Style 4: Showcase (Large Cover Canvas)
  // =========================================================================
  Column {
    id: showcaseCard
    visible: root.currentStyle === "showcase"
    width: parent.width
    height: visible ? implicitHeight : 0
    spacing: Style.spacing.md

    // Large Cover Canvas
    Rectangle {
      width: Math.min(parent.width - Style.spacing.xl * 2, Style.space(210))
      height: width
      anchors.horizontalCenter: parent.horizontalCenter
      radius: Style.cornerRadius * 1.5
      color: Style.hoverFill
      border.color: Color.muted
      border.width: Style.normalBorderWidth
      clip: true

      Image {
        anchors.fill: parent
        visible: root.localArtSource !== ""
        source: root.localArtSource
        sourceSize: Qt.size(width * Screen.devicePixelRatio, height * Screen.devicePixelRatio)
        fillMode: Image.PreserveAspectCrop
        asynchronous: true
      }
      Text {
        anchors.centerIn: parent
        visible: root.localArtSource === ""
        textFormat: Text.PlainText
        text: "󰝚"
        color: Color.muted
        font.family: Style.font.family
        font.pixelSize: Style.space(48)
      }

      // Quality badge overlay in top-right
      Rectangle {
        anchors.top: parent.top
        anchors.right: parent.right
        anchors.margins: Style.spacing.sm
        height: Style.space(20)
        width: scBadgeText.implicitWidth + Style.spacing.md * 2
        radius: height / 2
        color: Color.popups.background
        opacity: 0.92
        border.color: Color.accent
        border.width: 1

        Text {
          id: scBadgeText
          anchors.centerIn: parent
          textFormat: Text.PlainText
          text: root.provenanceShortBadge
          color: Color.accent
          font.family: Style.font.family
          font.pixelSize: Style.font.caption
          font.bold: true
        }
      }
    }

    // Centered metadata
    Column {
      width: parent.width
      spacing: 2
      Text {
        width: parent.width
        horizontalAlignment: Text.AlignHCenter
        textFormat: Text.PlainText
        text: root.service && root.service.trackTitle ? root.service.trackTitle : "Nothing playing"
        color: root.service && root.service.trackTitle ? Color.foreground : Color.muted
        font.family: Style.font.family
        font.pixelSize: Style.font.title + 2
        font.bold: true
        elide: Text.ElideRight
      }
      Row {
        anchors.horizontalCenter: parent.horizontalCenter
        spacing: Style.spacing.xs
        Text {
          textFormat: Text.PlainText
          text: root.service ? (root.service.trackArtist || "Unknown artist") : ""
          font.family: Style.font.family
          font.pixelSize: Style.font.body
          color: Color.muted
          font.underline: scArtistMouse.containsMouse
          MouseArea { id: scArtistMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: if (root.service && root.service.trackArtist) root.exploreArtist(root.service.currentArtistId || 0, root.service.trackArtist) }
        }
        Text { visible: !!root.service && !!root.service.trackAlbum; textFormat: Text.PlainText; text: "·"; color: Color.muted; font.pixelSize: Style.font.body }
        Text {
          visible: !!root.service && !!root.service.trackAlbum
          textFormat: Text.PlainText
          text: root.service ? root.service.trackAlbum : ""
          font.family: Style.font.family
          font.pixelSize: Style.font.body
          color: Color.muted
          font.underline: scAlbumMouse.containsMouse
          MouseArea { id: scAlbumMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: if (root.service && root.service.trackAlbum) root.exploreAlbum(root.service.currentAlbumId || 0, root.service.trackAlbum) }
        }
      }
    }

    // Centered Quick Action Buttons
    Row {
      anchors.horizontalCenter: parent.horizontalCenter
      spacing: Style.spacing.md
      Ui.PanelActionButton { size: Style.spacing.controlHeight; iconText: root.isFav ? "󰋑" : "󰋕"; foreground: root.isFav ? Color.accent : Color.foreground; tooltipText: "Favorite"; onClicked: if (root.service) root.service.toggleFavorite() }
      Ui.PanelActionButton { size: Style.spacing.controlHeight; iconText: "󰄘"; foreground: root.service && root.service.castActive ? Color.accent : Color.foreground; tooltipText: "Cast"; onClicked: root.chromecastRequested() }
      Ui.PanelActionButton { size: Style.spacing.controlHeight; iconText: "󰒓"; tooltipText: "Settings"; onClicked: root.settingsRequested() }
    }

    // Scrubber
    Row {
      width: parent.width
      spacing: Style.spacing.md
      Text { anchors.verticalCenter: parent.verticalCenter; textFormat: Text.PlainText; text: root.formatTime(scSeek.dragging ? scSeek.liveValue * root.duration : root.currentPos); color: Color.muted; font.family: Style.font.family; font.pixelSize: Style.font.caption }
      Ui.PanelSlider {
        id: scSeek
        bar: root.bar
        enabled: !!root.service && root.duration > 0
        width: Math.max(0, parent.width - parent.children[0].implicitWidth - parent.children[2].implicitWidth - Style.spacing.md * 2)
        value: root.progressValue
        onReleased: function(v) { if (root.service && root.duration > 0) { root.pendingSeek = v * root.duration; root.service.seek(root.pendingSeek) } }
        MouseArea { anchors.fill: parent; acceptedButtons: Qt.NoButton; onWheel: function(wheel) { wheel.accepted = true } }
      }
      Text { anchors.verticalCenter: parent.verticalCenter; textFormat: Text.PlainText; text: root.formatTime(root.duration); color: Color.muted; font.family: Style.font.family; font.pixelSize: Style.font.caption }
    }

    // Floating Pill Transport
    Rectangle {
      height: Style.spacing.controlHeight + Style.spacing.md
      width: scTransportRow.implicitWidth + Style.spacing.lg
      radius: height / 2
      color: Style.normalFill
      border.color: Color.muted
      border.width: Style.normalBorderWidth
      anchors.horizontalCenter: parent.horizontalCenter

      Row {
        id: scTransportRow
        anchors.centerIn: parent
        spacing: Style.spacing.md
        readonly property real controlButtonSize: Style.spacing.controlHeight
        Ui.PanelActionButton { iconText: "󰒝"; tooltipText: "Shuffle"; size: parent.controlButtonSize; foreground: root.service && root.service.shuffle ? Color.accent : Color.foreground; onClicked: if (root.service) root.service.toggleShuffle() }
        Ui.PanelActionButton { iconText: "󰒮"; tooltipText: "Previous"; size: parent.controlButtonSize; onClicked: if (root.service) root.service.previous() }
        Ui.PanelActionButton { iconText: root.isPlaying ? "󰏤" : "󰐊"; tooltipText: root.isPlaying ? "Pause" : "Play"; size: parent.controlButtonSize; foreground: Color.accent; bordered: true; onClicked: if (root.service) root.service.togglePlay() }
        Ui.PanelActionButton { iconText: "󰒭"; tooltipText: "Next"; size: parent.controlButtonSize; onClicked: if (root.service) root.service.next() }
        Ui.PanelActionButton { iconText: root.service && root.service.repeatMode === "one" ? "󰑘" : "󰑖"; tooltipText: "Repeat"; size: parent.controlButtonSize; foreground: root.service && root.service.repeatMode !== "off" ? Color.accent : Color.foreground; onClicked: if (root.service) root.service.cycleRepeat() }
      }
    }

    // Volume
    Row {
      width: parent.width
      spacing: Style.spacing.md
      Ui.PanelActionButton { iconText: root.sink && root.sink.audio && root.sink.audio.muted ? "󰖁" : "󰕾"; tooltipText: "Mute"; onClicked: root.toggleMute() }
      Ui.PanelSlider {
        id: scVolume
        bar: root.bar
        width: Math.max(0, parent.width - parent.children[0].implicitWidth - scVolumeLabel.implicitWidth - Style.spacing.md * 2)
        maximum: root.service && root.service.castActive ? 1.0 : 1.5
        value: root.service && root.service.castActive ? root.service.castVolume : (root.sink && root.sink.audio ? root.sink.audio.volume : 0)
        tickCount: 4
        onMoved: function(v) { root.setVolume(v) }
        onReleased: function(v) { root.setVolume(v) }
        onRightClicked: root.toggleMute()
        WheelHandler { acceptedDevices: PointerDevice.Mouse; onWheel: function(e) { root.handleWheelVolume(e.angleDelta.y); e.accepted = true } }
      }
      Text { id: scVolumeLabel; anchors.verticalCenter: parent.verticalCenter; textFormat: Text.PlainText; text: Math.round((scVolume.dragging ? scVolume.liveValue : scVolume.value) * 100) + "%"; color: Color.muted; font.family: Style.font.family; font.pixelSize: Style.font.caption }
    }
  }

  // =========================================================================
  // Style 8: Swiss Zen (Pure Typography - Zero Art)
  // =========================================================================
  Column {
    id: zenCard
    visible: root.currentStyle === "zen"
    width: parent.width
    height: visible ? implicitHeight : 0
    spacing: Style.spacing.md

    // Top Tag & Actions
    Row {
      width: parent.width
      Text {
        textFormat: Text.PlainText
        text: "NOW PLAYING  ·  " + (root.service ? root.service.audioQuality : "LOSSLESS")
        color: Color.accent
        font.family: Style.font.family
        font.pixelSize: Style.font.caption
        font.bold: true
        anchors.verticalCenter: parent.verticalCenter
      }
      Item { width: Math.max(0, parent.width - parent.children[0].implicitWidth - zenActions.implicitWidth); height: 1 }
      Row {
        id: zenActions
        spacing: Style.spacing.xs
        anchors.verticalCenter: parent.verticalCenter
        Ui.PanelActionButton { size: Style.spacing.controlHeight; iconText: root.isFav ? "󰋑" : "󰋕"; foreground: root.isFav ? Color.accent : Color.foreground; tooltipText: "Favorite"; onClicked: if (root.service) root.service.toggleFavorite() }
        Ui.PanelActionButton { size: Style.spacing.controlHeight; iconText: "󰄘"; foreground: root.service && root.service.castActive ? Color.accent : Color.foreground; tooltipText: "Cast"; onClicked: root.chromecastRequested() }
        Ui.PanelActionButton { size: Style.spacing.controlHeight; iconText: "󰒓"; tooltipText: "Settings"; onClicked: root.settingsRequested() }
      }
    }

    // Typographic Heading Section
    Column {
      width: parent.width
      spacing: Style.spacing.xs
      Text {
        width: parent.width
        textFormat: Text.PlainText
        text: root.service && root.service.trackTitle ? root.service.trackTitle : "Nothing playing"
        color: root.service && root.service.trackTitle ? Color.foreground : Color.muted
        font.family: Style.font.family
        font.pixelSize: Style.font.title + 4
        font.bold: true
        elide: Text.ElideRight
      }
      Row {
        width: parent.width
        spacing: Style.spacing.xs
        Text {
          textFormat: Text.PlainText
          text: root.service ? (root.service.trackArtist || "Unknown artist") : ""
          font.family: Style.font.family
          font.pixelSize: Style.font.body
          color: Color.muted
          font.underline: zenArtistMouse.containsMouse
          MouseArea { id: zenArtistMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: if (root.service && root.service.trackArtist) root.exploreArtist(root.service.currentArtistId || 0, root.service.trackArtist) }
        }
        Text { visible: !!root.service && !!root.service.trackAlbum; textFormat: Text.PlainText; text: "·"; color: Color.muted; font.pixelSize: Style.font.body }
        Text {
          visible: !!root.service && !!root.service.trackAlbum
          textFormat: Text.PlainText
          text: root.service ? root.service.trackAlbum : ""
          font.family: Style.font.family
          font.pixelSize: Style.font.body
          color: Color.muted
          font.underline: zenAlbumMouse.containsMouse
          MouseArea { id: zenAlbumMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: if (root.service && root.service.trackAlbum) root.exploreAlbum(root.service.currentAlbumId || 0, root.service.trackAlbum) }
        }
      }
    }

    // Hairline Separator
    Rectangle {
      width: parent.width
      height: 1
      color: Color.muted
      opacity: 0.35
    }

    // Hairline Scrubber
    Row {
      width: parent.width
      spacing: Style.spacing.md
      Text { anchors.verticalCenter: parent.verticalCenter; textFormat: Text.PlainText; text: root.formatTime(zenSeek.dragging ? zenSeek.liveValue * root.duration : root.currentPos) + " / " + root.formatTime(root.duration); color: Color.muted; font.family: Style.font.family; font.pixelSize: Style.font.caption }
      Ui.PanelSlider {
        id: zenSeek
        bar: root.bar
        enabled: !!root.service && root.duration > 0
        width: Math.max(0, parent.width - parent.children[0].implicitWidth - Style.spacing.md)
        value: root.progressValue
        onReleased: function(v) { if (root.service && root.duration > 0) { root.pendingSeek = v * root.duration; root.service.seek(root.pendingSeek) } }
        MouseArea { anchors.fill: parent; acceptedButtons: Qt.NoButton; onWheel: function(wheel) { wheel.accepted = true } }
      }
    }

    // Typographic Transport
    Row {
      anchors.horizontalCenter: parent.horizontalCenter
      spacing: Style.spacing.xl
      readonly property real controlButtonSize: Style.spacing.controlHeight + Style.spacing.sm
      Ui.PanelActionButton { iconText: "󰒝"; tooltipText: "Shuffle"; size: parent.controlButtonSize; foreground: root.service && root.service.shuffle ? Color.accent : Color.foreground; onClicked: if (root.service) root.service.toggleShuffle() }
      Ui.PanelActionButton { iconText: "󰒮"; tooltipText: "Previous"; size: parent.controlButtonSize; onClicked: if (root.service) root.service.previous() }
      Ui.PanelActionButton { iconText: root.isPlaying ? "󰏤" : "󰐊"; tooltipText: root.isPlaying ? "Pause" : "Play"; size: parent.controlButtonSize; foreground: Color.accent; bordered: true; onClicked: if (root.service) root.service.togglePlay() }
      Ui.PanelActionButton { iconText: "󰒭"; tooltipText: "Next"; size: parent.controlButtonSize; onClicked: if (root.service) root.service.next() }
      Ui.PanelActionButton { iconText: root.service && root.service.repeatMode === "one" ? "󰑘" : "󰑖"; tooltipText: "Repeat"; size: parent.controlButtonSize; foreground: root.service && root.service.repeatMode !== "off" ? Color.accent : Color.foreground; onClicked: if (root.service) root.service.cycleRepeat() }
    }

    // Minimal Volume
    Row {
      width: parent.width
      spacing: Style.spacing.md
      Ui.PanelActionButton { iconText: root.sink && root.sink.audio && root.sink.audio.muted ? "󰖁" : "󰕾"; tooltipText: "Mute"; onClicked: root.toggleMute() }
      Ui.PanelSlider {
        id: zenVolume
        bar: root.bar
        width: Math.max(0, parent.width - parent.children[0].implicitWidth - zenVolumeLabel.implicitWidth - Style.spacing.md * 2)
        maximum: root.service && root.service.castActive ? 1.0 : 1.5
        value: root.service && root.service.castActive ? root.service.castVolume : (root.sink && root.sink.audio ? root.sink.audio.volume : 0)
        tickCount: 4
        onMoved: function(v) { root.setVolume(v) }
        onReleased: function(v) { root.setVolume(v) }
        onRightClicked: root.toggleMute()
        WheelHandler { acceptedDevices: PointerDevice.Mouse; onWheel: function(e) { root.handleWheelVolume(e.angleDelta.y); e.accepted = true } }
      }
      Text { id: zenVolumeLabel; anchors.verticalCenter: parent.verticalCenter; textFormat: Text.PlainText; text: Math.round((zenVolume.dragging ? zenVolume.liveValue : zenVolume.value) * 100) + "%"; color: Color.muted; font.family: Style.font.family; font.pixelSize: Style.font.caption }
    }
  }
}
