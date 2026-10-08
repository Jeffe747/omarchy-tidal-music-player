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
        root.service.handleDaemonMessage({
          type: "state_change", authenticated: true, is_playing: true,
          track_title: "Test song", track_artist: "Test artist"
        })
        root.check(root.widget.barLabelText().indexOf("Test song") !== -1,
                   "Now-playing label must react to shared metadata")
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
    interval: 100
    onTriggered: {
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
