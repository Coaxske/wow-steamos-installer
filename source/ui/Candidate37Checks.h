#pragma once
inline int candidate37Checks(QQuickWindow *window,Bridge &bridge){
 int failures=0,reviewRequests=0;QVariantMap last;
 auto verify=[&](bool ok,const char*n){std::cout<<(ok?"PASS ":"FAIL ")<<n<<std::endl;if(!ok)++failures;};
 std::function<QQuickItem*(QQuickItem*,const QString&)> find=[&](QQuickItem*r,const QString&n)->QQuickItem*{if(r->objectName()==n)return r;for(auto*c:r->childItems())if(auto*f=find(c,n))return f;return nullptr;};
 auto get=[&](const char*n){return find(window->contentItem(),QString::fromUtf8(n));};
 auto seed=[&](){QVariant result;QMetaObject::invokeMethod(window,"seedLauncherBehaviorCheck",Q_RETURN_ARG(QVariant,result));QTest::qWait(50);return result.toBool();};
 bridge.setSelfCheckResponder([&](const QString&a,const QVariantMap&args){
  if(a=="review_exit"){++reviewRequests;last=args;return QVariantMap{{"ok",true},{"data",QVariantMap{{"kind","exit"},{"destructive",false},{"download",false},{"removal",QVariantList{}},{"token","reviewed-fixture"},{"summary",QVariantList{"BOTH Desktop and Gaming Mode"}},{"installation",QVariantMap{{"id","preview"},{"root","/fixture"}}}}}};}
  return QVariantMap{{"ok",false},{"error","No real action allowed in this simulated GUI check"}};
 });
 verify(seed(),"legacy/new launcher routes to normal behavior choice");
 auto*consent=get("exitInstallationConsent");auto*close=get("reviewCloseAfterPlay");auto*keep=get("reviewKeepLauncher");auto*scope=get("launcherBehaviorScope");
 verify(consent&&close&&keep&&scope,"normal choice controls rendered");
 if(!consent||!close||!keep||!scope)return 1;
 verify(!consent->property("checked").toBool()&&!close->isEnabled()&&keep->isEnabled(),"close opt-in defaults off; keep current is available");
 verify(scope->property("text").toString().contains("BOTH Desktop and Gaming Mode"),"installation-wide effect visible before review");
 consent->setProperty("checked",true);QTest::qWait(20);verify(close->isEnabled(),"explicit opt-in enables reviewed close action");
 QMetaObject::invokeMethod(close,"clicked");QTest::qWait(100);
 verify(reviewRequests==1&&last["choice"]=="close_after_play"&&last["confirm_installation_wide"].toBool(),"close click sends explicit consent and no automatic Apply");
 verify(window->property("page")=="review","choice opens normal reviewed plan");
 QMetaObject::invokeMethod(window,"back");QTest::qWait(50);
 verify(!consent->property("checked").toBool(),"return from review requires renewed opt-in");
 QMetaObject::invokeMethod(keep,"clicked");QTest::qWait(100);
 verify(reviewRequests==2&&last["choice"]=="keep_current"&&!last.contains("confirm_installation_wide"),"keep current has its own explicit review without vendor-change approval");
 verify(seed(),"choice page can be revisited");consent->setProperty("checked",true);window->setProperty("selectedId","another");QTest::qWait(20);
 verify(!consent->property("checked").toBool()&&!close->isEnabled(),"installation change invalidates opt-in");
 bridge.setSelfCheckResponder({});return failures?1:0;
}
