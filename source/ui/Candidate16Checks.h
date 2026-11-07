#pragma once
#include <QJsonDocument>
#include <QJsonObject>
#include <QFile>
#include <QDir>
extern "C" {char *wow_request(const char*);void wow_free(char*);void wow_reset_cancel();}
inline int candidate16Checks(QQuickWindow *window,Bridge &bridge,const QString &screenshot){
 int failures=0;auto verify=[&](bool ok,const char *label){std::cout<<(ok?"PASS ":"FAIL ")<<label<<std::endl;if(!ok)++failures;};
 auto engine=[](const QString&a,QVariantMap args=QVariantMap{}){wow_reset_cancel();args["action"]=a;auto bytes=QJsonDocument::fromVariant(args).toJson(QJsonDocument::Compact);char*p=wow_request(bytes.constData());auto out=QJsonDocument::fromJson(p).object().toVariantMap();wow_free(p);return out;};
 auto put=[](const QString&p,const QByteArray&bytes){QDir().mkpath(QFileInfo(p).absolutePath());QFile f(p);return f.open(QIODevice::WriteOnly)&&f.write(bytes)==bytes.size();};
 auto home=qEnvironmentVariable("HOME");if(!home.contains("c16-repair-fixture")){std::cerr<<"Refuse non-fixture HOME";return 40;}
 auto steam=home+"/.local/share/Steam";
 put(steam+"/userdata/123/config/shortcuts.vdf",QByteArray("\0shortcuts\0\10\10",13));
 put(steam+"/config/config.vdf","\"InstallConfigStore\" { \"Software\" { \"Valve\" { \"Steam\" { \"CompatToolMapping\" {} } } } }");
 put(steam+"/steamapps/common/Proton 11/proton","fixture");put(steam+"/steamapps/common/Proton 11/toolmanifest.vdf","\"manifest\" { \"commandline\" \"/proton %verb%\" }");put(steam+"/steamapps/common/Proton 11/version","1752000000 proton-11.0-1");put(steam+"/steamapps/appmanifest_4628710.acf","\"AppState\" { \"appid\" \"4628710\" \"StateFlags\" \"4\" \"SizeOnDisk\" \"1\" \"installdir\" \"Proton 11\" }");put(steam+"/steamapps/common/Proton 11/files/bin/wine","fixture");put(steam+"/steamapps/common/Proton 11/files/bin/wineserver","fixture");
 auto response=engine("review_setup",{{"steam",steam},{"user","123"},{"proton","proton_11"},{"location",home},{"editions",QStringList{"era","retail","forever"}}});
 verify(response.value("ok").toBool(),"real engine setup fixture review");if(!response.value("ok").toBool()){std::cerr<<QJsonDocument::fromVariant(response).toJson().constData();return 40;}
 auto data=home+"/.local/share/wow-steamos-rust";QFile file(data+"/review.json");file.open(QIODevice::ReadOnly);auto plan=QJsonDocument::fromJson(file.readAll()).object().toVariantMap();file.close();auto inst=plan["installation"].toMap();auto root=inst["root"].toString(),prefix=inst["prefix"].toString();auto id=inst["id"].toString();
 QByteArray pe(2048,'\0');pe[0]='M';pe[1]='Z';pe[0x3c]=char(0x80);pe.replace(0x80,4,QByteArray("PE\0\0",4));pe[0x84]=char(0x64);pe[0x85]=char(0x86);pe[0x98]=char(0x0b);pe[0x99]=char(0x02);put(root+"/Battle.net-Setup.exe",pe);
 verify(engine("apply",{{"token",response["data"].toMap()["token"]}})["ok"].toBool(),"real engine setup fixture apply");
 put(prefix+"/pfx/drive_c/Program Files (x86)/Battle.net/Battle.net.exe","MZ");put(prefix+"/pfx/drive_c/Program Files (x86)/Steam/steamclient.dll","MZ");
 auto parent=root+"/games/World of Warcraft";put(parent+"/_classic_era_/WowClassic.exe","MZ");put(parent+"/_retail_/Wow.exe","MZ");
 auto seed=[&](const QString&stage){QFile f(data+"/state.json");f.open(QIODevice::ReadOnly);auto state=QJsonDocument::fromJson(f.readAll()).object().toVariantMap();f.close();auto all=state["installations"].toList();auto i=all[0].toMap();i["stage"]=stage;all[0]=i;state["installations"]=all;put(data+"/state.json",QJsonDocument::fromVariant(state).toJson());auto initial=engine("init");window->setProperty("session",initial["data"]);window->setProperty("selectedId",id);window->setProperty("catalog",initial["data"].toMap()["catalog"]);};
 int reviewCalls=0;QString heldAction;QVariantMap heldArgs;bool defer=false;
 auto responder=[&](const QString&a,const QVariantMap&v)->QVariantMap{if(defer&&(a=="shortcuts"||a=="review_repair")){heldAction=a;heldArgs=v;return {};}if(a=="review_repair")++reviewCalls;return engine(a,v);};
 bridge.setSelfCheckResponder(responder);
 auto item=[&](const char*name){return window->findChild<QQuickItem*>(name);};
 auto activate=[&](QQuickItem*i){if(!i||!diagnosticWait("control visible and enabled",[&]{return i->isVisible()&&i->isEnabled();}))return false;i->forceActiveFocus();QTest::qWait(70);QTest::keyClick(window,Qt::Key_Space);QTest::qWait(90);return true;};
 window->requestActivate();verify(diagnosticWait("active repair window",[&]{return QGuiApplication::focusWindow()==window;}),"Qt window active");
 for(auto stage:{QString("test_games"),QString("complete")})for(auto route:{QString("home"),QString("games")}){
  seed(stage);window->setProperty("advanced",true);window->setProperty("page",route);QTest::qWait(80);auto *entry=item(route=="home"?"homeRepair":"advancedRepair");verify(diagnosticWait("repair entry visible",[&]{return entry&&entry->isVisible();}),"actual saved-stage repair entry visible");auto count=reviewCalls;activate(entry);
  verify(window->property("page")=="repairOptions"&&reviewCalls==count,"entry opens choice before engine repair without consent");
  auto *choice=item("repairPartialConsent"),*next=item("repairPartialContinue");verify(choice&&choice->isVisible()&&!choice->property("checked").toBool()&&next&&!next->isEnabled(),"explicit unchecked consent and disabled review");
  verify(item("repairInstalled")->property("text").toString().contains("Retail")&&item("repairMissing")->property("text").toString().contains("Forever"),"installed and missing editions rendered");
  if(!screenshot.isEmpty())window->grabWindow().save(screenshot+"-choice.png");activate(choice);verify(next->isEnabled(),"Space checks consent once and enables review");activate(next);
  verify(window->property("page")=="review"&&reviewCalls==count+1,"actual GUI input reaches real engine repair review");
  verify(window->property("plan").value<QJSValue>().toVariant().toMap()["choices"].toMap()["editions"].toList().size()==3,"review retains all selected editions");
  if(!screenshot.isEmpty())window->grabWindow().save(screenshot+"-review.png");
 }
 // Actual production SDL B handler, plus Steam-mapped Return/Enter activation.
 {
  Bridge input(false,nullptr,true);
  int device=SDL_JoystickAttachVirtual(SDL_JOYSTICK_TYPE_GAMECONTROLLER,SDL_CONTROLLER_AXIS_MAX,SDL_CONTROLLER_BUTTON_MAX,0);
  auto *joystick=device>=0?SDL_JoystickOpen(device):nullptr;
  if(!joystick||!input.selectSelfCheckController(SDL_JoystickInstanceID(joystick))||!diagnosticReady(input,window,joystick,true))return 33;
  auto press=[&](SDL_GameControllerButton button){SDL_JoystickSetVirtualButton(joystick,button,1);QTest::qWait(160);SDL_JoystickSetVirtualButton(joystick,button,0);QTest::qWait(90);};
  for(auto stage:{QString("test_games"),QString("complete")})for(auto route:{QString("home"),QString("games")})for(auto method:{QString("Space"),QString("Return"),QString("Enter"),QString("SDL_A"),QString("Mouse")}){
   seed(stage);QMetaObject::invokeMethod(window,"returnHome");window->setProperty("advanced",true);window->setProperty("page",route);QTest::qWait(80);activate(item(route=="home"?"homeRepair":"advancedRepair"));
   auto *choice=item("repairPartialConsent");choice->forceActiveFocus();QTest::qWait(50);
   if(method=="Mouse")QTest::mouseClick(window,Qt::LeftButton,Qt::NoModifier,choice->mapToScene(QPointF(choice->width()/2,choice->height()/2)).toPoint());else if(method=="SDL_A")press(SDL_CONTROLLER_BUTTON_A);else QTest::keyClick(window,method=="Return"?Qt::Key_Return:method=="Enter"?Qt::Key_Enter:Qt::Key_Space);
   QTest::qWait(50);
   std::cout<<"B_SEQUENCE stage="<<stage.toStdString()<<" route="<<route.toStdString()<<" activation="<<method.toStdString()<<" checked="<<choice->property("checked").toBool()<<" consent="<<!window->property("repairConsentKey").toString().isEmpty()<<" enabled="<<item("repairPartialContinue")->isEnabled()<<std::endl;
   verify(choice->property("checked").toBool()&&item("repairPartialContinue")->isEnabled(),"all A/keyboard activation routes synchronize consent and visual state");
   // B is injected into the selected virtual instance and consumed by production pollInput/sendKey(Escape).
   verify(input.selfCheckSelectedId()==SDL_JoystickInstanceID(joystick),"selected and injected controller instance IDs match before B");press(SDL_CONTROLLER_BUTTON_B);
   verify(window->property("page")==route,"actual SDL B handler returns to entry page");
   activate(item(route=="home"?"homeRepair":"advancedRepair"));
   verify(window->property("page")=="repairOptions"&&!choice->property("checked").toBool()&&window->property("repairConsentKey").toString().isEmpty()&&!item("repairPartialContinue")->isEnabled(),"actual SDL B/reentry resets BOTH visible checkbox and consent");
   // Also exercise the Escape event Steam desktop mapping can deliver, not footer Back.
   choice->forceActiveFocus();QTest::keyClick(window,Qt::Key_Return);QTest::qWait(40);QTest::keyClick(window,Qt::Key_Escape);QTest::qWait(80);activate(item(route=="home"?"homeRepair":"advancedRepair"));
   verify(window->property("page")=="repairOptions"&&!choice->property("checked").toBool()&&!item("repairPartialContinue")->isEnabled(),"mapped Escape B path resets visible consent on reentry");
  }
  // Inject the exact reported inconsistent state as a labelled fixture: a visual
  // check with no consent. B must reset visual state even if key was already empty.
  seed("test_games");QMetaObject::invokeMethod(window,"returnHome");QTest::qWait(60);activate(item("homeRepair"));
  item("repairPartialConsent")->setProperty("checked",true);
  verify(window->property("repairConsentKey").toString().isEmpty()&&!item("repairPartialContinue")->isEnabled(),"fixture creates checked-but-unconsented disabled-Review state");
  press(SDL_CONTROLLER_BUTTON_B);verify(!item("repairPartialConsent")->property("checked").toBool(),"SDL B clears stale visual check even with already-empty consent key");activate(item("homeRepair"));
  verify(!item("repairPartialConsent")->property("checked").toBool()&&!item("repairPartialContinue")->isEnabled(),"divergent-state B/reentry is visibly unchecked and disabled");
  SDL_JoystickClose(joystick);SDL_JoystickDetachVirtual(device);input.setDirectInput(false);
 }
 // Exact Back/Home reentry sequences through displayed controls, before any customization.
 for(auto stage:{QString("test_games"),QString("complete")})for(auto route:{QString("home"),QString("games")})for(auto leave:{QString("backButton"),QString("footerHome")}){
  seed(stage);QMetaObject::invokeMethod(window,"returnHome");window->setProperty("advanced",true);window->setProperty("page",route);QTest::qWait(80);
  auto count=reviewCalls;activate(item(route=="home"?"homeRepair":"advancedRepair"));activate(item("repairPartialConsent"));verify(item("repairPartialContinue")->isEnabled(),"cancellation sequence explicitly checked consent");
  verify(item(leave.toLatin1().constData())->isVisible(),"actual abandonment footer control visible");activate(item(leave.toLatin1().constData()));
  verify(window->property("repairConsentKey").toString().isEmpty()&&!item("repairPartialConsent")->property("checked").toBool(),"Back/Home abandonment clears actual checkbox and consent");
  // Back from Advanced returns games; Home explicitly returns home.
  window->setProperty("page",route);QTest::qWait(80);activate(item(route=="home"?"homeRepair":"advancedRepair"));
  verify(window->property("page")=="repairOptions"&&!item("repairPartialConsent")->property("checked").toBool()&&!item("repairPartialContinue")->isEnabled()&&reviewCalls==count,"Back/Home reentry requires fresh unchecked consent and no review");
  activate(item("repairPartialConsent"));activate(item("repairPartialContinue"));verify(window->property("page")=="review"&&reviewCalls==count+1,"fresh explicit consent after abandonment reaches real review");
  activate(item("backButton"));verify(window->property("page")=="repairOptions"&&!item("repairPartialConsent")->property("checked").toBool(),"Back from review clears consent too");
  activate(item("repairPartialConsent"));activate(item("repairPartialContinue"));activate(item("footerHome"));QTest::qWait(60);activate(item("homeRepair"));verify(window->property("page")=="repairOptions"&&!item("repairPartialContinue")->isEnabled(),"Home from review also requires fresh consent on reentry");
 }
 // Delayed discovery and review completion: actual Back/Cancel buttons while busy.
 for(auto route:{QString("home"),QString("games")})for(auto late:{QString("shortcuts"),QString("review_repair")})for(auto leave:{QString("backButton"),QString("operationCancel")}){
  seed("test_games");QMetaObject::invokeMethod(window,"returnHome");window->setProperty("advanced",true);window->setProperty("page",route);QTest::qWait(80);
  if(late=="review_repair"){activate(item(route=="home"?"homeRepair":"advancedRepair"));activate(item("repairPartialConsent"));}
  defer=true;heldAction.clear();activate(item(late=="shortcuts"?(route=="home"?"homeRepair":"advancedRepair"):"repairPartialContinue"));
  // Continue first refreshes discovery; deliver that response while deferral stays enabled to hold repair review.
  if(late=="review_repair"){auto a=heldAction;auto v=heldArgs;bridge.completeSelfCheckRequest(a,engine(a,v),v);QTest::qWait(80);}
  verify(bridge.busy()&&heldAction==late,"requested operation held at real async boundary");
  verify(item(leave.toLatin1().constData())->isVisible(),"Back/Cancel visible during deferred operation");activate(item(leave.toLatin1().constData()));auto page=window->property("page");auto generation=window->property("repairGeneration");
  auto a=heldAction;auto v=heldArgs;defer=false;bridge.completeSelfCheckRequest(a,engine(a,v),v);QTest::qWait(90);
  verify(window->property("page")==page&&window->property("repairGeneration")==generation&&window->property("repairConsentKey").toString().isEmpty()&&!item("repairPartialConsent")->property("checked").toBool(),"late discovery/review ignored after actual Back/Cancel");
  QMetaObject::invokeMethod(window,"returnHome");window->setProperty("page",route);QTest::qWait(80);activate(item(route=="home"?"homeRepair":"advancedRepair"));verify(window->property("page")=="repairOptions"&&!item("repairPartialContinue")->isEnabled(),"reentry after delayed cancellation needs new explicit consent");
 }
 // A scope change while review is in flight must invalidate its eventual success.
 seed("test_games");QMetaObject::invokeMethod(window,"returnHome");QTest::qWait(70);activate(item("homeRepair"));activate(item("repairPartialConsent"));
 defer=true;activate(item("repairPartialContinue"));{auto a=heldAction;auto v=heldArgs;bridge.completeSelfCheckRequest(a,engine(a,v),v);}QTest::qWait(60);
 verify(heldAction=="review_repair"&&bridge.busy(),"scope-race repair review actually deferred");window->setProperty("games",QVariantList{});auto changedPage=window->property("page");
 {auto a=heldAction;auto v=heldArgs;defer=false;bridge.completeSelfCheckRequest(a,engine(a,v),v);}QTest::qWait(70);
 verify(window->property("page")==changedPage&&window->property("page")!="review"&&window->property("repairConsentKey").toString().isEmpty(),"changed scope rejects delayed review success");
 seed("test_games");QMetaObject::invokeMethod(window,"returnHome");QTest::qWait(70);defer=true;activate(item("homeRepair"));window->setProperty("selectedId","other-installation");
 {auto a=heldAction;auto v=heldArgs;defer=false;bridge.completeSelfCheckRequest(a,engine(a,v),v);}QTest::qWait(70);
 verify(window->property("page")=="home"&&!window->property("repairFlowActive").toBool()&&window->property("repairConsentKey").toString().isEmpty(),"changed installation rejects delayed discovery");window->setProperty("selectedId",id);
 seed("test_games");window->setProperty("page","home");activate(item("homeRepair"));activate(item("repairPartialConsent"));
 item("downloadPartialConsent")->setProperty("checked",true);window->setProperty("selectedId","other-installation");QTest::qWait(40);verify(window->property("repairConsentKey").toString().isEmpty(),"installation change clears old consent");verify(!item("downloadPartialConsent")->property("checked").toBool(),"installation change clears old download-stage consent too");
 window->setProperty("selectedId",id);auto info=engine("shortcuts",{{"id",id}});window->setProperty("games",info["data"].toMap()["games"]);activate(item("repairPartialConsent"));
 window->setProperty("games",QVariantList{});QTest::qWait(40);verify(window->property("repairConsentKey").toString().isEmpty(),"changed missing set clears consent");
 put(parent+"/_forever_/Wow.exe","MZ");seed("complete");window->setProperty("page","home");QTest::qWait(80);activate(item("homeRepair"));if(window->property("page")!="review")std::cerr<<"Final page="<<window->property("page").toString().toStdString()<<" error="<<window->property("errorText").toString().toStdString()<<std::endl;verify(window->property("page")=="review","fully installed path reaches review without partial consent");
 bridge.setSelfCheckResponder({});return failures?41:0;
}
