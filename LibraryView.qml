import QtQuick
import QtQuick.Controls
import qs.Commons
import qs.Ui as Ui

Column {
  id: root
  property var service: null
  property string view: "favorites"
  property int keyboardIndex: -1
  property string playlistFilter: ""
  readonly property bool filterFocused: filterField.activeFocus
  property var activeItems: {
    if (!service) return []
    if (view === "search") return service.searchResults
    if (view === "explore") return service.explorationTracks
    if (view === "playlistTracks") return service.playlistTracks.filter(function(t) {
      return !playlistFilter || ((t.title || "") + " " + (t.artist || "")).toLowerCase().indexOf(playlistFilter) >= 0
    })
    if (view === "playlists") return service.playlists
    return service.favorites
  }
  signal selectTab(string tab)
  signal back()
  signal playTrack(var id)
  signal exploreArtist(var artistId, string artistName)
  signal exploreAlbum(var albumId, string albumTitle)
  signal openPlaylist(string id)

  width: parent ? parent.width : implicitWidth
  spacing: Style.spacing.md

  function formatTime(seconds) {
    var safe = Math.max(0, Number(seconds) || 0)
    return Math.floor(safe / 60) + ":" + ("0" + Math.floor(safe % 60)).slice(-2)
  }
  function activate(index) {
    var item = activeItems[index]
    if (!item) return
    if (view === "playlists") root.openPlaylist(item.uuid)
    else root.playTrack(item.id)
  }

  Row {
    visible: root.view === "favorites" || root.view === "playlists" || root.view === "playlistTracks"
    spacing: Style.spacing.md
    Ui.Button { text: "Favorites" + (root.service ? " " + root.service.favorites.length : ""); selected: root.view === "favorites"; onClicked: root.selectTab("favorites") }
    Ui.Button { text: "Playlists" + (root.service ? " " + root.service.playlists.length : ""); selected: root.view === "playlists" || root.view === "playlistTracks"; onClicked: root.selectTab("playlists") }
  }
  Ui.Button {
    visible: root.view === "playlistTracks"
    text: "← Back to playlists"
    onClicked: root.back()
  }
  Rectangle {
    visible: root.view === "explore"
    width: parent.width
    height: visible ? artistHeader.implicitHeight + Style.spacing.popupPadding * 2 : 0
    radius: Style.cornerRadius
    color: Style.normalFill
    Column {
      id: artistHeader
      anchors.fill: parent
      anchors.margins: Style.spacing.popupPadding
      spacing: Style.spacing.sm
      Ui.Button { objectName: "tidalExploreBack"; text: "← Back"; onClicked: root.back() }
      Text {
        objectName: "tidalExploreCategory"
        textFormat: Text.PlainText
        text: root.service && root.service.explorationView === "album" ? "󰀥 ALBUM" : "󰠃 ARTIST"
        color: Color.accent
        font.family: Style.font.family
        font.pixelSize: Style.font.caption
        font.bold: true
      }
      Text {
        objectName: "tidalExploreTitle"
        width: parent.width
        textFormat: Text.PlainText
        text: root.service ? root.service.explorationTitle : ""
        color: Color.foreground
        font.family: Style.font.family
        font.pixelSize: Style.font.title
        font.bold: true
        elide: Text.ElideRight
      }
      Text {
        objectName: "tidalExploreCount"
        textFormat: Text.PlainText
        text: (root.service && root.service.explorationView === "album" ? "Album · " : "Top Tracks · ") + root.activeItems.length + " tracks"
        color: Color.muted
        font.family: Style.font.family
        font.pixelSize: Style.font.caption
      }
    }
  }
  Ui.TextField {
    id: filterField
    visible: root.view === "playlistTracks"
    width: parent.width
    placeholderText: "Filter tracks by title or artist"
    onTextChanged: root.playlistFilter = text.trim().toLowerCase()
  }
  Text {
    objectName: "tidalFavoritesState"
    visible: root.view === "favorites" && !!root.service && (root.service.favoritesLoading || root.service.favoritesError || root.service.favorites.length === 0)
    width: parent.width
    textFormat: Text.PlainText
    text: !root.service ? "" : (root.service.favoritesLoading ? "Loading favorites..." : (root.service.favoritesError || "No favorite tracks yet. Add favorites in Tidal."))
    color: root.service && root.service.favoritesError ? Color.urgent : Color.muted
    font.family: Style.font.family
    font.pixelSize: Style.font.caption
    wrapMode: Text.WordWrap
  }
  Ui.Button { visible: root.view === "favorites" && !!root.service && !!root.service.favoritesError; text: "Retry favorites"; onClicked: root.service.loadFavorites() }
  Text {
    visible: !!root.service && (root.view === "search" || root.view === "explore" || root.view === "playlists" || root.view === "playlistTracks")
      && (root.view === "search" ? (root.service.searching || root.service.searchError || (root.service.searchCompleted && !root.service.searchResults.length))
        : root.view === "explore" ? (root.service.explorationLoading || root.service.explorationError || !root.service.explorationTracks.length)
        : root.view === "playlists" ? (root.service.playlistsLoading || root.service.playlistsError || !root.service.playlists.length)
        : (root.service.playlistTracksLoading || root.service.playlistTracksError || !root.activeItems.length))
    width: parent.width
    textFormat: Text.PlainText
    text: !root.service ? "" : root.view === "search" ? (root.service.searching ? "Searching..." : (root.service.searchError || "No tracks found"))
      : root.view === "explore" ? (root.service.explorationLoading ? "Loading tracks..." : (root.service.explorationError || "No tracks found"))
      : root.view === "playlists" ? (root.service.playlistsLoading ? "Loading playlists..." : (root.service.playlistsError || "No playlists found"))
      : (root.service.playlistTracksLoading ? "Loading tracks..." : (root.service.playlistTracksError || "No tracks found"))
    color: root.service && (root.service.searchError || root.service.explorationError || root.service.playlistsError || root.service.playlistTracksError) ? Color.urgent : Color.muted
    font.family: Style.font.family
    font.pixelSize: Style.font.caption
  }
  ListView {
    id: list
    objectName: "tidalFavoritesList"
    visible: root.visible
    width: parent.width
    height: count > 0 ? Math.min(contentHeight, Style.space(280)) : 0
    clip: true
    spacing: Style.spacing.sm
    currentIndex: root.keyboardIndex
    model: root.activeItems
    ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded; width: Style.spacing.md; contentItem: Rectangle { radius: Style.cornerRadius; color: Color.muted } }
    delegate: Loader {
      required property var modelData
      required property int index
      width: list.width
      sourceComponent: root.view === "playlists" ? playlistRow : trackRow
      onLoaded: {
        if (item) { item.rowData = modelData; item.rowIndex = index }
      }
    }
  }
  Component {
    id: trackRow
    TrackRow {
      property var rowData: ({})
      property int rowIndex: -1
      width: list.width
      track: rowData
      showArt: root.view === "search"
      durationText: root.formatTime(rowData.duration)
      current: !!root.service && root.service.currentTrackId === rowData.id
      cursor: root.keyboardIndex === rowIndex
      onPlay: root.playTrack(rowData.id)
      onExploreArtist: function(id, name) { root.exploreArtist(id, name) }
      onExploreAlbum: function(id, name) { root.exploreAlbum(id, name) }
    }
  }
  Component {
    id: playlistRow
    Rectangle {
      property var rowData: ({})
      property int rowIndex: -1
      width: list.width
      height: Style.spacing.popupRowHeight + Style.spacing.xxl
      radius: Style.cornerRadius
      color: root.keyboardIndex === rowIndex ? Style.hoverFill : Style.normalFill
      Text { anchors.left: parent.left; anchors.leftMargin: Style.spacing.md; anchors.verticalCenter: parent.verticalCenter; textFormat: Text.PlainText; text: parent.rowData.title || "Untitled playlist"; color: Color.foreground; font.family: Style.font.family; font.pixelSize: Style.font.body; width: parent.width - Style.spacing.xxl; elide: Text.ElideRight }
      MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: root.openPlaylist(parent.rowData.uuid) }
    }
  }
}
