#pragma once
// Simulated normal UI flow only: all engine replies/launches are mocked.
inline int candidate42Checks(QQuickWindow *window,Bridge &bridge){
 int failures=0,reviews=0,applies=0,launches=0;qulonglong chosen=0,launched=0;
 auto verify=[&](bool ok,const char *label){std::cout<<(ok?"PASS ":"FAIL ")<<label<<std::endl;if(!ok)++failures;};
 std::function<QQuickItem*(QQuickItem*,const QString&)> find=[&](QQuickItem*r,const QString &name)->QQuickItem*{if(r->objectName()==name)return r;for(auto*c:r->childItems())if(auto*f=find(c,name))return f;return nullptr;};
 const QVariantMap installation{{"id","shared-fixture"},{"root","/fixture/WoW"},{"stage","complete"},{"owned",true},{"proton","proton_11"},{"editions",QStringList{"era","retail"}}};
 const QVariantList games{QVariantMap{{"id","era"},{"name","WoW Classic Era"}},QVariantMap{{"id","retail"},{"name","World of Warcraft"}}};
 const QVariantList legacy{QVariantMap{{"appid",qulonglong(3000000001)},{"name","WoW Classic Era"},{"game",true}},QVariantMap{{"appid",qulonglong(3000000002)},{"name","World of Warcraft"},{"game",true},{"shared",true}},QVariantMap{{"appid",qulonglong(3000000003)},{"name","Battle.net"},{"game",false}}};
 const QVariantList candidates{QVariantMap{{"appid",qulonglong(3000000001)},{"name","WoW Classic Era"}},QVariantMap{{"appid",qulonglong(3000000002)},{"name","World of Warcraft"}}};
 bridge.setSelfCheckResponder([&](const QString &action,const QVariantMap &args)->QVariantMap{
  if(action=="shortcuts")return {{"ok",true},{"data",QVariantMap{{"shortcuts",legacy},{"games",games},{"shared_tile_choices",candidates}}}};
  if(action=="review_repair"){++reviews;chosen=args.value("shared_tile_appid").toULongLong();verify(args.value("id")=="shared-fixture","migration request remains installation-scoped");return {{"ok",true},{"data",QVariantMap{{"kind","repair"},{"token","fixture"},{"summary",QStringList{"Shared tile upgrade: chosen identity preserved"}},{"removal",QVariantList{}},{"destructive",false},{"installation",QVariantMap{{"id","shared-fixture"},{"root","/fixture/WoW"}}},{"choices",QVariantMap{{"editions",QStringList{"era","retail"}},{"launcher_tile",true},{"addon_manager","none"}}}}}};}
  if(action=="apply"||action=="guided_apply"){++applies;return {{"ok",false},{"error","No apply permitted in simulated UI check"}};}
  if(action=="launch"){++launches;launched=args.value("appid").toULongLong();return {{"ok",true},{"data",QVariantMap{{"message","Simulated launch only"}}}};}
  return {{"ok",false},{"error","No other engine action permitted"}};
 });
 window->setProperty("session",QVariantMap{{"state",QVariantMap{{"installations",QVariantList{installation}},{"current","shared-fixture"},{"tests",QVariantList{}}}}});
 window->setProperty("selectedId","shared-fixture");window->setProperty("games",games);window->setProperty("tiles",legacy);window->setProperty("page","games");QTest::qWait(100);
 QMetaObject::invokeMethod(window,"prepareReview");QTest::qWait(100);
 auto*combo=find(window->contentItem(),"sharedTileChoice");auto*next=find(window->contentItem(),"sharedTileContinue");
 verify(window->property("page")=="sharedTileUpgrade"&&reviews==0&&next&&!next->isEnabled(),"multiple legacy tiles require explicit choice before review");
 auto choose=[&](){if(combo){combo->setProperty("currentIndex",2);QMetaObject::invokeMethod(combo,"activated",Q_ARG(int,2));}QTest::qWait(40);};
 choose();verify(window->property("sharedTileChoice").toULongLong()==3000000002ULL&&next&&next->isEnabled(),"normal combo handler selects exact keeper ID");
 if(next)QMetaObject::invokeMethod(next,"clicked");QTest::qWait(60);
 verify(reviews==1&&chosen==3000000002ULL&&window->property("page")=="review"&&applies==0,"selected migration reaches review without applying");
 QMetaObject::invokeMethod(window,"back");QTest::qWait(60);
 verify(window->property("page")=="sharedTileUpgrade"&&window->property("sharedTileChoice").toULongLong()==0&&combo&&combo->property("currentIndex").toInt()==0,"Back clears both displayed and scoped migration choice");
 choose();if(next)QMetaObject::invokeMethod(next,"clicked");QTest::qWait(60);
 verify(reviews==2&&window->property("page")=="review","Back then renewed choice can reach a fresh scoped review");
 const auto oldGeneration=window->property("repairGeneration");
 window->setProperty("selectedId","other-fixture");QTest::qWait(40);
 verify(window->property("sharedTileChoice").toULongLong()==0,"installation change invalidates migration choice");
 bridge.completed("review_repair",QVariantMap{{"ok",true},{"repair_generation",oldGeneration},{"data",QVariantMap{{"kind","repair"}}}});
 verify(applies==0&&reviews==2,"stale completion cannot trigger apply or another review");
 window->setProperty("selectedId","shared-fixture");
 const QVariantList shared{QVariantMap{{"appid",qulonglong(3000000002)},{"name","World of Warcraft"},{"game",true},{"shared",true}},QVariantMap{{"appid",qulonglong(3000000003)},{"name","Battle.net"},{"game",false}}};
 window->setProperty("session",QVariantMap{{"state",QVariantMap{{"installations",QVariantList{installation}},{"current","shared-fixture"},{"tests",QVariantList{"shared-fixture:World of Warcraft"}}}}});
 window->setProperty("tiles",shared);window->setProperty("page","games");QTest::qWait(100);
 verify(!window->property("allGamesTested").toBool(),"legacy edition check does not credit the new shared-tile check");
 verify(window->property("gameTiles").toList().size()==1,"normal games page exposes exactly one shared game tile");
 int openCount=0;std::function<void(QQuickItem*)> open=[&](QQuickItem*r){if(r->isVisible()&&r->property("text")=="Open World of Warcraft"&&r->metaObject()->indexOfSignal("clicked()")>=0){++openCount;QMetaObject::invokeMethod(r,"clicked");}for(auto*c:r->childItems())open(c);};open(window->contentItem());QTest::qWait(40);
 verify(openCount==1&&launches==1&&launched==3000000002ULL,"normal shared Open action dispatches exact shared ID once");
 verify(applies==0,"all simulated migration/game navigation performs zero applies");
 bridge.setSelfCheckResponder({});return failures?68:0;
}
