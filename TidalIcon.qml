import QtQuick
import qs.Commons

Canvas {
  id: root

  property real iconSize: Style.font.icon
  property color color: Color.foreground

  implicitWidth: iconSize * 1.5
  implicitHeight: iconSize

  onColorChanged: requestPaint()
  onWidthChanged: requestPaint()
  onHeightChanged: requestPaint()

  onPaint: {
    var ctx = getContext("2d")
    ctx.clearRect(0, 0, width, height)
    ctx.save()
    ctx.scale(width / 24, height / 16)
    ctx.fillStyle = root.color

    function diamond(cx, cy) {
      ctx.beginPath()
      ctx.moveTo(cx, cy - 4)
      ctx.lineTo(cx + 4, cy)
      ctx.lineTo(cx, cy + 4)
      ctx.lineTo(cx - 4, cy)
      ctx.closePath()
      ctx.fill()
    }

    diamond(12, 4)
    diamond(12, 12)
    diamond(4, 4)
    diamond(20, 4)
    ctx.restore()
  }
}
