#pragma once
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QRect>
#include <QRectF>
#include <QRegularExpression>
#include <QtMath>
#include <stdexcept>

// Pure policy shared by the actual desktop path and focused fixture checks.
// No window class is exempt from capture/input obstruction checks.
namespace ControllerGeometry {
inline void require(bool ok,const char *message){if(!ok)throw std::runtime_error(message);}
inline QRectF rectF(const QJsonArray &a){
 require(a.size()==4,"Desktop geometry is unavailable; no controls changed.");
 for(const auto &v:a)require(v.isDouble()&&qIsFinite(v.toDouble()),"Desktop geometry is invalid; no controls changed.");
 QRectF r(a[0].toDouble(),a[1].toDouble(),a[2].toDouble(),a[3].toDouble());
 require(r.width()>0&&r.height()>0,"Desktop geometry is empty; no controls changed.");return r;
}
inline QRect frame(const QJsonObject &w){auto r=rectF(w["geometry"].toArray());return QRect(qRound(r.x()),qRound(r.y()),qRound(r.width()),qRound(r.height()));}
inline QJsonArray array(const QRect&r){return {r.x(),r.y(),r.width(),r.height()};}
inline QRect area(const QJsonObject&w,const QJsonObject&m){
 auto f=rectF(w["workarea"].toArray());QRect desktop(0,0,m["width"].toInt(),m["height"].toInt());
 require(!desktop.isEmpty()&&QRectF(desktop).contains(f),"KDE work area is unavailable or outside the approved desktop; no controls changed.");
 int x=qCeil(f.x()),y=qCeil(f.y());QRect r(x,y,qFloor(f.x()+f.width())-x,qFloor(f.y()+f.height())-y);
 require(!r.isEmpty(),"KDE work area is empty; no controls changed.");return r;
}
inline void stable(const QJsonObject&expected,const QJsonObject&actual){
 require(expected["id"].isString()&&!expected["id"].toString().isEmpty()&&expected["id"]==actual["id"]&&expected["pid"].isDouble()&&expected["pid"]==actual["pid"]&&expected["app"]==actual["app"],"The Steam window identity changed. Review and retry; no input sent.");
 require(actual["active"].toBool(),"The intended Steam window is not active. Setup stopped instead of sending input elsewhere.");
 require(expected["geometry"]==actual["geometry"]&&expected["workarea"]==actual["workarea"],"Steam geometry or its available work area changed. Review and retry; no input sent.");
}
// KDE floating panel frames can extend into MaximizeArea. Avoid their
// entire verified edge bounds; do not assume the overlap is transparent.
inline int shellEdge(const QJsonObject&m,const QJsonObject&w,const QJsonObject&item){
 auto name=item["app"].toString();if(!item["dock"].toBool()||item["popup"].toBool()||(name!="plasmashell"&&name!="org.kde.plasmashell"))return 0;
 auto a=area(w,m),r=frame(item);int width=m["width"].toInt(),height=m["height"].toInt();
 bool horizontal=r.x()<=a.x()&&r.x()+r.width()>=a.x()+a.width();
 bool vertical=r.y()<=a.y()&&r.y()+r.height()>=a.y()+a.height();
 if(horizontal&&r.y()<=0&&r.y()+r.height()>0&&r.y()+r.height()<=height)return 1;
 if(horizontal&&r.y()>=0&&r.y()<height&&r.y()+r.height()>=height)return 2;
 if(vertical&&r.x()<=0&&r.x()+r.width()>0&&r.x()+r.width()<=width)return 3;
 if(vertical&&r.x()>=0&&r.x()<width&&r.x()+r.width()>=width)return 4;
 return 0;
}
inline QRect placementArea(const QJsonObject&m,const QJsonObject&w){
 auto a=area(w,m);int left=a.x(),top=a.y(),right=left+a.width(),bottom=top+a.height();
 for(const auto&v:m["windows"].toArray()){auto item=v.toObject();if(item["id"]==w["id"])continue;auto edge=shellEdge(m,w,item);if(!edge)continue;auto r=frame(item);
  if(edge==1)top=qMax(top,r.y()+r.height());else if(edge==2)bottom=qMin(bottom,r.y());else if(edge==3)left=qMax(left,r.x()+r.width());else if(edge==4)right=qMin(right,r.x());
 }
 require(right>left&&bottom>top,"Known KDE edge panels leave no safe placement area; no controls changed.");return QRect(left,top,right-left,bottom-top);
}
inline void unobscured(const QJsonObject&m,const QJsonObject&expected,const QRect&region,bool preparing=false){
 const auto bounds=frame(expected);require(!region.isEmpty()&&bounds.contains(region),"The requested controls are outside the verified Steam frame.");
 area(expected,m);bool above=false,found=false;
 for(const auto&v:m["windows"].toArray()){
  auto item=v.toObject();
  require(!QRegularExpression("sign.?in|log.?in|verification|authentication",QRegularExpression::CaseInsensitiveOption).match(item["title"].toString()).hasMatch(),"Sign-in or authentication needs attention. Capture and input have stopped; sign in privately and retry.");
  if(item["id"]==expected["id"]){require(!found,"Duplicate desktop window identity; no input sent.");stable(expected,item);found=true;above=true;continue;}
  if(!above||!frame(item).intersects(region))continue;
  // Only a compositor-identified reserved panel may be passed during MOVE
  // preparation. It remains an obstruction for EVERY capture and input check.
  const bool reserved=preparing&&shellEdge(m,expected,item)!=0;
  require(reserved,"Another window covers the intended Steam controls. Setup stopped without guessing.");
 }
 require(found,"The target window closed; review before retrying.");
}
inline QRect fitBounds(const QJsonObject&m,const QJsonObject&w,int minimumWidth,int minimumHeight){
 auto f=frame(w);auto a=placementArea(m,w);if(a.contains(f))return f;
 require(w["moveable"].toBool(),"Steam cannot be moved into KDE's work area. Place it there normally and retry.");
 int width=qMin(f.width(),a.width()),height=qMin(f.height(),a.height());
 require((width==f.width()&&height==f.height())||w["resizeable"].toBool(),"Steam cannot fit the available work area without an unsupported resize. No controls changed.");
 auto min=w["minframe"].toArray();require(min.size()==2,"Steam minimum frame size is unknown; no resize attempted.");
 require(width>=qMax(minimumWidth,min[0].toInt())&&height>=qMax(minimumHeight,min[1].toInt()),"Steam's minimum size exceeds KDE's work area. Adjust its normal window before retrying; no controls changed.");
 return QRect(qBound(a.x(),f.x(),a.x()+a.width()-width),qBound(a.y(),f.y(),a.y()+a.height()-height),width,height);
}
inline QRect fit(const QJsonObject&m,const QJsonObject&w){
 require(w["app"]=="steam"&&w["active"].toBool()&&w["managed"].toBool()&&!w["dock"].toBool()&&!w["popup"].toBool()&&!w["modal"].toBool()&&!w["fullscreen"].toBool(),"Steam window cannot be prepared safely. Restore its normal Desktop window and retry.");
 return fitBounds(m,w,900,108);
}
// Only a verified ordinary CurseForge main window may use this capability.
// UID/executable and reviewed managed-route ownership are checked by the caller.
inline QRect curseForgeMainFrame(const QJsonObject&m,const QJsonObject&w){
 require(w["app"]=="curseforge"&&QRegularExpression("^CurseForge$",QRegularExpression::CaseInsensitiveOption).match(w["title"].toString()).hasMatch()&&w["active"].toBool()&&w["managed"].toBool()&&!w["dock"].toBool()&&!w["popup"].toBool()&&!w["modal"].toBool()&&!w["fullscreen"].toBool(),"The active managed CurseForge main window is not verified. No fitting or input allowed.");
 auto min=w["minframe"].toArray();require(min.size()==2&&min[0].isDouble()&&min[1].isDouble()&&qIsFinite(min[0].toDouble())&&qIsFinite(min[1].toDouble())&&min[0].toDouble()>0&&min[1].toDouble()>0,"CurseForge minimum frame size is unknown; no resize attempted.");
 try{unobscured(m,w,frame(w),true);auto result=fitBounds(m,w,900,400);require(result.width()>=qMax(900,qCeil(min[0].toDouble()))&&result.height()>=qMax(400,qCeil(min[1].toDouble())),"The minimum frame size exceeds the safe work area.");return result;}
 catch(const std::exception&e){throw std::runtime_error((QString("CurseForge setup: ")+QString::fromUtf8(e.what()).replace("Steam","CurseForge")).toStdString());}
}
inline QRect plannedFrame(const QJsonObject&m,const QJsonObject&w){unobscured(m,w,frame(w),true);return fit(m,w);}
inline QString metadataScript(const QString&receiver,const QString&object,const QString&activate,const QString&closeWindow,const QJsonObject&prepare={}){
 QJsonObject args{{"receiver",receiver},{"object",object},{"activate",activate},{"close",closeWindow},{"prepare",prepare}};
 return "const request="+QString::fromUtf8(QJsonDocument(args).toJson(QJsonDocument::Compact))+R"JS(;
function geometry(r){return [r.x,r.y,r.width,r.height];}
function same(a,b){return JSON.stringify(a)===JSON.stringify(b);}
function workarea(w){try{return geometry(workspace.clientArea(KWin.MaximizeArea,w));}catch(e){return [];}}
let prepareResult='not_requested';
// Apply only to the exact preflight snapshot, without making a stale request
// current again. This never closes a popup/panel or edits window-manager rules.
if(Object.keys(request.prepare).length){
 prepareResult='target_changed';let p=request.prepare;
 for(const w of workspace.stackingOrder){
  if(String(w.internalId)!==p.id)continue;
  const picker=p.kind==='curseforge_folder_picker',cfmain=p.kind==='curseforge_main';
  const allowed=picker?(String(w.resourceClass)==='org.freedesktop.impl.portal.desktop.kde'&&w.modal&&/^Select (?:a )?Folder$/i.test(String(w.caption))):cfmain?(String(w.resourceClass)==='curseforge'&&!w.modal&&/^CurseForge$/i.test(String(w.caption))):(p.kind===undefined&&String(w.resourceClass)==='steam'&&!w.modal);
  if(w.pid!==p.pid||!allowed||workspace.activeWindow!==w||w.deleted||w.minimized||!w.managed||w.dock||w.popupWindow||w.popupMenu||w.appletPopup||w.fullScreen)break;
  if(!same(geometry(w.frameGeometry),p.from)||!same(workarea(w),p.workarea))break;
  if(cfmain){const min=w.minSize;if(!Array.isArray(p.minimum)||p.minimum.length!==2||!min||!Number.isFinite(min.width)||!Number.isFinite(min.height)||min.width<=0||min.height<=0||min.width!==p.minimum[0]||min.height!==p.minimum[1]||p.to[2]<Math.max(900,min.width)||p.to[3]<Math.max(400,min.height))break;}
  if(!w.moveable||(!w.resizeable&&(p.to[2]!==p.from[2]||p.to[3]!==p.from[3])))break;
  function intersects(a,b){return a[0]<b[0]+b[2]&&b[0]<a[0]+a[2]&&a[1]<b[1]+b[3]&&b[1]<a[1]+a[3];}
  let after=false,blocked=false;
  for(const other of workspace.stackingOrder){
   if(other.deleted||other.minimized||other.hidden)continue;
   if(/sign.?in|log.?in|verification|authentication/i.test(String(other.caption))){blocked=true;break;}
   if(other===w){after=true;continue;}
   if(!after)continue;
   if(intersects(geometry(other.frameGeometry),p.to)){blocked=true;break;}
   if(!intersects(geometry(other.frameGeometry),p.from))continue;
   const r=geometry(other.frameGeometry),a=p.workarea,name=String(other.resourceClass);
   const horizontal=r[0]<=a[0]&&r[0]+r[2]>=a[0]+a[2],vertical=r[1]<=a[1]&&r[1]+r[3]>=a[1]+a[3];
   const edge=(horizontal&&r[1]<=0&&r[1]+r[3]>0&&r[1]+r[3]<=workspace.virtualScreenSize.height)||(horizontal&&r[1]>=0&&r[1]<workspace.virtualScreenSize.height&&r[1]+r[3]>=workspace.virtualScreenSize.height)||(vertical&&r[0]<=0&&r[0]+r[2]>0&&r[0]+r[2]<=workspace.virtualScreenSize.width)||(vertical&&r[0]>=0&&r[0]<workspace.virtualScreenSize.width&&r[0]+r[2]>=workspace.virtualScreenSize.width);
   const reserved=other.dock&&(name==='org.kde.plasmashell'||name==='plasmashell')&&!other.popupWindow&&!other.popupMenu&&!other.appletPopup&&edge;
   if(!reserved){blocked=true;break;}
  }
  if(blocked){prepareResult='obstructed';break;}
  if(picker||cfmain)w.setMaximize(false,false);
  w.frameGeometry={x:p.to[0],y:p.to[1],width:p.to[2],height:p.to[3]};prepareResult='requested';break;
 }
}
if(request.close){for(const w of workspace.stackingOrder)if(String(w.internalId)===request.close&&!w.deleted)w.closeWindow();}
if(request.activate){for(const w of workspace.stackingOrder)if(String(w.internalId)===request.activate&&!w.deleted&&!w.minimized)workspace.activeWindow=w;}
let rows=[],dormantDolphin=[];
for(const w of workspace.stackingOrder){
 if(w.deleted)continue;
 // Diagnostic inventory only: never add hidden/minimized windows to capture,
 // obstruction, activation or input eligibility. No caption/body/path recorded.
 if(w.minimized||w.hidden){if(String(w.resourceClass)==='org.kde.dolphin'&&dormantDolphin.length<16)dormantDolphin.push({id:String(w.internalId),pid:w.pid,app:String(w.resourceClass),geometry:geometry(w.frameGeometry),active:workspace.activeWindow===w,minimized:!!w.minimized,hidden:!!w.hidden});continue;}
 let g=w.frameGeometry,c=w.clientGeometry;
 let min=(w.minSize&&c)?[Math.max(0,w.minSize.width+g.width-c.width),Math.max(0,w.minSize.height+g.height-c.height)]:[];
 rows.push({id:String(w.internalId),pid:w.pid,app:String(w.resourceClass),title:String(w.caption),active:workspace.activeWindow===w,geometry:geometry(g),workarea:workarea(w),managed:!!w.managed,moveable:!!w.moveable,resizeable:!!w.resizeable,minframe:min,dock:!!w.dock,popup:!!(w.popupWindow||w.popupMenu||w.appletPopup),modal:!!w.modal,fullscreen:!!w.fullScreen});
}
callDBus(request.receiver,request.object,'org.wowinstaller.ControllerMetadata','record',JSON.stringify({width:workspace.virtualScreenSize.width,height:workspace.virtualScreenSize.height,windows:rows,dormant_dolphin:dormantDolphin,prepare_result:prepareResult}));
)JS";
}
}
