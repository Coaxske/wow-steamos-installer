import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Item {
    id: root
    property string failure: ""
    property bool busy: false
    property bool photo: false
    property bool example: false
    property var report: null
    property string reportText: ""
    property int question: 0
    property int photoPage: 0
    property bool asking: false
    property bool details: false
    property bool advanceAfterSave: false
    signal request(string action, var args)
    signal leave()
    signal copy(string text)
    property var manual: report ? report.manual : []
    property var automatic: report ? report.automatic : []
    property var allRows: automatic.map(function(r){return {label:"AUTO · "+autoName(r.id),outcome:r.outcome}}).concat(manual.map(function(r){return {label:"YOU · "+manualName(r.id),outcome:r.outcome}}))
    property int pages: Math.max(1,Math.ceil(allRows.length/22))
    function focusFirst(){
        function seek(item){if(!item.visible||!item.enabled)return null;if(item.activeFocusOnTab)return item;for(var n=0;n<item.children.length;n++){var f=seek(item.children[n]);if(f)return f}return null}
        var f=seek(root);if(f)f.forceActiveFocus();
    }
    function reveal(item){
        var flick=testScroll.contentItem;if(!item||!flick||!flick.contentItem)return;
        var p=item,inside=false;while(p){if(p===flick.contentItem){inside=true;break}p=p.parent}if(!inside)return;
        var point=item.mapToItem(flick.contentItem,0,0);
        if(point.y<flick.contentY)flick.contentY=Math.max(0,point.y-10);
        else if(point.y+item.height>flick.contentY+flick.height)flick.contentY=Math.max(0,Math.min(flick.contentHeight-flick.height,point.y+item.height-flick.height+10));
    }
    onPhotoChanged: Qt.callLater(focusFirst)
    onAskingChanged: Qt.callLater(focusFirst)
    function status(s){return {pass:"PASS",attention:"CHECK",not_tested:"NOT TESTED",fail:"PROBLEM",skipped:"SKIPPED"}[s]||"NOT TESTED"}
    function tint(s){return s==="pass"?"#96e4b1":s==="fail"?"#ffacac":s==="attention"?"#ffe099":"#c0cbd7"}
    function autoName(id){return {state:"Saved progress",recovery:"Unfinished changes",scan:"Known-location scan",steam:"Native Steam",proton:"Proton available",files:"Game files found",shortcuts:"Shortcut checks",duplicates:"Exact duplicates"}[id]||"Check"}
    function autoHelp(id){return {
        state:"CHECK: saved progress could not be read. It was left untouched.",
        recovery:"CHECK: a saved operation needs attention. Return Home to resume it.",
        scan:"CHECK: the known-location scan had warnings or could not finish. Use Find installed games for details. This is not a whole-drive search.",
        steam:"CHECK: no usable native Steam installation was found. Open Steam and sign in once. Flatpak Steam is not supported yet.",
        proton:"CHECK: the selected tool is missing, or no installed tool was found. Install it in Steam or review your compatibility choice.",
        files:"PASS means game files were found, not that downloads are complete or games run. NOT TESTED is expected before installation.",
        shortcuts:"CHECK: a managed tile is missing or has a missing target, wrong environment, or unavailable Proton mapping. NOT TESTED means there are no managed tiles to check.",
        duplicates:"CHECK means exact repeated game targets and launch options were found. Other kinds of duplicate or unrecognized tiles can still exist. Nothing was removed."
    }[id]||""}
    function edition(id){return {retail:"Retail",forever:"Forever",era:"Classic Era",progression:"Classic Progression",anniversary:"Anniversary",ptr:"Retail PTR",ptr_next:"Retail PTR 2",beta:"Retail Beta",classic_ptr:"Classic PTR",classic_beta:"Classic / Forever Beta",era_ptr:"Era PTR",alpha:"Retail Alpha",titan:"Titan Reforged",other:"Other edition"}[id]||"Other edition"}
    function manualName(id){return id.indexOf("game_")===0?"Game: "+edition(id.slice(5)):{startup:"App opens",controls:"Controller + mouse",navigation:"Back / cancel / choices",battle_net:"Battle.net login / update",addons:"Addon manager / update",gaming_mode:"Gaming Mode",repair:"Repeat shortcut repair",fresh_install:"Fresh install",clean_install:"Removal / reinstall",restore:"Settings restore"}[id]||"Test"}
    function prompt(id){if(id.indexOf("game_")===0)return "Open the "+edition(id.slice(5))+" Steam tile. Check the right edition opens through Battle.net, then enter the game and play briefly. A launch request or login screen alone is not a pass. Test one copy; other copies remain untested.";return {
        startup:"Did the extracted single app file open by double-click, with readable text and no blank window? No terminal command should be needed.",
        controls:"Use all four D-pad and left-stick directions, hold a direction to repeat, then alternate with mouse clicks. Test before and after exiting Steam. Look for skipped controls, double presses, or input continuing while another app has focus. If you did not test a controller, skip this check.",
        navigation:"Try dropdowns, scrolling, the folder chooser, Back and Cancel. Can you leave each screen without being stuck or making an unwanted change? Cancel a review and check that nothing changed.",
        battle_net:"Open Battle.net, sign in there and check for updates. Reopen it to check remembered sign-in if enabled. Setup must not ask for your account password. KDE Connect is optional. A successful window launch alone is not enough.",
        addons:"Open your chosen addon manager from Steam. Confirm it finds the right game folders and successfully checks or updates an addon. Skip if you do not use addons.",
        gaming_mode:"In Gaming Mode, open a game tile and check its controller layout. Come back here after testing. Skip if this system does not use Gaming Mode.",
        repair:"Run shortcut repair twice for the same copy. Check that it adds no duplicate tiles, retains your names and controller layouts, and leaves unrelated games alone. Only mark Worked after inspecting Steam.",
        fresh_install:"Optional. Use a separate new copy to test Battle.net installation, game download, sign-in and the final tiles. No operating-system wipe is needed. Skip unless you completed that flow.",
        clean_install:"Optional and destructive. Only test with a disposable copy: cancel the removal preview first, then complete a reviewed removal/reinstall. Confirm only the chosen copy is removed. This test screen never removes anything itself.",
        restore:"Optional. After a reinstall with settings preservation, check the restored addons and game settings in-game. Existing settings must not be silently overwritten. Skip if you did not test preservation and restore."
    }[id]||""}
    function count(rows,s){return rows.filter(function(r){return r.outcome===s}).length}
    function outcomeSummary(rows){return count(rows,"pass")+" passed · "+(count(rows,"fail")+count(rows,"attention"))+" to check · "+count(rows,"not_tested")+" untested · "+count(rows,"skipped")+" skipped"}
    function systemName(){return report?({bazzite:"Bazzite",steam_os:"SteamOS",other_linux:"Other Linux"}[report.system]+" · "+{wayland:"Wayland",x11:"X11",other:"Session unknown"}[report.session]):""}
    function tools(){return report?report.tools.map(function(t){return {valve11:"Valve 11",valve10:"Valve 10",experimental:"Experimental",cachy:"Cachy",ge:"GE",other:"Other"}[t]||"Other"}).join(", "):""}
    function receive(action,d){
        report=d.report;reportText=d.text||"";
        if(action==="device_run"){question=0;asking=false;photo=false;photoPage=0}
        if(action==="device_mark"&&advanceAfterSave){advanceAfterSave=false;if(question+1<manual.length)question++;else asking=false;}
        if(question>=manual.length)question=0;
    }
    function mark(value){advanceAfterSave=true;request("device_mark",{test:manual[question].id,outcome:value})}
    function showPhoto(){asking=false;photoPage=0;photo=true;request("device_result",{})}
    function photoFits(){
        var valid=true;var nav=[];
        function check(item){if(!item.visible)return;if(item.objectName==="photoLine"){
            var pt=item.mapToItem(root,0,0);if(pt.y<0||pt.y+item.height>photoButtons.mapToItem(root,0,0).y-4)valid=false;
        }if(item.objectName==="photoNavButton"){var p=item.mapToItem(root,0,0);nav.push({x:p.x,right:p.x+item.width});if(p.x<0||p.x+item.width>root.width)valid=false;}for(var n=0;n<item.children.length;n++)check(item.children[n])}
        check(photoCard);nav.sort(function(a,b){return a.x-b.x});for(var i=1;i<nav.length;i++)if(nav[i].x<nav[i-1].right)valid=false;return valid&&photoButtons.mapToItem(root,0,0).y+photoButtons.height<=root.height;
    }
    function seedMany(){var r=JSON.parse(JSON.stringify(report));r.manual=r.manual.concat(["ptr","ptr_next","beta","classic_ptr","classic_beta","era_ptr","alpha","titan","other"].map(function(id){return {id:"game_"+id,outcome:"not_tested"}}));report=r;photoPage=pages-1;}
    function seedPreview(){
        example=true;
        report={version:Qt.application.version,source:"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",run:1000000000,system:"bazzite",session:"wayland",scoped:true,installation_count:1,game_count:3,tools:["valve11"],stale:false,
        automatic:["state","recovery","scan","steam","proton","files","shortcuts","duplicates"].map(function(id){return {id:id,outcome:"pass",count:0}}),
        manual:["startup","controls","navigation","battle_net","addons","gaming_mode","repair","fresh_install","clean_install","restore","game_retail","game_forever","game_era"].map(function(id,index){return {id:id,outcome:index<5?"pass":index===5?"fail":"not_tested"}})};
    }
    Rectangle{anchors.fill:parent;color:"#101825"}
    ColumnLayout{
        visible:!root.photo;anchors.fill:parent;anchors.margins:24;spacing:12
        Label{text:root.asking?"Try it on your device":"Test this build";color:"#f2f6fc";font.pixelSize:28;font.bold:true;Layout.fillWidth:true;wrapMode:Text.WordWrap}
        InfoText{visible:root.failure!=="";Layout.fillWidth:true;text:root.failure;color:"#ffacac"}
        ScrollView{
            id:testScroll
            Layout.fillWidth:true;Layout.fillHeight:true;clip:true;contentWidth:availableWidth
            ColumnLayout{
                width:parent.width;spacing:14
                InfoText{visible:!root.asking;Layout.fillWidth:true;text:"Run the automatic checks, then record what you try. Your results are saved on this device. Nothing is uploaded."}
                InfoText{visible:!root.asking;Layout.fillWidth:true;text:"The check reads your setup and saves a separate results record. It does not install, launch, repair, update or remove games. It can run while Steam is open; changes made during the scan may require another check.";font.pixelSize:15}
                ActionButton{visible:!root.asking;text:root.report?"Start a new check":"Run automatic checks";primary:true;Layout.fillWidth:true;enabled:!root.busy;onClicked:{if(root.report)resetDialog.open();else root.request("device_run",{})}}
                InfoText{visible:!root.asking&&!!root.report;Layout.fillWidth:true;text:root.report?"Automatic: "+root.outcomeSummary(root.automatic)+"\nYour observations: "+root.outcomeSummary(root.manual):""}
                InfoText{visible:!!root.report&&root.report.stale;Layout.fillWidth:true;text:"Setup changed after this check. The automatic results may be out of date. Finish your changes, then start a new check.";color:"#ffe099"}
                ActionButton{visible:!root.asking&&!!root.report;text:"Record what you tested";Layout.fillWidth:true;enabled:!root.busy;onClicked:{root.question=0;root.asking=true}}
                ActionButton{objectName:"showTestPhoto";visible:!root.asking&&!!root.report;text:"Show results for a photo";Layout.fillWidth:true;enabled:!root.busy;onClicked:root.showPhoto()}
                CheckBox{visible:!root.asking&&!!root.report;text:"Explain automatic results";checked:root.details;onToggled:root.details=checked}
                Repeater{model:!root.asking&&root.details?root.automatic:[];InfoText{required property var modelData;Layout.fillWidth:true;text:root.autoName(modelData.id)+": "+root.status(modelData.outcome)+"\n"+root.autoHelp(modelData.id);font.pixelSize:15}}
                InfoText{visible:root.asking&&root.manual.length>0;Layout.fillWidth:true;font.pixelSize:22;text:root.manual.length?(root.question+1)+" of "+root.manual.length+" · "+root.manualName(root.manual[root.question].id):""}
                InfoText{visible:root.asking&&root.manual.length>0;Layout.fillWidth:true;text:root.manual.length?root.prompt(root.manual[root.question].id):""}
                InfoText{visible:root.asking&&root.manual.length>0;Layout.fillWidth:true;text:root.manual.length?"Recorded: "+root.status(root.manual[root.question].outcome)+". Only mark Worked for something you actually tested on this build.":"";font.pixelSize:15}
                Flow{visible:root.asking;Layout.fillWidth:true;spacing:12
                    ActionButton{objectName:"testWorked";text:"Worked";primary:true;enabled:!root.busy;onClicked:root.mark("pass")}
                    ActionButton{text:"Problem";enabled:!root.busy;onClicked:root.mark("fail")}
                    ActionButton{text:"Skip";enabled:!root.busy;onClicked:root.mark("skipped")}
                    ActionButton{text:"Not tested yet";enabled:!root.busy;onClicked:root.mark("not_tested")}
                }
                Flow{visible:root.asking;Layout.fillWidth:true;spacing:12
                    ActionButton{text:"Previous";enabled:root.question>0&&!root.busy;onClicked:root.question--}
                    ActionButton{text:"Results";enabled:!root.busy;onClicked:root.asking=false}
                }
                ActionButton{visible:!root.asking;text:"Return to setup";enabled:!root.busy;onClicked:root.leave()}
                InfoText{Layout.fillWidth:true;text:"Test the normal setup first, then come back here to record it. Fresh install, removal and restore checks are optional. There is no need to wipe your system.";font.pixelSize:14}
            }
        }
    }
    ColumnLayout{
        id:photoCard;objectName:"testPhotoCard";visible:root.photo;anchors.fill:parent;anchors.margins:18;spacing:8
        RowLayout{Layout.fillWidth:true
            Label{text:root.example?"Example — test results":"WoW installer — test results";font.pixelSize:23;font.bold:true;color:"#ffffff";Layout.fillWidth:true}
            Label{text:(root.photoPage+1)+" / "+root.pages;font.pixelSize:18;color:"#ffffff"}
        }
        InfoText{Layout.fillWidth:true;font.pixelSize:15;text:root.report?root.report.version+" · Build "+root.report.source.slice(0,12)+" · Run "+root.report.run+"\n"+root.systemName()+" · Proton: "+(root.tools()||"none detected")+" · "+(root.report.scoped?"Selected copy":"Detected copies"):""}
        InfoText{Layout.fillWidth:true;font.pixelSize:14;color:root.report&&root.report.stale?"#ffe099":"#c0cbd7";text:root.example?"Sample values for the layout preview. These are not device results.":root.report&&root.report.stale?"OUT OF DATE: setup changed after the automatic check.":"Snapshot only. AUTO = file/configuration checks. YOU = your observations."}
        RowLayout{Layout.fillWidth:true;Layout.fillHeight:true;spacing:18
            Repeater{model:2;ColumnLayout{required property int index;Layout.fillWidth:true;Layout.preferredWidth:1;Layout.alignment:Qt.AlignTop;spacing:4
                Repeater{model:root.allRows.slice(root.photoPage*22+index*11,root.photoPage*22+index*11+11);RowLayout{required property var modelData;objectName:"photoLine";Layout.fillWidth:true;Layout.preferredHeight:26;spacing:4
                    Label{text:modelData.label;color:"#f2f6fc";font.pixelSize:15;Layout.fillWidth:true;elide:Text.ElideRight}
                    Label{text:root.status(modelData.outcome);color:root.tint(modelData.outcome);font.bold:true;font.pixelSize:14;Layout.preferredWidth:91;horizontalAlignment:Text.AlignRight}
                }}
                Item{Layout.fillHeight:true}
            }}
        }
        InfoText{Layout.fillWidth:true;font.pixelSize:14;text:"No account names, emails or personal paths. Photograph only this window."}
        Item{id:photoButtons;Layout.fillWidth:true;Layout.preferredHeight:48
            Row{anchors.left:parent.left;anchors.verticalCenter:parent.verticalCenter;spacing:10
                ActionButton{objectName:"photoNavButton";width:72;text:"Back";onClicked:root.photo=false}
                ActionButton{objectName:"photoNavButton";visible:root.pages>1;width:110;text:"Previous";enabled:root.photoPage>0;onClicked:root.photoPage--}
                ActionButton{objectName:"photoNavButton";visible:root.pages>1;width:80;text:"Next";enabled:root.photoPage+1<root.pages;onClicked:root.photoPage++}
            }
            ActionButton{objectName:"photoNavButton";anchors.right:parent.right;anchors.verticalCenter:parent.verticalCenter;width:140;text:"Copy report";enabled:root.reportText.length>0;onClicked:root.copy(root.reportText)}
        }
    }
    Dialog{id:resetDialog;title:"Start a new check?";modal:true;anchors.centerIn:parent;width:Math.min(480,root.width-32);implicitHeight:250;height:250;standardButtons:Dialog.Ok|Dialog.Cancel
        onAccepted:root.request("device_run",{})
        contentItem: InfoText{text:"This replaces the saved test record and clears your observations. Take a photo or copy the current report first if you want to keep it. Your game setup stays in place."}
    }
}
