import QtQuick
import Quickshell
import qs.Ui as Ui

ShellRoot {
  id: root

  property var widget: null
  property var service: null
  property bool failed: false

  function check(condition, message) {
    if (!condition) {
      root.failed = true
      console.error(message)
    }
  }

  function findObject(object, name) {
    if (!object) return null
    if (Array.isArray(object) || (typeof object.length === "number" && !("objectName" in object))) {
      for (var k = 0; k < object.length; k++) {
        var entry = findObject(object[k], name)
        if (entry) return entry
      }
      return null
    }
    if (object.objectName === name) return object
    var children = object.children || []
    for (var i = 0; i < children.length; i++) {
      var found = findObject(children[i], name)
      if (found) return found
    }
    var resources = object.resources || []
    for (var j = 0; j < resources.length; j++) {
      var resource = resources[j]
      if (resource.objectName === name) return resource
      if ("contentItem" in resource) {
        var content = findObject(resource.contentItem, name)
        if (content) return content
      }
    }
    return null
  }

  QtObject {
    id: shellApi
    property var sharedService: null
    function serviceFor(id) {
      return id === "jaj.tidal" ? sharedService : null
    }
  }

  Ui.PluginBarApi {
    id: barApi
    pluginId: "jaj.tidal"
    moduleName: "jaj.tidal"
    shell: shellApi
    fontFamily: "monospace"
    barSize: 32
  }

  Item {
    id: slot
    implicitWidth: root.widget && root.widget.visible ? root.widget.implicitWidth : 0
    implicitHeight: root.widget && root.widget.visible ? root.widget.implicitHeight : 0
    width: implicitWidth
    height: implicitHeight
  }

  Component.onCompleted: {
    var component = Qt.createComponent("file://" + Quickshell.env("TIDAL_PLUGIN_DIR") + "/BarWidget.qml")
    if (component.status !== Component.Ready) {
      console.error(component.errorString())
      Qt.quit()
      return
    }
    root.widget = component.createObject(slot, { manageIpc: false })
    if (!root.widget) {
      console.error("Tidal widget creation failed")
      Qt.quit()
      return
    }
    root.widget.anchors.fill = slot
    root.check(root.widget.tidalService === null, "An uninjected widget must not create a private service")

    var serviceComponent = Qt.createComponent("file://" + Quickshell.env("TIDAL_TEST_SERVICE"))
    if (serviceComponent.status !== Component.Ready) {
      console.error(serviceComponent.errorString())
      root.failed = true
      return
    }
    root.service = serviceComponent.createObject(null)
    root.check(root.service !== null, "Tidal service creation failed")
    if (root.service) root.service.startAuth()
  }

  Timer {
    id: injectionTimer
    interval: 500
    running: true
    onTriggered: {
      if (!root.widget) return
      root.check(root.widget.visible && slot.width > 0 && slot.height > 0,
                 "Tidal widget must remain visible with nonzero bar-slot dimensions")
      root.widget.bar = barApi
      root.check(root.widget.tidalService === null, "Missing host service must not create a fallback client")
      shellApi.sharedService = root.service
      root.check(root.widget.tidalService === root.service, "Widget must resolve the shared host service")
    }
  }

  Timer {
    interval: 5000
    running: true
    onTriggered: {
      if (root.widget && root.service) {
        if (Quickshell.env("TIDAL_TEST_MODE") === "missing") {
          root.check(!root.service.daemonBinaryExists, "Missing daemons must not be reported as executable")
          root.check(root.service.authError === root.service.buildScriptMessage,
                     "Missing bundle and fallback must show an actionable error")
          root.check(root.service.pendingCommands.length === 0, "Missing binaries must clear deferred commands")
          root.check(!root.service.authPending && !root.service.searching,
                     "Missing binaries must clear pending UI state")
          if (!root.failed) console.log("TIDAL_QML_TEST_PASS")
          root.service.destroy()
          Qt.quit()
          return
        }
        root.check(root.service.binaryPath === Quickshell.env("TIDAL_TEST_EXPECTED_BINARY"),
                   "Daemon selection must prefer an executable bundle, then the development fallback")
        if (!root.service.authenticated) {
          for (var i = 0; i < root.service.resources.length; i++) {
            var resource = root.service.resources[i]
            if ("command" in resource) console.log("process", resource.command, resource.running)
            if ("connected" in resource) console.log("socket", resource.connected, resource.path)
            if ("interval" in resource) console.log("timer", resource.interval, resource.running)
          }
        }
        root.check(root.service.authenticated,
                   "Login command issued before connection was lost: binary=" + root.service.binaryPath
                   + ", exists=" + root.service.daemonBinaryExists + ", error=" + root.service.authError)
        root.check(root.service.pendingCommands.length === 0, "Commands must drain after connection")
        root.check(root.widget.isAuthenticated, "Widget must react to shared authentication state")
        root.check(!root.service.favoritesLoading && root.service.favoritesError === "",
                   "Favorites request must finish without error")
        root.check(root.service.favorites.length === 1, "Favorites must load after authentication")
        var list = findObject(root.widget, "tidalFavoritesList")
        root.check(list !== null && list.count === 1 && list.height > 0 && list.clip,
                   "Favorites must render in a bounded, clipped list")
        var click = findObject(list, "tidalFavoriteClick")
        root.check(click !== null, "Favorite row must expose a click target")
        if (click) click.clicked(null)
        root.service.searchResults = [{ id: 42, title: "Test song", artist: "Test artist" }]
        delegateTimer.start()
      } else {
        root.check(false, "Widget and service must both load")
        Qt.quit()
      }
    }
  }

  Timer {
    id: delegateTimer
    interval: 500
    onTriggered: {
      root.check(root.service.currentTrackId === 42 && root.service.isPlaying,
                 "Favorite click must dispatch play_track and receive playback_started")
      root.check(root.widget.barLabelText().indexOf("Test song") !== -1,
                 "Now-playing label must react to daemon metadata")
      var title = findObject(root.widget, "tidalNowPlayingTitle")
      var art = findObject(root.widget, "tidalNowPlayingArt")
      root.check(title !== null && title.text === "Test song",
                 "Hero card must render the current track title")
      root.check(art !== null && art.source.toString() === root.service.trackArtUrl && art.status === Image.Ready,
                 "Hero artwork must load and render without errors")
      root.check(root.service.trackAlbum === "Test album" && root.service.trackDuration === 180,
                 "Album and duration must follow playback status")
      var state = findObject(root.widget, "tidalFavoritesState")
      root.service.handleDaemonMessage({ type: "favorites_error", error: "Favorites unavailable" })
      root.check(state !== null && state.visible && state.text === "Favorites unavailable",
                 "Favorites errors must render visibly")
      root.service.loadFavorites()
      root.check(root.service.favoritesLoading && state.text === "Loading favorites...",
                 "Retrying favorites must render the loading state")
      root.service.handleDaemonMessage({ type: "favorites_loaded", tracks: [] })
      root.check(!root.service.favoritesLoading && root.service.favoritesError === ""
                 && state.text.indexOf("No favorite tracks") !== -1,
                 "An empty favorites response must clear errors and show the empty state")
      root.service.handleDaemonMessage({ type: "playback_error", error: "Stream unavailable" })
      var playbackError = findObject(root.widget, "tidalPlaybackError")
      root.check(playbackError !== null && playbackError.visible && playbackError.text === "Stream unavailable",
                 "Playback failures must render visibly")
      var library = findObject(root.widget, "tidalLibraryView")
      var search = findObject(root.widget, "tidalSearchField")
      root.check(library !== null && search !== null, "Library and search controls must exist")
      if (library && search) {
        root.service.favorites = [{id: 1, title: "One"}, {id: 2, title: "Two"}, {id: 3, title: "Three"}]
        for (var n = 0; n < 50; n++) root.widget.moveCursor(1)
        root.check(root.widget.keyboardIndex === 2, "Keyboard cursor must clamp to the active list")
        root.widget.libraryTab = "playlists"
        root.service.playlists = [{uuid: "a", title: "Playlist A"}]
        root.widget.keyboardIndex = 0
        root.check(library.view === "playlists", "Playlists must become the active view")
        search.text = "Miles"
        root.check(library.view === "search" && !library.children[0].visible,
                   "Search must hide library tabs")
        root.check(root.service.searching && !root.service.searchCompleted,
                   "Search debounce must show a pending state")
        root.widget.goBack()
        root.check(search.text === "" && library.view === "playlists",
                   "Escape must clear search and restore the previous library tab")
        root.service.search("old")
        var oldId = root.service.searchRequestId
        root.service.search("new")
        root.service.handleDaemonMessage({type: "search_results", request_id: oldId, results: [{id: 99}]})
        root.check(root.service.searchResults.length === 0 && root.service.searching,
                   "Stale search results must be ignored")
        root.service.clearSearch()
      }
      root.check(root.widget.visible && slot.width > 0 && slot.height === barApi.barSize,
                 "Authenticated widget must keep a nonzero bar slot")
      barApi.vertical = true
      root.check(slot.width === barApi.barSize && slot.height > 0,
                 "Vertical bar must use the bar size for widget width")
      shellApi.sharedService = null
      root.check(root.widget.tidalService === null && root.widget.visible && slot.width > 0,
                 "Removing the service must not hide the widget")
      if (!root.failed) console.log("TIDAL_QML_TEST_PASS")
      if (root.service) root.service.destroy()
      Qt.quit()
    }
  }
}
