#pragma once
#include <QTemporaryDir>
#include <QFile>
#include <QUrl>
// Simulated engine responses; actual normal button -> Bridge -> xdg-open argument is captured by a harmless stub.
inline int candidate44Checks(QQuickWindow*window,Bridge&bridge){
 int failures=0,opens=0,refreshes=0;auto verify=[&](bool ok,const char*s){std::cout<<(ok?"PASS ":"FAIL ")<<s<<std::endl;if(!ok)++failures;};
 QTemporaryDir tmp;if(!tmp.isValid())return 71;
 const QString folder=tmp.path()+"/World of Warcraft",bin=tmp.path()+"/bin",capture=tmp.path()+"/opened-url";
 QDir().mkpath(folder);QDir().mkpath(bin);
 QFile stub(bin+"/xdg-open");stub.open(QIODevice::WriteOnly);stub.write("#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$WOW44_FOLDER_CAPTURE\"\n");stub.close();stub.setPermissions(QFile::ReadOwner|QFile::WriteOwner|QFile::ExeOwner);
 const auto oldPath=qgetenv("PATH"),oldCapture=qgetenv("WOW44_FOLDER_CAPTURE");qputenv("PATH",(bin+":"+QString::fromLocal8Bit(oldPath)).toLocal8Bit());qputenv("WOW44_FOLDER_CAPTURE",capture.toLocal8Bit());
 QVariantMap install{{"id","folder-fixture"},{"root","/fixture/Games"},{"stage","complete"},{"owned",true},{"launcher","/fixture/Battle.net.exe"},{"launcher_exit_choice","close_after_play"},{"launcher_exit_reviewed",true},{"addon_manager","none"}};
 window->setProperty("session",QVariantMap{{"state",QVariantMap{{"installations",QVariantList{install}},{"current","folder-fixture"},{"tests",QVariantList{}}}}});window->setProperty("selectedId","folder-fixture");
 window->setProperty("singleTileReady",true);window->setProperty("launcherExitStatus",QVariantMap{{"ready",true},{"configured",true}});
 window->setProperty("tiles",QVariantList{QVariantMap{{"appid",qulonglong(3000000001)},{"name","Battle.net"},{"game",true},{"shared",true}}});
 window->setProperty("folderInfo",QVariantMap{{"id","folder-fixture"},{"path",folder},{"available",true},{"links",QVariantList{}},{"messages",QStringList{}}});window->setProperty("page","games");QTest::qWait(100);
 std::function<QQuickItem*(QQuickItem*,const QString&)> find=[&](QQuickItem*r,const QString&name)->QQuickItem*{if(r->objectName()==name)return r;for(auto*c:r->childItems())if(auto*v=find(c,name))return v;return nullptr;};
 bridge.setSelfCheckResponder([&](const QString&a,const QVariantMap&args)->QVariantMap{
   verify(args.value("id")=="folder-fixture","folder action stays installation-scoped");
   if(a=="open_folders"){++opens;return{{"ok",true},{"data",QVariantMap{{"id","folder-fixture"},{"path",folder},{"open_path",folder},{"available",true},{"links",QVariantList{}},{"messages",QStringList{}}}}};}
   if(a=="refresh_folders"){++refreshes;return{{"ok",true},{"data",QVariantMap{{"id","folder-fixture"},{"path",folder},{"available",true},{"links",QVariantList{}},{"messages",QStringList{}}}}};}
   return{{"ok",false},{"error","No mutation or game launch permitted in this UI fixture"}};
 });
 auto*button=find(window->contentItem(),"openFolders");auto*location=find(window->contentItem(),"foldersLocation");
 verify(button&&button->isVisible()&&button->isEnabled()&&location&&location->property("text")==folder,"normal completion shows actual folder and enabled Open folders");
 if(button)QMetaObject::invokeMethod(button,"clicked");
 for(int n=0;n<100&&!QFile::exists(capture);++n)QTest::qWait(20);
 QFile log(capture);QString url;if(log.open(QIODevice::ReadOnly))url=QString::fromUtf8(log.readAll()).trimmed();
 verify(opens==1&&QUrl(url).toLocalFile()==folder&&QFileInfo(folder).isDir(),"actual file-manager command resolves to the intended existing directory");
 const auto before=url;
 bridge.completed("open_folders",QVariantMap{{"ok",true},{"data",QVariantMap{{"id","other-fixture"},{"path","/unrelated"},{"open_path","/unrelated"},{"available",true}}}});
 QTest::qWait(30);QFile again(capture);again.open(QIODevice::ReadOnly);verify(QString::fromUtf8(again.readAll()).trimmed()==before,"foreign installation response cannot open another location");
 window->setProperty("folderInfo",QVariantMap{{"path",folder},{"available",false}});QTest::qWait(30);verify(button&&!button->isEnabled(),"unavailable or occupied folder disables Open folders");
 auto*refresh=find(window->contentItem(),"refreshFolders");if(refresh)QMetaObject::invokeMethod(refresh,"clicked");QTest::qWait(50);
 verify(refreshes==1&&window->property("folderInfo").toMap().value("available").toBool(),"normal refresh action updates completion without launch or tile changes");
 qputenv("PATH",oldPath);if(oldCapture.isNull())qunsetenv("WOW44_FOLDER_CAPTURE");else qputenv("WOW44_FOLDER_CAPTURE",oldCapture);
 bridge.setSelfCheckResponder({});QTest::qWait(20);return failures?72:0;
}
