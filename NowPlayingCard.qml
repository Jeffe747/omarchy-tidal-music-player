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

  width: parent ? parent.width : implicitWidth
  spacing: Style.spacing.lg
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
        id: cover
        objectName: "tidalNowPlayingArt"
        anchors.fill: parent
        visible: !!root.service && !!root.service.trackArtUrl
        source: root.service && root.service.trackArtUrl ? root.service.trackArtUrl : ""
        sourceSize: Qt.size(width * Screen.devicePixelRatio, height * Screen.devicePixelRatio)
        fillMode: Image.PreserveAspectCrop
        asynchronous: true
      }
      Text { anchors.centerIn: parent; visible: !cover.visible || cover.status !== Image.Ready; textFormat: Text.PlainText; text: "󰝚"; color: Color.muted; font.family: Style.font.family; font.pixelSize: Style.font.title }
    }
    Column {
      width: Math.max(0, parent.width - Style.space(60) - Style.spacing.lg)
      anchors.verticalCenter: parent.verticalCenter
      spacing: Style.spacing.xs
      Row {
        width: parent.width
        spacing: Style.spacing.sm
        Text {
          id: title
          objectName: "tidalNowPlayingTitle"
          width: Math.max(0, parent.width - settings.implicitWidth - (chromecast.visible ? chromecast.implicitWidth + Style.spacing.sm : 0) - (favorite.visible ? favorite.implicitWidth + Style.spacing.sm : 0) - Style.spacing.sm)
          textFormat: Text.PlainText
          text: root.service && root.service.trackTitle ? root.service.trackTitle : (root.service && root.service.trackLoading ? "Loading track…" : "Nothing playing")
          color: root.service && (root.service.trackTitle || root.service.trackLoading) ? Color.foreground : Color.muted
          font.family: Style.font.family
          font.pixelSize: Style.font.title
          font.bold: true
          elide: Text.ElideRight
        }
        Ui.PanelActionButton {
          id: favorite
          visible: true
          enabled: !!root.service && !!root.service.trackTitle
          size: Style.spacing.controlHeight
          iconText: root.service && root.service.currentTrackFavorite ? "󰋑" : "󰋕"
          foreground: enabled ? (root.service && root.service.currentTrackFavorite ? Color.accent : Color.foreground) : Color.muted
          opacity: enabled ? 1.0 : 0.4
          tooltipText: enabled ? "Toggle favorite" : ""
          Accessible.role: Accessible.Button
          Accessible.name: "Toggle favorite"
          onClicked: if (root.service && enabled) root.service.toggleFavorite()
        }
        Ui.PanelActionButton {
          id: chromecast
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
          id: settings
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
        height: Math.max(artist.implicitHeight, nullSubtitle.implicitHeight)
        Text {
          id: nullSubtitle
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
          id: artist
          visible: !nullSubtitle.visible
          textFormat: Text.PlainText
          text: root.service ? root.service.trackArtist : ""
          width: Math.min(implicitWidth, parent.width * 0.45)
          elide: Text.ElideRight
          color: Color.muted
          font.family: Style.font.family
          font.pixelSize: Style.font.caption
          font.underline: artistMouse.containsMouse && !!root.service && !!root.service.trackArtist
          MouseArea { id: artistMouse; Accessible.role: Accessible.Link; Accessible.name: root.service ? root.service.trackArtist : ""; anchors.fill: parent; hoverEnabled: true; cursorShape: root.service && root.service.trackArtist ? Qt.PointingHandCursor : Qt.ArrowCursor; onClicked: if (root.service && root.service.trackArtist) root.exploreArtist(root.service.currentArtistId || 0, root.service.trackArtist) }
        }
        Text {
          visible: !nullSubtitle.visible && !!root.service && (!!root.service.trackArtist || !!root.service.trackAlbum)
          textFormat: Text.PlainText
          text: "·"
          color: Color.muted
          font.family: Style.font.family
          font.pixelSize: Style.font.caption
        }
        Text {
          id: album
          visible: !nullSubtitle.visible
          textFormat: Text.PlainText
          text: root.service ? root.service.trackAlbum : ""
          width: Math.max(0, parent.width - artist.width - Style.spacing.xs * 2 - Style.spacing.sm)
          elide: Text.ElideRight
          color: Color.muted
          font.family: Style.font.family
          font.pixelSize: Style.font.caption
          font.underline: albumMouse.containsMouse && !!root.service && !!root.service.trackAlbum
          MouseArea { id: albumMouse; Accessible.role: Accessible.Link; Accessible.name: root.service ? root.service.trackAlbum : ""; anchors.fill: parent; hoverEnabled: true; cursorShape: root.service && root.service.trackAlbum ? Qt.PointingHandCursor : Qt.ArrowCursor; onClicked: if (root.service && root.service.trackAlbum) root.exploreAlbum(root.service.currentAlbumId || 0, root.service.trackAlbum) }
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
    Text { anchors.verticalCenter: parent.verticalCenter; textFormat: Text.PlainText; text: root.formatTime(!root.service || !root.service.trackTitle ? 0 : (seek.dragging ? seek.liveValue * root.service.trackDuration : (root.pendingSeek >= 0 ? root.pendingSeek : root.service.trackPosition))); color: Color.muted; font.family: Style.font.family; font.pixelSize: Style.font.caption }
    Ui.PanelSlider {
      id: seek
      Accessible.role: Accessible.Slider
      Accessible.name: "Seek position"
      objectName: "tidalSeekSlider"
      bar: root.bar
      enabled: !!root.service && root.service.trackDuration > 0
      width: Math.max(0, parent.width - parent.children[0].implicitWidth - parent.children[2].implicitWidth - Style.spacing.md * 2)
      value: root.service && root.service.trackDuration > 0 ? Math.max(0, Math.min(1, root.service.trackPosition / root.service.trackDuration)) : 0
      onReleased: function(v) { if (root.service && root.service.trackDuration > 0) { root.pendingSeek = v * root.service.trackDuration; root.service.seek(root.pendingSeek) } }
      // PanelSlider's wheel handler lives inside the component. Disable it here.
      MouseArea { anchors.fill: parent; acceptedButtons: Qt.NoButton; onWheel: function(wheel) { wheel.accepted = true } }
      Rectangle {
        id: pulseHalo
        visible: !!root.service && root.service.trackLoading
        width: seek.knobSize + Style.space(12)
        height: width
        radius: width / 2
        anchors.verticalCenter: parent.verticalCenter
        readonly property real knobCenterX: Math.max(0, Math.min(seek.width - seek.knobSize, seek.width * seek.progress - seek.knobSize / 2)) + seek.knobSize / 2
        x: knobCenterX - width / 2
        color: Color.accent
        opacity: 0.35
        z: 2
        SequentialAnimation on scale {
          running: pulseHalo.visible
          loops: Animation.Infinite
          NumberAnimation { from: 0.85; to: 1.45; duration: 650; easing.type: Easing.InOutQuad }
          NumberAnimation { from: 1.45; to: 0.85; duration: 650; easing.type: Easing.InOutQuad }
        }
        SequentialAnimation on opacity {
          running: pulseHalo.visible
          loops: Animation.Infinite
          NumberAnimation { from: 0.45; to: 0.1; duration: 650; easing.type: Easing.InOutQuad }
          NumberAnimation { from: 0.1; to: 0.45; duration: 650; easing.type: Easing.InOutQuad }
        }
      }
      Rectangle {
        id: pulseRing
        visible: pulseHalo.visible
        width: seek.knobSize + Style.space(4)
        height: width
        radius: width / 2
        anchors.verticalCenter: parent.verticalCenter
        x: pulseHalo.knobCenterX - width / 2
        color: "transparent"
        border.color: Color.accent
        border.width: Style.space(2)
        z: 3
        SequentialAnimation on opacity {
          running: pulseRing.visible
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
    Ui.PanelActionButton { objectName: "tidalPlayButton"; Accessible.role: Accessible.Button; Accessible.name: root.service && root.service.isPlaying ? "Pause" : "Play"; iconText: root.service && root.service.isPlaying ? "󰏤" : "󰐊"; tooltipText: root.service && root.service.isPlaying ? "Pause" : "Play"; size: parent.controlButtonSize; foreground: Color.accent; bordered: true; onClicked: if (root.service) root.service.togglePlay() }
    Ui.PanelActionButton { objectName: "tidalNextButton"; Accessible.role: Accessible.Button; Accessible.name: "Next track"; iconText: "󰒭"; tooltipText: "Next"; size: parent.controlButtonSize; onClicked: if (root.service) root.service.next() }
    Ui.PanelActionButton { objectName: "tidalRepeatButton"; Accessible.role: Accessible.Button; Accessible.name: "Repeat"; iconText: root.service && root.service.repeatMode === "one" ? "󰑘" : "󰑖"; tooltipText: "Repeat"; size: parent.controlButtonSize; foreground: root.service && root.service.repeatMode !== "off" ? Color.accent : Color.foreground; onClicked: if (root.service) root.service.cycleRepeat() }
  }
  Row {
    width: parent.width
    spacing: Style.spacing.md
    visible: !!root.sink && !!root.sink.audio
    Ui.PanelActionButton { Accessible.role: Accessible.Button; Accessible.name: "Mute audio"; iconText: root.sink && root.sink.audio && root.sink.audio.muted ? "󰖁" : "󰕾"; tooltipText: "Mute audio"; onClicked: if (root.sink && root.sink.audio) root.sink.audio.muted = !root.sink.audio.muted }
    Ui.PanelSlider {
      id: volume
      Accessible.role: Accessible.Slider
      Accessible.name: "Volume"
      bar: root.bar
      width: Math.max(0, parent.width - parent.children[0].implicitWidth - volumeLabel.implicitWidth - Style.spacing.md * 2)
      maximum: 1.5
      value: root.sink && root.sink.audio ? root.sink.audio.volume : 0
      tickCount: 4
      onMoved: function(v) {
        if (root.service && root.service.castActive) {
          root.service.setCastVolume(Math.min(1.0, v))
        } else if (root.sink && root.sink.audio) {
          root.sink.audio.volume = v
        }
      }
      onRightClicked: if (root.sink && root.sink.audio) root.sink.audio.muted = !root.sink.audio.muted
      WheelHandler {
        acceptedDevices: PointerDevice.Mouse
        onWheel: function(event) {
          root.wheelRemainder += event.angleDelta.y
          var steps = Math.trunc(root.wheelRemainder / 120)
          if (steps) {
            root.wheelRemainder -= steps * 120
            if (root.service && root.service.castActive) {
              var castVol = Math.max(0, Math.min(1.0, volume.value + steps * 0.05))
              root.service.setCastVolume(castVol)
            } else if (root.sink && root.sink.audio) {
              var newVol = Math.max(0, Math.min(1.5, root.sink.audio.volume + steps * 0.05))
              root.sink.audio.volume = newVol
            }
          }
          event.accepted = true
        }
      }
    }
    Text { id: volumeLabel; anchors.verticalCenter: parent.verticalCenter; textFormat: Text.PlainText; text: Math.round((volume.dragging ? volume.liveValue : volume.value) * 100) + "%"; color: Color.muted; font.family: Style.font.family; font.pixelSize: Style.font.caption }
  }
}
