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
  property bool creatingPlaylist: false
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
  signal playTrack(var id, var track)
  signal exploreArtist(var artistId, string artistName)
  signal exploreAlbum(var albumId, string albumTitle)
  signal openPlaylist(string id)
  signal contextMenuRequested(var track, real x, real y)

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
    else root.playTrack(item.id, item)
  }
  function cancelCreatingPlaylist() { creatingPlaylist = false; newPlaylistField.text = "" }

  Row {
    visible: root.view === "favorites" || root.view === "playlists" || root.view === "playlistTracks"
    spacing: Style.spacing.md
    Ui.Button {
      text: "Favorites " + (root.service ? (root.service.favoritesTotal ? root.service.favoritesTotal : root.service.favorites.length) : "")
      selected: root.view === "favorites"
      onClicked: root.selectTab("favorites")
    }
    Ui.Button {
      text: "Playlists " + (root.service ? root.service.playlists.length : "")
      selected: root.view === "playlists" || root.view === "playlistTracks"
      onClicked: root.selectTab("playlists")
    }
  }
  Item {
    id: libraryProgressBar
    width: parent.width
    height: Style.space(2)
    visible: !!root.service && (root.service.favoritesLoading || root.service.favoritesInProgress || root.service.playlistsLoading || root.service.playlistTracksLoading || root.service.explorationLoading || root.service.searching)
    clip: true
    Rectangle {
      anchors.fill: parent
      color: Style.hoverFill
    }
    Rectangle {
      id: progressIndicator
      height: parent.height
      width: Math.max(Style.space(60), parent.width * 0.35)
      radius: height / 2
      color: Color.accent
      SequentialAnimation on x {
        running: libraryProgressBar.visible
        loops: Animation.Infinite
        NumberAnimation {
          from: -progressIndicator.width
          to: libraryProgressBar.width
          duration: 900
          easing.type: Easing.InOutCubic
        }
      }
    }
  }
  Ui.Button {
    objectName: "tidalCreatePlaylistButton"
    visible: root.view === "playlists" && !root.creatingPlaylist
    text: "+ Create playlist"
    iconText: "󰐕"
    onClicked: {
      root.creatingPlaylist = true
      Qt.callLater(function() { newPlaylistField.forceActiveFocus() })
    }
  }
  Row {
    visible: root.view === "playlists" && root.creatingPlaylist
    width: parent.width
    spacing: Style.spacing.sm
    SafeTextField {
      id: newPlaylistField
      objectName: "tidalNewPlaylistField"
      width: Math.max(0, parent.width - submitPlaylistBtn.implicitWidth - cancelPlaylistBtn.implicitWidth - Style.spacing.sm * 2)
      placeholderText: "Playlist name…"
      onAccepted: {
        if (text.trim() && root.service) {
          root.service.createPlaylist(text.trim())
          text = ""
          root.creatingPlaylist = false
        }
      }
    }
    Ui.Button {
      id: submitPlaylistBtn
      objectName: "tidalSubmitPlaylistButton"
      text: "Create"
      enabled: newPlaylistField.text.trim().length > 0
      onClicked: {
        if (newPlaylistField.text.trim() && root.service) {
          root.service.createPlaylist(newPlaylistField.text.trim())
          newPlaylistField.text = ""
          root.creatingPlaylist = false
        }
      }
    }
    Ui.Button {
      id: cancelPlaylistBtn
      objectName: "tidalCancelPlaylistButton"
      text: "Cancel"
      onClicked: root.cancelCreatingPlaylist()
    }
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
        text: root.service && root.service.explorationView === "album" ? "󰀥 ALBUM" : root.service && root.service.explorationView === "radio" ? "󰐹 SONG RADIO" : "󰝚 ARTIST"
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
        text: (root.service && root.service.explorationView === "album" ? "Album · " : root.service && root.service.explorationView === "radio" ? "Song Radio · " : "Top Tracks · ") + root.activeItems.length + " tracks"
        color: Color.muted
        font.family: Style.font.family
        font.pixelSize: Style.font.caption
      }
    }
  }
  SafeTextField {
    id: filterField
    visible: root.view === "playlistTracks"
    width: parent.width
    placeholderText: "Filter tracks by title or artist"
    onTextChanged: filterDebounce.restart()
  }
  Timer {
    id: filterDebounce
    interval: 150
    onTriggered: root.playlistFilter = filterField.text.trim().toLowerCase()
  }
  Rectangle {
    id: favoritesLoadingCard
    visible: root.view === "favorites" && !!root.service && root.service.favoritesLoading && root.service.favorites.length === 0
    width: parent.width
    height: Style.space(160)
    radius: Style.cornerRadius
    color: Style.normalFill
    Column {
      anchors.centerIn: parent
      spacing: Style.spacing.md
      Text {
        anchors.horizontalCenter: parent.horizontalCenter
        textFormat: Text.PlainText
        text: "Loading favorites…"
        color: Color.muted
        font.family: Style.font.family
        font.pixelSize: Style.font.body
      }
    }
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
  Column {
    id: skeletonContainer
    visible: !!root.service && ((root.view === "playlistTracks" && root.service.playlistTracksLoading) || (root.view === "explore" && root.service.explorationLoading))
    width: parent.width
    spacing: Style.spacing.sm
    Repeater {
      model: 4
      Rectangle {
        width: skeletonContainer.width
        height: Style.spacing.popupRowHeight + Style.spacing.xxl
        radius: Style.cornerRadius
        color: Style.normalFill
        opacity: 0.6
        SequentialAnimation on opacity {
          running: skeletonContainer.visible
          loops: Animation.Infinite
          NumberAnimation { from: 0.35; to: 0.75; duration: 650; easing.type: Easing.InOutSine }
          NumberAnimation { from: 0.75; to: 0.35; duration: 650; easing.type: Easing.InOutSine }
        }
        Row {
          anchors.fill: parent
          anchors.margins: Style.spacing.sm
          spacing: Style.spacing.md
          Rectangle {
            width: parent.height
            height: parent.height
            radius: Style.cornerRadius
            color: Style.hoverFill
          }
          Column {
            anchors.verticalCenter: parent.verticalCenter
            spacing: Style.spacing.xs
            width: Math.max(0, parent.width - parent.height - Style.spacing.md * 2)
            Rectangle {
              width: parent.width * 0.55
              height: Style.space(12)
              radius: Style.space(3)
              color: Style.hoverFill
            }
            Rectangle {
              width: parent.width * 0.35
              height: Style.space(10)
              radius: Style.space(3)
              color: Style.hoverFill
            }
          }
        }
      }
    }
  }
  Text {
    visible: !!root.service && (root.view === "search" || root.view === "explore" || root.view === "playlists" || root.view === "playlistTracks")
      && (root.view === "search" ? (root.service.searching || root.service.searchError || (root.service.searchCompleted && !root.service.searchResults.length))
        : root.view === "explore" ? (!root.service.explorationLoading && (root.service.explorationError || !root.service.explorationTracks.length))
        : root.view === "playlists" ? (root.service.playlistsLoading || root.service.playlistsError || !root.service.playlists.length)
        : (!root.service.playlistTracksLoading && (root.service.playlistTracksError || !root.activeItems.length)))
    width: parent.width
    textFormat: Text.PlainText
    text: !root.service ? "" : root.view === "search" ? (root.service.searching ? "Searching..." : (root.service.searchError || "No tracks found"))
      : root.view === "explore" ? (root.service.explorationError || "No tracks found")
      : root.view === "playlists" ? (root.service.playlistsLoading ? "Loading playlists..." : (root.service.playlistsError || "No playlists found"))
      : (root.service.playlistTracksError || "No tracks found")
    color: root.service && (root.service.searchError || root.service.explorationError || root.service.playlistsError || root.service.playlistTracksError) ? Color.urgent : Color.muted
    font.family: Style.font.family
    font.pixelSize: Style.font.caption
  }
  Item {
    width: parent.width
    height: list.count > 0 ? Math.min(list.contentHeight, Style.space(280)) : 0
    visible: root.visible

    ListView {
      id: list
      objectName: "tidalFavoritesList"
      anchors.fill: parent
      clip: true
      spacing: Style.spacing.sm
      currentIndex: root.keyboardIndex
      model: root.activeItems
      delegate: Loader {
        id: rowLoader
        required property var modelData
        required property int index
        width: list.contentHeight > list.height ? list.width - Style.space(10) : list.width
        height: Style.spacing.popupRowHeight + Style.spacing.xxl
        sourceComponent: root.view === "playlists" ? playlistRow : trackRow
        onModelDataChanged: if (item) item.rowData = modelData
        onIndexChanged: if (item) item.rowIndex = index
        Binding {
          target: rowLoader.item
          property: "rowData"
          value: rowLoader.modelData
          when: rowLoader.item !== null
        }
        Binding {
          target: rowLoader.item
          property: "rowIndex"
          value: rowLoader.index
          when: rowLoader.item !== null
        }
        onLoaded: {
          if (item) { item.rowData = modelData; item.rowIndex = index }
        }
      }
    }
    ScrollDot {
      targetList: list
    }
  }
  Rectangle {
    id: favoritesProgressBanner
    visible: root.view === "favorites" && !!root.service && root.service.favoritesInProgress && root.service.favorites.length > 0
    width: parent.width
    height: Style.spacing.popupRowHeight
    radius: Style.cornerRadius
    color: Style.normalFill
    Row {
      anchors.centerIn: parent
      spacing: Style.spacing.sm
      Text {
        textFormat: Text.PlainText
        text: root.service ? "Loading more tracks (" + root.service.favorites.length + (root.service.favoritesTotal ? " / " + root.service.favoritesTotal : "") + ")…" : ""
        color: Color.muted
        font.family: Style.font.family
        font.pixelSize: Style.font.caption
      }
    }
  }
  Component {
    id: trackRow
    TrackRow {
      property var rowData: ({})
      property int rowIndex: -1
      width: list.contentHeight > list.height ? list.width - Style.space(10) : list.width
      service: root.service
      track: rowData
      showArt: root.view === "search"
      durationText: root.formatTime(rowData.duration)
      current: !!root.service && root.service.currentTrackId === rowData.id
      cursor: root.keyboardIndex === rowIndex
      onPlay: root.playTrack(rowData.id, rowData)
      onContextMenuRequested: function(track, x, y) {
        var p = mapToItem(root, x, y)
        root.contextMenuRequested(track, p.x, p.y)
      }
      onExploreArtist: function(id, name) { root.exploreArtist(id, name) }
      onExploreAlbum: function(id, name) { root.exploreAlbum(id, name) }
    }
  }
  Component {
    id: playlistRow
    Rectangle {
      id: pRow
      Accessible.role: Accessible.ListItem
      Accessible.name: (pRow.rowData && pRow.rowData.title) ? pRow.rowData.title : "Untitled playlist"
      property var rowData: ({})
      property int rowIndex: -1
      width: list.contentHeight > list.height ? list.width - Style.space(10) : list.width
      height: Style.spacing.popupRowHeight + Style.spacing.xxl
      radius: Style.cornerRadius
      color: root.keyboardIndex === rowIndex ? Style.hoverFill : Style.normalFill
      Text { anchors.left: parent.left; anchors.leftMargin: Style.spacing.md; anchors.verticalCenter: parent.verticalCenter; textFormat: Text.PlainText; text: (pRow.rowData && pRow.rowData.title) ? pRow.rowData.title : "Untitled playlist"; color: Color.foreground; font.family: Style.font.family; font.pixelSize: Style.font.body; width: parent.width - Style.spacing.xxl; elide: Text.ElideRight }
      MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: if (pRow.rowData && pRow.rowData.uuid) root.openPlaylist(pRow.rowData.uuid) }
    }
  }
}
