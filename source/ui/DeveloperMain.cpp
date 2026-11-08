#include "Bridge.h"
#include "ControllerAutomation.h"
#include <QJsonDocument>
#include <QJsonArray>
#include <QJsonObject>
#include <QFileInfo>
#include <QApplication>
#include <QClipboard>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickWindow>
#include <QQuickItem>
#include <QQuickStyle>
#include <QLockFile>
#include <QStandardPaths>
#include <QDir>
#include <QMessageBox>
#include <QTimer>
#include <QTest>
#include <iostream>
#include "CheckboxCheck.h"
#include "WorkflowChecks.h"
#include "Candidate12Checks.h"
#include "Candidate14Checks.h"
#include "Candidate15Checks.h"
#include "Candidate16Checks.h"
#include "Candidate17Checks.h"
#include "Candidate18Checks.h"
#include "Candidate19Checks.h"
#include "Candidate20Checks.h"
#include "Candidate21Checks.h"
#include "Candidate22Checks.h"
#include "Candidate23Checks.h"
#include "Candidate37Checks.h"
#include "Candidate38Checks.h"
#include "Candidate40Checks.h"
#include "Candidate42Checks.h"
#include "Candidate43Checks.h"
#include "Candidate44Checks.h"
#include "Candidate45Checks.h"
#include "Candidate24Checks.h"
#include "Candidate25Checks.h"
#include "Candidate30Checks.h"
extern "C" {const char *wow_version();char *wow_request(const char*);void wow_free(char*);}
static QByteArray engineReply;
char *requestEngine(const char*s){char *r=wow_request(s);engineReply=r;wow_free(r);return engineReply.data();}
int controllerSafety39Checks();
int wowDeveloperMain(int argc,char **argv){
 // Refuse legacy pointer entry points before QApplication or any portal access.
 QStringList earlyArgs;for(int n=1;n<argc;++n)earlyArgs.append(QString::fromUtf8(argv[n]));
 const QStringList blockedPointerFlags{"--controller-permission-probe","--controller-input-probe","--controller-live-cancel-check","--controller-live-noop-check","--controller-live-correction-check","--controller25-gui-check"};
 bool pointerBlocked=false;for(const auto &flag:blockedPointerFlags)pointerBlocked=pointerBlocked||earlyArgs.contains(flag);
 if(earlyArgs.contains("--controller-driver"))pointerBlocked=pointerBlocked||QFileInfo(earlyArgs.value(earlyArgs.indexOf("--controller-driver")+1)).fileName()=="controller-job.json";
 if(pointerBlocked){std::cout<<QJsonDocument(QJsonObject{{"ok",false},{"error","Automatic pointer configuration is unavailable in this version. Use Gaming Mode pointer setup for manual instructions."},{"results",QJsonArray{}}}).toJson(QJsonDocument::Compact).constData()<<std::endl;return 1;}
 if(argc>1&&QString::fromUtf8(argv[1])=="--engine-json"){QCoreApplication core(argc,argv);std::string input((std::istreambuf_iterator<char>(std::cin)),{});extern char *requestEngine(const char*);auto output=requestEngine(input.c_str());std::cout<<output<<std::endl;return QByteArray(output).contains("\"ok\":false")?1:0;}
 // Use CPU rendering for this lightweight setup UI; do not alter child game environments.
 QQuickWindow::setSceneGraphBackend("software");
 QApplication app(argc,argv);app.setApplicationName("WoW SteamOS Installer");app.setOrganizationName("WoWSteamOSRust");app.setDesktopFileName("org.wowsteamos.Installer");app.setApplicationVersion(QString::fromUtf8(wow_version()));
 const auto args=app.arguments();if(args.contains("--self-check")&&args.contains("--addon-lifecycle35-check"))return addonLifecycle35Check();if(args.contains("--folder-transition-observe34"))return addonFolderObserve34();if(args.contains("--self-check")&&args.contains("--addon-exit33-check"))return addonExit33Check();if(args.contains("--self-check")&&args.contains("--registration30-clipboard-check"))return addonRegistrationClipboardCheck();if(args.contains("--self-check")&&args.contains("--registration30-check"))return addonRegistrationCheck();if(args.contains("--controller-driver")){auto reply=runControllerDriver(args.value(args.indexOf("--controller-driver")+1));std::cout<<reply.constData()<<std::endl;return QJsonDocument::fromJson(reply).object()["ok"].toBool()?0:1;}if(args.contains("--self-check")&&args.contains("--controller-permission-probe"))return controllerPermissionProbe();if(args.contains("--self-check")&&args.contains("--controller-input-probe"))return controllerInputProbe();if(args.contains("--controller-safety39-check"))return controllerSafety39Checks();if(args.contains("--controller-driver-check"))return controllerDriverCheck();if(args.contains("--self-check")&&args.contains("--controller-portal-fixture-check"))return controllerPortalFixtureCheck();if(args.contains("--controller-metadata-check")){try{ControllerPortal portal;auto m=portal.metadata();std::cout<<QJsonDocument(QJsonObject{{"width",m["width"]},{"height",m["height"]},{"metadata_available",true}}).toJson(QJsonDocument::Compact).constData()<<std::endl;return 0;}catch(const std::exception&e){std::cerr<<e.what()<<std::endl;return 1;}}const bool check=args.contains("--self-check");const bool demo=args.contains("--demo")||check;
 QQuickStyle::setStyle("Basic");
 QString path=QStandardPaths::writableLocation(QStandardPaths::GenericDataLocation)+"/wow-steamos-rust";
 std::unique_ptr<QLockFile> lock;
 if(!demo){if(!QDir().mkpath(path)){QMessageBox::critical(nullptr,"Cannot start setup","The local setup data folder could not be created.");return 2;}lock=std::make_unique<QLockFile>(path+"/window.lock");if(!lock->tryLock()){QMessageBox::information(nullptr,"Setup already open","Another setup window is open. Return to that window.");return 2;}}
 DiagnosticExtraController extra(check&&args.contains("--extra-controller"));
 if(check&&args.contains("--extra-controller")&&extra.device<0)return 33;
 Bridge bridge(demo,nullptr,check);QQmlApplicationEngine engine;engine.rootContext()->setContextProperty("backend",&bridge);
 QObject::connect(&engine,&QQmlApplicationEngine::objectCreated,&app,[&](QObject *obj,const QUrl&){if(!obj){std::cerr<<"The QML interface could not load.\n";QCoreApplication::exit(3);}},Qt::QueuedConnection);
 QVariantMap initial;
 if(args.contains("--size")){auto wh=args.value(args.indexOf("--size")+1).split('x');if(wh.size()==2){initial["width"]=wh[0].toInt();initial["height"]=wh[1].toInt();}}
 if(args.contains("--page"))initial["page"]=args.value(args.indexOf("--page")+1);
 engine.setInitialProperties(initial);
 engine.load(QUrl("qrc:/ui/Main.qml"));if(engine.rootObjects().isEmpty())return 3;
 auto *window=qobject_cast<QQuickWindow*>(engine.rootObjects().first());
 if(args.contains("--input-trace")){bool ok=false;int seconds=args.value(args.indexOf("--input-trace-seconds")+1).toInt(&ok);if(!args.contains("--input-trace-seconds")||!ok)seconds=60;if(!args.contains("--input-trace-pause-file")||!bridge.startInputTrace(args.value(args.indexOf("--input-trace")+1),args.value(args.indexOf("--input-trace-pause-file")+1),seconds)){std::cerr<<"Input trace requires a new absolute output path, an absolute private-phase pause-file path and duration 1..90 seconds.\n";return 45;}}
 if(!check&&(args.contains("--controller-live-cancel-check")||args.contains("--controller-live-noop-check")||args.contains("--controller-live-correction-check"))){auto cancel=args.contains("--controller-live-cancel-check");auto correction=args.contains("--controller-live-correction-check");auto id=args.value(args.indexOf(cancel?"--controller-live-cancel-check":correction?"--controller-live-correction-check":"--controller-live-noop-check")+1);auto tile=args.value(args.indexOf("--controller-test-tile")+1).toUInt();auto root=args.value(args.indexOf("--controller-test-root")+1);auto photo=args.contains("--controller-cancel-photo")?args.value(args.indexOf("--controller-cancel-photo")+1):QString();QTimer::singleShot(150,&app,[&,id,tile,root,photo,cancel,correction]{app.exit(candidate25LiveCancelCheck(window,bridge,id,tile,root,photo,cancel,correction,args.contains("--controller-test-factory-default")));});return app.exec();}
 if(check&&args.contains("--registration30-gui-check")){QTimer::singleShot(150,&app,[&]{app.exit(candidate30GuiChecks(window,bridge));});return app.exec();}
 if(check&&args.contains("--lifecycle19-check")){QTimer::singleShot(150,&app,[&]{app.exit(candidate19Checks(window,bridge));});return app.exec();}
 if(check&&args.contains("--classic22-resume-check")){QTimer::singleShot(150,&app,[&]{bool ok=window->property("classicMode")=="hardcore";std::cout<<(ok?"PASS":"FAIL")<<" cold process reloads guide preference\n";app.exit(ok?0:53);});return app.exec();}
 if(check&&args.contains("--curseforge24-check")){QTimer::singleShot(150,&app,[&]{app.exit(candidate24Checks(window,bridge));});return app.exec();}
 if(check&&args.contains("--proton23-check")){QTimer::singleShot(150,&app,[&]{app.exit(candidate23Checks(window,bridge));});return app.exec();}
 if(check&&args.contains("--classic22-check")){QTimer::singleShot(150,&app,[&]{app.exit(candidate22Checks(window,bridge));});return app.exec();}
 if(check&&args.contains("--editions21-check")){QTimer::singleShot(150,&app,[&]{app.exit(candidate21Checks(window,bridge));});return app.exec();}
 if(check&&args.contains("--repair45-check")){QTimer::singleShot(150,&app,[&]{app.exit(candidate45Checks(window,bridge,args.contains("--screenshot")?args.value(args.indexOf("--screenshot")+1):QString()));});return app.exec();}
 if(check&&args.contains("--folders44-check")){QTimer::singleShot(150,&app,[&]{app.exit(candidate44Checks(window,bridge));});return app.exec();}
 if(check&&args.contains("--single43-check")){QTimer::singleShot(150,&app,[&]{app.exit(candidate43Checks(window,bridge));});return app.exec();}
 if(check&&args.contains("--shared42-check")){QTimer::singleShot(150,&app,[&]{app.exit(candidate42Checks(window,bridge));});return app.exec();}
 if(check&&args.contains("--manual40-check")){QTimer::singleShot(150,&app,[&]{app.exit(candidate40Checks(window,bridge));});return app.exec();}
 if(check&&args.contains("--pointer20-check")){QTimer::singleShot(150,&app,[&]{app.exit(candidate20Checks(window,bridge));});return app.exec();}
 if(check&&args.contains("--handoff18-check")){QTimer::singleShot(150,&app,[&]{app.exit(candidate18Checks(window));});return app.exec();}
 if(check && args.contains("--workflow-check")){QTimer::singleShot(150,&app,[&]{app.exit(candidate17Checks(window,bridge));});return app.exec();}
 if(check && args.contains("--repair-check")){QTimer::singleShot(150,&app,[&]{app.exit(candidate16Checks(window,bridge,args.contains("--screenshot")?args.value(args.indexOf("--screenshot")+1):QString()));});return app.exec();}
 if(check && args.contains("--candidate12-check")){QTimer::singleShot(150,&app,[&]{app.exit(candidate12Checks(window,bridge,args.contains("--screenshot")?args.value(args.indexOf("--screenshot")+1):QString()));});return app.exec();}
 if(check && args.contains("--handoff-check")){QTimer::singleShot(150,&app,[&]{QVariant result;QMetaObject::invokeMethod(window,"checkAddonHandoff",Q_RETURN_ARG(QVariant,result));QTimer::singleShot(100,&app,[&,result]{QQuickItem *button=nullptr;std::function<void(QQuickItem*)> visit=[&](QQuickItem *item){if(item->objectName()=="copyAddonFolder")button=item;for(auto *child:item->childItems())visit(child);};visit(window->contentItem());bool ok=result.toBool()&&button&&QMetaObject::invokeMethod(button,"clicked")&&QGuiApplication::clipboard()->text()=="/fixture/Program Files (x86)/World of Warcraft";std::cout<<(ok?"PASS":"FAIL")<<" simulated GUI guided folder copy\n";app.exit(ok?0:37);});});return app.exec();}
 if(check && args.contains("--folder-check")){QTimer::singleShot(150,&app,[&]{app.exit(folderCheck(window,&engine,args.value(args.indexOf("--folder-check")+1),args.contains("--screenshot")?args.value(args.indexOf("--screenshot")+1):QString()));});return app.exec();}
 if(check && args.contains("--removal38-check")){QTimer::singleShot(150,&app,[&]{app.exit(candidate38Checks(window,bridge,args.contains("--screenshot")?args.value(args.indexOf("--screenshot")+1):QString()));});return app.exec();}
 if(check && args.contains("--launcher37-check")){QTimer::singleShot(150,&app,[&]{app.exit(candidate37Checks(window,bridge));});return app.exec();}
 if(check && args.contains("--selection-check")){QTimer::singleShot(150,&app,[&]{QVariant result;QMetaObject::invokeMethod(window,"checkSelectionSafety",Q_RETURN_ARG(QVariant,result));std::cout<<(result.toBool()?"PASS":"FAIL")<<" simulated GUI vanished-selection safety\n";app.exit(result.toBool()?0:36);});return app.exec();}
 if(check && args.contains("--addon-check")){QTimer::singleShot(150,&app,[&]{app.exit(addonCheck(window));});return app.exec();}
 if(check && args.contains("--controller25-gui-check")){QTimer::singleShot(150,&app,[&]{app.exit(candidate25GuiChecks(window,bridge));});return app.exec();}
 if(check && args.contains("--checkbox-check")){QTimer::singleShot(150,&app,[&]{app.exit(checkboxCheck(window,args.contains("--steam-input-check")));});return app.exec();}
 if(check){QVariant result;QMetaObject::invokeMethod(window,"checkGuidance",Q_RETURN_ARG(QVariant,result));if(!result.toBool()){std::cerr<<"Guided flow checks failed\n";return 10;}}
 if(check&&args.contains("--scenario"))QMetaObject::invokeMethod(window,"previewScenario",Q_ARG(QVariant,QVariant(args.value(args.indexOf("--scenario")+1))));
 if(check&&(args.contains("--test-photo")||args.contains("--test-question"))){
  window->setProperty("page","testing");
  auto *center=window->findChild<QObject*>("testCenter");
  center->setProperty(args.contains("--test-photo")?"photo":"asking",true);
  if(args.contains("--test-many"))QMetaObject::invokeMethod(center,"seedMany");
 }
 if(check&&args.contains("--navigation-check")){
  QTimer::singleShot(350,&app,[window,&app]{
   auto *first=window->findChild<QQuickItem*>("homeContinue");auto *second=window->findChild<QQuickItem*>("homeAdvanced");
   first->forceActiveFocus();QTest::keyClick(window,Qt::Key_Down);QCoreApplication::processEvents();
   if(window->activeFocusItem()!=second){std::cerr<<"Down navigation failed\n";app.exit(11);return;}
   QTest::keyClick(window,Qt::Key_Up);if(window->activeFocusItem()!=first){std::cerr<<"Up navigation failed\n";app.exit(12);return;}
   window->setProperty("page","help");
  });
  QTimer::singleShot(600,&app,[window,&app]{
   auto *home=window->findChild<QQuickItem*>("footerHome");auto *back=window->findChild<QQuickItem*>("backButton");
   back->forceActiveFocus();QTest::keyClick(window,Qt::Key_Right);
   if(window->activeFocusItem()!=home){std::cerr<<"Right navigation failed\n";app.exit(13);return;}
   QTest::keyClick(window,Qt::Key_Left);
   if(window->activeFocusItem()!=back){std::cerr<<"Left navigation failed\n";app.exit(14);return;}
  });
 }
 if(check&&args.contains("--controller-check")){
  bridge.setDirectInput(true);
  int device=SDL_JoystickAttachVirtual(SDL_JOYSTICK_TYPE_GAMECONTROLLER,SDL_CONTROLLER_AXIS_MAX,SDL_CONTROLLER_BUTTON_MAX,0);
  if(device<0){std::cerr<<"Virtual controller unavailable\n";return 15;}
  auto *joystick=SDL_JoystickOpen(device);
  QTimer::singleShot(0,&app,[window,&app,joystick,device,&bridge]{
   auto cleanup=[&]{SDL_JoystickClose(joystick);SDL_JoystickDetachVirtual(device);bridge.setDirectInput(false);};
   if(!joystick||!bridge.selectSelfCheckController(SDL_JoystickInstanceID(joystick))||!diagnosticReady(bridge,window,joystick,false)){cleanup();app.exit(33);return;}
   auto *first=window->findChild<QQuickItem*>("homeContinue");auto *second=window->findChild<QQuickItem*>("homeAdvanced");
   first->forceActiveFocus();
   SDL_JoystickSetVirtualButton(joystick,SDL_CONTROLLER_BUTTON_DPAD_DOWN,1);
   bool down=diagnosticWait("D-pad behavior",[&]{return window->activeFocusItem()==second;},1000,false);
   SDL_JoystickSetVirtualButton(joystick,SDL_CONTROLLER_BUTTON_DPAD_DOWN,0);
   if(!down){std::cerr<<"INPUT FAIL Controller D-pad failed\n";cleanup();app.exit(16);return;}
   if(!diagnosticWait("direction released",[&]{SDL_GameControllerUpdate();return SDL_JoystickGetButton(joystick,SDL_CONTROLLER_BUTTON_DPAD_DOWN)==0;},1000)){cleanup();app.exit(33);return;}
   SDL_JoystickSetVirtualAxis(joystick,SDL_CONTROLLER_AXIS_LEFTY,-25000);
   bool up=diagnosticWait("stick behavior",[&]{return window->activeFocusItem()==first;},1000,false);
   SDL_JoystickSetVirtualAxis(joystick,SDL_CONTROLLER_AXIS_LEFTY,0);cleanup();
   if(!up){std::cerr<<"INPUT FAIL Controller stick failed\n";app.exit(17);return;}
   std::cout<<"SIMULATED controller D-pad/stick PASS\n";app.exit(0);
  });
 }
 if(check && args.contains("--live-phase")){window->setProperty("page","live");auto *center=window->findChild<QObject*>("liveCenter");QMetaObject::invokeMethod(center,"seedPhase",Q_ARG(QVariant,QVariant(args.value(args.indexOf("--live-phase")+1))));}
 if(check && args.contains("--live-photo")){window->setProperty("page","live");auto *center=window->findChild<QObject*>("liveCenter");center->setProperty("photo",true);}
 if(check && args.contains("--resize-check"))QTimer::singleShot(400,&app,[window]{window->resize(800,600);});
 if(check && args.contains("--focus-check"))QTimer::singleShot(600,&app,[window]{for(int n=0;n<20;n++){QTest::keyClick(window,Qt::Key_Tab);QCoreApplication::processEvents();}});
 if(check && args.contains("--interaction-check")) {
  QTimer::singleShot(350,&app,[window]{window->setProperty("page","help");});
  QTimer::singleShot(500,&app,[window]{auto *button=window->findChild<QQuickItem*>("controllerTestButton");if(button)button->forceActiveFocus();});
  QTimer::singleShot(700,&app,[window]{QTest::keyClick(window,Qt::Key_Return);});
  QTimer::singleShot(800,&app,[window]{auto *button=window->findChild<QQuickItem*>("controllerTestButton");if(button)QTest::mouseClick(window,Qt::LeftButton,Qt::NoModifier,button->mapToScene(QPointF(button->width()/2,button->height()/2)).toPoint());});
  QTimer::singleShot(1000,&app,[window,&app]{if(window->property("testedInput").toInt()!=2){std::cerr<<"Keyboard selection failed\n";app.exit(6);return;}QTest::keyClick(window,Qt::Key_Escape);if(window->property("page").toString()!="home"){std::cerr<<"Back navigation failed\n";app.exit(7);}});
 }
 if(check&&!args.contains("--controller-check")){QTimer::singleShot(1200,&app,[&]{
  if(args.contains("--screenshot")){QString file=args.value(args.indexOf("--screenshot")+1);if(!window->grabWindow().save(file)){std::cerr<<"Screenshot failed\n";app.exit(4);return;}}
  if(args.contains("--test-photo")){
   auto *center=window->findChild<QObject*>("testCenter");QVariant fit;
   QMetaObject::invokeMethod(center,"photoFits",Q_RETURN_ARG(QVariant,fit));
   if(!fit.toBool()){std::cerr<<"Test photo content is clipped\n";app.exit(18);return;}
  }
  auto *back=window->findChild<QObject*>("backButton");auto *footer=window->findChild<QObject*>("footerBar");if(!back||!footer){app.exit(5);return;}auto *footerItem=qobject_cast<QQuickItem*>(footer);if(!footerItem || (footerItem->isVisible() && footerItem->mapToScene(QPointF(0,0)).y()+footerItem->height()>window->height()+1)){std::cerr<<"Footer is outside window\n";app.exit(8);return;}
  std::cout<<"{\"version\":\""<<wow_version()<<"\",\"qml_loaded\":true,\"footer_present\":true,\"demo\":true,\"hardware_tested\":false}\n";app.quit();});}
 return app.exec();
}

