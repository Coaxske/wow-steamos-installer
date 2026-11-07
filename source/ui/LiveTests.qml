import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Item {
    id: root
    property bool busy: false
    property bool example: false
    property bool running: false
    property var run: null
    property string reportText: ""
    property string failure: ""
    property string location: ""
    property string notice: ""
    property bool photo: false
    property int photoPage: 0
    signal request(string action, var args)
    signal configure()
    signal leave()
    signal copy(string text)
    function focusFirst(){
        function seek(item){if(!item.visible||!item.enabled)return null;if(item.activeFocusOnTab)return item;for(var n=0;n<item.children.length;n++){var f=seek(item.children[n]);if(f)return f}return null}
        var item=seek(root);if(item)item.forceActiveFocus();
    }
    function reveal(item){
        var flick=liveScroll.contentItem;if(!item||!flick||!flick.contentItem)return;
        var p=item,inside=false;while(p){if(p===flick.contentItem){inside=true;break}p=p.parent}if(!inside)return;
        var point=item.mapToItem(flick.contentItem,0,0);
        if(point.y<flick.contentY)flick.contentY=Math.max(0,point.y-10);
        else if(point.y+item.height>flick.contentY+flick.height)flick.contentY=Math.max(0,Math.min(flick.contentHeight-flick.height,point.y+item.height-flick.height+10));
    }
    onPhotoChanged:Qt.callLater(focusFirst)

    function receive(action,d){failure="";notice=d.message||"";run=d.run;Qt.callLater(focusFirst);reportText=d.text||"";location=d.location||"";if(run&&run.automatic&&running)nextTick.restart();else running=false;}
    function step(args){if(!run)return;var a=args||{};a.token=run.token;a.confirm_forever_beta=beta.checked;request("live_step",a)}
    function resume(args){failure="";running=true;step(args)}
    function seed(){example=true;run={cleanup:false,reinstall:false,game:"",phase:"done",automatic:false,version:Qt.application.version,build:"0123456789ab",title:"Example test results",message:"Preview only. These are not device results.",rows:[{label:"Fresh setup",status:"PASS"},{label:"Installer launch",status:"REQUESTED"},{label:"Launcher handoff",status:"PASS"},{label:"Shortcut repair",status:"PASS"},{label:"Repair adds no duplicates",status:"PASS"},{label:"You: World of Warcraft",status:"NOT TESTED"}]}}
    function seedPhase(phase){seed();var r=JSON.parse(JSON.stringify(run));r.phase=phase;r.cleanup=true;r.reinstall=true;r.title="Live test preview";r.game=phase==="observed"?"World of Warcraft":"";run=r;}
    Timer{id:nextTick;interval:400;onTriggered:if(root.visible&&root.running&&!root.busy)root.step()}
    onVisibleChanged:if(!visible){running=false;nextTick.stop()}
    Rectangle{anchors.fill:parent;color:"#101825"}
    ColumnLayout{anchors.fill:parent;anchors.margins:24;spacing:12
        Label{text:root.photo?(root.example?"Example — live test results":"Live test results"):root.run?root.run.title:"Run an installation test";color:"white";font.pixelSize:26;font.bold:true;Layout.fillWidth:true;wrapMode:Text.WordWrap}
        InfoText{visible:root.photo;Layout.fillWidth:true;text:root.run?root.run.version+" · Build "+root.run.build+" · Page "+(root.photoPage+1):"";font.pixelSize:15}
        ScrollView{id:liveScroll;visible:!root.photo;Layout.fillWidth:true;Layout.fillHeight:true;clip:true;contentWidth:availableWidth
            ColumnLayout{width:parent.width;spacing:12
                InfoText{Layout.fillWidth:true;text:root.run?root.run.message:"This runs real installation, shortcut and launch operations using a new test copy. Choose your games first; review the location and removal option before anything changes."}
                InfoText{visible:root.notice!=="";Layout.fillWidth:true;text:root.notice;color:"#ffe099"}
                InfoText{visible:root.location!=="";Layout.fillWidth:true;text:"Test folder: "+root.location}
                InfoText{visible:root.run&&root.run.phase==="review";Layout.fillWidth:true;text:root.run&&root.run.cleanup?"Removal is ON. At the end, the runner will remove this test copy’s WoW/Battle.net folders and game tiles. It preserves settings first. Existing copies are outside the test.":"Removal is OFF. The new test copy stays installed at the end.";color:"#ffe099"}
                InfoText{visible:root.run&&root.run.phase==="review"&&root.run.reinstall;Layout.fillWidth:true;text:"Reinstall is ON. After the first game checks, the runner preserves settings, removes this test copy and starts a second installation. You will download the games twice.";color:"#ffe099"}
                InfoText{visible:root.failure!=="";Layout.fillWidth:true;text:root.failure;color:"#ffacac"}
                InfoText{visible:!!(root.run&&root.run.game);Layout.fillWidth:true;text:root.run&&root.run.game?"Current game: "+root.run.game:""}
                CheckBox{id:beta;visible:root.run&&["downloaded","repair"].indexOf(root.run.phase)>=0;text:"I installed Forever in the Classic Beta folder";Layout.fillWidth:true}
                ActionButton{id:start;objectName:"liveStart";Layout.fillWidth:true;primary:true;enabled:!root.busy&&!root.running;visible:!root.run||root.run.phase!=="observed";text:!root.run?"Choose test games":root.run.phase==="review"?"Start this test run":root.run.phase==="done"?"Show results for a photo":"Continue test";onClicked:{if(!root.run)root.configure();else if(root.run.phase==="done"){root.photoPage=0;root.photo=true}else root.resume({approved:root.run.phase==="review"})}}
                Flow{visible:root.run&&root.run.phase==="observed";Layout.fillWidth:true;spacing:10
                    ActionButton{text:"Worked";enabled:!root.busy;onClicked:root.resume({outcome:"PASS"})}
                    ActionButton{text:"Problem";enabled:!root.busy;onClicked:root.resume({outcome:"FAIL"})}
                    ActionButton{text:"Skip";enabled:!root.busy;onClicked:root.resume({outcome:"SKIPPED"})}
                }
                ActionButton{visible:root.run&&root.run.phase==="restore";text:"Skip settings restore";enabled:!root.busy;onClicked:root.resume({skip:true})}
                ActionButton{visible:root.run&&["installed","downloaded","observed"].indexOf(root.run.phase)>=0;text:"Retry launch";enabled:!root.busy;onClicked:root.request("live_retry_launch",{token:root.run.token})}
                ActionButton{visible:root.run&&["review","done"].indexOf(root.run.phase)>=0;text:root.run&&root.run.phase==="done"?"Start another test (save your photo first)":"Choose different test options";enabled:!root.busy;onClicked:root.configure()}
                ActionButton{visible:root.run&&root.run.phase!=="done";text:"End this test; keep its files";enabled:!root.busy;onClicked:{root.running=false;root.request("live_stop",{token:root.run.token})}}
                ActionButton{visible:!!root.run;text:"Show results so far";enabled:!root.busy;onClicked:{root.running=false;root.photoPage=0;root.photo=true}}
                ActionButton{text:root.running?"Pause after this step":"Pause and return";enabled:true;onClicked:{root.running=false;if(!root.busy)root.leave()}}
                InfoText{Layout.fillWidth:true;font.pixelSize:14;text:"A launch request is not a gameplay pass. Sign-in, game downloads, addon updates, controller feel and Gaming Mode need your observations. Cancel/recovery and destructive edge cases are also tested separately with disposable fixtures; the app does not deliberately corrupt your Steam files."}
                Repeater{model:root.run?root.run.rows:[];InfoText{required property var modelData;Layout.fillWidth:true;text:modelData.label+": "+modelData.status;font.pixelSize:15}}
            }
        }
        ColumnLayout{visible:root.photo;Layout.fillWidth:true;Layout.fillHeight:true;spacing:5
            Repeater{model:root.run?root.run.rows.slice(root.photoPage*10,root.photoPage*10+10):[];RowLayout{required property var modelData;Layout.fillWidth:true
                Label{text:modelData.label;color:"#edf3fa";font.pixelSize:16;Layout.fillWidth:true;elide:Text.ElideRight}
                Label{text:modelData.status;color:modelData.status==="PASS"?"#96e4b1":"#ffe099";font.pixelSize:16}
            }}
            Item{Layout.fillHeight:true}
            InfoText{Layout.fillWidth:true;font.pixelSize:14;text:root.example?"Preview only. These are sample results, not device test evidence.":"REQUESTED = request sent; YOU = your observation. Other features remain untested on this device. No personal paths or account details."}
            Flow{Layout.fillWidth:true;spacing:10
                ActionButton{text:"Back";onClicked:root.photo=false}
                ActionButton{text:"Previous";enabled:root.photoPage>0;onClicked:root.photoPage--}
                ActionButton{text:"Next";enabled:!!root.run&&(root.photoPage+1)*10<root.run.rows.length;onClicked:root.photoPage++}
                ActionButton{text:"Copy report";onClicked:root.copy(root.reportText)}
            }
        }
    }
}
