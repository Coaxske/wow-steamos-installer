#include "ControllerGeometry.h"
#include <QCoreApplication>
#include <QJSEngine>
#include <iostream>
#include <functional>
using namespace ControllerGeometry;
static int count=0;
static void check(bool value,const char*name){if(!value)throw std::runtime_error(name);++count;std::cout<<"PASS "<<name<<'\n';}
static void refuses(const std::function<void()>&f,const char*name){bool denied=false;try{f();}catch(const std::exception&){denied=true;}check(denied,name);}
static QJsonObject steam(){return {{"id","steam-local"},{"pid",123},{"app","steam"},{"title","Steam"},{"active",true},{"geometry",QJsonArray{0,150,1010,600}},{"workarea",QJsonArray{0,0,1280,734}},{"managed",true},{"moveable",true},{"resizeable",true},{"minframe",QJsonArray{900,600}},{"dock",false},{"popup",false},{"modal",false},{"fullscreen",false}};}
static QJsonObject panel(){return {{"id","panel"},{"pid",777},{"app","org.kde.plasmashell"},{"title","Panel"},{"geometry",QJsonArray{0,734,1280,66}},{"dock",true},{"popup",false}};}
static QJsonObject popup(){return {{"id","popup"},{"pid",777},{"app","org.kde.plasmashell"},{"title","Public menu"},{"geometry",QJsonArray{779,337,163,413}},{"dock",false},{"popup",true}};}
static QJsonObject scene(const QJsonObject&w,const QJsonArray&above={}){QJsonArray a{w};for(const auto&v:above)a.append(v);return {{"width",1280},{"height",800},{"windows",a}};}
static QJsonObject runScript(const QJsonObject&snapshot,const QJsonObject&w,const QRect&destination,const QString&change={},const QString&kind={}){
 QJSEngine engine;auto input=QString::fromUtf8(QJsonDocument(snapshot).toJson(QJsonDocument::Compact));
 QString setup="var input="+input+R"JS(;
var writes=0,unmaximized=0,reply='';var KWin={MaximizeArea:1};
var items=input.windows.map(r=>{
 let current={x:r.geometry[0],y:r.geometry[1],width:r.geometry[2],height:r.geometry[3]};
 let w={internalId:r.id,pid:r.pid,resourceClass:r.app,caption:r.title||'',managed:r.managed===true,moveable:r.moveable===true,resizeable:r.resizeable===true,dock:r.dock===true,popupWindow:r.popup===true,popupMenu:false,appletPopup:false,modal:r.modal===true,fullScreen:r.fullscreen===true,deleted:false,minimized:false,hidden:false,minSize:{width:(r.minframe||[900,600])[0],height:(r.minframe||[900,600])[1]},clientGeometry:current,area:r.workarea};
 w.setMaximize=(v,h)=>{if(v||h)throw Error('unsupported maximize request');unmaximized++;};
 Object.defineProperty(w,'frameGeometry',{get:()=>current,set:r=>{writes++;current=r;}});return w;
});
var workspace={stackingOrder:items,activeWindow:items[0],virtualScreenSize:{width:input.width,height:input.height},clientArea:(option,w)=>{if(!w.area)throw Error('no area');return {x:w.area[0],y:w.area[1],width:w.area[2],height:w.area[3]};}};
function callDBus(a,b,c,d,value){reply=value;}
)JS";
 auto first=engine.evaluate(setup);if(first.isError())throw std::runtime_error(first.toString().toStdString());
 if(!change.isEmpty()){auto result=engine.evaluate(change);if(result.isError())throw std::runtime_error(result.toString().toStdString());}
 QJsonObject prepare{{"id",w["id"]},{"pid",w["pid"]},{"from",w["geometry"]},{"workarea",w["workarea"]},{"to",array(destination)}};
 if(!kind.isEmpty())prepare["kind"]=kind;if(kind=="curseforge_main")prepare["minimum"]=w["minframe"];
 auto result=engine.evaluate(metadataScript(":test","/test",{}, {},prepare));if(result.isError())throw std::runtime_error(result.toString().toStdString());
 auto response=QJsonDocument::fromJson(engine.globalObject().property("reply").toString().toUtf8()).object();response["writes"]=engine.globalObject().property("writes").toInt();response["unmaximized"]=engine.globalObject().property("unmaximized").toInt();return response;
}
int main(int argc,char**argv){QCoreApplication app(argc,argv);try{
 auto diagnosticSteam=steam();auto dw=diagnosticSteam;dw["id"]="dormant-dolphin";dw["app"]="org.kde.dolphin";dw["title"]="PRIVATE-SENTINEL";
 auto diagnosticScene=scene(diagnosticSteam,{dw});auto diag=runScript(diagnosticScene,diagnosticSteam,frame(diagnosticSteam),"items[1].minimized=true;");check(diag["windows"].toArray().size()==1&&diag["dormant_dolphin"].toArray().size()==1,"generated compositor script separates minimized Dolphin from capture/input list");check(!QJsonDocument(diag["dormant_dolphin"].toArray()).toJson().contains("PRIVATE-SENTINEL"),"dormant metadata omits caption/body/path");
 diag=runScript(diagnosticScene,diagnosticSteam,frame(diagnosticSteam),"items[1].hidden=true;");check(diag["windows"].toArray().size()==1&&diag["dormant_dolphin"].toArray().size()==1,"hidden Dolphin cannot become capture or input eligible");auto unrelated=dw;unrelated["app"]="unrelated";diag=runScript(scene(diagnosticSteam,{unrelated}),diagnosticSteam,frame(diagnosticSteam),"items[1].minimized=true;");check(diag["dormant_dolphin"].toArray().isEmpty(),"unrelated minimized applications omitted from diagnostics");

 auto w=steam(),p=panel(),pop=popup();auto original=frame(w);auto m=scene(w,{p});
 check(original.intersected(frame(p)).height()==16,"exact Deck panel overlap is sixteen pixels");
 refuses([&]{unobscured(m,w,original);},"C25-equivalent full-frame capture must refuse panel overlap");
 auto destination=plannedFrame(m,w);check(destination==QRect(0,134,1010,600),"production plan moves exact Deck window inside reserved work area without resize");
 auto fitted=w;fitted["geometry"]=array(destination);unobscured(scene(fitted,{p}),fitted,destination);check(true,"strict capture passes only after frame is actually clear of panel");
 refuses([&]{plannedFrame(scene(w,{p,pop}),w);},"identified genuine popup blocks preparation, not just capture");
 refuses([&]{unobscured(scene(fitted,{p,pop}),fitted,destination);},"popup remains obstruction after fitting");
 auto notPanel=p;notPanel["dock"]=false;refuses([&]{plannedFrame(scene(w,{notPanel}),w);},"plasmashell name alone never exempts unknown surface");
 auto foreign=p;foreign["app"]="other.app";refuses([&]{plannedFrame(scene(w,{foreign}),w);},"dock role from another app not exempted");
 auto intruding=p;intruding["geometry"]=QJsonArray{0,720,1280,50};refuses([&]{plannedFrame(scene(w,{intruding}),w);},"non-edge dock intruding into work area still refuses");
 auto floating=w;floating["workarea"]=QJsonArray{0,0,1280,750};check(area(floating,scene(floating,{p})).contains(frame(floating)),"floating-panel fixture initially fits reported work area");check(plannedFrame(scene(floating,{p}),floating)==destination,"full panel-frame bounds avoided despite sixteen-pixel work-area intrusion");
 auto host=floating;host["geometry"]=QJsonArray{0,430,1010,600};host["workarea"]=QJsonArray{0,0,1920,1030};auto hostPanel=p;hostPanel["app"]="plasmashell";hostPanel["geometry"]=QJsonArray{0,1014,1920,66};auto hostScene=scene(host,{hostPanel});hostScene["width"]=1920;hostScene["height"]=1080;check(plannedFrame(hostScene,host)==QRect(0,414,1010,600),"actual Bazzite panel/work-area geometry handled with observed class identity");
 auto tooTall=w;tooTall["geometry"]=QJsonArray{0,0,1280,800};tooTall["minframe"]=QJsonArray{900,800};refuses([&]{plannedFrame(scene(tooTall,{p}),tooTall);},"minimum height beyond work area is not forced");
 auto fixed=tooTall;fixed["minframe"]=QJsonArray{900,600};fixed["resizeable"]=false;refuses([&]{plannedFrame(scene(fixed,{p}),fixed);},"non-resizeable oversized window refused");
 auto shrink=tooTall;shrink["minframe"]=QJsonArray{900,600};check(plannedFrame(scene(shrink,{p}),shrink)==QRect(0,0,1280,734),"permitted oversize resize uses complete work area");
 auto idle=fitted;idle["moveable"]=false;check(plannedFrame(scene(idle,{p}),idle)==destination,"already fitting frame remains no-op without move permission");
 auto missing=w;missing.remove("workarea");refuses([&]{plannedFrame(scene(missing,{p}),missing);},"missing work area fails closed");
 auto impossible=w;impossible["workarea"]=QJsonArray{0,0,1400,734};refuses([&]{plannedFrame(scene(impossible),impossible);},"work area outside approved display refuses");
 auto moved=fitted;moved["geometry"]=QJsonArray{1,134,1010,600};refuses([&]{unobscured(scene(moved,{p}),fitted,destination);},"changed geometry before capture refuses");
 refuses([&]{stable(fitted,moved);},"changed geometry after capture or before click refuses");
 auto fraction=fitted;fraction["geometry"]=QJsonArray{0.25,134,1010,600};refuses([&]{stable(fitted,fraction);},"fractional geometry changes are not rounded away");
 auto inactive=fitted;inactive["active"]=false;refuses([&]{unobscured(scene(inactive,{p}),fitted,destination);},"inactive target refuses");
 auto swapped=fitted;swapped["pid"]=124;refuses([&]{stable(fitted,swapped);},"changed process identity refuses");
 auto areaChanged=fitted;areaChanged["workarea"]=QJsonArray{0,0,1280,700};refuses([&]{stable(fitted,areaChanged);},"changed work area invalidates observation");
 auto auth=pop;auth["title"]="Authentication required";auth["geometry"]=QJsonArray{1100,0,100,100};refuses([&]{unobscured(scene(fitted,{auth}),fitted,destination);},"auth cue refuses even outside target crop");
 auto ready=runScript(m,w,destination);check(ready["prepare_result"]=="requested"&&ready["writes"]==1,"actual generated compositor script changes only verified target once");
 auto changed=runScript(m,w,destination,"items[0].frameGeometry.x=1;");check(changed["prepare_result"]=="target_changed"&&changed["writes"]==0,"generated script rejects stale geometry without write");
 auto differentArea=runScript(m,w,destination,"items[0].area=[0,0,1280,700];");check(differentArea["prepare_result"]=="target_changed"&&differentArea["writes"]==0,"generated script rejects changed work area without write");
 auto race=runScript(scene(w,{p,pop}),w,destination);check(race["prepare_result"]=="obstructed"&&race["writes"]==0,"popup appearing at compositor boundary prevents placement");
 auto floatingScript=runScript(scene(floating,{p}),floating,destination);check(floatingScript["prepare_result"]=="requested"&&floatingScript["writes"]==1,"generated script accepts safe placement above floating-panel frame");
 auto panelRace=runScript(m,w,destination,"items[1].frameGeometry.y=720;items[1].frameGeometry.height=80;");check(panelRace["prepare_result"]=="obstructed"&&panelRace["writes"]==0,"changed panel geometry covering destination prevents placement");
 auto identity=runScript(m,w,destination,"items[0].pid=124;");check(identity["prepare_result"]=="target_changed"&&identity["writes"]==0,"generated script preserves PID guard");
 auto picker=w;picker["id"]="cf-picker";picker["app"]="org.freedesktop.impl.portal.desktop.kde";picker["title"]="Select Folder";picker["modal"]=true;picker["geometry"]=QJsonArray{0,0,1920,1030};picker["workarea"]=QJsonArray{0,0,1920,1030};auto dock=p;dock["app"]="plasmashell";dock["geometry"]=QJsonArray{0,1014,1920,66};auto pm=scene(picker,{dock});pm["width"]=1920;pm["height"]=1080;QRect safe(0,0,1920,1014);
 auto pickerMoved=runScript(pm,picker,safe,{},"curseforge_folder_picker");check(pickerMoved["prepare_result"]=="requested"&&pickerMoved["writes"]==1&&pickerMoved["unmaximized"]==1,"actual picker script safely restores and fits only exact modal target");
 auto deniedPicker=[&](const QString&edit,const char*message){auto x=runScript(pm,picker,safe,edit,"curseforge_folder_picker");check(x["prepare_result"]=="target_changed"&&x["writes"]==0&&x["unmaximized"]==0,message);};
 deniedPicker("items[0].modal=false;","non-modal portal window refused without mutation");
 deniedPicker("items[0].caption='Select File';","unknown picker title refused without mutation");
 deniedPicker("items[0].pid=999;","picker PID race refused before restore/resize");
 deniedPicker("items[0].frameGeometry.x=1;writes=0;","picker geometry race refused before restore/resize");
 deniedPicker("items[0].area=[0,0,1920,1000];","picker work-area race refused before restore/resize");
 auto po=popup();po["geometry"]=QJsonArray{500,300,200,200};auto pb=pm;pb["windows"]=QJsonArray{picker,dock,po};auto deniedPopup=runScript(pb,picker,safe,{},"curseforge_folder_picker");check(deniedPopup["prepare_result"]=="obstructed"&&deniedPopup["writes"]==0&&deniedPopup["unmaximized"]==0,"genuine popup blocks picker fitting at actual script boundary");
 auto unknownKind=runScript(pm,picker,safe,{},"unknown");check(unknownKind["writes"]==0&&unknownKind["unmaximized"]==0,"unknown preparation kind cannot authorize a window change");
 auto steamAsPicker=runScript(m,w,destination,{},"curseforge_folder_picker");check(steamAsPicker["writes"]==0,"picker capability cannot prepare Steam or another app");
 deniedPicker("items[0].popupWindow=true;","popup masquerading as picker refused without mutation");
 auto cf=steam();cf["app"]="curseforge";cf["title"]="CurseForge";cf["id"]="owned-cf";cf["geometry"]=QJsonArray{0,0,1280,800};cf["workarea"]=QJsonArray{0,0,1280,750};cf["minframe"]=QJsonArray{992,716};auto cm=scene(cf,{p});auto target=curseForgeMainFrame(cm,cf);check(target==QRect(0,0,1280,734),"exact C30 Deck frame fits full panel bounds at734, not MaximizeArea750");
 auto prepared=runScript(cm,cf,target,{},"curseforge_main");check(prepared["prepare_result"]=="requested"&&prepared["writes"]==1&&prepared["unmaximized"]==1,"actual generated CF script restores and fits known main window once");
 auto cfDone=cf;cfDone["geometry"]=array(target);auto doneScene=scene(cfDone,{p});unobscured(doneScene,cfDone,target);check(curseForgeMainFrame(doneScene,cfDone)==target,"already safely fitting CF is a strict no-op");
 auto fakeCF=cf;fakeCF["title"]="CurseForge sign in";refuses([&]{curseForgeMainFrame(scene(fakeCF,{p}),fakeCF);},"unknown/private CF title cannot authorize fitting");fakeCF=cf;fakeCF["modal"]=true;refuses([&]{curseForgeMainFrame(scene(fakeCF,{p}),fakeCF);},"modal CF cannot impersonate main window");fakeCF=cf;fakeCF["managed"]=false;refuses([&]{curseForgeMainFrame(scene(fakeCF,{p}),fakeCF);},"unmanaged CF refuses");fakeCF=cf;fakeCF["fullscreen"]=true;refuses([&]{curseForgeMainFrame(scene(fakeCF,{p}),fakeCF);},"fullscreen CF refuses");
 auto bigCF=cf;bigCF["minframe"]=QJsonArray{992,735};refuses([&]{curseForgeMainFrame(scene(bigCF,{p}),bigCF);},"minimum735 refuses even when reported work area750 would fit");bigCF["minframe"]=QJsonArray{1281,716};refuses([&]{curseForgeMainFrame(scene(bigCF,{p}),bigCF);},"minimum width beyond display refuses");bigCF=cf;bigCF.remove("minframe");refuses([&]{curseForgeMainFrame(scene(bigCF,{p}),bigCF);},"unknown CF minimum refuses");bigCF["minframe"]=QJsonArray{992,0};refuses([&]{curseForgeMainFrame(scene(bigCF,{p}),bigCF);},"invalid zero minimum refuses");bigCF=cf;bigCF["resizeable"]=false;refuses([&]{curseForgeMainFrame(scene(bigCF,{p}),bigCF);},"fixed oversized CF refuses");
 refuses([&]{curseForgeMainFrame(scene(cf,{p,pop}),cf);},"genuine popup refuses main-window preparation");refuses([&]{unobscured(scene(cfDone,{p,pop}),cfDone,target);},"popup after main fitting still refuses capture/input");auto minimumEdge=cf;minimumEdge["minframe"]=QJsonArray{992,734};check(curseForgeMainFrame(scene(minimumEdge,{p}),minimumEdge)==target,"exact minimum height734 accepted without forcing below it");
 auto cfRace=[&](const QString&change,const char*name){auto result=runScript(cm,cf,target,change,"curseforge_main");check(result["writes"]==0&&result["unmaximized"]==0,name);};
 cfRace("items[0].pid=999;","CF PID race refuses before mutation");cfRace("items[0].caption='Other';","CF title race refuses before mutation");cfRace("items[0].frameGeometry.x=1;writes=0;","CF frame race refuses before mutation");cfRace("items[0].area=[0,0,1280,700];","CF work-area race refuses before mutation");cfRace("items[0].minSize.height=735;","CF minimum-size race refuses before mutation");cfRace("items[0].modal=true;","CF modal race refuses before mutation");cfRace("items[1].frameGeometry.y=720;items[1].frameGeometry.height=80;","panel entering requested CF destination refuses before mutation");
 auto obstructedCF=runScript(scene(cf,{p,pop}),cf,target,{},"curseforge_main");check(obstructedCF["prepare_result"]=="obstructed"&&obstructedCF["writes"]==0,"actual generated script refuses real CF obstruction");auto notCF=runScript(m,w,destination,{},"curseforge_main");check(notCF["writes"]==0,"CF capability cannot prepare Steam");
 std::cout<<"PASS ALL "<<count<<" production geometry/actual-script simulated checks; no live desktop/Deck proof\n";return 0;
 }catch(const std::exception&e){std::cerr<<"FAIL "<<e.what()<<'\n';return 1;}}
