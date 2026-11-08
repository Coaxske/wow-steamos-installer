#pragma once
#include <QJsonDocument>
#include <QJsonObject>
#include <QFile>
extern "C" {char *wow_request(const char*);void wow_free(char*);}
// Focused normal GUI -> real core check in externally prepared disposable fixtures.
// Only Steam close/reopen orchestration is replaced; no Steam/game/device input acceptance.
inline int candidate45Checks(QQuickWindow*window,Bridge&bridge,const QString&photo){
 const auto home=qEnvironmentVariable("HOME");QFile marker(home+"/repair-fixture.json");
 if(!home.contains("c45-repair-fixture-")||!marker.open(QIODevice::ReadOnly)){std::cerr<<"Refuse non-fixture HOME\n";return 73;}
 const auto expected=QJsonDocument::fromJson(marker.readAll()).object();const auto id=expected["id"].toString();const auto appid=expected["appid"].toVariant().toULongLong();
 int failures=0,reviews=0,applies=0;auto verify=[&](bool ok,const char*label){std::cout<<(ok?"PASS ":"FAIL ")<<label<<std::endl;if(!ok)++failures;};
 auto core=[](const QString&a,QVariantMap args=QVariantMap{}){args["action"]=a;const auto bytes=QJsonDocument::fromVariant(args).toJson(QJsonDocument::Compact);char*p=wow_request(bytes.constData());auto reply=QJsonDocument::fromJson(p).object().toVariantMap();wow_free(p);return reply;};
 auto init=core("init");verify(init["ok"].toBool(),"actual engine reads disposable existing installation");
 window->setProperty("session",init["data"]);window->setProperty("selectedId",id);window->setProperty("catalog",init["data"].toMap()["catalog"]);
 auto scan=core("scan");window->setProperty("scan",scan["data"].toMap()["scan"]);
 bridge.setSelfCheckResponder([&](const QString&a,const QVariantMap&v)->QVariantMap{
  if(a=="review_repair")++reviews;
  if(a=="guided_apply"){++applies;return core("apply",v);}
  if(a=="shortcuts"||a=="review_repair"||a=="scan"||a=="init"||a=="refresh_folders")return core(a,v);
  return{{"ok",false},{"error","No launch or unrelated action is allowed in this fixture"}};
 });
 std::function<QQuickItem*(QQuickItem*,const QString&)> find=[&](QQuickItem*r,const QString&name)->QQuickItem*{if(r->objectName()==name)return r;for(auto*x:r->childItems())if(auto*found=find(x,name))return found;return nullptr;};
 auto inspect=core("shortcuts",{{"id",id}});auto info=inspect["data"].toMap();window->setProperty("tiles",info["shortcuts"]);window->setProperty("games",info["games"]);window->setProperty("launcherExitStatus",info["launcher_exit_status"]);window->setProperty("singleTileReady",false);window->setProperty("page","home");QTest::qWait(80);
 auto*repair=find(window->contentItem(),"homeRepair");verify(repair&&repair->isVisible()&&repair->isEnabled()&&repair->property("text")=="Repair Battle.net tile","existing-copy home exposes Repair Battle.net tile");
 if(!photo.isEmpty())window->grabWindow().save(photo+"-home.png");
 if(repair)QMetaObject::invokeMethod(repair,"clicked");QTest::qWait(120);
 verify(reviews==1&&window->property("page")=="review","normal repair button reaches real repair review");
 auto plan=window->property("plan").value<QJSValue>().toVariant().toMap();const auto summary=plan["summary"].toStringList().join("\n");
 verify(plan["kind"]=="repair"&&!plan["download"].toBool()&&!plan["destructive"].toBool()&&summary.contains("Restore missing Battle.net tile: keep its saved ID "+QString::number(appid)),"review identifies original saved identity and requires no download/removal");
 auto*consent=find(window->contentItem(),"reviewSteamConsent");auto*apply=find(window->contentItem(),"reviewApply");
 verify(consent&&apply&&!apply->isEnabled(),"review requires the existing explicit Steam-change consent");
 if(!photo.isEmpty())window->grabWindow().save(photo+"-review.png");
 if(consent)consent->setProperty("checked",true);QTest::qWait(30);if(apply)QMetaObject::invokeMethod(apply,"clicked");QTest::qWait(140);
 auto after=core("shortcuts",{{"id",id}});const auto rows=after["data"].toMap()["shortcuts"].toList();
 verify(applies==1&&rows.size()==1&&rows[0].toMap()["appid"].toULongLong()==appid&&rows[0].toMap()["name"]=="Battle.net","normal reviewed Continue restores one original Battle.net identity through real apply");
 if(!photo.isEmpty())window->grabWindow().save(photo+"-after.png");
 bridge.setSelfCheckResponder({});return failures?74:0;
}
