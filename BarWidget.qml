import QtQuick
import Quickshell
import Quickshell.Services.Pipewire
import qs.Commons
import qs.Ui
import qs.Ui as Ui

Panel {
  id: root
  moduleName: "jaj.tidal"
  ipcTarget: "jaj.tidal.widget"

  readonly property var tidalService: bar && bar.shell && typeof bar.shell.serviceFor === "function" ? bar.shell.serviceFor("jaj.tidal") : null
  readonly property bool isAuthenticated: !!tidalService && tidalService.authenticated === true
  readonly property var defaultAudioSink: Pipewire.defaultAudioSink
  readonly property bool isSystemMuted: !!defaultAudioSink && !!defaultAudioSink.audio && defaultAudioSink.audio.muted
  property string libraryTab: "favorites"
  property bool settingsOpen: false
  property bool confirmLogout: false
  property int keyboardIndex: -1
  property bool enterPending: false
  property bool iconOnly: true
  property var contextMenuTrack: null
  property real contextMenuX: 0
  property real contextMenuY: 0
  property bool contextMenuOpen: false
  property bool playlistSubmenuOpen: false
  readonly property string barLabel: {
    if (iconOnly || !tidalService || !tidalService.trackTitle) return ""
    var label = tidalService.trackTitle + " — " + tidalService.trackArtist
    return (tidalService.isPlaying ? "󰏤 " : "󰐊 ") + (label.length > 24 ? label.slice(0, 23) + "…" : label)
  }
  readonly property string view: {
    if (tidalService && tidalService.explorationView) return "explore"
    if (searchField.text.trim()) return "search"
    if (libraryTab === "playlists" && tidalService && tidalService.currentPlaylistId) return "playlistTracks"
    return libraryTab
  }
  onViewChanged: keyboardIndex = -1
  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  function barLabelText() { return tidalService && tidalService.trackTitle ? tidalService.trackTitle + " — " + tidalService.trackArtist : "" }
  function openContextMenu(track, x, y) {
    contextMenuTrack = track
    playlistSubmenuOpen = false
    contextMenuX = Math.max(Style.spacing.sm, Math.min(keyCatcher.width - trackContextMenu.width - Style.spacing.sm, x))
    contextMenuY = Math.max(Style.spacing.sm, Math.min(keyCatcher.height - trackContextMenu.height - Style.spacing.sm, y))
    contextMenuOpen = true
  }
  function closeContextMenu() { contextMenuOpen = false; playlistSubmenuOpen = false; contextMenuTrack = null }
  function goBack() {
    if (contextMenuOpen) { closeContextMenu(); return }
    if (libraryView && libraryView.creatingPlaylist) { libraryView.cancelCreatingPlaylist(); return }
    if (confirmLogout) { confirmLogout = false; return }
    if (tidalService && tidalService.explorationView) {
      var restoreSearch = !!searchField.text.trim() && (tidalService.explorationSearchRequestId >= 0 || tidalService.searching)
      tidalService.closeExploration()
      if (restoreSearch) tidalService.search(searchField.text.trim())
      keyCatcher.forceActiveFocus()
      return
    }
    if (searchField.text !== "") { searchField.text = ""; searchDebounce.stop(); if (tidalService) tidalService.clearSearch(); keyCatcher.forceActiveFocus(); return }
    if (tidalService && tidalService.currentPlaylistId) { tidalService.closePlaylist(); libraryView.playlistFilter = ""; return }
    if (settingsOpen) { settingsOpen = false; return }
    close()
  }
  function moveCursor(dy) {
    var count = libraryView.activeItems.length
    keyboardIndex = count ? Math.max(0, Math.min(count - 1, keyboardIndex + dy)) : -1
  }
  function activateCursor() {
    if (confirmLogout) { if (confirmDialog.selectedIndex === 1) { tidalService.logout(); settingsOpen = false }; confirmLogout = false; return }
    if (enterPending) { enterPending = false; if (!settingsOpen) libraryView.activate(keyboardIndex) }
    else if (!settingsOpen && tidalService) tidalService.togglePlay()
  }
  function handleTextKey(key) {
    if (key === "/") { searchField.forceActiveFocus(); return }
    if (key.length === 1 && key !== " " && !settingsOpen && !searchField.activeFocus) {
      searchField.text += key
      searchField.forceActiveFocus()
    }
  }

  WidgetButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    hasVisualContent: true
    text: root.barLabel
    fixedWidth: root.iconOnly || root.barLabel === "" ? (vertical ? barSize : Style.space(32)) : -1
    tooltipText: tidalService && tidalService.trackTitle ? tidalService.trackTitle + " — " + tidalService.trackArtist : "Tidal Music"
    Accessible.role: Accessible.Button
    Accessible.name: tooltipText
    TidalIcon { anchors.centerIn: parent; visible: root.iconOnly || root.barLabel === ""; color: button.foreground }
    onPressed: function(b) {
      if (b === Qt.MiddleButton) { if (tidalService) tidalService.togglePlay() }
      else if (b === Qt.RightButton) { if (tidalService) tidalService.next() }
      else root.toggle()
    }
    onWheelMoved: function(delta) { if (tidalService) { if (delta > 0) tidalService.previous(); else tidalService.next() } }
  }

  KeyboardPanel {
    id: panel
    anchorItem: button
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(Style.space(380))
    contentHeight: panel.fittedContentHeight(root.settingsOpen ? (settingsColumn.implicitHeight + Style.spacing.popupPadding * 2) : mainColumn.implicitHeight)
    onOpenChanged: {
      if (!open) root.closeContextMenu()
      if (open && tidalService) {
        tidalService.initializeForWidget()
        if (tidalService.authenticated && tidalService.playlists.length === 0) tidalService.loadPlaylists()
      }
    }

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      onCloseRequested: root.goBack()
      onMoveRequested: function(dx, dy) {
        if (root.confirmLogout) { if (dx) confirmDialog.selectedIndex = dx > 0 ? 1 : 0; return }
        if (dy) root.moveCursor(dy)
        else if (dx && tidalService && !searchField.activeFocus && !libraryView.filterFocused) player.seekRelative(dx * 5)
      }
      onReturnRequested: root.enterPending = true
      onActivateRequested: root.activateCursor()
      onTextKey: function(t) { root.handleTextKey(t) }
      onTabRequested: function(direction) { root.switchPanel(direction) }

      Column {
        id: mainColumn
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        spacing: Style.spacing.xxl

        NowPlayingCard {
          id: player
          visible: root.isAuthenticated
          width: parent.width
          service: tidalService
          bar: root.bar
          onSettingsRequested: root.settingsOpen = !root.settingsOpen
          onChromecastRequested: if (tidalService && typeof tidalService.requestChromecast === "function") tidalService.requestChromecast()
          onExploreArtist: function(id, name) { searchDebounce.stop(); if (tidalService) tidalService.exploreArtist(id, name) }
          onExploreAlbum: function(id, name) { searchDebounce.stop(); if (tidalService) tidalService.exploreAlbum(id, name) }
        }
        Column {
          visible: !root.isAuthenticated
          width: parent.width
          spacing: Style.spacing.lg
          Text {
            width: parent.width
            textFormat: Text.PlainText
            text: "Connect Tidal to start listening"
            color: Color.foreground
            font.family: Style.font.family
            font.pixelSize: Style.font.title
            font.bold: true
          }
          Text {
            width: parent.width
            visible: !!tidalService && !tidalService.daemonBinaryExists
            textFormat: Text.PlainText
            text: tidalService ? tidalService.buildScriptMessage : ""
            color: Color.urgent
            font.family: Style.font.family
            font.pixelSize: Style.font.caption
            wrapMode: Text.WordWrap
          }
          Text {
            width: parent.width
            visible: !!tidalService && !!tidalService.authPending
            textFormat: Text.PlainText
            text: "Enter " + (tidalService ? tidalService.authCode : "") + " at link.tidal.com"
            color: Color.accent
            font.family: Style.font.family
            font.pixelSize: Style.font.body
          }
          Ui.Button { visible: !!tidalService && !!tidalService.authPending; text: "Open link.tidal.com"; onClicked: if (tidalService) Quickshell.execDetached(["xdg-open", tidalService.authUrl]) }
          Text { visible: !!tidalService && !!tidalService.authError; textFormat: Text.PlainText; text: tidalService ? tidalService.authError : ""; color: Color.urgent; font.family: Style.font.family; font.pixelSize: Style.font.caption; wrapMode: Text.WordWrap; width: parent.width }
          Ui.Button { visible: !tidalService || !tidalService.authPending; enabled: !!tidalService; text: "Login with Tidal"; onClicked: tidalService.startAuth() }
        }
        Ui.TextField {
          id: searchField
          objectName: "tidalSearchField"
          visible: root.isAuthenticated && !root.settingsOpen && root.view !== "explore"
          width: parent.width
          placeholderText: "Search Tidal tracks…  /"
          onTextChanged: {
            root.keyboardIndex = -1
            searchDebounce.stop()
            if (tidalService) {
              if (text.trim()) { tidalService.prepareSearch(); searchDebounce.start() }
              else tidalService.clearSearch()
            }
          }
        }
        Timer {
          id: searchDebounce
          interval: 350
          onTriggered: if (tidalService && root.view !== "explore" && searchField.text.trim()) tidalService.search(searchField.text.trim())
        }
        LibraryView {
          id: libraryView
          objectName: "tidalLibraryView"
          visible: root.isAuthenticated && !root.settingsOpen
          width: parent.width
          service: tidalService
          view: root.view
          keyboardIndex: root.keyboardIndex
          onSelectTab: function(tab) {
            root.libraryTab = tab
            if (tab === "playlists" && tidalService && tidalService.playlists.length === 0) tidalService.loadPlaylists()
          }
          onBack: root.goBack()
          onPlayTrack: function(id) { if (tidalService) tidalService.playTrack(id) }
          onExploreArtist: function(id, name) { searchDebounce.stop(); if (tidalService) tidalService.exploreArtist(id, name) }
          onExploreAlbum: function(id, name) { searchDebounce.stop(); if (tidalService) tidalService.exploreAlbum(id, name) }
          onOpenPlaylist: function(id) { if (tidalService) tidalService.loadPlaylistTracks(id) }
          onContextMenuRequested: function(track, lx, ly) {
            var pos = libraryView.mapToItem(keyCatcher, lx, ly)
            root.openContextMenu(track, pos.x, pos.y)
          }
        }
      }
      MouseArea {
        objectName: "tidalContextMenuBackdrop"
        anchors.fill: parent
        z: 19
        visible: root.contextMenuOpen
        acceptedButtons: Qt.AllButtons
        onClicked: root.closeContextMenu()
      }
      Rectangle {
        id: trackContextMenu
        objectName: "tidalTrackContextMenu"
        z: 20
        visible: root.contextMenuOpen && !!root.contextMenuTrack
        x: root.contextMenuX
        y: root.contextMenuY
        width: Style.space(190)
        height: root.playlistSubmenuOpen ? Math.min(Style.space(280), Style.space(48) + Math.max(1, tidalService ? tidalService.playlists.length : 0) * Style.spacing.popupRowHeight) : Style.space(96)
        color: Color.popups.background
        border.color: Color.muted
        border.width: Style.normalBorderWidth
        radius: Style.cornerRadius
        clip: true
        Column {
          anchors.fill: parent
          spacing: 0
          Rectangle {
            objectName: "tidalContextFavoriteAction"
            visible: !root.playlistSubmenuOpen
            width: parent.width
            height: Style.spacing.popupRowHeight + Style.spacing.sm
            color: favoriteMouse.containsMouse ? Style.hoverFill : Style.normalFill
            Row { anchors.fill: parent; anchors.margins: Style.spacing.md; spacing: Style.spacing.md
              Text { textFormat: Text.PlainText; text: tidalService && root.contextMenuTrack && tidalService.isTrackFavorite(root.contextMenuTrack.id) ? "󰋕" : "󰋑"; color: Color.accent; font.family: Style.font.family; font.pixelSize: Style.font.body }
              Text { textFormat: Text.PlainText; text: tidalService && root.contextMenuTrack && tidalService.isTrackFavorite(root.contextMenuTrack.id) ? "Remove favorite" : "Favorite"; color: Color.foreground; font.family: Style.font.family; font.pixelSize: Style.font.body }
            }
            MouseArea { id: favoriteMouse; anchors.fill: parent; hoverEnabled: true; onClicked: { if (tidalService && root.contextMenuTrack) tidalService.toggleTrackFavorite(root.contextMenuTrack.id); root.closeContextMenu() } }
          }
          Rectangle {
            objectName: "tidalContextPlaylistAction"
            visible: !root.playlistSubmenuOpen
            width: parent.width
            height: Style.spacing.popupRowHeight + Style.spacing.sm
            color: playlistMouse.containsMouse ? Style.hoverFill : Style.normalFill
            Row { anchors.fill: parent; anchors.margins: Style.spacing.md; spacing: Style.spacing.md
              Text { textFormat: Text.PlainText; text: "󰲸"; color: Color.accent; font.family: Style.font.family; font.pixelSize: Style.font.body }
              Text { textFormat: Text.PlainText; text: "Add to playlist"; color: Color.foreground; font.family: Style.font.family; font.pixelSize: Style.font.body; width: parent.width - Style.spacing.xxl; elide: Text.ElideRight }
              Text { textFormat: Text.PlainText; text: "›"; color: Color.muted; font.family: Style.font.family; font.pixelSize: Style.font.body }
            }
            MouseArea { id: playlistMouse; anchors.fill: parent; hoverEnabled: true; onClicked: { root.playlistSubmenuOpen = true; if (tidalService && !tidalService.playlists.length) tidalService.loadPlaylists() } }
          }
          Rectangle {
            visible: root.playlistSubmenuOpen
            width: parent.width
            height: Style.spacing.popupRowHeight
            color: Style.hoverFill
            Text { anchors.fill: parent; anchors.margins: Style.spacing.md; verticalAlignment: Text.AlignVCenter; textFormat: Text.PlainText; text: "‹  Select playlist"; color: Color.foreground; font.family: Style.font.family; font.pixelSize: Style.font.body }
            MouseArea { anchors.fill: parent; onClicked: root.playlistSubmenuOpen = false }
          }
          ListView {
            objectName: "tidalContextPlaylistList"
            visible: root.playlistSubmenuOpen && !!tidalService && tidalService.playlists.length > 0
            width: parent.width
            height: Math.max(0, parent.height - Style.spacing.popupRowHeight)
            model: tidalService ? tidalService.playlists : []
            clip: true
            delegate: Rectangle {
              required property var modelData
              width: ListView.view.width
              height: Style.spacing.popupRowHeight
              color: itemMouse.containsMouse ? Style.hoverFill : Style.normalFill
              Text { anchors.fill: parent; anchors.margins: Style.spacing.md; verticalAlignment: Text.AlignVCenter; textFormat: Text.PlainText; text: modelData.title || "Untitled playlist"; color: Color.foreground; font.family: Style.font.family; font.pixelSize: Style.font.body; elide: Text.ElideRight }
              MouseArea { id: itemMouse; anchors.fill: parent; hoverEnabled: true; onClicked: { if (tidalService && root.contextMenuTrack) tidalService.addTrackToPlaylist(modelData.uuid, root.contextMenuTrack.id); root.closeContextMenu() } }
            }
          }
          Text {
            visible: root.playlistSubmenuOpen && (!tidalService || !tidalService.playlists.length)
            width: parent.width
            height: Style.spacing.popupRowHeight
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
            textFormat: Text.PlainText
            text: tidalService && tidalService.playlistsLoading ? "Loading playlists…" : "No playlists found"
            color: Color.muted
            font.family: Style.font.family
            font.pixelSize: Style.font.caption
          }
        }
      }
      Rectangle {
        id: settingsOverlay
        visible: root.settingsOpen
        anchors.fill: parent
        z: 5
        clip: true
        color: Color.popups.background
        radius: Style.cornerRadius
        border.color: Color.muted
        border.width: Style.normalBorderWidth
        MouseArea {
          objectName: "tidalSettingsBackdropMouseArea"
          anchors.fill: parent
          acceptedButtons: Qt.AllButtons
        }
        Column {
          id: settingsColumn
          anchors.left: parent.left
          anchors.right: parent.right
          anchors.top: parent.top
          anchors.margins: Style.spacing.popupPadding
          spacing: Style.spacing.lg
          Item {
            width: parent.width
            height: Math.max(Style.spacing.controlHeight, settingsTitle.implicitHeight)
            Text {
              id: settingsTitle
              anchors.left: parent.left
              anchors.verticalCenter: parent.verticalCenter
              text: "Settings"
              textFormat: Text.PlainText
              color: Color.foreground
              font.family: Style.font.family
              font.pixelSize: Style.font.title
              font.bold: true
            }
            Ui.PanelActionButton {
              id: closeButton
              objectName: "tidalSettingsCloseButton"
              anchors.right: parent.right
              anchors.verticalCenter: parent.verticalCenter
              size: Style.spacing.controlHeight
              iconText: "󰅖"
              tooltipText: "Close"
              Accessible.role: Accessible.Button
              Accessible.name: tooltipText
              onClicked: root.settingsOpen = false
            }
          }
          Text { text: "Audio quality for the next track"; textFormat: Text.PlainText; color: Color.muted; font.family: Style.font.family; font.pixelSize: Style.font.caption }
          Ui.ButtonGroup {
            options: [{value:"HI_RES_LOSSLESS",label:"Hi-Res"},{value:"LOSSLESS",label:"Lossless"},{value:"HIGH",label:"High AAC"}]
            value: tidalService ? tidalService.preferredAudioQuality : "LOSSLESS"
            onChanged: function(v) { if (tidalService) tidalService.setAudioQuality(v) }
          }
          Text { visible: !!tidalService && !!tidalService.trackTitle; textFormat: Text.PlainText; text: tidalService ? "Playing: " + tidalService.audioQuality : ""; color: Color.muted; font.family: Style.font.family; font.pixelSize: Style.font.caption }
          PanelSeparator { width: parent.width }
          Text { text: "Daemon"; textFormat: Text.PlainText; color: Color.foreground; font.family: Style.font.family; font.pixelSize: Style.font.body; font.bold: true }
          Text { width: parent.width; textFormat: Text.PlainText; text: !tidalService ? "Unavailable" : ((tidalService.daemonSocket && tidalService.daemonSocket.connected ? "Connected" : "Disconnected") + " · " + (tidalService.daemonBinaryExists ? "Binary ready" : "Binary missing")); color: tidalService && tidalService.daemonSocket && tidalService.daemonSocket.connected ? Color.accent : Color.urgent; font.family: Style.font.family; font.pixelSize: Style.font.caption; wrapMode: Text.WordWrap }
          Text { width: parent.width; textFormat: Text.PlainText; text: tidalService ? tidalService.binaryPath : ""; color: Color.muted; font.family: Style.font.family; font.pixelSize: Style.font.caption; elide: Text.ElideMiddle }
          Row {
            spacing: Style.spacing.md
            Ui.Button { text: "Logout"; onClicked: root.confirmLogout = true }
            Ui.Button { text: "Close"; onClicked: root.settingsOpen = false }
          }
        }
      }
      Ui.ConfirmDialog {
        id: confirmDialog
        anchors.fill: parent
        z: 10
        opened: root.confirmLogout
        message: "Log out of Tidal on this device?"
        confirmText: "Logout"
        onCanceled: root.confirmLogout = false
        onConfirmed: { if (tidalService) tidalService.logout(); root.confirmLogout = false; root.settingsOpen = false }
      }
    }
  }
}
