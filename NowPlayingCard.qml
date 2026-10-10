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
      visible: !!root.service && !!root.service.trackTitle
      width: visible ? Style.space(60) : 0
      height: width
      radius: Style.cornerRadius
      color: Style.hoverFill
      clip: true
      Image {
        id: cover
        objectName: "tidalNowPlayingArt"
        anchors.fill: parent
        source: root.service ? root.service.trackArtUrl : ""
        sourceSize: Qt.size(width * Screen.devicePixelRatio, height * Screen.devicePixelRatio)
        fillMode: Image.PreserveAspectCrop
        asynchronous: true
      }
      Text { anchors.centerIn: parent; visible: cover.status !== Image.Ready; textFormat: Text.PlainText; text: "󰝚"; color: Color.muted; font.family: Style.font.family; font.pixelSize: Style.font.title }
    }
    Column {
      width: Math.max(0, parent.width - (cover.parent.visible ? cover.parent.width + Style.spacing.lg : 0))
      anchors.verticalCenter: parent.verticalCenter
      spacing: Style.spacing.xs
      Row {
        width: parent.width
        spacing: Style.spacing.sm
        Text {
          id: title
          objectName: "tidalNowPlayingTitle"
          width: Math.max(0, parent.width - settings.implicitWidth - (quality.visible ? quality.implicitWidth + favorite.implicitWidth + Style.spacing.sm * 3 : Style.spacing.sm))
          textFormat: Text.PlainText
          text: root.service && root.service.trackTitle ? root.service.trackTitle : "Nothing playing"
          color: root.service && root.service.trackTitle ? Color.foreground : Color.muted
          font.family: Style.font.family
          font.pixelSize: Style.font.title
          font.bold: true
          elide: Text.ElideRight
        }
        Text {
          id: quality
          visible: !!root.service && !!root.service.trackTitle
          textFormat: Text.PlainText
          text: root.service && root.service.audioQuality === "HI_RES_LOSSLESS" ? "HI-RES" : (root.service && root.service.audioQuality === "HIGH" ? "HIGH" : "LOSSLESS")
          color: Color.accent
          font.family: Style.font.family
          font.pixelSize: Style.font.caption
          font.bold: true
          anchors.verticalCenter: parent.verticalCenter
        }
        Ui.PanelActionButton {
          id: favorite
          visible: !!root.service && !!root.service.trackTitle
          size: Style.spacing.controlHeight
          iconText: root.service && root.service.currentTrackFavorite ? "󰋑" : "󰋕"
          tooltipText: "Toggle favorite"
          Accessible.role: Accessible.Button
          Accessible.name: tooltipText
          onClicked: if (root.service) root.service.toggleFavorite()
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
        visible: !!root.service && !!root.service.trackTitle
        width: parent.width
        spacing: Style.spacing.xs
        Text {
          id: artist
          textFormat: Text.PlainText
          text: root.service ? root.service.trackArtist : ""
          width: Math.min(implicitWidth, parent.width * 0.45)
          elide: Text.ElideRight
          color: Color.muted
          font.family: Style.font.family
          font.pixelSize: Style.font.caption
          font.underline: artistMouse.containsMouse && !!root.service && !!root.service.trackArtist
          MouseArea { id: artistMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: root.service && root.service.trackArtist ? Qt.PointingHandCursor : Qt.ArrowCursor; onClicked: if (root.service && root.service.trackArtist) root.exploreArtist(root.service.currentArtistId || 0, root.service.trackArtist) }
        }
        Text { textFormat: Text.PlainText; text: "·"; color: Color.muted; font.family: Style.font.family; font.pixelSize: Style.font.caption }
        Text {
          textFormat: Text.PlainText
          text: root.service ? root.service.trackAlbum : ""
          width: Math.max(0, parent.width - artist.width - Style.spacing.xs * 2 - Style.spacing.sm)
          elide: Text.ElideRight
          color: Color.muted
          font.family: Style.font.family
          font.pixelSize: Style.font.caption
          font.underline: albumMouse.containsMouse && !!root.service && !!root.service.trackAlbum
          MouseArea { id: albumMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: root.service && root.service.trackAlbum ? Qt.PointingHandCursor : Qt.ArrowCursor; onClicked: if (root.service && root.service.trackAlbum) root.exploreAlbum(root.service.currentAlbumId || 0, root.service.trackAlbum) }
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
    visible: !!root.service && !!root.service.trackTitle
    width: parent.width
    spacing: Style.spacing.md
    Text { anchors.verticalCenter: parent.verticalCenter; textFormat: Text.PlainText; text: root.formatTime(!root.service ? 0 : (seek.dragging ? seek.liveValue * root.service.trackDuration : (root.pendingSeek >= 0 ? root.pendingSeek : root.service.trackPosition))); color: Color.muted; font.family: Style.font.family; font.pixelSize: Style.font.caption }
    Ui.PanelSlider {
      id: seek
      objectName: "tidalSeekSlider"
      bar: root.bar
      width: Math.max(0, parent.width - parent.children[0].implicitWidth - parent.children[2].implicitWidth - Style.spacing.md * 2)
      value: root.service && root.service.trackDuration > 0 ? Math.max(0, Math.min(1, root.service.trackPosition / root.service.trackDuration)) : 0
      onReleased: function(v) { if (root.service) { root.pendingSeek = v * root.service.trackDuration; root.service.seek(root.pendingSeek) } }
      // PanelSlider's wheel handler lives inside the component. Disable it here.
      MouseArea { anchors.fill: parent; acceptedButtons: Qt.NoButton; onWheel: function(wheel) { wheel.accepted = true } }
    }
    Text { anchors.verticalCenter: parent.verticalCenter; textFormat: Text.PlainText; text: root.formatTime(root.service ? root.service.trackDuration : 0); color: Color.muted; font.family: Style.font.family; font.pixelSize: Style.font.caption }
  }
  Row {
    visible: !!root.service
    anchors.horizontalCenter: parent.horizontalCenter
    spacing: Style.spacing.lg
    readonly property real controlButtonSize: Style.spacing.controlHeight + Style.spacing.md
    Ui.PanelActionButton { objectName: "tidalShuffleButton"; iconText: "󰒝"; tooltipText: "Shuffle"; size: parent.controlButtonSize; foreground: root.service && root.service.shuffle ? Color.accent : Color.foreground; onClicked: if (root.service) root.service.toggleShuffle() }
    Ui.PanelActionButton { objectName: "tidalPrevButton"; iconText: "󰒮"; tooltipText: "Previous"; size: parent.controlButtonSize; onClicked: if (root.service) root.service.previous() }
    Ui.PanelActionButton { objectName: "tidalPlayButton"; iconText: root.service && root.service.isPlaying ? "󰏤" : "󰐊"; tooltipText: root.service && root.service.isPlaying ? "Pause" : "Play"; size: parent.controlButtonSize; foreground: Color.accent; bordered: true; onClicked: if (root.service) root.service.togglePlay() }
    Ui.PanelActionButton { objectName: "tidalNextButton"; iconText: "󰒭"; tooltipText: "Next"; size: parent.controlButtonSize; onClicked: if (root.service) root.service.next() }
    Ui.PanelActionButton { objectName: "tidalRepeatButton"; iconText: root.service && root.service.repeatMode === "one" ? "󰑘" : "󰑖"; tooltipText: "Repeat"; size: parent.controlButtonSize; foreground: root.service && root.service.repeatMode !== "off" ? Color.accent : Color.foreground; onClicked: if (root.service) root.service.cycleRepeat() }
  }
  Row {
    width: parent.width
    spacing: Style.spacing.md
    visible: !!root.sink && !!root.sink.audio
    Ui.PanelActionButton { iconText: root.sink && root.sink.audio && root.sink.audio.muted ? "󰖁" : "󰕾"; tooltipText: "Mute audio"; onClicked: if (root.sink && root.sink.audio) root.sink.audio.muted = !root.sink.audio.muted }
    Ui.PanelSlider {
      id: volume
      bar: root.bar
      width: Math.max(0, parent.width - parent.children[0].implicitWidth - volumeLabel.implicitWidth - Style.spacing.md * 2)
      maximum: 1.5
      value: root.sink && root.sink.audio ? root.sink.audio.volume : 0
      tickCount: 4
      onMoved: function(v) { if (root.sink && root.sink.audio) root.sink.audio.volume = v }
      onRightClicked: if (root.sink && root.sink.audio) root.sink.audio.muted = !root.sink.audio.muted
      WheelHandler {
        acceptedDevices: PointerDevice.Mouse
        onWheel: function(event) {
          root.wheelRemainder += event.angleDelta.y
          var steps = Math.trunc(root.wheelRemainder / 120)
          if (steps && root.sink && root.sink.audio) {
            root.wheelRemainder -= steps * 120
            root.sink.audio.volume = Math.max(0, Math.min(1.5, root.sink.audio.volume + steps * 0.05))
          }
          event.accepted = true
        }
      }
    }
    Text { id: volumeLabel; anchors.verticalCenter: parent.verticalCenter; textFormat: Text.PlainText; text: Math.round((volume.dragging ? volume.liveValue : volume.value) * 100) + "%"; color: Color.muted; font.family: Style.font.family; font.pixelSize: Style.font.caption }
  }
}
