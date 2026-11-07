import QtQuick
import QtQuick.Controls

CheckBox {
    activeFocusOnTab: true
    // Steam desktop mappings may send Return instead of SDL's Space.
    // Keep native Space/mouse handling and emit the normal user-toggle signal.
    function activateReturn(event) {
        if (enabled && !event.isAutoRepeat) {
            toggle()
            toggled()
        }
        event.accepted = true
    }
    Keys.onReturnPressed: function(event) { activateReturn(event) }
    Keys.onEnterPressed: function(event) { activateReturn(event) }
}
