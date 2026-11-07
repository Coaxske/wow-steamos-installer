#pragma once
// Rendered Qt input with the production-engine fixture from Candidate21; not hardware acceptance.
inline int candidate22Checks(QQuickWindow *window,Bridge &bridge){
 int base=candidate21Checks(window,bridge);if(base)return base;
 int failures=0;auto verify=[&](bool ok,const char*n){std::cout<<(ok?"PASS ":"FAIL ")<<n<<std::endl;if(!ok)++failures;};
 auto read=[](const QString&p){QFile f(p);f.open(QIODevice::ReadOnly);return f.readAll();};
 const auto home=qEnvironmentVariable("HOME"),data=home+"/.local/share/wow-steamos-rust";
 const auto state=read(data+"/state.json"),shortcuts=read(home+"/.local/share/Steam/userdata/123/config/shortcuts.vdf"),config=read(home+"/.local/share/Steam/config/config.vdf");
 std::function<QQuickItem*(QQuickItem*,const QString&)> find=[&](QQuickItem*r,const QString&n)->QQuickItem*{if(r->objectName()==n)return r;for(auto*c:r->childItems())if(auto*f=find(c,n))return f;return nullptr;};
 auto get=[&](const char*n){return find(window->contentItem(),QString::fromUtf8(n));};
 auto click=[&](const char*n){auto*i=get(n);if(!i||!i->isVisible()||!i->isEnabled())return false;i->forceActiveFocus();QTest::qWait(60);QTest::mouseClick(window,Qt::LeftButton,Qt::NoModifier,i->mapToScene(QPointF(20,i->height()/2)).toPoint());QCoreApplication::processEvents();return true;};
 QMetaObject::invokeMethod(window,"beginFreshSetup");window->setProperty("editions",QVariantList{"era"});QTest::qWait(80);
 verify(click("setupClassicGuide")&&window->property("page")=="classicMode","setup opens actual mode guide");
 auto*combo=get("classicModeChoice");verify(combo&&combo->property("count").toInt()==3,"only verified shared Classic modes offered");
 auto choose=[&](int index,const QString&mode){QTest::qWait(100);combo->forceActiveFocus();QTest::keyClick(window,Qt::Key_Down,Qt::AltModifier);QTest::qWait(60);QTest::keyClick(window,Qt::Key_Home);for(int k=0;k<index;++k)QTest::keyClick(window,Qt::Key_Down);QTest::keyClick(window,Qt::Key_Return);QTest::qWait(70);verify(window->property("classicMode")==mode,"normal keyboard changes intended guide mode");};
 choose(1,"hardcore");auto*steps=get("classicModeSteps");verify(steps&&steps->property("text").toString().contains("Launch Into → Hardcore")&&steps->property("text").toString().contains("verify"),"Hardcore guide includes manual launcher and realm verification");
 QMetaObject::invokeMethod(window,"back");verify(window->property("page")=="setup"&&window->property("editions").value<QJSValue>().toVariant().toStringList()==QStringList{"era"},"Back keeps one client and original edition choice");
 QMetaObject::invokeMethod(window,"returnHome");QMetaObject::invokeMethod(window,"beginFreshSetup");verify(window->property("classicMode")=="hardcore","Home and resumed setup preserve guide preference");
 window->setProperty("page","games");verify(click("gamesClassicGuide")&&window->property("page")=="classicMode","existing saved Classic client exposes same guide");choose(2,"sod");verify(steps->property("text").toString().contains("Season of Discovery"),"SoD instructions match selected preference");choose(0,"era");verify(steps->property("text").toString().contains("Launch Into → Classic Era"),"Era instructions match selected preference");choose(1,"hardcore");
 window->grabWindow().save(home+"/classic-mode-guide.png");
 QMetaObject::invokeMethod(window,"back");QMetaObject::invokeMethod(window,"beginFreshSetup");window->setProperty("editions",QVariantList{"era"});
 bridge.setSelfCheckResponder([&](const QString&a,const QVariantMap&args){wow_reset_cancel();auto request=args;request["action"]=a;auto bytes=QJsonDocument::fromVariant(request).toJson(QJsonDocument::Compact);char*p=wow_request(bytes.constData());auto out=QJsonDocument::fromJson(p).object().toVariantMap();wow_free(p);return out;});
 QMetaObject::invokeMethod(window,"reviewSetup");verify(diagnosticWait("mode preference production review",[&]{return window->property("page")=="review";}),"mode choice still reaches production setup review");
 const auto reviewed=QJsonDocument::fromJson(read(data+"/review.json")).object().toVariantMap()["installation"].toMap();verify(reviewed["editions"].toStringList()==QStringList{"era"}&&!reviewed.contains("classicMode"),"review contains one real client, no invented flavor identity");bridge.setSelfCheckResponder({});
 Bridge resumed(true,nullptr,true);verify(resumed.classicGuideMode()=="hardcore","new backend instance reloads persisted presentation preference");resumed.setClassicGuideMode("anniversary");verify(resumed.classicGuideMode()=="hardcore","unverified client/mode value rejected");
 verify(read(data+"/state.json")==state&&read(home+"/.local/share/Steam/userdata/123/config/shortcuts.vdf")==shortcuts&&read(home+"/.local/share/Steam/config/config.vdf")==config,"mode choices do not change engine state, client identity, shortcuts or Proton bindings");
 return failures?52:0;
}
