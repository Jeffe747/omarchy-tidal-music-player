import QtQuick
import QtQuick.Shapes
import qs.Commons

Item {
  id: root
  property real iconSize: Style.font.icon
  property color color: Color.foreground
  implicitWidth: iconSize * 1.5
  implicitHeight: iconSize
  Shape {
    anchors.fill: parent
    preferredRendererType: Shape.GeometryRenderer
    ShapePath {
      strokeWidth: -1
      fillColor: root.color
      startX: 4 * root.width / 24
      startY: 0.6000000000000001 * root.height / 16
      PathLine { x: 7.4 * root.width / 24; y: 4 * root.height / 16 }
      PathLine { x: 4 * root.width / 24; y: 7.4 * root.height / 16 }
      PathLine { x: 0.6000000000000001 * root.width / 24; y: 4 * root.height / 16 }
      PathLine { x: 4 * root.width / 24; y: 0.6000000000000001 * root.height / 16 }
    }
    ShapePath {
      strokeWidth: -1
      fillColor: root.color
      startX: 12 * root.width / 24
      startY: 0.6000000000000001 * root.height / 16
      PathLine { x: 15.4 * root.width / 24; y: 4 * root.height / 16 }
      PathLine { x: 12 * root.width / 24; y: 7.4 * root.height / 16 }
      PathLine { x: 8.6 * root.width / 24; y: 4 * root.height / 16 }
      PathLine { x: 12 * root.width / 24; y: 0.6000000000000001 * root.height / 16 }
    }
    ShapePath {
      strokeWidth: -1
      fillColor: root.color
      startX: 20 * root.width / 24
      startY: 0.6000000000000001 * root.height / 16
      PathLine { x: 23.4 * root.width / 24; y: 4 * root.height / 16 }
      PathLine { x: 20 * root.width / 24; y: 7.4 * root.height / 16 }
      PathLine { x: 16.6 * root.width / 24; y: 4 * root.height / 16 }
      PathLine { x: 20 * root.width / 24; y: 0.6000000000000001 * root.height / 16 }
    }
    ShapePath {
      strokeWidth: -1
      fillColor: root.color
      startX: 12 * root.width / 24
      startY: 8.6 * root.height / 16
      PathLine { x: 15.4 * root.width / 24; y: 12 * root.height / 16 }
      PathLine { x: 12 * root.width / 24; y: 15.4 * root.height / 16 }
      PathLine { x: 8.6 * root.width / 24; y: 12 * root.height / 16 }
      PathLine { x: 12 * root.width / 24; y: 8.6 * root.height / 16 }
    }
  }
}
