import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ColumnLayout {
    id: guideScreen
    property var guide: ({selected_by_default:false, folders:[], message:""})
    property bool busy: false
    signal openPublisherRequested(string publisher)
    signal copyPathRequested(string path)
    signal refreshRequested()
    signal skipRequested()
    signal doneRequested()
    spacing: 14
    Label {text:"CurseForge (optional)";font.pixelSize:27;font.bold:true;color:"#f2f6fc";Layout.fillWidth:true;wrapMode:Text.WordWrap}
    InfoText {Layout.fillWidth:true;text:"CurseForge is a separate app for finding and updating WoW addons. You do not need it to play. This optional guide is off by default."}
    InfoText {Layout.fillWidth:true;text:"CurseForge does not publish a checksum or signature for its Linux download, so Deckhand will not download it for you. Nothing opens or installs unless you choose it yourself."}
    InfoText {Layout.fillWidth:true;text:"1. Open CurseForge's official download page below. Choose Linux and download its AppImage yourself."}
    ActionButton {objectName:"curseForgeOfficialPage";Layout.fillWidth:true;text:"Open CurseForge's Linux download page";enabled:!guideScreen.busy;onClicked:guideScreen.openPublisherRequested("curseforge")}
    InfoText {Layout.fillWidth:true;text:"2. If you want Gear Lever, open Discover on the desktop, find Gear Lever and choose its Flathub listing. Review its download and permissions before choosing Install. It may need an additional Flatpak runtime. Deckhand will not install it."}
    ActionButton {objectName:"gearLeverOfficialPage";Layout.fillWidth:true;text:"View Gear Lever on Flathub";enabled:!guideScreen.busy;onClicked:guideScreen.openPublisherRequested("gearlever")}
    InfoText {Layout.fillWidth:true;text:"3. Open Gear Lever and add the AppImage you downloaded. Review its trust prompt yourself, then use its option to add the app to your app menu. Start CurseForge from your app menu."}
    InfoText {Layout.fillWidth:true;text:"4. In CurseForge, add your installed WoW game. Use the matching game folder below if it asks for the game, or the matching AddOns folder if it asks for addons. Deckhand does not change CurseForge's settings or promise that it will detect a game."}
    Label {text:"Your detected game and AddOns folders";font.pixelSize:22;color:"#f2f6fc";Layout.fillWidth:true;wrapMode:Text.WordWrap}
    InfoText {objectName:"curseForgeFolderStatus";Layout.fillWidth:true;text:guideScreen.guide.message||"No installed WoW edition was detected. Finish installing a game or reconnect its drive, then refresh detected folders."}
    Repeater {
        model:guideScreen.guide.folders||[]
        ColumnLayout {
            required property var modelData
            Layout.fillWidth:true;spacing:6
            Label {text:modelData.edition;color:"#edf3fa";font.bold:true;Layout.fillWidth:true}
            InfoText {Layout.fillWidth:true;visible:!!modelData.game_folder;text:"Game folder: "+(modelData.game_folder||"")}
            ActionButton {text:"Copy game folder";Layout.fillWidth:true;visible:!!modelData.game_folder;enabled:!guideScreen.busy;onClicked:guideScreen.copyPathRequested(modelData.game_folder)}
            InfoText {Layout.fillWidth:true;visible:!!modelData.addons_folder;text:modelData.edition+" AddOns: "+(modelData.addons_folder||"")}
            ActionButton {text:"Copy AddOns folder";Layout.fillWidth:true;visible:!!modelData.addons_folder;enabled:!guideScreen.busy;onClicked:guideScreen.copyPathRequested(modelData.addons_folder)}
            InfoText {Layout.fillWidth:true;text:modelData.message||""}
        }
    }
    ActionButton {objectName:"refreshCurseForgeFolders";Layout.fillWidth:true;text:"Refresh detected folders";enabled:!guideScreen.busy;onClicked:guideScreen.refreshRequested()}
    InfoText {Layout.fillWidth:true;text:"This guide does not download, install, update or configure CurseForge or Gear Lever. Existing addons stay in place. CurseForge's publisher officially supports Ubuntu; using it on other systems may need extra steps."}
    ActionButton {objectName:"skipCurseForgeGuide";Layout.fillWidth:true;text:"Continue without CurseForge";enabled:!guideScreen.busy;onClicked:guideScreen.skipRequested()}
    ActionButton {objectName:"finishCurseForgeGuide";Layout.fillWidth:true;primary:true;text:"Done";enabled:!guideScreen.busy;onClicked:guideScreen.doneRequested()}
}
