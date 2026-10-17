import QtQuick
import Quickshell.Io
import qs.Commons
import qs.Ui as Ui

// Clipboard-safe single-line editor for marketplace reviews 11726 / 11229 / 10880.
//
// Affected Qt builds (6.11.2 / 6.12.0) connect QClipboard::dataChanged to
// QQuickTextInput::q_canPasteChanged(), which calls QMimeData::text() and pulls
// the entire foreign clipboard offer on every change — with no byte cap and no
// overall deadline — even when nothing is pasted, and before any validator or
// maximumLength runs. A read-only editor is skipped by both that path and the
// canPaste getter, so the shared shell can never be stalled or exhausted by a
// clipboard provider. This field therefore stays read-only for its whole
// lifetime; text edits and paste are bounded and handled explicitly here.
Ui.TextField {
  id: root

  readOnly: true
  Component.onCompleted: if (root.contentItem) root.contentItem.textFormat = Text.PlainText
  maximumLength: 1024
  cursorVisible: activeFocus

  cursorDelegate: Rectangle {
    id: caret
    width: 2
    radius: 1
    height: Math.max(1, root.height - root.topPadding - root.bottomPadding)
    color: root.color
    visible: root.activeFocus
    opacity: 1
    SequentialAnimation on opacity {
      running: caret.visible && Qt.styleHints.cursorFlashTime > 0
      loops: Animation.Infinite
      NumberAnimation { to: 0.0; duration: Math.max(1, Qt.styleHints.cursorFlashTime / 2) }
      NumberAnimation { to: 1.0; duration: Math.max(1, Qt.styleHints.cursorFlashTime / 2) }
    }
  }
  selectByMouse: true
  clip: true

  function replaceSelection(replacement) {
    var start = selectionStart
    var end = selectionEnd
    var room = Math.max(0, maximumLength - (text.length - (end - start)))
    var bounded = replacement.slice(0, room)
    var last = bounded.charCodeAt(bounded.length - 1)
    if (last >= 0xd800 && last <= 0xdbff) bounded = bounded.slice(0, -1)
    text = text.slice(0, start) + bounded + text.slice(end)
    cursorPosition = start + bounded.length
    root.textEdited()
  }

  function isPasteGesture(event) {
    if (event.key === Qt.Key_V && (event.modifiers & (Qt.ControlModifier | Qt.MetaModifier)))
      return true
    if (event.key === Qt.Key_Insert && (event.modifiers & Qt.ShiftModifier)
        && !(event.modifiers & (Qt.ControlModifier | Qt.MetaModifier | Qt.AltModifier)))
      return true
    return false
  }

  function requestPaste() {
    pasteProc.running = false
    pasteProc.running = true
  }

  function handleSpecialKey(event) { return false }

  function handleKey(event) {
    if (root.handleSpecialKey(event)) return
    if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
      root.accepted()
      event.accepted = true
      return
    }
    if (isPasteGesture(event)) {
      requestPaste()
      event.accepted = true
      return
    }
    var shortcut = event.modifiers & (Qt.ControlModifier | Qt.MetaModifier | Qt.AltModifier)
    if (shortcut) {
      if (event.key === Qt.Key_A && (event.modifiers & Qt.ControlModifier)) {
        selectAll()
        event.accepted = true
      }
      return
    }
    if (event.key === Qt.Key_Backspace || event.key === Qt.Key_Delete) {
      if (selectionStart === selectionEnd) {
        var position = cursorPosition
        if (event.key === Qt.Key_Backspace && position > 0) {
          var previous = text.charCodeAt(position - 1)
          var count = previous >= 0xdc00 && previous <= 0xdfff && position > 1 ? 2 : 1
          select(position - count, position)
        } else if (event.key === Qt.Key_Delete && position < text.length) {
          var next = text.charCodeAt(position)
          var count = next >= 0xd800 && next <= 0xdbff && position + 1 < text.length ? 2 : 1
          select(position, position + count)
        }
      }
      replaceSelection("")
    } else if (event.text && !/[\x00-\x1f\x7f]/.test(event.text)) {
      replaceSelection(event.text)
    } else {
      return
    }
    event.accepted = true
  }

  Keys.priority: Keys.BeforeItem
  Keys.onPressed: function(event) { root.handleKey(event) }

  Process {
    id: pasteProc
    running: false
    command: ["sh", "-c", "timeout -k 1 2 wl-paste -n 2>/dev/null | head -c 512"]
    stdout: StdioCollector {
      id: pasteOut
      waitForEnd: true
      onStreamFinished: {
        var t = pasteOut.text.replace(/[\x00-\x1f\x7f]+/g, " ").replace(/^\s+|\s+$/g, "")
        if (t.length > 0) root.replaceSelection(t)
      }
    }
  }
}
