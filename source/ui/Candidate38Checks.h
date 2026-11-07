#pragma once
inline int candidate38Checks(QQuickWindow *window,Bridge &bridge,const QString &screenshot){
 if(!bridge.demo())return 1;
 int failures=0;
 std::function<QQuickItem*(QQuickItem*)> find=[&](QQuickItem*r)->QQuickItem*{if(r->objectName()=="reviewChoices")return r;for(auto*c:r->childItems())if(auto*f=find(c))return f;return nullptr;};
 auto verify=[&](bool ok,const char*name){std::cout<<(ok?"PASS ":"FAIL ")<<name<<std::endl;if(!ok)++failures;};
 auto seed=[&](const QString &kind){
  QVariantMap choices{{"editions",QVariantList{"era"}},{"addon_manager","none"},{"launcher_tile",true}};
  QVariantMap plan{{"kind",kind},{"choices",choices},{"summary",QVariantList{"All editions inside the listed removal folders are removed."}},{"removal",QVariantList{"/fixture/disposable"}},{"destructive",true},{"installation",QVariantMap{{"root","/fixture/disposable"}}}};
  window->setProperty("plan",plan);window->setProperty("page","review");QTest::qWait(100);
  auto *item=find(window->contentItem());return item?item->property("text").toString():QString();
 };
 for(const auto &kind:{QString("clean"),QString("clean_owned")}){
  const auto text=seed(kind);
  verify(text.startsWith("Original setup selection (all editions inside the listed removal folders are removed): ")&&text.contains("WoW Classic Era"),kind=="clean"?"scoped removal labels original selection and complete folder scope":"owned removal labels original selection and complete folder scope");
 }
 if(!screenshot.isEmpty())verify(window->grabWindow().save(screenshot),"rendered removal review screenshot saved");
 verify(seed("repair").startsWith("Installed editions using the single Battle.net tile: "),"repair retains installed-edition label");
 verify(seed("setup").startsWith("Games: "),"new setup retains requested-games label");
 verify(seed("reinstall").startsWith("Games: "),"replacement setup retains requested-games label");
 return failures?1:0;
}
