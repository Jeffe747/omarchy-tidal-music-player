import QtQuick
import qs.Commons

Rectangle {
  id: root
  property var track: ({})
  property bool current: false
  property bool cursor: false
  property bool showArt: false
  property string durationText: ""
  signal play()
  signal exploreArtist(var artistId, string artistName)
  signal exploreAlbum(var albumId, string albumTitle)

  height: Style.spacing.popupRowHeight + Style.spacing.xxl
  radius: Style.cornerRadius
  color: cursor ? Style.hoverFill : (current ? Style.selectedAccentFill : Style.normalFill)
  Accessible.role: Accessible.ListItem
  Accessible.name: (track.title || "Untitled") + " by " + (track.artist || "Unknown artist")
  clip: true

  Row {
    anchors.fill: parent
    anchors.margins: Style.spacing.sm
    spacing: Style.spacing.md
    Rectangle {
      visible: root.showArt
      width: visible ? parent.height : 0
      height: parent.height
      radius: Style.cornerRadius
      color: Style.hoverFill
      clip: true
      Image {
        id: artwork
        anchors.fill: parent
        source: root.showArt ? (root.track.art_url || "") : ""
        sourceSize: Qt.size(width * Screen.devicePixelRatio, height * Screen.devicePixelRatio)
        fillMode: Image.PreserveAspectCrop
        asynchronous: true
      }
      Text {
        anchors.centerIn: parent
        textFormat: Text.PlainText
        visible: artwork.status !== Image.Ready
        text: "󰝚"
        color: Color.muted
        font.family: Style.font.family
        font.pixelSize: Style.font.body
      }
    }
    Column {
      anchors.verticalCenter: parent.verticalCenter
      width: Math.max(0, parent.width - (root.showArt ? parent.height + Style.spacing.md : 0) - duration.implicitWidth - Style.spacing.md)
      spacing: Style.spacing.xxs
      Text {
        width: parent.width
        textFormat: Text.PlainText
        text: root.track.title || "Untitled"
        color: root.current ? Color.accent : Color.foreground
        font.family: Style.font.family
        font.pixelSize: Style.font.body
        elide: Text.ElideRight
      }
      Row {
        width: parent.width
        spacing: Style.spacing.xs
        Text {
          id: artistLink
          textFormat: Text.PlainText
          text: root.track.artist || "Unknown artist"
          width: Math.min(implicitWidth, parent.width * 0.48)
          elide: Text.ElideRight
          color: Color.muted
          font.family: Style.font.family
          font.pixelSize: Style.font.caption
          font.underline: artistMouse.containsMouse && !!root.track.artist
          MouseArea {
            id: artistMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: root.track.artist ? Qt.PointingHandCursor : Qt.ArrowCursor
            onClicked: if (root.track.artist) root.exploreArtist(root.track.artist_id || 0, root.track.artist)
          }
        }
        Text {
          visible: !!root.track.album
          textFormat: Text.PlainText
          text: "·"
          color: Color.muted
          font.family: Style.font.family
          font.pixelSize: Style.font.caption
        }
        Text {
          id: albumLink
          visible: !!root.track.album
          textFormat: Text.PlainText
          text: root.track.album || ""
          width: Math.max(0, parent.width - artistLink.width - Style.spacing.xs * 2 - Style.spacing.sm)
          elide: Text.ElideRight
          color: Color.muted
          font.family: Style.font.family
          font.pixelSize: Style.font.caption
          font.underline: albumMouse.containsMouse && !!root.track.album
          MouseArea {
            id: albumMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: root.track.album ? Qt.PointingHandCursor : Qt.ArrowCursor
            onClicked: if (root.track.album) root.exploreAlbum(root.track.album_id || 0, root.track.album)
          }
        }
      }
    }
    Text {
      id: duration
      anchors.verticalCenter: parent.verticalCenter
      textFormat: Text.PlainText
      text: root.durationText
      color: Color.muted
      font.family: Style.font.family
      font.pixelSize: Style.font.caption
    }
  }
  MouseArea {
    objectName: "tidalFavoriteClick"
    z: -1
    anchors.fill: parent
    cursorShape: Qt.PointingHandCursor
    onClicked: root.play()
  }
}
