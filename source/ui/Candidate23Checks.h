#pragma once
inline int candidate23Checks(QQuickWindow *window,Bridge &bridge){
 int base=candidate21Checks(window,bridge);if(base)return base;
 int failures=0,requests=0;bool failRequest=false;bool correctArgs=true;auto verify=[&](bool ok,const char*n){std::cout<<(ok?"PASS ":"FAIL ")<<n<<std::endl;if(!ok)++failures;};
 auto engine=[](const QString&a,QVariantMap args=QVariantMap{}){wow_reset_cancel();args["action"]=a;auto bytes=QJsonDocument::fromVariant(args).toJson(QJsonDocument::Compact);char*p=wow_request(bytes.constData());auto out=QJsonDocument::fromJson(p).object().toVariantMap();wow_free(p);return out;};
 auto home=qEnvironmentVariable("HOME"),steam=home+"/.local/share/Steam",acf=steam+"/steamapps/appmanifest_4628710.acf";
 QFile file(acf);file.open(QIODevice::ReadOnly);auto complete=file.readAll();file.close();
 auto write=[&](const QByteArray&data){QFile f(acf);return f.open(QIODevice::WriteOnly)&&f.write(data)==data.size();};
 auto refresh=[&](){auto r=engine("scan");auto data=r["data"].toMap();window->setProperty("scan",data["scan"]);QMetaObject::invokeMethod(window,"refreshSession",Q_ARG(QVariant,data["session"]));QCoreApplication::processEvents();};
 bridge.setSelfCheckResponder([&](const QString&a,const QVariantMap&args){if(a=="proton_install"){++requests;correctArgs=correctArgs&&args["steam"]==steam&&args["proton"]=="proton_11";if(failRequest)return QVariantMap{{"ok",false},{"error","Steam rejected the install request (fixture)"}};return QVariantMap{{"ok",true},{"data",QVariantMap{{"steam",steam},{"requested",true},{"ready",false},{"message","Installation requested in Steam—not yet complete. Confirm Install in Steam."}}}};}return engine(a,args);});
 std::function<QQuickItem*(QQuickItem*,const QString&)>find=[&](QQuickItem*r,const QString&n)->QQuickItem*{if(r->objectName()==n)return r;for(auto*c:r->childItems())if(auto*f=find(c,n))return f;return nullptr;};
 auto get=[&](const char*n){return find(window->contentItem(),QString::fromUtf8(n));};
 auto activate=[&](const char*n){auto*i=get(n);return i&&i->isVisible()&&i->isEnabled()&&QMetaObject::invokeMethod(i,"clicked");};
 QFile::remove(acf);refresh();QMetaObject::invokeMethod(window,"beginFreshSetup");QMetaObject::invokeMethod(window,"toggleEdition",Q_ARG(QVariant,"era"),Q_ARG(QVariant,true));window->setProperty("wantedAddon","curseforge");window->setProperty("wantLauncherTile",false);QTest::qWait(60);
 QVariant beforeArgs;QMetaObject::invokeMethod(window,"setupArguments",Q_RETURN_ARG(QVariant,beforeArgs));
 auto editions=window->property("editions").value<QJSValue>().toVariant();
 verify(window->property("effectiveProton").toString().isEmpty()&&get("setupReview")&&!get("setupReview")->isEnabled(),"missing Proton disables actual packaged Continue");
 verify(activate("protonGet"),"visible Get Proton action invokes GUI handoff");verify(diagnosticWait("Proton handoff response",[&]{return !bridge.busy();})&&requests==1,"simulated Steam request acknowledged without completion claim");
 verify(!get("setupReview")->isEnabled()&&window->property("notice").toString().contains("not yet complete"),"request alone never enables setup");
 verify(activate("protonStop")&&!window->property("protonWaiting").toBool(),"Stop checking cancels polling without resetting choices");
 QMetaObject::invokeMethod(window,"returnHome");QMetaObject::invokeMethod(window,"beginFreshSetup");verify(window->property("editions").value<QJSValue>().toVariant()==editions&&window->property("wantedAddon")=="curseforge"&&!window->property("wantLauncherTile").toBool(),"handoff Back/Home resume preserves edition/addon/tile choices");
 failRequest=true;verify(activate("protonGet"),"retry invokes request after cancellation");diagnosticWait("rejected Proton handoff",[&]{return !bridge.busy();});verify(!window->property("protonWaiting").toBool()&&!get("setupReview")->isEnabled(),"failed Steam request stops polling and keeps setup blocked");failRequest=false;
 verify(activate("protonGet"),"cancelled handoff can be retried");diagnosticWait("Proton retry response",[&]{return !bridge.busy();});
 write("\"AppState\" {\"appid\" \"4628710\" \"StateFlags\" \"1026\" \"SizeOnDisk\" \"1\" \"installdir\" \"Proton 11\"}");activate("protonCheck");diagnosticWait("partial Proton response",[&]{return !bridge.busy();});verify(!get("setupReview")->isEnabled(),"partial download files remain blocked after Check again");
 QVariant stale=QVariantMap{{"steam","/other/Steam"},{"ready",true},{"protons",QVariantList{QVariantMap{{"name","proton_11"}}}}};QMetaObject::invokeMethod(window,"receiveProton",Q_ARG(QVariant,stale));verify(!get("setupReview")->isEnabled(),"stale response for another Steam root cannot authorize Continue");
 write(complete);activate("protonCheck");diagnosticWait("completed Proton response",[&]{return !bridge.busy();});verify(window->property("effectiveProton")=="proton_11"&&get("setupReview")->isEnabled(),"real engine completion rescan enables Continue");
 verify(!window->property("protonWaiting").toBool()&&window->property("editions").value<QJSValue>().toVariant()==editions,"completion stops polling and preserves selections");
 QVariant afterArgs;QMetaObject::invokeMethod(window,"setupArguments",Q_RETURN_ARG(QVariant,afterArgs));auto before=beforeArgs.toMap(),after=afterArgs.toMap();for(auto key:{"editions","location","language","addon_manager","launcher_tile","user"})verify(before[key]==after[key],"handoff preserves setup argument");verify(correctArgs,"GUI requests configured Proton for selected Steam root");
 QMetaObject::invokeMethod(window,"reviewSetup");verify(diagnosticWait("post-Proton production review",[&]{return window->property("page")=="review";}),"continued actual GUI reaches production review after completion");
 bridge.setSelfCheckResponder({});return failures?54:0;
}
