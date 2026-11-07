#pragma once
#include "ControllerFixture.h"
#include <QQmlComponent>
#include <QQmlEngine>
#include <QSignalSpy>
#include <QUrl>
#include <cmath>
#include <functional>
#include <QColor>

inline int folderCheck(QQuickWindow *window, QQmlEngine *engine, const QString &path, const QString &screenshot=QString()) {
 int failures=0;
 auto verify=[&](bool ok,const char *name){std::cout<<(ok?"PASS ":"FAIL ")<<name<<std::endl;if(!ok)++failures;};
 window->setProperty("page","setup");window->setProperty("setupAdvanced",true);
 QQmlComponent module(engine);module.setData("import Qt.labs.folderlistmodel\nFolderListModel {}",QUrl());
 auto *model=module.create();verify(model!=nullptr,"packaged Qt.labs.folderlistmodel loads");delete model;
 auto *browse=window->findChild<QObject*>("locationBrowse");
 auto *dialog=window->findChild<QObject*>("locationFolderDialog");
 auto *location=window->findChild<QObject*>("installLocation");
 if(!browse||!dialog||!location)return 34;
 verify(QMetaObject::invokeMethod(browse,"clicked"),"Browse signal invokes real handler");
 verify(diagnosticWait("folder dialog visible",[&]{return dialog->property("visible").toBool();}),"Browse opens real FolderDialog");
 QQuickItem *titleLabel=nullptr;
 std::function<void(QQuickItem*)> findTitle=[&](QQuickItem *item){if(item->objectName()=="dialogTitleBarLabel")titleLabel=item;for(auto *child:item->childItems())findTitle(child);};findTitle(window->contentItem());
 auto luminance=[](QColor c){auto linear=[](double x){return x<=0.04045?x/12.92:std::pow((x+0.055)/1.055,2.4);};return 0.2126*linear(c.redF())+0.7152*linear(c.greenF())+0.0722*linear(c.blueF());};
 QColor foreground,background;
 if(titleLabel){foreground=titleLabel->property("color").value<QColor>();for(auto *parent=titleLabel->parentItem();parent;parent=parent->parentItem()){auto *back=qvariant_cast<QQuickItem*>(parent->property("background"));if(back&&back->property("color").isValid()){background=back->property("color").value<QColor>();break;}}}
 double x=luminance(foreground),y=luminance(background);double ratio=(std::max(x,y)+0.05)/(std::min(x,y)+0.05);
 std::cout<<"Folder title colors "<<foreground.name().toStdString()<<" / "<<background.name().toStdString()<<" ratio "<<ratio<<std::endl;
 verify(titleLabel&&foreground.isValid()&&background.isValid()&&ratio>=4.5,"rendered FolderDialog title/background contrast >=4.5:1");
 auto breadcrumbReadable=[&]{
  QQuickItem *breadcrumb=nullptr;
  std::function<void(QQuickItem*)> findCrumb=[&](QQuickItem *item){if(QString::fromLatin1(item->metaObject()->className()).contains("FolderBreadcrumbBar"))breadcrumb=item;for(auto *child:item->childItems())findCrumb(child);};findCrumb(window->contentItem());
  QColor crumbBackground;if(breadcrumb){auto *back=qvariant_cast<QQuickItem*>(breadcrumb->property("background"));if(back)crumbBackground=back->property("color").value<QColor>();}
  int labels=0;bool readable=crumbBackground.isValid();
  std::function<void(QQuickItem*)> checkCrumb=[&](QQuickItem *item){
   auto *content=qvariant_cast<QQuickItem*>(item->property("contentItem"));
   if(!item->property("text").toString().isEmpty()&&content&&content->property("color").isValid()){
    auto color=content->property("color").value<QColor>();double a=luminance(color),b=luminance(crumbBackground);double contrast=(std::max(a,b)+0.05)/(std::min(a,b)+0.05);readable=readable&&color.isValid()&&contrast>=4.5;++labels;
   }
   for(auto *child:item->childItems())checkCrumb(child);
  };
  if(breadcrumb)checkCrumb(breadcrumb);
  return breadcrumb&&labels>0&&readable;
 };
 verify(diagnosticWait("breadcrumb rendered and contrast applied",breadcrumbReadable,2000,false),"rendered FolderDialog breadcrumb text contrast >=4.5:1");
 QTest::qWait(100);if(!screenshot.isEmpty())verify(window->grabWindow().save(screenshot),"render visible packaged folder dialog evidence");
 verify(dialog->setProperty("selectedFolder",QUrl::fromLocalFile(path)),"select fixture folder in real dialog");
 verify(QMetaObject::invokeMethod(dialog,"accept"),"accept real FolderDialog");
 verify(diagnosticWait("selected path returned",[&]{return location->property("text").toString()==path;}),"selected path returns to Install location");
 std::cout<<"PACKAGED RUNTIME / PROGRAMMATIC DIALOG CHECK; no desktop or physical-input certification\n";
 return failures?34:0;
}

