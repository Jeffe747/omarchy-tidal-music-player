import QtQuick
import qs.Commons

Item {
  id: root
  property var targetList: null
  property real dotSize: Style.space(6)
  property real rightMargin: Style.space(2)
  property real verticalPadding: Style.space(4)
  property real trackWidth: Style.space(12)

  anchors.top: targetList ? targetList.top : undefined
  anchors.bottom: targetList ? targetList.bottom : undefined
  anchors.right: targetList ? targetList.right : undefined
  anchors.rightMargin: rightMargin
  width: trackWidth
  z: 10

  readonly property bool canScroll: !!targetList && targetList.contentHeight > targetList.height
  visible: canScroll && targetList.visible

  readonly property real maxScroll: canScroll ? Math.max(1, targetList.contentHeight - targetList.height) : 1
  readonly property real currentScroll: canScroll ? (targetList.contentY - (targetList.originY || 0)) : 0
  readonly property real scrollRatio: canScroll ? Math.max(0.0, Math.min(1.0, currentScroll / maxScroll)) : 0.0

  readonly property real availableTravel: Math.max(0, height - dotSize - verticalPadding * 2)
  readonly property real dotY: verticalPadding + (scrollRatio * availableTravel)

  Rectangle {
    id: dot
    anchors.horizontalCenter: parent.horizontalCenter
    y: root.dotY
    width: root.dotSize
    height: root.dotSize
    radius: root.dotSize / 2
    color: trackMouse.pressed ? Color.accent : (trackMouse.containsMouse || (root.targetList && root.targetList.moving) ? Color.foreground : Color.muted)
    opacity: trackMouse.pressed || trackMouse.containsMouse ? 0.95 : ((root.targetList && root.targetList.moving) ? 0.75 : 0.45)

    Behavior on opacity { NumberAnimation { duration: 150 } }
    Behavior on color { ColorAnimation { duration: 150 } }
  }

  MouseArea {
    id: trackMouse
    anchors.fill: parent
    hoverEnabled: true
    cursorShape: Qt.PointingHandCursor
    preventStealing: true

    function updateScroll(mouseY) {
      if (!root.canScroll || root.availableTravel <= 0 || !root.targetList) return
      var clampedY = Math.max(0, Math.min(root.availableTravel, mouseY - root.verticalPadding - root.dotSize / 2))
      var ratio = clampedY / root.availableTravel
      var targetY = (root.targetList.originY || 0) + (ratio * (root.targetList.contentHeight - root.targetList.height))
      root.targetList.contentY = targetY
    }

    onPressed: function(mouse) {
      updateScroll(mouse.y)
    }

    onPositionChanged: function(mouse) {
      if (pressed) {
        updateScroll(mouse.y)
      }
    }

    onWheel: function(wheel) {
      if (root.targetList) {
        root.targetList.flick(0, wheel.angleDelta.y * 5)
      }
    }
  }
}
