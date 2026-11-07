import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import "Guidance.js" as Guide

ApplicationWindow {
    id: win
    visible: true
    width: 1100; height: 740
    minimumWidth: 780; minimumHeight: 540
    color: "#101825"
    title: "WoW SteamOS Installer · " + Qt.application.version
    font.family: "Sans Serif"
    font.pixelSize: 16
    palette.window: "#101825"
    // Qt Basic FolderDialog header uses palette.light, not palette.window.
    palette.light: "#26374e"
    palette.base: "#1a2638"
    palette.text: "#edf3fa"
    palette.windowText: "#edf3fa"
    palette.placeholderText: "#93a5bf"
    palette.buttonText: "#edf3fa"
    palette.button: "#233147"
    palette.highlight: "#2b858a"
    palette.highlightedText: "#ffffff"
    property string page: "home"
    property var history: []
    property int registrationGeneration: 0
    property var registrationReview: ({})
    property var registrationEvidence: ({})
    function clearRegistration(){registrationGeneration++;registrationReview={};registrationEvidence={};if(registrationConsent)registrationConsent.checked=false;if(registrationAuth)registrationAuth.checked=false}
    property var addonHandoff: []
    property var session: ({state:{installations:[],addon_pending:false},catalog:[],home:"",data:""})
    property var scan: ({steam:[],games:[],launchers:[],warnings:[],steam_running:false})
    property var plan: ({summary:[],removal:[],destructive:false})
    property var catalog: []
    property var tiles: []
    property string pointerOwner: ""
    property var pointerTarget: ({})
    property bool pointerScopeValid: !!current.id && pointerOwner===selectedId
    property var pointerTiles: (singleTileReady||["launcher_install","games"].indexOf(current.stage)>=0)&&tiles.filter(function(t){return !t.addon}).length===1?tiles.filter(function(t){return !t.addon&&t.name==="Battle.net"}):[]
    property bool pointerTargetValid: pointerScopeValid && pointerTiles.some(function(t){return t.appid===pointerTarget.appid&&t.name===pointerTarget.name})
    function openPointerHelp() {
        if(!current.id||!selectedId){errorText="Choose the intended installation before opening its control guide.";return}
        pointerOwner=selectedId;pointerTarget=({});go("pointerHelp");if(!backend.demo)call("shortcuts",{id:selectedId,_controller_generation:controllerGeneration,_pointer_refresh:true})
    }
    function choosePointerTile(tile) {
        clearControllerFlow();
        if(!pointerScopeValid||!pointerTiles.some(function(t){return t.appid===tile.appid&&t.name===tile.name})){errorText="The shortcut selection changed. Return and choose the intended tile again.";return}
        pointerTarget={appid:tile.appid,name:tile.name};go("pointerSteps")
    }
    property var folderInfo: ({available:false,path:"",links:[],messages:[]})
    property bool folderRefreshAttempted: false
    property var games: []
    property var adoption: ({candidates:[],prefix:""})
    property var selectedAdoption: []
    property string selectedId: ""
    property bool selectionInitialized: false
    property bool selectionRequired: false
    property var availableInstallations: (session.state.installations||[]).filter(function(i){return (session.unavailable_installations||[]).indexOf(i.id)<0})
    property var editions: []
    property string classicMode: backend.classicGuideMode()
    function classicModeName(){return ({era:"Classic Era",hardcore:"Hardcore",sod:"Season of Discovery"})[classicMode]||"Classic Era"}
    function chooseClassicMode(mode){if(["era","hardcore","sod"].indexOf(mode)<0)return;classicMode=mode;backend.setClassicGuideMode(mode)}
    property string classicModeInstructions: "Open the single Battle.net Steam tile. Select Classic in Battle.net. In Battle.net, choose Launch Into → "+classicModeName()+". Press Play, then verify the mode and realm selection in game."
    property var freshEditionDraft: []
    property var reinstallEditionDrafts: ({})
    property string errorText: ""
    property int controllerGeneration: 0
    function clearControllerFlow(){controllerGeneration++}
    property string notice: ""
    property string operation: ""
    property string folderPurpose: "search"
    property string diagnosticText: ""
    property var settingsBackups: []
    property var legacyItems: []
    property var legacySelected: []
    property bool reinstallMode: false
    property bool liveMode: false
    property int testedInput: 0
    property bool downloadsChecked: false
    property bool checkingDownloads: false
    property var missingDownloads: (current.editions||[]).filter(function(id){return !games.some(function(g){return g.id===id||(id==="forever"&&g.id==="classic_beta"&&foreverBeta.checked)})})
    function checkDownloads(){checkingDownloads=true;call("shortcuts",{id:selectedId})}
    function addonName(id){return ({none:"No addon manager",curseforge:"CurseForge",wowup_cf:"WowUp with CurseForge",wowup_wago:"WowUp with Wago"})[id]||"No addon manager"}
    function editionName(id){var e=catalog.find(function(e){return e.id===id});return e?e.name:id}
    property bool wantLauncherTile: true
    property var sharedTileChoices: []
    property double sharedTileChoice: 0
    property string wantedAddon: "none"
    property string settingsBackupRoot: ""
    property bool advanced: false
    property bool setupAdvanced: false
    property bool reviewDetails: false
    property string selectedProfile: ""
    property string effectiveProfile: steamInfo.users.length===1?steamInfo.users[0]:steamInfo.users.indexOf(selectedProfile)>=0?selectedProfile:""
    property string selectedProton: ""
    property string afterScan: ""
    property var launcherExitStatus: ({ready:false,configured:false,state:"unavailable"})
    property bool singleTileReady: false
    property string setupStep: Guide.step(current,launcherExitStatus,singleTileReady)
    property var gameTiles: tiles.filter(function(t){return t.shared&&t.name==="Battle.net"})
    property var legacyProductTiles: tiles.filter(function(t){return t.game&&!t.shared})
    property bool allGamesTested: gameTiles.length>0 && gameTiles.every(function(t){return (session.state.tests||[]).indexOf(selectedId+":"+(t.shared?"single-battlenet-c43":t.name))>=0})
    property string requestedProton: reinstallMode&&current.proton?current.proton:(selectedProton||"proton_11")
    property string effectiveProton: selectedProton?(steamInfo.protons.some(function(p){return p.name===selectedProton})?selectedProton:""):Guide.defaultProton(steamInfo.protons)
    property bool protonWaiting: false
    property int protonPolls: 0
    property string missingProtonRuntime: ""
    property string protonRuntimeSteam: ""
    function requestProton(runtime){protonPolls=0;protonWaiting=true;call("proton_install",{steam:steamInfo.path,proton:requestedProton,install_runtime:runtime===true})}
    function checkProton(){call("proton_status",{steam:steamInfo.path,proton:requestedProton})}
    function receiveProton(d){
        if(d.steam!==steamInfo.path||d.proton&&d.proton!==requestedProton)return;
        if(d.missing_runtime!==undefined){missingProtonRuntime=d.missing_runtime||"";protonRuntimeSteam=d.steam}
        if(d.protons){var copy=Object.assign({},scan);copy.steam=scan.steam.map(function(s){return s.path===d.steam?Object.assign({},s,{protons:d.protons}):s});scan=copy}
        notice=d.message||"";if(d.ready)protonWaiting=false;
    }
    Timer{interval:3000;repeat:true;running:page==="setup"&&protonWaiting&&!backend.busy&&protonPolls<100;onTriggered:{protonPolls++;checkProton();if(protonPolls===100){protonWaiting=false;notice="Automatic checking paused. Your choices are kept. Finish the Steam download, then choose Check again."}}}
    function next(){
        liveMode=false;
        if(session.pending_cleanup||session.pending_transaction||session.pending_restore){openMaintenance("recover");return}
        if(current.id){openGames();return}
        if(selectionRequired){
            // Home Install is an explicit new-setup intent only after the last
            // managed installation and launcher have disappeared. Never select
            // another remaining (even unavailable) installation implicitly.
            if(!(session.state.installations||[]).length&&!scan.launchers.length){selectionRequired=false;beginFreshSetup();return}
            errorText="Choose an installation, or explicitly start a new copy under Advanced.";return
        }
        if(scan.launchers.length===1){call("adoption_candidates",{prefix:scan.launchers[0].prefix});return}
        if(scan.launchers.length)go("found");else beginFreshSetup()
    }
    property int steamRecoveryAttempts: 0
    function openMaintenance(op){maintenanceDialog.savedOperation=op;maintenanceConsent.checked=false;maintenanceDialog.open()}
    Timer{id:steamRecoveryTimer;interval:3000;repeat:true;running:!!session.steam_restart_pending&&!backend.busy&&steamRecoveryAttempts<2;onTriggered:{steamRecoveryAttempts++;call("reopen_steam")}}
    function launcherTile(){return tiles.filter(function(t){return !t.addon&&(t.shared||!t.game)&&t.name==="Battle.net"})[0]||tiles.filter(function(t){return !t.addon&&!t.game})[0]}
    function launchBattleNet(){var t=launcherTile();if(t)call("launch",{id:selectedId,appid:t.appid})}
    function clearRepairConsent(){
        repairConsentKey="";
        if(repairPartialConsent)repairPartialConsent.checked=false;
        if(partialClients)partialClients.checked=false;
    }
    property bool repairFlowActive: false
    property int repairGeneration: 0
    property string repairReviewScopeKey: ""
    onSelectedIdChanged: {folderInfo=({available:false,path:"",links:[],messages:[]});folderRefreshAttempted=false;launcherExitStatus=({ready:false,configured:false,state:"unavailable"});singleTileReady=false;sharedTileChoice=0;sharedTileChoices=[];if(exitConsent)exitConsent.checked=false;clearRegistration();clearControllerFlow();if(repairFlowActive&&repairCheckId!==selectedId)abandonRepair()}
    function abandonRepair(){
        repairGeneration++;repairFlowActive=false;repairCheckPending=false;repairCheckId="";
        clearRepairConsent();repairReviewScopeKey="";sharedTileChoice=0;partialClients.checked=false;reviewConsent.checked=false;
    }
    function cancelOperation(){clearRegistration();if(page==="pointerSteps")clearControllerFlow();abandonRepair();backend.cancel();notice="Cancellation requested. A started Steam transaction must reach a safe checkpoint."}
    function returnHome(){clearRegistration();clearControllerFlow();abandonRepair();history=[];page="home"}
    property bool repairCheckPending: false
    property string repairCheckId: ""
    property string repairConsentKey: ""
    property string repairScopeKey: JSON.stringify([selectedId,(current.editions||[]).slice().sort(),games.map(function(g){return g.id}).sort(),missingDownloads.slice().sort()])
    onRepairScopeKeyChanged: clearRepairConsent()
    function prepareReview(){
        var downloadConsent=setupStep==="download"&&partialClients.checked;
        if(!repairFlowActive){repairFlowActive=true;repairGeneration++;clearRepairConsent()}
        if(downloadConsent)repairConsentKey=repairScopeKey;
        repairCheckPending=true;repairCheckId=selectedId;
        call("shortcuts",{id:selectedId,_repair_generation:repairGeneration})
    }
    function finishRepairCheck(){
        if(launcherExitStatus.ready!==true){go("launcherBehavior");return}
        if(missingDownloads.length>0&&repairConsentKey!==repairScopeKey){go("repairOptions");return}
        if(sharedTileChoices.length>1&&!sharedTileChoices.some(function(t){return t.appid===sharedTileChoice})){go("sharedTileUpgrade");return}
        repairReviewScopeKey=repairScopeKey;
        call("review_repair",{id:selectedId,_repair_generation:repairGeneration,allow_missing:missingDownloads.length>0&&repairConsentKey===repairScopeKey,installed_only_scope:{id:selectedId,selected:(current.editions||[]).slice().sort(),missing:missingDownloads.slice().sort()},confirm_forever_beta:foreverBeta.checked,shared_tile_appid:sharedTileChoice||null})
    }
    function languageCode(){return languageChoice.currentValue||"enUS"}
    function routeAfterScan(){var destination=afterScan;afterScan="";if(selectionRequired&&!selectedId){go("home");return}if(destination==="games"&&selectedId)openGames();else if(destination){go(destination);if(destination==="addons"&&selectedId)call("shortcuts",{id:selectedId})}}
    function focusPage(){
        if(page==="live"){liveCenter.focusFirst();return}
        if(page==="testing"){testCenter.focusFirst();return}
        var root=contentScroll.contentItem.contentItem;
        function seek(item){
            if(!item.visible||!item.enabled)return null;
            if(item.primary===true&&item.activeFocusOnTab)return item;
            for(var n=0;n<item.children.length;n++){var f=seek(item.children[n]);if(f)return f}
            return null;
        }
        var target=seek(root);if(target)target.forceActiveFocus();else backButton.forceActiveFocus();
    }
    function checkGuidance(){
        return Guide.defaultProton([{name:"Cachy-Proton"},{name:"proton_11"}])==="proton_11"
            && Guide.defaultProton([{name:"Cachy-Proton"},{name:"GE-Proton10-11"}])===""
            && Guide.defaultProton([{name:"proton_110"}])==="proton_110"
            && Guide.step({id:"1",stage:"launcher_install"})==="launcher"
            && Guide.step({id:"1",stage:"games"})==="download"
            && Guide.step({id:"1",stage:"test_games"})==="test"
            && Guide.step({id:"1",stage:"test_games",restore_backup:"saved"})==="restore"
            && Guide.step({id:"1",stage:"complete"})==="complete"
            && Guide.afterApply("setup")==="games" && Guide.afterApply("adopt")==="games"
            && Guide.selection([{id:"working"}],"removed","working",true,[])===""
            && Guide.selection([{id:"working"}],"","working",true,[])===""
            && Guide.selection([{id:"working"}],"working","",true,["working"])===""
            && Guide.selection([{id:"working"}],"","working",false,[])==="working"
            && Guide.afterApply("repair")==="games" && Guide.afterApply("addon")==="addons";
    }
    property var current: {
        var all=session.state.installations || []
        for(var n=0;n<all.length;n++)if(all[n].id===selectedId)return all[n]
        return ({id:"",stage:"",root:"",owned:false,proton:""})
    }
    property int selectedSteam: 0
    property var steamInfo: scan.steam.length ? scan.steam[Math.min(selectedSteam,scan.steam.length-1)] : ({users:[],protons:[],path:""})
    function go(next) { if(page==="addons"&&next!=="addons")clearRegistration(); if(repairFlowActive&&["repairOptions","sharedTileUpgrade","review"].indexOf(next)<0)abandonRepair();if(page!==next){history=history.concat([page]);page=next;contentScroll.contentItem.contentY=0;if(next==="addons"&&selectedId){addonHandoff=[];call("shortcuts",{id:selectedId})}} }
    function back() {
        clearRegistration();
        if(page==="pointerSteps")clearControllerFlow();
        abandonRepair();
        if(maintenanceDialog.visible){maintenanceDialog.close();return}
        if(folderDialog.visible){folderDialog.close();return}
        if(appImageDialog.visible){appImageDialog.close();return}
        if(backend.busy){backend.cancel();notice="Cancellation requested. If a transaction has started, setup finishes or pauses at a recoverable checkpoint.";return}
        if(page==="live"){liveCenter.running=false;if(liveCenter.photo){liveCenter.photo=false;return}}
        if(page==="testing"&&testCenter.photo){testCenter.photo=false;return}
        if(page==="testing"&&testCenter.asking){testCenter.asking=false;return}
        if(history.length){var h=history.slice();page=h.pop();history=h}else{page="home"}
    }
    function call(action,args) {
        args=args||{};
        if(action.indexOf("addon_registration_")===0)args._registration_generation=registrationGeneration;
        if(action!=="review_setup" && Object.prototype.hasOwnProperty.call(args,"id") && (!args.id || !selectedId || args.id!==selectedId || !current.id)) {errorText="Choose the intended installation explicitly before continuing.";return}
        if(["apply","guided_apply"].indexOf(action)>=0 && ["setup","adopt"].indexOf(plan.kind)<0){
            if(!selectedId || !plan.installation || selectedId!==plan.installation.id){errorText="The installation selection changed. Scan and review the intended copy again.";return}
            args.id=selectedId;
        }
        errorText="";testCenter.failure="";operation=action;backend.request(action,args)
    }
    function chooseFolder(purpose) {folderPurpose=purpose;folderDialog.open()}
    function rememberEditionDraft() {
        if(reinstallMode&&selectedId){var drafts=Object.assign({},reinstallEditionDrafts);drafts[selectedId]=editions.slice();reinstallEditionDrafts=drafts}
        else freshEditionDraft=editions.slice()
    }
    function beginFreshSetup(){reinstallMode=false;editions=freshEditionDraft.slice();go("setup")}
    function beginReinstallSetup(){
        if(!current.id)return;
        reinstallMode=true;wantedAddon=current.addon_manager||"none";wantLauncherTile=true;
        editions=Object.prototype.hasOwnProperty.call(reinstallEditionDrafts,current.id)?reinstallEditionDrafts[current.id].slice():(current.editions||[]).slice();go("setup")
    }
    function toggleEdition(id,enabled) {var e=editions.slice();var n=e.indexOf(id);if(enabled&&n<0)e.push(id);if(!enabled&&n>=0)e.splice(n,1);editions=e;rememberEditionDraft()}
    function refreshSession(value) {
        session=value;if(!value.steam_restart_pending)steamRecoveryAttempts=0;catalog=value.catalog||catalog
        var all=session.state.installations||[]
        var previous=selectedId;
        selectedId=Guide.selection(all,previous,session.state.current,selectionInitialized,session.unavailable_installations||[]);
        selectionInitialized=true;
        if(!selectedId&&all.length>0)selectionRequired=true;
        if(previous && !selectedId){selectionRequired=true;tiles=[];games=[];addonHandoff=[];plan=({summary:[],removal:[],destructive:false});deletionConsent.checked=false;reviewConsent.checked=false;history=[];page="home";notice="The previous installation is no longer available. Choose the intended installation before making changes."}
        if(!locationField.text)locationField.text=value.home||""
    }
    function openGames() {if(!selectedId){go("found");return}if(setupStep==="addons")go("addons");call("shortcuts",{id:selectedId})}
    function setupArguments(){return {id:selectedId,cleanup:liveCleanup.checked,reinstall:liveReinstall.checked,keep_settings:keepSettings.checked,language:languageCode(),steam:steamInfo.path,user:effectiveProfile,proton:effectiveProton,location:locationField.text,editions:editions,launcher_tile:wantLauncherTile,addon_manager:"none"}}
    function reviewSetup(){if(!editions.length){errorText="Choose at least one edition.";return}if(!reinstallMode&&!effectiveProton){errorText="Finish installing the selected Proton in Steam, then choose Check again.";return}rememberEditionDraft();call(liveMode?"live_prepare":reinstallMode?"review_reinstall":"review_setup",setupArguments())}
    function seedC12Checks(){
        if(!backend.demo)return false;
        previewScenario("complete");var i=session.state.installations[0];i.addon_manager="curseforge";i.editions=["era"];
        session={home:"/home/player",state:{installations:[i],current:"preview",tests:[],addon_pending:false},catalog:catalog};page="help";return true;
    }
    function checkAddonHandoff() {
        previewScenario("installed");page="addons";
        addonHandoff=[{name:"WoW Classic Era",path:"/fixture/Program Files (x86)/World of Warcraft",mode:"guided"}];
        return addonHandoff.length===1 && addonHandoff[0].path.indexOf("Program Files (x86)")>=0;
    }
    function checkSelectionSafety() {
        var a={id:"removed",root:"/fixture/removed",editions:[],stage:"test_games"};
        var b={id:"working",root:"/fixture/working",editions:[],stage:"test_games"};
        selectionInitialized=false;selectedId="";
        refreshSession({state:{installations:[a,b],current:"removed"}});
        if(selectedId!=="removed")return false;
        refreshSession({state:{installations:[b],current:"working"}});
        if(selectedId!==""||!selectionRequired||page!=="home")return false;
        var previous=operation;call("review_clean",{id:"working"});
        if(operation!==previous||!errorText.length)return false;
        refreshSession({state:{installations:[b],current:"working"}});
        if(selectedId!=="")return false;
        selectedId="working";selectionRequired=false;
        refreshSession({state:{installations:[b],current:"working"},unavailable_installations:["working"]});
        if(selectedId!==""||!selectionRequired)return false;
        selectionInitialized=false;selectionRequired=false;
        refreshSession({state:{installations:[b],current:"removed"}});
        return selectedId===""&&selectionRequired;
    }
    function previewScenario(name){
        if(!backend.demo)return;
        var i={id:"preview",root:"/home/player/Games/World of Warcraft",prefix:"/home/player/Games/World of Warcraft/prefix",proton:"proton_11",stage:name,user:"Preview",editions:["forever"],restore_backup:null};
        if(name==="restore"){i.stage="test_games";i.restore_backup="preview"}
        session={home:"/home/player",state:{installations:[i],current:"preview",tests:[],addon_pending:false},catalog:catalog};selectedId="preview";
        tiles=[{appid:1,name:"Battle.net",game:name!=="launcher_install",shared:name!=="launcher_install",addon:false}];singleTileReady=name!=="launcher_install";launcherExitStatus=({configured:true,ready:true});
        games=name==="launcher_install"?[]:[{id:"forever",name:"World of Warcraft Forever",path:"/home/player/Games/World of Warcraft/_forever_"}];
        page="games";
    }
    function seedLauncherBehaviorCheck(){
        if(!backend.demo)return false;
        previewScenario("complete");
        var i=Object.assign({},session.state.installations[0]);
        i.launcher="/fixture/Battle.net.exe";i.launcher_exit_choice=null;i.launcher_exit_reviewed=false;
        session={home:"/home/player",state:{installations:[i],current:"preview",tests:[],addon_pending:false},catalog:catalog};
        go("launcherBehavior");return setupStep==="launcher_behavior";
    }
    function previewSeed(){
        catalog=[{id:"retail",name:"World of Warcraft",note:"Retail client"},{id:"forever",name:"World of Warcraft Forever",note:"Confirm the product inside Battle.net"},{id:"era",name:"WoW Classic Era",note:"Hardcore and Season of Discovery share this client"},{id:"progression",name:"WoW Classic Progression",note:"Expansion changes over time"}]
        session={home:"/home/player",state:{installations:[],addon_pending:false},catalog:catalog}
        scan={steam:[{path:"/home/player/.local/share/Steam",users:["Preview profile"],protons:[{name:"proton_11",label:"Proton 11"}]}],games:[],launchers:[],warnings:[],steam_running:false}
        plan={token:"preview",kind:"setup",summary:["Create a separate Battle.net environment in your selected game folder.","Use your selected installed Proton version.","Download Blizzard’s installer over HTTPS and add its Steam tile.","Keep existing games and shortcuts in place."],removal:[],destructive:false}
    }
    Component.onCompleted: {if(backend.demo){previewSeed();testCenter.seedPreview();liveCenter.seed();Qt.callLater(focusPage)}else call("scan")}
    onPageChanged: {if(page==="launcherBehavior")exitConsent.checked=false;if(repairFlowActive&&["home","games","repairOptions","sharedTileUpgrade","review"].indexOf(page)<0)abandonRepair();Qt.callLater(focusPage)}
    function revealFocusedControl(){
        var item=win.activeFocusItem
        if(page==="live"){liveCenter.reveal(item);return}
        if(page==="testing"){testCenter.reveal(item);return}
        var flick=contentScroll.contentItem
        if(!item||!flick||!flick.contentItem)return
        var ancestor=item;var inside=false
        while(ancestor){if(ancestor===flick.contentItem){inside=true;break}ancestor=ancestor.parent}
        if(!inside)return
        var point=item.mapToItem(flick.contentItem,0,0)
        if(point.y<flick.contentY)flick.contentY=Math.max(0,point.y-12)
        else if(point.y+item.height>flick.contentY+flick.height)flick.contentY=Math.max(0,Math.min(flick.contentHeight-flick.height,point.y+item.height-flick.height+12))
    }
    onActiveFocusItemChanged: revealTimer.restart()
    onErrorTextChanged: revealTimer.restart()
    onNoticeChanged: revealTimer.restart()
    Timer {id:revealTimer;interval:30;onTriggered:win.revealFocusedControl()}
    onClosing: function(close){if(backend.busy){close.accepted=false;notice="An operation is running. Use Cancel and wait for the current checkpoint before closing."}}
    Shortcut {sequence:"Escape";onActivated:win.back()}
    Connections {
        target: backend
        function onBackRequested(){win.back()}
        function onCompleted(action,response){
            if(response.registration_generation!==undefined&&response.registration_generation!==registrationGeneration)return;
            if(response.controller_generation!==undefined&&response.controller_generation!==controllerGeneration)return;
            if(response.repair_generation!==undefined&&(!repairFlowActive||response.repair_generation!==repairGeneration))return;
            if(action==="review_repair"&&response.repair_generation!==undefined&&repairReviewScopeKey!==repairScopeKey){abandonRepair();errorText="Installed or missing editions changed. Choose repair again.";return}
            if(!response.ok){if(action==="addon_registration_result")return;if(action==="addon_registration_apply"){registrationConsent.checked=false;call("addon_registration_result",{id:selectedId,token:registrationReview.token})}if(action==="proton_install"||action==="proton_status")protonWaiting=false;if(action.indexOf("live_")===0){liveCenter.running=false;liveCenter.failure=response.error||"Test paused.";return}if(action.indexOf("device_")===0){testCenter.photo=false;testCenter.failure=response.error||"The check could not finish. Try again.";testCenter.advanceAfterSave=false;return}repairCheckPending=false;checkingDownloads=false;errorText=response.error||"The operation did not complete.";if(["apply","guided_apply","guided_existing","reopen_steam","recover","review_clean","review_reinstall","review_repair","restore_settings","launch","select_installation","link_game","shortcuts","review_launcher","review_addon","prepare_gear"].indexOf(action)>=0)backend.request("init",{});return}
            var d=response.data
            if(action.indexOf("live_")===0){liveCenter.receive(action,d);go("live");return}
            if(action.indexOf("device_")===0){testCenter.receive(action,d);return}
            if(action==="scan"){scan=d.scan;if(selectedSteam>=scan.steam.length)selectedSteam=0;refreshSession(d.session);routeAfterScan();Qt.callLater(focusPage)}
            else if(action==="init")refreshSession(d)
            else if(action==="addon_registration_result"){registrationEvidence=d;notice=d.message}
            else if(action==="addon_registration_review"){registrationEvidence={};if(d.id!==selectedId){clearRegistration();return}registrationReview=d;registrationConsent.checked=false;notice=d.message}
            else if(action==="addon_registration_apply"){clearRegistration();registrationEvidence=d;notice=d.message}
            else if(action==="prepare_gear"){appImagePath.text=d.desktop;notice=d.message;call("review_addon",{id:selectedId,manager:"curseforge"})}
            else if(action==="proton_install"||action==="proton_status"){receiveProton(d)}
            else if(action==="reopen_steam"){notice=d.message;call("init")}
            else if(action==="shortcuts"){
                if(!d.installation||d.installation.id!==selectedId)return;
                tiles=d.shortcuts;games=d.games;addonHandoff=d.addon_handoff||[];sharedTileChoices=d.shared_tile_choices||[];launcherExitStatus=d.launcher_exit_status||({ready:false,configured:false,state:"unavailable"});singleTileReady=d.single_tile_ready===true;folderInfo=d.folders||({available:false,path:"",links:[],messages:[]});
                if(setupStep==="complete"&&!repairCheckPending&&!checkingDownloads&&!folderRefreshAttempted&&folderInfo.state==="not_created"){folderRefreshAttempted=true;call("refresh_folders",{id:selectedId})}
                if(response.pointer_refresh)return;
                if(repairCheckPending){repairCheckPending=false;if(repairCheckId!==selectedId){clearRepairConsent();errorText="Installation changed. Choose it and review repair again.";return}finishRepairCheck();return}
                go(setupStep==="launcher_behavior"?"launcherBehavior":"games");
                if(checkingDownloads){checkingDownloads=false;downloadsChecked=true;if(missingDownloads.length===0||partialClients.checked)prepareReview()}
            }
            else if(["refresh_folders","open_folders","remove_folder_links"].indexOf(action)>=0){
                if(d.id!==selectedId)return;
                folderInfo=d;notice=d.message||"";
                if(action==="open_folders"&&d.available&&d.open_path===d.path)backend.openFolder(d.open_path)
            }
            else if(action==="adoption_candidates"){adoption=d;selectedAdoption=[];
                var groups={};d.candidates.forEach(function(x){if(x.proton){var key=x.steam+"|"+x.user+"|"+x.proton;if(!groups[key])groups[key]=[];groups[key].push(x)}});
                var keys=Object.keys(groups);if(keys.length===1)selectedAdoption=groups[keys[0]];
                go("adopt")}
            else if(action.indexOf("review_")===0||action==="set_product"){if(action==="review_repair")clearRepairConsent();plan=d;reviewDetails=false;deletionConsent.checked=false;reviewConsent.checked=false;go("review")}
            else if(action==="diagnostics"){diagnosticText=d.text;go("diagnostics")}
            else if(action==="legacy_candidates"){legacyItems=d.items;legacySelected=[];go("legacy")}
            else if(action==="download_addon"){appImagePath.text=d.path;notice=d.message}
            else if(action==="settings_backups"){settingsBackups=d.backups;settingsBackupRoot=d.location||"";go("restore")}
            else {
                notice=d.steam_message|| (action==="launch"?(d.message||"Launch requested. Check Steam and confirm the window opens."):action==="mark_tested"?"Game checked.":"")
                if((action==="apply"||action==="guided_apply")||action==="guided_existing"||action==="recover"||action==="undo"||action==="cancel_cleanup"||action==="abandon_cleanup"||action==="reset_state"){
                    if((action==="apply"||action==="guided_apply")&&plan.kind==="setup"){freshEditionDraft=[]}foreverBeta.checked=false;downloadsChecked=false;partialClients.checked=false;history=[];if((action==="apply"||action==="guided_apply")&&["setup","reinstall","adopt"].indexOf(plan.kind)>=0&&d.id){selectedId=d.id;selectionRequired=false;}afterScan=(action==="apply"||action==="guided_apply")?Guide.afterApply(plan.kind):"home";call("scan")
                }
                else if(action==="add_root")call("scan")
                else if(action==="link_game")openGames()
                else if(action==="addon_done"){afterScan="home";call("scan")}
                else if(action==="restore_settings"||action==="skip_restore"){afterScan="games";call("scan")}
                else if(action==="mark_tested"||action==="select_installation")call("init")
            }
        }
    }
    FolderDialog {
        id:folderDialog;objectName:"locationFolderDialog"
        options: FolderDialog.DontUseNativeDialog
        onVisibleChanged: if(visible)Qt.callLater(function(){backend.styleFolderDialog(win)})
        title:folderPurpose==="location"?"Choose an existing parent folder for the new copy":"Choose the folder to search"
        onAccepted:{var p=backend.localPath(selectedFolder);if(folderPurpose==="location")locationField.text=p;else if(folderPurpose==="link")call("link_game",{id:selectedId,path:p});else call("add_root",{path:p})}
    }
    FileDialog {
        id:appImageDialog
        options: FileDialog.DontUseNativeDialog
        title:"Choose the publisher's Linux AppImage"
        nameFilters:["Linux AppImages (*.AppImage)"]
        onAccepted:appImagePath.text=backend.localPath(selectedFile)
    }
    header: Rectangle {
        height:(page==="live"&&liveCenter.photo)||(page==="testing"&&testCenter.photo)?0:62;visible:height>0;color:"#18212c"
        RowLayout{anchors.fill:parent;anchors.margins:18
            Label{text:"WoW SteamOS Installer";color:"#f1f5fb";font.pixelSize:22;font.bold:true}
            Item{Layout.fillWidth:true}
            Label{text:backend.demo?"Preview":Qt.application.version;color:"#a7b5c5";font.pixelSize:13}
        }
    }
    footer: Rectangle {
        id:footerBar;objectName:"footerBar"
        height:(page==="live"&&liveCenter.photo)||(page==="testing"&&testCenter.photo)?0:82;visible:height>0;color:"#152135"
        RowLayout {
            anchors.fill:parent;anchors.margins:16;spacing:12
            ActionButton{id:backButton;objectName:"backButton";text:folderDialog.visible||appImageDialog.visible?"Cancel selection":page==="home"?"Close setup":"Back";onClicked:{if(page==="home"&&!backend.busy&&!folderDialog.visible&&!appImageDialog.visible)win.close();else win.back()}}
            InfoText{Layout.fillWidth:true;text:backend.busy?"Please wait…":"D-pad: move    A: select    B: back";font.pixelSize:13}
            BusyIndicator{running:backend.busy;visible:running;implicitHeight:32;implicitWidth:32}
            ActionButton{visible:backend.busy;text:"Cancel";objectName:"operationCancel";onClicked:cancelOperation()}
            ActionButton{objectName:"footerHome";visible:!backend.busy&&page!=="home";text:"Home";onClicked:returnHome()}
        }
    }
    LiveTests {
        id:liveCenter;objectName:"liveCenter";anchors.fill:parent;visible:page==="live";busy:backend.busy
        onRequest:function(action,args){call(action,args)}
        onConfigure:{liveMode=true;beginFreshSetup()}
        onLeave:{liveMode=false;history=[];page="home";call("scan")}
        onCopy:function(text){backend.copyText(text)}
    }
    TestCenter {
        id:testCenter;objectName:"testCenter";anchors.fill:parent;visible:page==="testing";busy:backend.busy
        onRequest:function(action,args){if(action==="device_run")args.id=selectedId;call(action,args)}
        onLeave:{history=[];page="home"}
        onCopy:function(text){backend.copyText(text)}
    }
    RowLayout {
        visible:page!=="testing"&&page!=="live";anchors.fill:parent;spacing:0
        ColumnLayout {
            Layout.fillWidth:true;Layout.fillHeight:true;spacing:0
            Rectangle {
                visible:errorText!=="";Layout.fillWidth:true;implicitHeight:errorRow.implicitHeight+24;color:"#482630"
                RowLayout{id:errorRow;anchors.fill:parent;anchors.margins:12;InfoText{Layout.fillWidth:true;text:errorText;color:"#ffdae0"}ActionButton{text:"Dismiss";onClicked:errorText=""}}
            }
            Rectangle {
                visible:notice!==""&&!backend.busy;Layout.fillWidth:true;implicitHeight:noticeLabel.implicitHeight+20;color:"#17353e"
                InfoText{id:noticeLabel;anchors.fill:parent;anchors.margins:10;text:notice;color:"#b9eee4";font.pixelSize:14}
            }
            ScrollView {
                id:contentScroll;objectName:"mainContentScroll";Layout.fillWidth:true;Layout.fillHeight:true;clip:true
                onHeightChanged:revealTimer.restart()
                onWidthChanged:revealTimer.restart()
                contentWidth:availableWidth
                ColumnLayout {
                    width:contentScroll.availableWidth;spacing:18
                    ColumnLayout {
                        visible:page==="home";Layout.fillWidth:true;Layout.margins:24;spacing:16
                        ActionButton{visible:!!session.steam_restart_pending;text:"Reopen Steam desktop controls";enabled:!backend.busy;onClicked:call("reopen_steam")}
                        InfoText{visible:!!session.state_error;Layout.fillWidth:true;text:"Setup records could not be read. Resetting archives those records and unfinished plans. Your games and Steam shortcuts stay in place. Then scan to reconnect them."}
                        ActionButton{visible:!!session.state_error;Layout.fillWidth:true;text:"Reset setup records and scan again";enabled:!backend.busy;onClicked:call("reset_state",{confirm:true})}
                        Label{text:backend.busy?"Checking your installation…":current.id?"World of Warcraft":"Install World of Warcraft";font.pixelSize:28;font.bold:true;color:"#f2f6fc";Layout.fillWidth:true;wrapMode:Text.WordWrap}
                        InfoText{Layout.fillWidth:true;text:backend.busy?"Looking for Battle.net and installed games.":session.pending_cleanup||session.pending_transaction||session.pending_restore?"There’s an unfinished change. Continue it before starting something else.":current.id?(setupStep==="complete"?"Setup is complete. Open a game or check for updates in Battle.net.":"Your progress is saved. Continue where you left off."):scan.launchers.length?"Battle.net is already installed. You can use it without downloading everything again.":"We’ll install Battle.net, help you download your games, and add them to Steam."}
                        InfoText{visible:!backend.busy&&scan.games.length>0;Layout.fillWidth:true;text:"Found: "+scan.games.map(function(g){return g.name}).join(", ")}
                        ComboBox{id:currentChoice;objectName:"installationChoice";visible:availableInstallations.length>0;Layout.fillWidth:true;model:availableInstallations;textRole:"root";displayText:currentIndex<0?"Choose an installation":currentText;currentIndex:availableInstallations.findIndex(function(i){return i.id===selectedId});onActivated:{selectedId=model[currentIndex].id;selectionRequired=false;foreverBeta.checked=false;call("select_installation",{id:selectedId})}}
                        ActionButton{objectName:"homeContinue";text:session.pending_cleanup||session.pending_transaction||session.pending_restore?"Continue unfinished change":current.id?(setupStep==="complete"?"Open games":"Continue setup"):scan.launchers.length?"Use existing installation":"Install";primary:true;Layout.fillWidth:true;enabled:!backend.busy&&!session.state_error;onClicked:next()}
                        ActionButton{visible:!!current.id&&setupStep!=="launcher"&&setupStep!=="download";Layout.fillWidth:true;objectName:"homeRepair";text:"Repair";enabled:!backend.busy;onClicked:prepareReview()}
                        ActionButton{visible:false;Layout.fillWidth:true;text:"Manage addons";enabled:!backend.busy;onClicked:go("addons")}
                        ActionButton{visible:!!current.id;text:"Remove";Layout.fillWidth:true;enabled:!backend.busy;onClicked:go("remove")}
                        ActionButton{objectName:"homeAdvanced";text:"Advanced";Layout.fillWidth:true;enabled:!backend.busy;onClicked:go("help")}
                        ActionButton{visible:!!session.pending_cleanup;Layout.fillWidth:true;text:"Cancel removal before changes start";enabled:!backend.busy;onClicked:call("cancel_cleanup")}
                        InfoText{visible:!!session.pending_cleanup;Layout.fillWidth:true;text:"If continuing fails, you can stop here. Changes already saved will stay; remaining game files will be kept."}
                        ActionButton{visible:!!session.pending_cleanup;Layout.fillWidth:true;text:"Stop this removal and keep remaining files";enabled:!backend.busy;onClicked:openMaintenance("abandon_cleanup")}

                    }
                    ColumnLayout {
                        visible:page==="classicMode";Layout.fillWidth:true;Layout.margins:24;spacing:16
                        Label{text:"Choose your Classic mode";font.pixelSize:27;color:"#f2f6fc"}
                        InfoText{Layout.fillWidth:true;text:"Classic Era, Hardcore and Season of Discovery share the Classic Era client. This preference only changes this guide; it does not change Battle.net or create another download or tile."}
                        ComboBox{id:classicModeChoice;objectName:"classicModeChoice";Layout.fillWidth:true;model:[{name:"Classic Era",value:"era"},{name:"Hardcore",value:"hardcore"},{name:"Season of Discovery",value:"sod"}];textRole:"name";valueRole:"value";currentIndex:model.findIndex(function(m){return m.value===classicMode});onActivated:chooseClassicMode(currentValue)}
                        InfoText{objectName:"classicModeSteps";Layout.fillWidth:true;text:classicModeInstructions}
                        InfoText{Layout.fillWidth:true;text:"You must make this choice in Battle.net yourself, even if you chose it here. Verify the intended mode and realm before playing. Other Classic clients and test products are not covered by this guide."}
                    }
                    ColumnLayout {
                        visible:page==="found";Layout.fillWidth:true;Layout.margins:24;spacing:16
                        Label{text:"Choose an installation";font.pixelSize:28;font.bold:true;color:"#f2f6fc"}
                        InfoText{Layout.fillWidth:true;text:scan.launchers.length>1?"More than one Battle.net installation was found. Choose the one you use.":"These are the games and Battle.net installations we found."}
                        Repeater{model:scan.warnings;InfoText{required property string modelData;Layout.fillWidth:true;text:modelData;color:"#efc387"}}
                        Repeater{model:scan.launchers;ColumnLayout{required property var modelData;required property int index;Layout.fillWidth:true
                            ActionButton{text:"Use installation "+(index+1);Layout.fillWidth:true;enabled:!backend.busy;onClicked:call("adoption_candidates",{prefix:modelData.prefix})}
                            InfoText{Layout.fillWidth:true;text:modelData.prefix;font.pixelSize:13}
                        }}
                        Repeater{model:scan.games;InfoText{required property var modelData;Layout.fillWidth:true;text:modelData.name}}
                        InfoText{visible:scan.launchers.length===0;Layout.fillWidth:true;text:"No Battle.net installation found. If your games are on another drive, connect it and search that folder."}
                        ActionButton{text:"Search another folder";Layout.fillWidth:true;enabled:!backend.busy;onClicked:chooseFolder("search")}
                        ActionButton{text:"Check again";Layout.fillWidth:true;enabled:!backend.busy;onClicked:call("scan")}
                        ActionButton{text:"Install a separate copy";Layout.fillWidth:true;enabled:!backend.busy;onClicked:{liveMode=false;beginFreshSetup()}}

                    }
                    ColumnLayout {
                        visible:page==="setup";Layout.fillWidth:true;Layout.margins:24;spacing:16
                        Label{text:liveMode?"Choose games for the test":reinstallMode?"Reinstall World of Warcraft":"Choose your games";font.pixelSize:28;font.bold:true;color:"#f2f6fc"}
                        InfoText{Layout.fillWidth:true;text:reinstallMode?"The next screen will show exactly what will be removed. You’ll need to download these games again.":"Supported core release: Retail and Classic Era. Choose at least one. Availability depends on your Battle.net account. Experimental choices are not covered by core acceptance."}
                        Repeater{model:["retail","era"].map(function(id){return catalog.find(function(e){return e.id===id})}).filter(function(e){return !!e});InstallerCheckBox{required property var modelData;Layout.fillWidth:true;objectName:"edition_"+modelData.id;text:modelData.id==="retail"?"World of Warcraft (Retail)":modelData.name;checked:editions.indexOf(modelData.id)>=0;onToggled:toggleEdition(modelData.id,checked)}}
                        InstallerCheckBox{id:otherEditions;objectName:"moreEditions";text:"Experimental editions and test clients — outside the supported core release";Layout.fillWidth:true}
                        Repeater{model:otherEditions.checked?catalog.filter(function(e){return ["retail","era"].indexOf(e.id)<0}):[];ColumnLayout{required property var modelData;Layout.fillWidth:true;InstallerCheckBox{objectName:"edition_"+modelData.id;Layout.fillWidth:true;text:modelData.name;checked:editions.indexOf(modelData.id)>=0;onToggled:toggleEdition(modelData.id,checked)}InfoText{Layout.fillWidth:true;text:modelData.note;font.pixelSize:14}}}
                        InfoText{Layout.fillWidth:true;text:"Exactly one Battle.net Steam tile opens the ordinary edition chooser for all selected games. Separate WoW and edition tiles are deferred."}
                        InfoText{Layout.fillWidth:true;text:"Classic Era also serves Hardcore and Season of Discovery. Era, Hardcore and Season of Discovery share one client and download. Other Classic products use separate clients; installed editions use the single Battle.net tile; this mode guide does not select them.";font.pixelSize:14}
                        ActionButton{objectName:"setupClassicGuide";visible:editions.indexOf("era")>=0;Layout.fillWidth:true;text:"Classic mode: "+classicModeName()+" — selection guide";onClicked:go("classicMode")}
                        Label{text:"Addon manager (optional, experimental — not required for core setup)";color:"#edf3fa"}
                        ComboBox{id:setupAddon;objectName:"setupAddon";Layout.fillWidth:true;model:[{name:"No addon manager (first release)",value:"none"}];textRole:"name";valueRole:"value";currentIndex:Math.max(0,model.findIndex(function(m){return m.value===wantedAddon}));onActivated:wantedAddon=currentValue}
                        Label{text:"Battle.net language";color:"#edf3fa";font.pixelSize:18}
                        ComboBox{id:languageChoice;Layout.fillWidth:true;textRole:"name";valueRole:"code";model:[{name:"English (US)",code:"enUS"},{name:"English (UK)",code:"enGB"},{name:"Deutsch",code:"deDE"},{name:"Español (España)",code:"esES"},{name:"Español (Latinoamérica)",code:"esMX"},{name:"Français",code:"frFR"},{name:"Italiano",code:"itIT"},{name:"한국어",code:"koKR"},{name:"Polski",code:"plPL"},{name:"Português (Brasil)",code:"ptBR"},{name:"Русский",code:"ruRU"},{name:"繁體中文",code:"zhTW"},{name:"简体中文",code:"zhCN"}]}
                        InfoText{Layout.fillWidth:true;text:reinstallMode?"Keep the current compatibility tool: "+current.proton:effectiveProton?"Compatibility tool: "+(steamInfo.protons.find(function(p){return p.name===effectiveProton})||{label:effectiveProton}).label: "Proton 11 and its required Steam runtime must be ready. Choose Check again to check installed prerequisites. If needed, install the missing tool through Steam below. Your game and addon choices are kept.";color:effectiveProton||reinstallMode?"#b7c3d2":"#efc387"}
                        ActionButton{objectName:"protonCheck";visible:!effectiveProton&&!reinstallMode;text:"Check again";enabled:!backend.busy;onClicked:checkProton()}
                        ActionButton{objectName:"protonGet";visible:page==="setup"&&!effectiveProton&&!reinstallMode&&!(protonRuntimeSteam===steamInfo.path&&missingProtonRuntime);text:"Get Proton in Steam";enabled:!backend.busy&&!!steamInfo.path;onClicked:requestProton()}
                        InfoText{visible:!effectiveProton&&protonRuntimeSteam===steamInfo.path&&!!missingProtonRuntime;Layout.fillWidth:true;text:"Proton 11 is installed. Required Steam runtime app "+missingProtonRuntime+" is missing or incomplete; keep the installed Proton copy."}
                        ActionButton{objectName:"protonRuntimeGet";visible:!effectiveProton&&!reinstallMode&&protonRuntimeSteam===steamInfo.path&&!!missingProtonRuntime;text:"Get required Steam runtime";enabled:!backend.busy;onClicked:requestProton(true)}
                        InfoText{visible:!effectiveProton&&!reinstallMode;Layout.fillWidth:true;text:"Steam may ask you to confirm Install or sign in. We check completion; opening Steam is not completion. If Steam just opens its Library, choose Get Proton in Steam again. Cancelling there keeps your choices here."}
                        ActionButton{objectName:"protonStop";visible:protonWaiting;Layout.fillWidth:true;text:"Stop checking (keep setup choices)";onClicked:{protonWaiting=false;notice="Checking stopped. Any Steam download is controlled in Steam. Your setup choices are kept."}}
                        InstallerCheckBox{text:"Advanced: location and compatibility";checked:setupAdvanced;onToggled:setupAdvanced=checked}
                        ColumnLayout{visible:setupAdvanced||scan.steam.length===0||steamInfo.users.length!==1;Layout.fillWidth:true;spacing:12
                            InfoText{Layout.fillWidth:true;visible:scan.steam.length===0;text:"Open Steam and sign in once, then choose Check again."}
                            ComboBox{Layout.fillWidth:true;visible:scan.steam.length>1;model:scan.steam;textRole:"path";currentIndex:selectedSteam;onActivated:{selectedSteam=currentIndex;selectedProton="";selectedProfile=""}}
                            ComboBox{id:profileChoice;enabled:!reinstallMode;Layout.fillWidth:true;model:steamInfo.users;currentIndex:model.indexOf(effectiveProfile);onActivated:selectedProfile=model[currentIndex];displayText:reinstallMode?"Steam profile: "+current.user:effectiveProfile?"Steam profile: "+effectiveProfile:count?"Choose your Steam profile":"No Steam profile found"}
                            ComboBox{id:protonChoice;objectName:"protonChoice";enabled:!reinstallMode;Layout.fillWidth:true;model:steamInfo.protons;textRole:"label";valueRole:"name";currentIndex:model.findIndex(function(p){return p.name===effectiveProton});displayText:reinstallMode?current.proton:currentIndex<0?"Choose a compatibility tool":(model.find(function(p){return p.name===effectiveProton})||{label:"Choose a compatibility tool"}).label;onActivated:selectedProton=model[currentIndex].name}
                            Label{text:"Install location";color:"#edf3fa"}
                            RowLayout{Layout.fillWidth:true;TextField{id:locationField;objectName:"installLocation";Layout.fillWidth:true;placeholderText:"Choose a folder";selectByMouse:true}ActionButton{objectName:"locationBrowse";text:"Browse";onClicked:chooseFolder("location")}}
                            InfoText{Layout.fillWidth:true;text:"A new folder will be created here. Existing installations stay in place.";font.pixelSize:14}
                        }
                        InstallerCheckBox{id:liveReinstall;visible:liveMode;text:"Also test reinstall and settings restore (downloads games twice)";Layout.fillWidth:true}
                        InstallerCheckBox{id:liveCleanup;visible:liveMode;text:"Remove the test copy when the run finishes";Layout.fillWidth:true}
                        ActionButton{objectName:"setupReview";text:liveMode?"Review test run":reinstallMode?"Review reinstall":"Continue";primary:true;Layout.fillWidth:true;enabled:!backend.busy&&scan.steam.length>0&&(reinstallMode||effectiveProfile!=="")&&editions.length>0&&(reinstallMode||effectiveProton!=="");onClicked:reviewSetup()}

                    }
                    ColumnLayout {
                        visible:page==="review";Layout.fillWidth:true;Layout.margins:24;spacing:16
                        Label{text:plan.destructive?"Confirm removal":"Ready to continue?";font.pixelSize:28;font.bold:true;color:"#f2f6fc"}
                        InfoText{Layout.fillWidth:true;text:Guide.planText(plan.kind)}
                        InfoText{visible:!!plan.installation;Layout.fillWidth:true;text:plan.installation?"Installation: "+plan.installation.root:"";font.pixelSize:14}
                        InfoText{objectName:"reviewChoices";visible:!!plan.choices;Layout.fillWidth:true;text:plan.choices?(plan.kind==="repair"?"Installed editions using the single Battle.net tile: ":["clean","clean_owned"].indexOf(plan.kind)>=0?"Original setup selection (all editions inside the listed removal folders are removed): ":"Games: ")+(plan.choices.editions||[]).map(editionName).join(", ")+"\nAddon manager: "+addonName(plan.choices.addon_manager)+"\nBattle.net tile: "+(plan.choices.launcher_tile?"yes":"no"):""}
                        Repeater{model:plan.destructive||["legacy","product"].indexOf(plan.kind)>=0?plan.summary:[];InfoText{required property string modelData;Layout.fillWidth:true;text:modelData}}
                        Repeater{model:plan.removal;InfoText{required property string modelData;Layout.fillWidth:true;text:modelData;color:"#ffc4cc"}}
                        Repeater{model:plan.kind==="repair"&&!reviewDetails?(plan.summary||[]).filter(function(s){return s.indexOf("Remove duplicate shortcut:")===0||s.indexOf("Shared ")===0||s.indexOf("Before applying, create")===0}):[];InfoText{required property string modelData;Layout.fillWidth:true;text:modelData}}
                        InstallerCheckBox{visible:!plan.destructive&&["legacy","product"].indexOf(plan.kind)<0;text:"Show details";checked:reviewDetails;onToggled:reviewDetails=checked}
                        Repeater{model:reviewDetails&&!plan.destructive?plan.summary:[];InfoText{required property string modelData;Layout.fillWidth:true;text:modelData}}
                        InfoText{Layout.fillWidth:true;text:"Keep Steam open while choosing options. Setup will close it only for the reviewed changes, finish that work automatically, then reopen Steam before asking you to continue. Close games and Battle.net yourself first. Steam will never be force-closed. Built-in desktop controls may pause during this automatic interval; no further input is required until Steam restoration is attempted.";color:"#efc387"}
                        InstallerCheckBox{id:reviewConsent;Layout.fillWidth:true;text:"I’m ready. Close and reopen Steam when needed."}
                        InstallerCheckBox{id:deletionConsent;visible:plan.destructive;Layout.fillWidth:true;text:"Delete the listed folders. I understand the games must be downloaded again.";contentItem:Text{text:deletionConsent.text;color:"#ffc4cc";font.pixelSize:15;wrapMode:Text.WordWrap;leftPadding:deletionConsent.indicator.width+deletionConsent.spacing}}
                        ActionButton{text:plan.kind==="reinstall"?"Remove and reinstall":plan.destructive?"Remove selected items":"Continue";primary:!plan.destructive;danger:plan.destructive;Layout.fillWidth:true;enabled:!backend.busy&&reviewConsent.checked&&(!plan.destructive||deletionConsent.checked);onClicked:call("guided_apply",{manage_steam:true,token:plan.token,confirmation:plan.destructive&&deletionConsent.checked?"REMOVE THIS COPY":""})}

                    }
                    ColumnLayout {
                        visible:page==="sharedTileUpgrade";Layout.fillWidth:true;Layout.margins:24;spacing:16
                        Label{text:"Choose the shared tile's controls";font.pixelSize:28;color:"#f2f6fc"}
                        InfoText{Layout.fillWidth:true;text:"Your existing launcher or edition tiles may have different custom controller layouts. Choose which tile's identity and controls Battle.net will keep. The other confirmed installer-owned launcher and edition shortcuts are consolidated; their controller and artwork files are preserved, not merged or deleted."}
                        InfoText{Layout.fillWidth:true;text:"Save/export any layouts you want through Steam before continuing. Setup also makes a verified private metadata/controller backup on this device before consolidation. It keeps the installed games, addons, vendor preferences, prefix and Proton."}
                        ComboBox{objectName:"sharedTileChoice";Layout.fillWidth:true;textRole:"name";valueRole:"appid";model:[{name:"Choose an existing tile…",appid:0}].concat(sharedTileChoices);currentIndex:sharedTileChoice?sharedTileChoices.findIndex(function(t){return t.appid===sharedTileChoice})+1:0;onActivated:sharedTileChoice=Number(currentValue)}
                        InfoText{Layout.fillWidth:true;text:sharedTileChoice?"Keep existing tile ID "+sharedTileChoice+". All editions will use this tile's controls.":"No tile has been chosen. No changes will be applied."}
                        ActionButton{objectName:"sharedTileContinue";text:"Review shared tile";primary:true;Layout.fillWidth:true;enabled:!backend.busy&&sharedTileChoices.some(function(t){return t.appid===sharedTileChoice});onClicked:prepareReview()}
                    }
                    ColumnLayout {
                        visible:page==="repairOptions";Layout.fillWidth:true;Layout.margins:24;spacing:16
                        Label{text:"Repair installed editions";font.pixelSize:28;font.bold:true;color:"#f2f6fc"}
                        InfoText{Layout.fillWidth:true;text:"Installation: "+current.root}
                        InfoText{objectName:"repairInstalled";Layout.fillWidth:true;text:"Installed: "+games.map(function(g){return g.id==="retail"?"World of Warcraft (Retail)":editionName(g.id)}).join(", ")}
                        InfoText{objectName:"repairMissing";Layout.fillWidth:true;text:"Missing: "+missingDownloads.map(editionName).join(", ")}
                        InfoText{Layout.fillWidth:true;text:"Finish these downloads in Battle.net, or explicitly review repair for the editions already installed. Your selected editions stay saved."}
                        InstallerCheckBox{id:repairPartialConsent;objectName:"repairPartialConsent";Layout.fillWidth:true;text:"Use installed editions only for this repair";checked:false;onToggled:{if(checked){repairFlowActive=true;repairConsentKey=repairScopeKey}else clearRepairConsent()}}
                        ActionButton{objectName:"repairPartialContinue";text:"Review repair";primary:true;Layout.fillWidth:true;enabled:!backend.busy&&(missingDownloads.length===0||repairConsentKey===repairScopeKey);onClicked:prepareReview()}
                    }
                    ColumnLayout {
                        visible:page==="launcherBehavior";Layout.fillWidth:true;Layout.margins:24;spacing:16
                        Label{text:"Required: close Battle.net after Play";font.pixelSize:28;color:"#f2f6fc"}
                        InfoText{objectName:"launcherBehaviorScope";Layout.fillWidth:true;text:"Choose for this installation: "+(current.root||"")+". Close after Play makes Battle.net exit completely while WoW keeps running. It affects BOTH Desktop and Gaming Mode, including other people using this same installation. This is not a Gaming Mode-only switch."}
                        InfoText{Layout.fillWidth:true;text:"Close Battle.net and WoW normally before applying. The reviewed change saves a private local backup and keeps other vendor settings, game data and Steam tiles. Your choice is remembered on reruns; setup will not silently reapply or reset it. Battle.net can still change its own preferences."}
                        InfoText{visible:!!current.launcher_exit_choice;Layout.fillWidth:true;text:"Saved choice: "+(current.launcher_exit_choice==="close_after_play"?"Close after Play":"Keep current behavior")+(current.launcher_exit_reviewed?". The current setting is checked again; review is required if Battle.net changed it.":". This fresh environment needs its own review and backup.")}
                        InfoText{objectName:"launcherCurrentStatus";Layout.fillWidth:true;text:launcherExitStatus.message||"Check this installation's current launcher preference before continuing."}
                        ActionButton{objectName:"exitOpenLauncher";text:"Open this Battle.net";Layout.fillWidth:true;enabled:!backend.busy&&!!launcherTile();onClicked:launchBattleNet()}
                        ActionButton{objectName:"exitCheckAgain";text:"Check again (after closing Battle.net and WoW)";Layout.fillWidth:true;enabled:!backend.busy&&!!selectedId;onClicked:call("shortcuts",{id:selectedId})}
                        InstallerCheckBox{id:exitConsent;objectName:"exitInstallationConsent";Layout.fillWidth:true;checked:false;text:"I approve changing Battle.net for everyone using this installation in BOTH Desktop and Gaming Mode."}
                        ActionButton{objectName:"reviewCloseAfterPlay";text:"Review Close after Play (recommended)";Layout.fillWidth:true;primary:true;enabled:!!selectedId&&!backend.busy&&exitConsent.checked;onClicked:call("review_exit",{id:selectedId,choice:"close_after_play",confirm_installation_wide:exitConsent.checked})}
                        ActionButton{objectName:"reviewKeepLauncher";text:launcherExitStatus.configured?"Use already-enabled Close after Play":"Leave settings unchanged and pause setup";Layout.fillWidth:true;enabled:!!selectedId&&!backend.busy;onClicked:call("review_exit",{id:selectedId,choice:"keep_current"})}
                        InfoText{Layout.fillWidth:true;text:"Automatic launcher exit is required to finish this release setup. Leaving an unverified setting unchanged preserves shared preferences and keeps setup incomplete. An already-enabled setting can be confirmed without changing vendor preferences. Steam’s Exit game stops the whole tile; it does not close only Battle.net."}
                    }
                    ColumnLayout {
                        visible:page==="games";Layout.fillWidth:true;Layout.margins:24;spacing:16
                        Label{text:setupStep==="launcher"?"Install Battle.net":setupStep==="download"?"Install your games":setupStep==="restore"?"Restore your settings":setupStep==="complete"?"Your games":"Your Battle.net Steam tile";font.pixelSize:28;font.bold:true;color:"#f2f6fc"}
                        InfoText{Layout.fillWidth:true;text:setupStep==="launcher"?"Open the installer below. Finish installing Battle.net, then close Battle.net and return here.":setupStep==="download"?(games.length?"Installed games were found. Check them below to prepare your Steam shortcuts. Open Battle.net if you need updates or another edition.":"Open Battle.net, sign in, and download the editions you chose. Use the default install folder. When downloads finish, close Battle.net and return here."):setupStep==="restore"?"Your addon and game settings were saved before reinstalling. Restore them before opening the games, or start with new settings and keep the backup for later.":setupStep==="complete"?"Open Battle.net below, choose an installed edition and press Play.":"Open Battle.net from Steam. Choose the installed edition in Battle.net and press Play. Addon-manager integration is deferred for this one-tile release. Existing addons and managers are preserved. Optional launch checks are under Advanced."}
                        InfoText{visible:setupStep==="launcher"||setupStep==="download";Layout.fillWidth:true;text:"Keep Steam open for Deck trackpad and keyboard controls. Battle.net uses mouse/touch input. In Gaming Mode use Gaming Mode pointer setup for ordinary trackpad mouse/click; Steam-held pointer is only a temporary workaround. KDE Connect also works for text. Sign in only inside Battle.net; remembered sign-in is optional there. If Battle.net lists multiple WoW accounts, select the account that owns the intended edition before pressing Play. This installer never asks for passwords or authentication codes.";font.pixelSize:14}
                        ActionButton{visible:setupStep==="launcher"||setupStep==="download";text:setupStep==="launcher"?"Open Battle.net installer":"Open Battle.net";Layout.fillWidth:true;primary:true;enabled:!backend.busy&&!!launcherTile();onClicked:launchBattleNet()}
                        ActionButton{visible:setupStep==="launcher";text:"Battle.net is installed — continue";Layout.fillWidth:true;enabled:!backend.busy;onClicked:call("review_launcher",{id:selectedId})}
                        ActionButton{visible:setupStep==="download";text:downloadsChecked?"Continue with these games":games.length?"Check games and add Steam shortcuts":"Downloads finished — check games";Layout.fillWidth:true;enabled:!backend.busy;onClicked:checkDownloads()}
                        InfoText{visible:setupStep==="download"&&downloadsChecked&&missingDownloads.length>0;Layout.fillWidth:true;text:"Still missing: "+missingDownloads.map(editionName).join(", ")+". Finish these downloads, or continue with the games already installed."}
                        InstallerCheckBox{id:partialClients;objectName:"downloadPartialConsent";visible:setupStep==="download"&&downloadsChecked&&missingDownloads.length>0;Layout.fillWidth:true;text:"Continue with only the games already installed"}
                        InstallerCheckBox{id:foreverBeta;Layout.fillWidth:true;visible:setupStep==="download"&&games.filter(function(g){return g.id==="classic_beta"}).length===1&&games.every(function(g){return g.id!=="forever"});text:"This beta download is World of Warcraft Forever (I checked in Battle.net)";contentItem:Text{text:foreverBeta.text;color:"#edf3fa";font.pixelSize:15;wrapMode:Text.WordWrap;leftPadding:foreverBeta.indicator.width+foreverBeta.spacing}}
                        ActionButton{visible:setupStep==="restore";text:"Restore saved settings";primary:true;Layout.fillWidth:true;enabled:!backend.busy;onClicked:call("restore_settings",{id:selectedId,backup:current.restore_backup})}
                        ActionButton{visible:setupStep==="restore";text:"Use new settings and keep the backup";Layout.fillWidth:true;enabled:!backend.busy;onClicked:skipRestoreDialog.open()}
                        InfoText{Layout.fillWidth:true;text:"Gaming Mode: Steam's default gamepad layout may not provide a mouse. The single Battle.net tile uses one controller layout for all editions. Before using Battle.net or a game menu, verify ordinary right-trackpad mouse and click on that tile. Existing custom layouts are not changed."}
                        InfoText{visible:setupStep==="test"||setupStep==="complete";Layout.fillWidth:true;text:"After playing, use WoW’s own menu to quit normally. In Gaming Mode, wait for Steam to return to this same Battle.net tile with green Play, then launch it again. Choose Retail, Classic Era or Forever in Battle.net and press Play. The shared Steam tile does not select an edition. Steam’s Exit game stops the whole tile and is not a way to close only Battle.net."}
                        ActionButton{objectName:"gamesClassicGuide";visible:(current.editions||[]).indexOf("era")>=0||games.some(function(g){return g.id==="era"});Layout.fillWidth:true;text:"Classic mode: "+classicModeName()+" — selection guide";onClicked:go("classicMode")}
                        ActionButton{objectName:"gamesPointerGuide";text:"Gaming Mode pointer setup";Layout.fillWidth:true;enabled:!!selectedId&&!backend.busy;onClicked:openPointerHelp()}
                        Repeater{model:setupStep==="test"||setupStep==="complete"?gameTiles:[];RowLayout{required property var modelData;Layout.fillWidth:true
                            ActionButton{Layout.fillWidth:true;text:"Open "+modelData.name;enabled:!backend.busy;onClicked:call("launch",{id:selectedId,appid:modelData.appid})}
                            ActionButton{visible:advanced;text:(session.state.tests||[]).indexOf(selectedId+":"+(modelData.shared?"single-battlenet-c43":modelData.name))>=0?"Battle.net tile checked":"Battle.net tile works";enabled:!backend.busy&&(session.state.tests||[]).indexOf(selectedId+":"+(modelData.shared?"single-battlenet-c43":modelData.name))<0;onClicked:call("mark_tested",{id:selectedId,name:modelData.shared?"single-battlenet-c43":modelData.name})}
                        }}
                        ActionButton{objectName:"prepareSingleTile";visible:setupStep==="shortcuts";text:"Review the single Battle.net tile";primary:true;Layout.fillWidth:true;enabled:!backend.busy;onClicked:prepareReview()}
                        ActionButton{visible:setupStep==="test";Layout.fillWidth:true;text:"Finish core setup";primary:true;enabled:!backend.busy&&singleTileReady&&launcherExitStatus.ready===true;onClicked:call("addon_done",{id:selectedId})}
                        
                        ActionButton{visible:false;Layout.fillWidth:true;text:"Manage addons";onClicked:go("addons")}
                        ColumnLayout{visible:setupStep==="test"||setupStep==="complete";Layout.fillWidth:true;spacing:8
                            Label{text:"Your games and AddOns folders";font.pixelSize:22;color:"#f2f6fc"}
                            InfoText{objectName:"foldersLocation";Layout.fillWidth:true;text:folderInfo.path||"~/World of Warcraft"}
                            InfoText{objectName:"foldersStatus";Layout.fillWidth:true;text:(folderInfo.message||"")+(folderInfo.messages&&folderInfo.messages.length?"\n"+folderInfo.messages.join("\n"):"")}
                            RowLayout{Layout.fillWidth:true
                                ActionButton{objectName:"openFolders";text:"Open folders";primary:true;enabled:!!selectedId&&!backend.busy&&folderInfo.available===true;onClicked:call("open_folders",{id:selectedId})}
                                ActionButton{objectName:"refreshFolders";text:"Create / refresh links";enabled:!!selectedId&&!backend.busy;onClicked:call("refresh_folders",{id:selectedId})}
                            }
                            InfoText{Layout.fillWidth:true;text:"In Desktop Mode, open this folder and extract each addon into the matching Retail AddOns, Classic Era AddOns or Forever AddOns link. Keep the addon's own folder directly inside AddOns. Links use detected folders, including connected drives; your games and addons stay in place. Missing or occupied paths are reported without overwriting content."}
                            Repeater{model:folderInfo.links||[];InfoText{required property var modelData;Layout.fillWidth:true;text:modelData.name+(modelData.pending?" — pending":!modelData.owned_unchanged?" — changed; preserved":!modelData.target_available?" — target unavailable; reconnect its drive":"")}}
                        }
                        InfoText{visible:setupStep==="complete";Layout.fillWidth:true;text:"Add the other supported edition later: open Battle.net for this selected copy, download Retail or Classic Era into its default folder, then close Battle.net and WoW. Review the shortcuts below. Keep this copy; do not reinstall or create another environment. The review preserves existing game files and tile identities. Reuse the single Battle.net tile and its existing controls; no extra edition tile is created."}
                        ActionButton{objectName:"addEditionReview";visible:setupStep==="complete";text:"Check for another edition and review shortcuts";Layout.fillWidth:true;enabled:!backend.busy;onClicked:prepareReview()}
                        InstallerCheckBox{text:"Advanced";checked:advanced;onToggled:advanced=checked}
                        ColumnLayout{visible:advanced;Layout.fillWidth:true;spacing:12
                            InfoText{Layout.fillWidth:true;text:"Current installation: "+current.root+"\nCompatibility tool: "+current.proton}
                            ActionButton{objectName:"advancedRepair";text:"Check installed games and repair shortcuts";Layout.fillWidth:true;enabled:!backend.busy;onClicked:prepareReview()}
                            ActionButton{text:"Find games in another folder";Layout.fillWidth:true;enabled:!backend.busy;onClicked:chooseFolder("link")}
                            ActionButton{text:"Review Battle.net behavior after Play";Layout.fillWidth:true;enabled:!backend.busy;onClicked:{exitConsent.checked=false;go("launcherBehavior")}}
                            Repeater{model:games;ColumnLayout{required property var modelData;Layout.fillWidth:true;InfoText{Layout.fillWidth:true;text:modelData.name+"\n"+modelData.path}ActionButton{text:"Copy folder path";onClicked:backend.copyText(modelData.path)}}}
                            InstallerCheckBox{id:showAdvancedRoutes;visible:legacyProductTiles.length>0;text:"Experimental: change a game's Battle.net product code (automatic routing is not accepted)"}
                            ComboBox{id:productTile;visible:showAdvancedRoutes.checked&&legacyProductTiles.length>0;Layout.fillWidth:true;model:legacyProductTiles;textRole:"name"}
                            RowLayout{visible:showAdvancedRoutes.checked&&legacyProductTiles.length>0;Layout.fillWidth:true;TextField{id:productCode;Layout.fillWidth:true;placeholderText:"Confirmed product code"}ActionButton{text:"Review change";enabled:!backend.busy&&productTile.currentIndex>=0&&productCode.text.length>0;onClicked:call("set_product",{id:selectedId,appid:legacyProductTiles[productTile.currentIndex].appid,product:productCode.text})}}
                        }

                    }
                    ColumnLayout {
                        visible:page==="pointerHelp";Layout.fillWidth:true;Layout.margins:26;spacing:14
                        Label{text:"Gaming Mode pointer setup";font.pixelSize:28;color:"#f2f6fc"}
                        InfoText{objectName:"pointerScopeText";Layout.fillWidth:true;text:pointerScopeValid?"Selected installation: "+current.root:"Installation changed. Return and choose the intended installation again."}
                        InfoText{Layout.fillWidth:true;text:"Choose the exact Steam tile for manual pointer instructions. This installer does not edit controller layouts; keep your existing custom or shared bindings."}
                        Repeater{model:pointerScopeValid?pointerTiles:[];ActionButton{required property var modelData;objectName:"pointerTile_"+modelData.appid;text:modelData.name+" · Steam tile "+modelData.appid;Layout.fillWidth:true;onClicked:choosePointerTile(modelData)}}
                        InfoText{visible:pointerScopeValid&&pointerTiles.length===0;Layout.fillWidth:true;text:"Finish the reviewed single Battle.net tile setup first. Retiring edition tiles and addon shortcuts are not pointer-guide targets."}
                    }
                    ColumnLayout {
                        visible:page==="pointerSteps";Layout.fillWidth:true;Layout.margins:26;spacing:14
                        Label{text:"Set up this tile’s pointer";font.pixelSize:28;color:"#f2f6fc"}
                        InfoText{objectName:"pointerTargetText";Layout.fillWidth:true;text:pointerTargetValid?pointerTarget.name+" · Steam tile "+pointerTarget.appid:"The installation or shortcut changed. Return and select it again."}
                        ColumnLayout{visible:pointerTargetValid;Layout.fillWidth:true;spacing:14
                            InfoText{Layout.fillWidth:true;text:"Follow these manual Steam steps for this exact tile in Desktop or Gaming Mode. No screen-sharing or input-control permission is needed for this guide. Brightness, screen power, sleep and lock remain under OS control. Keep working custom or shared layouts unchanged; save/export through Steam before any manual change."}

                            InfoText{objectName:"pointerInstructions";Layout.fillWidth:true;text:"1. In Steam Desktop or Gaming Mode, open Library → this exact tile → controller icon. Select the controller you are using.\n2. Keep your deliberate custom layout. If pointer movement and click already work without holding Steam, leave its bindings unchanged. Otherwise save/export the original layout through Steam before editing.\n3. If a change is needed and Steam says the layout is shared, open Layout Options (gear) → Switch To Unique Configuration For This Device. Confirm the layout is unique before editing. This preserves the original shared layout for your other device. If this option is unavailable, stop and keep the shared layout unchanged.\n4. For a new/default tile only, you may first use Templates → Gamepad with Mouse Trackpad → Apply Layout. Then open Edit Layout → Trackpads: set Right Trackpad Behavior to Mouse; set Right Trackpad Click/Press to Left Mouse Click. Do not use As Joystick or Right Stick Click for these two actions. A template name alone does not guarantee clicking; verify both bindings after applying it.\n5. Leave other gamepad buttons, sticks and triggers unchanged. Save/apply in Steam and reopen the layout to verify the bindings. Repeat only for a tile that needs pointer setup; do not replace already-working layouts.\n6. Launch this tile. Without holding Steam, verify pointer movement and one ordinary click in the app/menu, then exit normally. Choose the edition and press Play inside Battle.net. Battle.net and the installed WoW editions use this same tile and its controls."}
                            InfoText{Layout.fillWidth:true;text:"These manual Steam steps cover menu/pointer control, not a full WoW gameplay configuration. Change only this tile’s movement/click bindings if needed. Leave already-working bindings, Desktop controls, other tile/global layouts, Proton and Battle.net preferences as they are.";color:"#efc387"}
                        }
                    }
                    ColumnLayout {
                        visible:page==="adopt";Layout.fillWidth:true;Layout.margins:26;spacing:14
                        Label{text:"Use the existing setup";font.pixelSize:29;font.bold:true;color:"#f2f6fc"}
                        InfoText{Layout.fillWidth:true;text:"Choose the Steam shortcuts to use. Matching shortcuts are selected when there’s only one clear group. Your game files and current compatibility tool will be kept."}
                        InfoText{Layout.fillWidth:true;text:adoption.prefix;font.pixelSize:13}
                        Repeater{model:adoption.candidates;InstallerCheckBox{required property var modelData;Layout.fillWidth:true;text:modelData.name+" · profile "+modelData.user+" · "+(modelData.proton||"Proton not explicitly set");checked:selectedAdoption.some(function(x){return x.appid===modelData.appid&&x.user===modelData.user&&x.steam===modelData.steam});onToggled:{var a=selectedAdoption.slice();if(checked){a=a.filter(function(x){return x.user===modelData.user&&x.steam===modelData.steam&&x.proton===modelData.proton});a.push(modelData)}else a=a.filter(function(x){return x.appid!==modelData.appid||x.user!==modelData.user||x.steam!==modelData.steam});selectedAdoption=a}}}
                        InfoText{visible:adoption.candidates.length===0;Layout.fillWidth:true;text:"No matching Battle.net Steam shortcut was found. Add Battle.net through Steam and choose its working Proton version first, or create a separate new copy.";color:"#efc387"}
                        ActionButton{text:"Continue";primary:true;enabled:!backend.busy&&selectedAdoption.length>0;onClicked:{var s=selectedAdoption[0];call("review_adopt",{prefix:adoption.prefix,steam:s.steam,user:s.user,appid:s.appid,selected:selectedAdoption.map(function(x){return x.appid})})}}
                    }
                    ColumnLayout {
                        visible:page==="addons";Layout.fillWidth:true;Layout.margins:26;spacing:14
                        Label{text:"Set up an addon manager";font.pixelSize:29;font.bold:true;color:"#f2f6fc"}
                        InfoText{Layout.fillWidth:true;text:"Experimental optional features, outside Retail/Classic Era core acceptance. You can skip addons and finish core setup. Choose an addon manager. CurseForge setup reuses your managed copy, or downloads its official Linux AppImage and integrates it through Gear Lever."}
                        ComboBox{id:managerChoice;Layout.fillWidth:true;model:[{name:"CurseForge",id:"curseforge"},{name:"WowUp with CurseForge",id:"wowup_cf"},{name:"WowUp with Wago",id:"wowup_wago"}];textRole:"name";valueRole:"id";currentIndex:Math.max(0,model.findIndex(function(m){return m.id===current.addon_manager}))}
                        InstallerCheckBox{id:addonAdvanced;text:"Advanced: choose a local AppImage or download link";visible:managerChoice.currentValue==="curseforge"}
                        TextField{id:addonUrl;visible:addonAdvanced.checked&&managerChoice.currentValue==="curseforge";Layout.fillWidth:true;placeholderText:"Official Linux AppImage download link (optional)"}
                        ActionButton{objectName:"addonDownload";visible:managerChoice.currentValue!=="curseforge"||addonAdvanced.checked;text:"Download selected manager";enabled:!backend.busy&&(managerChoice.currentValue!=="curseforge"||addonUrl.text.length>0);onClicked:call("download_addon",{manager:managerChoice.currentValue,url:addonUrl.text})}
                        InfoText{visible:managerChoice.currentValue==="curseforge";Layout.fillWidth:true;text:"Set up downloads the official Linux AppImage only when needed, installs Gear Lever if missing, then reviews its native Steam tile. Your existing managed copy and settings are reused. If Gear Lever needs approval, finish in your software store and retry. Updates stay separate in Gear Lever and may need your action.";font.pixelSize:14}
                        ActionButton{visible:managerChoice.currentValue!=="curseforge"||addonAdvanced.checked;text:managerChoice.currentValue==="curseforge"?"Open the official download page":"Open the manager’s website";onClicked:backend.openPublisher(managerChoice.currentValue)}
                        RowLayout{visible:managerChoice.currentValue!=="curseforge"||addonAdvanced.checked;Layout.fillWidth:true;TextField{id:appImagePath;objectName:"appImagePath";Layout.fillWidth:true;placeholderText:"Choose an existing Linux AppImage";selectByMouse:true}ActionButton{objectName:"appImageBrowse";text:"Browse";onClicked:appImageDialog.open()}}
                        ActionButton{objectName:"addonSetup";text:managerChoice.currentValue==="curseforge"?"Set up CurseForge and review Steam tile":"Add manager to Steam";primary:true;enabled:!!selectedId&&(managerChoice.currentValue==="curseforge"||appImagePath.text.length>0)&&!backend.busy;onClicked:managerChoice.currentValue==="curseforge"?call("prepare_gear",{id:selectedId,confirm:true,path:addonAdvanced.checked?appImagePath.text:""}):call("review_addon",{id:selectedId,path:appImagePath.text,manager:managerChoice.currentValue})}
                        Label{visible:managerChoice.currentValue==="curseforge";text:"Connect your WoW folder";font.pixelSize:23;color:"#f2f6fc"}
                        InfoText{visible:managerChoice.currentValue==="curseforge";Layout.fillWidth:true;text:"Guided connection: if CurseForge does not find WoW, copy the detected folder below. In CurseForge open World of Warcraft → Locate Game → Manually add a game. Paste the folder into the folder picker's Name or Location field, then choose Open. Check that the intended Classic or Retail client appears. This does not register the game automatically."}
                        InfoText{visible:managerChoice.currentValue==="curseforge"&&!addonHandoff.length;Layout.fillWidth:true;text:"No verified installed game folder is available for this selection. Finish the download in Battle.net, then refresh here."}
                        ActionButton{objectName:"reviewGameRegistration";visible:managerChoice.currentValue==="curseforge";Layout.fillWidth:true;text:"Connect detected games automatically";enabled:!!selectedId&&addonHandoff.length>0&&!backend.busy;onClicked:call("addon_registration_review",{id:selectedId})}
                        ColumnLayout{visible:!!registrationReview.token;Layout.fillWidth:true
                            InfoText{Layout.fillWidth:true;text:registrationReview.message||""}
                            Repeater{model:registrationReview.targets||[];InfoText{required property var modelData;Layout.fillWidth:true;text:editionName(modelData.edition)+"\n"+modelData.client}}
                            InstallerCheckBox{id:registrationConsent;objectName:"registrationConsent";text:"Allow screen/input and public-folder clipboard access, plus normal CurseForge close/reopen to verify these paths"}
                            InstallerCheckBox{id:registrationAuth;text:"I have finished a previously requested private sign-in (only if paused)"}
                            ActionButton{objectName:"applyGameRegistration";text:"Connect and verify games";primary:true;Layout.fillWidth:true;enabled:!!registrationReview.token&&registrationReview.id===selectedId&&registrationConsent.checked&&!backend.busy;onClicked:call("addon_registration_apply",{id:selectedId,token:registrationReview.token,confirm:true,auth_complete:registrationAuth.checked})}
                        }
                        ColumnLayout{visible:registrationEvidence.results!==undefined;Layout.fillWidth:true
                            InfoText{Layout.fillWidth:true;text:registrationEvidence.message||""}
                            Repeater{model:registrationEvidence.results||[];InfoText{required property var modelData;Layout.fillWidth:true;text:editionName(modelData.edition)+": "+(modelData.path_verified?"path verified":"path unverified")+"; "+(modelData.persistence_verified?"reopen persistence verified":"reopen persistence unverified")+"\n"+(modelData.verified_path||"")}}
                            InfoText{Layout.fillWidth:true;text:!registrationEvidence.initial_selection_observed?"Original selection has not been observed; restoration unverified.":registrationEvidence.initial_selection_required?(registrationEvidence.initial_selection_restored?"Original selection restored.":"Original selection is not verified restored."):"No initial selection to restore."}
                        }
                        ActionButton{visible:managerChoice.currentValue==="curseforge";text:"Refresh detected folders";enabled:!!selectedId&&!backend.busy;onClicked:{addonHandoff=[];call("shortcuts",{id:selectedId})}}
                        Repeater{model:managerChoice.currentValue==="curseforge"?addonHandoff:[];ColumnLayout{required property var modelData;Layout.fillWidth:true;InfoText{Layout.fillWidth:true;text:modelData.name+"\n"+modelData.path}ActionButton{objectName:"copyAddonFolder";text:"Copy detected WoW folder";onClicked:{backend.copyText(modelData.path);notice="WoW folder copied. In CurseForge choose Locate Game → Manually add a game and paste it into the folder picker."}}}}
                        Repeater{model:tiles.filter(function(t){return t.addon});ActionButton{required property var modelData;text:"Open "+modelData.name;Layout.fillWidth:true;enabled:!backend.busy;onClicked:call("launch",{id:selectedId,appid:modelData.appid})}}
                        ActionButton{objectName:"addonPointerGuide";text:"Gaming Mode pointer setup";Layout.fillWidth:true;enabled:!!selectedId&&!backend.busy;onClicked:openPointerHelp()}
                        InfoText{Layout.fillWidth:true;text:"Open the manager and check that it lists your games before finishing. You can skip this if you don’t use addons."}
                        Flow{Layout.fillWidth:true;spacing:10
                            ActionButton{text:"Finish setup";enabled:!!selectedId&&!backend.busy&&(["complete","addons","test"].indexOf(setupStep)>=0);onClicked:call("addon_done",{id:selectedId})}
                            ActionButton{text:"Skip addons and finish";enabled:!!selectedId&&!backend.busy&&(["complete","addons","test"].indexOf(setupStep)>=0);onClicked:call("addon_done",{id:selectedId})}
                        }
                    }
                    ColumnLayout {
                        visible:page==="help";Layout.fillWidth:true;Layout.margins:26;spacing:14
                        Label{text:"Help & advanced options";font.pixelSize:28;font.bold:true;color:"#f2f6fc"}
                        ActionButton{text:"Find installed games";Layout.fillWidth:true;enabled:!backend.busy;onClicked:go("found")}
                        ActionButton{text:"Scan again";Layout.fillWidth:true;enabled:!backend.busy;onClicked:call("scan")}
                        ActionButton{visible:false;text:"Run an installation test";Layout.fillWidth:true;enabled:!backend.busy;onClicked:{go("live");call("live_result")}}
                        ActionButton{text:"Test this build";Layout.fillWidth:true;enabled:!backend.busy;onClicked:{go("testing");call("device_result")}}
                        Label{text:"Controller input";font.pixelSize:20;color:"#8ce1d5"}
                        InfoText{Layout.fillWidth:true;text:"Keep Steam open for built-in desktop controls. Setup closes it only for automatic guarded work and reopens it before asking you to continue. Direct SDL navigation depends on accessible devices and is not a guaranteed closed-Steam fallback. Deck HIDAPI takeover stays off to preserve trackpad mouse mode. If controls are unavailable, restore Steam first."}
                        Switch{text:"Direct controller navigation (mouse stays available)";checked:backend.directInput;onToggled:backend.directInput=checked}
                        InfoText{Layout.fillWidth:true;text:backend.controllerStatus}
                        ActionButton{text:"Reopen Steam desktop controls";enabled:!backend.busy;onClicked:call("reopen_steam")}
                        InfoText{Layout.fillWidth:true;text:"Deck trackpad and virtual keyboard behavior outside this app is managed by Steam. If its keyboard shrinks, dismiss/reopen it or use KDE Connect. The Gaming Mode pointer guide explains manual per-tile changes in Steam. This installer does not edit controller layouts or change system input permissions."}
                        ActionButton{objectName:"controllerTestButton";text:"Test button: "+testedInput+" presses";onClicked:testedInput++}
                        InfoText{Layout.fillWidth:true;text:"KDE Connect is an optional phone keyboard. Setup does not change controller permissions or approve system prompts.";font.pixelSize:14}
                        Label{text:"Start over with a fresh copy";font.pixelSize:20;color:"#8ce1d5"}
                        InfoText{Layout.fillWidth:true;text:"Removing a copy is separate from repairing shortcuts or keeping it while adding another. Reviewed conventional folders in owned or exclusively used native Steam environments can be removed. Shared or unusual environments are blocked."}
                        InstallerCheckBox{id:keepSettings;text:"Keep addon and game settings";checked:false}
                        ActionButton{text:"Review removal of selected copy";danger:true;enabled:!!current.id&&!backend.busy;onClicked:call("review_clean",{id:selectedId,keep_settings:keepSettings.checked})}
                        ActionButton{text:"Review reinstall of selected copy";enabled:!!current.id&&!backend.busy;onClicked:{liveMode=false;beginReinstallSetup()}}
                        ActionButton{text:"Install another copy — keep current";onClicked:{liveMode=false;beginFreshSetup()}}
                        ActionButton{text:"Review old or duplicate Steam tiles";enabled:!!current.id&&!backend.busy;onClicked:call("legacy_candidates",{id:selectedId})}
                        Flow{Layout.fillWidth:true;spacing:10
                            ActionButton{text:"Restore preserved settings";enabled:!!selectedId&&!backend.busy;onClicked:call("settings_backups")}
                            ActionButton{text:"Undo last configuration change";enabled:!backend.busy;onClicked:openMaintenance("undo")}
                            ActionButton{text:"View private diagnostics";enabled:!backend.busy;onClicked:call("diagnostics")}
                        }
                        InfoText{Layout.fillWidth:true;text:"Setup does not run in the background. Open Battle.net for game updates and your addon manager for addon updates.";font.pixelSize:14}
                    }
                    ColumnLayout {
                        visible:page==="remove";Layout.fillWidth:true;Layout.margins:24;spacing:16
                        Label{text:"Remove this installation";font.pixelSize:28;color:"#f2f6fc"}
                        InfoText{Layout.fillWidth:true;text:"Review the exact folders before deleting. Other installations, shared addon managers and Steam/Proton are kept."}
                        InstallerCheckBox{id:removeKeep;text:"Keep addon and game settings in a separate backup";checked:true}
                        InstallerCheckBox{id:removeComplete;visible:!!current.owned;text:"Remove the complete dedicated environment if it is exclusively owned";checked:true}
                        InfoText{visible:!current.owned;Layout.fillWidth:true;text:"This installation was adopted. Only conventional game/launcher folders can be removed; surrounding shared components are retained."}
                        InfoText{Layout.fillWidth:true;text:"Access links in ~/World of Warcraft are separate from game data. Remove links only to keep every target in place. Full removal below still affects only its separately reviewed game/environment folders."}
                        ActionButton{objectName:"removeFolderLinks";text:"Remove access links only — keep games and addons";enabled:!!current.id&&!backend.busy;onClicked:call("remove_folder_links",{id:selectedId,confirmed:true})}
                        ActionButton{text:"Review removal";danger:true;enabled:!!selectedId&&!backend.busy;onClicked:call("review_clean",{id:selectedId,keep_settings:removeKeep.checked,complete_owned:!!current.owned&&removeComplete.checked})}
                    }
                    ColumnLayout {
                        visible:page==="restore";Layout.fillWidth:true;Layout.margins:26;spacing:14
                        Label{text:"Restore saved addon settings";font.pixelSize:27;color:"#f2f6fc"}
                        InfoText{Layout.fillWidth:true;text:"Close WoW first. After downloading the same editions, restore before first opening WoW. Restore matches edition IDs and refuses to overwrite an existing Interface or WTF folder. If they already exist, move them to a separate safe location first; never delete your only copy. This restores Interface/WTF only, not game downloads, launcher sign-in or an entire installation. The backup stays available. Backup folder: "+settingsBackupRoot}
                        ComboBox{id:backupChoice;Layout.fillWidth:true;model:settingsBackups}
                        ActionButton{text:"Restore to selected installation";enabled:!!selectedId&&backupChoice.currentIndex>=0&&!backend.busy;onClicked:call("restore_settings",{id:selectedId,backup:backupChoice.currentText})}
                    }
                    ColumnLayout {
                        visible:page==="legacy";Layout.fillWidth:true;Layout.margins:26;spacing:14
                        Label{text:"Review old Steam tiles";font.pixelSize:27;color:"#f2f6fc"}
                        InfoText{Layout.fillWidth:true;text:"These names may belong to other installations. Inspect each target. Select only the old tiles you want removed; game files stay in place."}
                        Repeater{model:legacyItems;ColumnLayout{required property var modelData;Layout.fillWidth:true
                            InstallerCheckBox{Layout.fillWidth:true;text:modelData.name;onToggled:{var a=legacySelected.slice();if(checked)a.push(modelData.appid);else a=a.filter(function(n){return n!==modelData.appid});legacySelected=a}}
                            InfoText{Layout.fillWidth:true;text:modelData.target+"\n"+modelData.options;font.pixelSize:13}
                        }}
                        ActionButton{text:"Review selected tile removal";enabled:legacySelected.length>0&&!backend.busy;onClicked:call("review_legacy",{id:selectedId,selected:legacySelected})}
                    }
                    ColumnLayout {
                        visible:page==="diagnostics";Layout.fillWidth:true;Layout.margins:26;spacing:14
                        Label{text:"Review before sharing";font.pixelSize:28;color:"#f2f6fc"}
                        InfoText{Layout.fillWidth:true;text:"Only version, counts, edition IDs, and pending-operation flags are included. Copying is local; nothing is automatically sent."}
                        TextArea{Layout.fillWidth:true;text:diagnosticText;readOnly:true;wrapMode:TextEdit.Wrap;font.family:"monospace";selectByMouse:true}
                        ActionButton{text:"Copy diagnostic text";onClicked:{backend.copyText(diagnosticText);notice="Diagnostic text copied."}}
                    }
                }
            }
        }
    }
    Dialog {
        id:maintenanceDialog;objectName:"steamMaintenanceDialog";property string savedOperation:"recover"
        title:savedOperation==="undo"?"Undo last configuration change?":savedOperation==="abandon_cleanup"?"Stop saved removal?":"Continue saved operation?"
        modal:true;anchors.centerIn:parent;width:540
        ColumnLayout{width:parent.width;spacing:16
            InfoText{Layout.fillWidth:true;text:"Steam stays open while you decide. After you confirm, setup will close it normally, complete the saved operation automatically, then reopen Steam before asking you to continue. Close games and Battle.net yourself first. No game or Steam will be force-closed."}
            InfoText{Layout.fillWidth:true;text:maintenanceDialog.savedOperation==="undo"?"Undo restores only the unchanged configuration snapshot, not deleted game files.":maintenanceDialog.savedOperation==="recover"?"Continue the previously reviewed saved scope. Existing ownership, backup and conflict checks still apply.":"Stop only the saved removal; committed changes stay and remaining files are kept. A partly staged deletion may require recovery instead."}
            InstallerCheckBox{id:maintenanceConsent;objectName:"maintenanceConsent";Layout.fillWidth:true;text:"Close and reopen Steam when needed for this saved operation"}
        }
        footer:RowLayout{ActionButton{objectName:"maintenanceCancel";text:"Cancel";onClicked:maintenanceDialog.close()}ActionButton{objectName:"maintenanceContinue";text:"Continue";enabled:maintenanceConsent.checked&&!backend.busy;onClicked:{var op=maintenanceDialog.savedOperation;maintenanceDialog.close();call("guided_existing",{operation:op,manage_steam:true})}}}
    }
    Dialog{id:skipRestoreDialog;title:"Skip restoring saved settings?";modal:true;anchors.centerIn:parent;width:500;standardButtons:Dialog.Ok|Dialog.Cancel;onAccepted:call("skip_restore",{id:selectedId,confirmed:true});InfoText{width:parent.width;text:"Continue with fresh game settings. Your preserved Interface and WTF backup stays available under Help for later restoration."}}
}