inline int addonCheck(QQuickWindow *window) {
 int failures=0;
 auto verify=[&](bool ok,const char *name){std::cout<<(ok?"PASS ":"FAIL ")<<name<<std::endl;if(!ok)++failures;};
 window->setProperty("page","setup");window->requestActivate();
 if(!diagnosticWait("Qt active",[&]{return QGuiApplication::focusWindow()==window;}))return 33;
 auto *combo=window->findChild<QQuickItem*>("setupAddon");if(!combo)return 35;
 auto *popup=qvariant_cast<QObject*>(combo->property("popup"));if(!popup)return 35;
 Bridge input(true,nullptr,true);input.setDirectInput(true);
 int device=SDL_JoystickAttachVirtual(SDL_JOYSTICK_TYPE_GAMECONTROLLER,SDL_CONTROLLER_AXIS_MAX,SDL_CONTROLLER_BUTTON_MAX,0);
 auto *joystick=device>=0?SDL_JoystickOpen(device):nullptr;
 if(!joystick)return 33;
 input.selectSelfCheckController(SDL_JoystickInstanceID(joystick));
 if(!diagnosticReady(input,window,joystick,false))return 33;
 auto press=[&](SDL_GameControllerButton button){SDL_JoystickSetVirtualButton(joystick,button,1);QTest::qWait(90);SDL_JoystickSetVirtualButton(joystick,button,0);QTest::qWait(90);};
 combo->forceActiveFocus();
 verify(combo->isVisible()&&window->activeFocusItem()==combo,"addon dropdown remains focusable in shared-tile setup");
 combo->setProperty("currentIndex",0);combo->forceActiveFocus();press(SDL_CONTROLLER_BUTTON_A);
 verify(popup->property("visible").toBool(),"SDL A opens addon dropdown");
 press(SDL_CONTROLLER_BUTTON_DPAD_DOWN);press(SDL_CONTROLLER_BUTTON_A);
 verify(!popup->property("visible").toBool()&&window->property("wantedAddon")=="curseforge","SDL Down/A selects CurseForge and closes popup");
 combo->forceActiveFocus();QTest::keyClick(window,Qt::Key_Space);QTest::keyClick(window,Qt::Key_Down);QTest::keyClick(window,Qt::Key_Return);QTest::qWait(50);
 verify(window->property("wantedAddon")=="wowup_cf","keyboard Space/Down/Return selects next addon");
 combo->forceActiveFocus();QTest::keyClick(window,Qt::Key_Return);QTest::qWait(50);
 verify(popup->property("visible").toBool(),"Return / Steam-mapped A opens closed addon dropdown");
 if(popup->property("visible").toBool())QTest::keyClick(window,Qt::Key_Escape);
 combo->forceActiveFocus();QTest::keyClick(window,Qt::Key_Enter);QTest::qWait(50);
 verify(popup->property("visible").toBool(),"keypad Enter opens closed addon dropdown");
 if(popup->property("visible").toBool())QTest::keyClick(window,Qt::Key_Escape);
 combo->forceActiveFocus();press(SDL_CONTROLLER_BUTTON_A);press(SDL_CONTROLLER_BUTTON_DPAD_DOWN);press(SDL_CONTROLLER_BUTTON_B);
 verify(!popup->property("visible").toBool()&&window->property("wantedAddon")=="wowup_cf","SDL B cancels without changing selection");
 combo->forceActiveFocus();QTest::keyPress(window,Qt::Key_Return);
 for(int n=0;n<3;n++){QKeyEvent repeat(QEvent::KeyPress,Qt::Key_Return,Qt::NoModifier,QString(),true);QCoreApplication::sendEvent(window,&repeat);}
 QTest::keyRelease(window,Qt::Key_Return);
 verify(popup->property("visible").toBool()&&window->property("wantedAddon")=="wowup_cf","held Return opens once without committing a selection");QTest::keyClick(window,Qt::Key_Escape);
 QTest::mouseClick(window,Qt::LeftButton,Qt::NoModifier,combo->mapToScene(QPointF(combo->width()/2,combo->height()/2)).toPoint());QTest::qWait(50);
 verify(popup->property("visible").toBool(),"mouse still opens addon dropdown");QTest::keyClick(window,Qt::Key_Escape);
 auto *touch=QTest::createTouchDevice();auto point=combo->mapToScene(QPointF(combo->width()/2,combo->height()/2)).toPoint();
 QTest::touchEvent(window,touch).press(0,point,window);QTest::touchEvent(window,touch).release(0,point,window);QTest::qWait(50);
 verify(popup->property("visible").toBool(),"simulated touch still opens addon dropdown");QTest::keyClick(window,Qt::Key_Escape);
 combo->setEnabled(false);combo->forceActiveFocus();press(SDL_CONTROLLER_BUTTON_A);
 verify(!popup->property("visible").toBool(),"disabled dropdown does not open");combo->setEnabled(true);
 SDL_JoystickClose(joystick);SDL_JoystickDetachVirtual(device);input.setDirectInput(false);
 std::cout<<"SIMULATED SDL/keyboard only; physical controller, mouse/touch acceptance separate\n";
 return failures?35:0;
}
