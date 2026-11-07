import QtQuick
import QtQuick.Controls
Button {
    id: control
    implicitHeight: Math.max(48, contentItem.implicitHeight + 20)
    activeFocusOnTab: true
    hoverEnabled: true
    Keys.onReturnPressed: function(event) { if (enabled) clicked(); event.accepted = true }
    Keys.onEnterPressed: function(event) { if (enabled) clicked(); event.accepted = true }
    font.pixelSize: 16
    property bool primary: false
    property bool danger: false
    contentItem: Text {
        text: control.text
        color: control.enabled ? "#f4f7fb" : "#7b899e"
        font: control.font
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        wrapMode: Text.WordWrap
        elide: Text.ElideNone
    }
    background: Rectangle {
        radius: 8
        color: control.down ? "#37527b" : control.danger ? "#71313c" : control.primary ? "#235d83" : "#233147"
        opacity: control.enabled ? 1 : 0.55
        border.width: control.activeFocus ? 3 : 1
        border.color: control.activeFocus ? "#72e1da" : control.hovered ? "#6aa8bd" : "#40506a"
    }
    leftPadding: 18; rightPadding: 18
}

