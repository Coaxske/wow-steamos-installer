// Preview-only regression: real QML controls and Bridge, simulated input only.
#pragma once
#include <QSignalSpy>
#include <QTest>
#include <QKeyEvent>
#include <sys/prctl.h>
#include <sys/wait.h>
#include <unistd.h>
#include <signal.h>
#include <iostream>
#include <functional>
#include "ControllerFixture.h"
inline int checkboxCheck(QQuickWindow *window, bool steamMode) {
 int failures=0;
 auto verify=[&](bool ok,const char *name){std::cout<<(ok?"PASS ":"FAIL ")<<name<<std::endl;if(!ok)++failures;};
 window->setProperty("page","setup");window->requestActivate();if(!diagnosticWait("Qt window active",[&]{return QGuiApplication::applicationState()==Qt::ApplicationActive&&QGuiApplication::focusWindow()==window;}))return 33;
 std::function<QQuickItem*(QQuickItem*,QString)> find=[&](QQuickItem *root,QString text)->QQuickItem*{
  if(root->isVisible()&&root->property("text").toString()==text)return root;
  for(auto *child:root->childItems())if(auto *r=find(child,text))return r;
  return nullptr;
 };
 auto *era=find(window->contentItem(),"WoW Classic Era");
 auto *foreverBox=find(window->contentItem(),"World of Warcraft Forever");
 auto *other=find(window->contentItem(),"Show test and regional editions");
 QSignalSpy frame(window,SIGNAL(frameSwapped()));window->requestUpdate();
 if(!diagnosticWait("setup scene rendered",[&]{return frame.count()>0;}))return 33;
 if(!era||!foreverBox||!other){verify(false,"real setup checkboxes found");return 30;}
 QSignalSpy spy(era,SIGNAL(toggled()));
 verify(spy.isValid(),"toggled signal available");
 auto selected=[&]{return window->property("editions").toStringList().contains("era");};
 auto checkDelta=[&](int before,bool checked,const char *name){verify(spy.count()==before+1&&era->property("checked").toBool()!=checked&&selected()==era->property("checked").toBool(),name);};
 foreverBox->forceActiveFocus();QTest::keyClick(window,Qt::Key_Down);QTest::qWait(40);
 verify(window->activeFocusItem()==era,"direction navigation reaches edition checkbox");
 other->forceActiveFocus();bool tabFound=false;
 for(int i=0;i<35;i++){QTest::keyClick(window,Qt::Key_Backtab,Qt::ShiftModifier);if(window->activeFocusItem()==era){tabFound=true;break;}}
 verify(tabFound,"tab navigation reaches edition checkbox");
 auto key=[&](Qt::Key value,const char *name){era->forceActiveFocus();int before=spy.count();bool checked=era->property("checked").toBool();QTest::keyClick(window,value);QTest::qWait(30);checkDelta(before,checked,name);};
 key(Qt::Key_Space,"keyboard Space toggles exactly once");
 key(Qt::Key_Return,"keyboard Return / Steam mapped A toggles exactly once");
 key(Qt::Key_Enter,"keypad Enter toggles exactly once");
 // A held Return can generate auto-repeat events: only initial press selects.
 era->forceActiveFocus();int count=spy.count();bool state=era->property("checked").toBool();
 QTest::keyPress(window,Qt::Key_Return);
 for(int i=0;i<3;i++){QKeyEvent repeat(QEvent::KeyPress,Qt::Key_Return,Qt::NoModifier,QString(),true);QCoreApplication::sendEvent(window,&repeat);}
 QTest::keyRelease(window,Qt::Key_Return);checkDelta(count,state,"Return auto-repeat does not toggle repeatedly");
 auto mouse=[&](const char *name){int before=spy.count();bool checked=era->property("checked").toBool();QTest::mouseClick(window,Qt::LeftButton,Qt::NoModifier,era->mapToScene(QPointF(era->width()/2,era->height()/2)).toPoint());QTest::qWait(30);checkDelta(before,checked,name);};
 mouse("mouse toggles exactly once");
 Bridge input(false,nullptr,true); // Input-only instance: no engine requests or account/Steam files.
 verify(SDL_GetHintBoolean(SDL_HINT_JOYSTICK_HIDAPI_STEAMDECK,SDL_TRUE)==SDL_FALSE,"Deck HIDAPI trackpad takeover disabled");
 int device=SDL_JoystickAttachVirtual(SDL_JOYSTICK_TYPE_GAMECONTROLLER,SDL_CONTROLLER_AXIS_MAX,SDL_CONTROLLER_BUTTON_MAX,0);
 auto *joystick=device>=0?SDL_JoystickOpen(device):nullptr;
 if(!joystick){verify(false,"virtual controller attached");return 31;}
 {Bridge normalSelectionGuard(true);verify(!normalSelectionGuard.selectSelfCheckController(SDL_JoystickInstanceID(joystick)),"normal-mode instance override rejected");}
 if(!input.selectSelfCheckController(SDL_JoystickInstanceID(joystick))){SDL_JoystickClose(joystick);SDL_JoystickDetachVirtual(device);return 33;}
 auto a=[&]{SDL_JoystickSetVirtualButton(joystick,SDL_CONTROLLER_BUTTON_A,1);QTest::qWait(160);SDL_JoystickSetVirtualButton(joystick,SDL_CONTROLLER_BUTTON_A,0);QTest::qWait(70);};
 pid_t steam=-1;
 if(steamMode){
  steam=fork();if(steam==0){signal(SIGTERM,SIG_DFL);prctl(PR_SET_NAME,"steam",0,0,0);for(;;)pause();}
  verify(steam>0,"controlled Steam process-name fixture created");
  if(!diagnosticWait("Steam-running fixture detected",[&]{return input.selfCheckSteamKnown()&&input.selfCheckSteamPresent()&&input.selfCheckSelectedId()<0&&input.controllerStatus().contains("Steam desktop controls are active");})){if(steam>0){kill(steam,SIGTERM);waitpid(steam,nullptr,0);}SDL_JoystickClose(joystick);SDL_JoystickDetachVirtual(device);return 33;}
  verify(input.controllerStatus().contains("Steam desktop controls are active"),"production Steam-running guard active");
  verify(input.selfCheckSelectedId()<0,"Steam-running releases direct controller; no SDL injected into an unselected device");
  key(Qt::Key_Return,"Steam-running mapped Return toggles once");
  key(Qt::Key_Space,"Steam-running mapped Space toggles once");
  mouse("Steam-running mouse still works");
  kill(steam,SIGTERM);waitpid(steam,nullptr,0);steam=-1;
 }
 if(!diagnosticReady(input,window,joystick,true)){SDL_JoystickClose(joystick);SDL_JoystickDetachVirtual(device);return 33;}
 verify(!input.controllerStatus().contains("Steam desktop controls are active"),"Steam-closed direct input resumed");
 foreverBox->forceActiveFocus();SDL_JoystickSetVirtualButton(joystick,SDL_CONTROLLER_BUTTON_DPAD_DOWN,1);diagnosticWait("D-pad checkbox behavior",[&]{return window->activeFocusItem()==era;},1000,false);SDL_JoystickSetVirtualButton(joystick,SDL_CONTROLLER_BUTTON_DPAD_DOWN,0);QTest::qWait(60);
 verify(window->activeFocusItem()==era,"direct SDL D-pad reaches checkbox");
 era->forceActiveFocus();count=spy.count();state=era->property("checked").toBool();a();checkDelta(count,state,"direct SDL A held press toggles once");
 count=spy.count();state=era->property("checked").toBool();a();checkDelta(count,state,"second direct SDL A press toggles once");
 era->setEnabled(false);era->forceActiveFocus();count=spy.count();state=era->property("checked").toBool();
 a();for(auto k:{Qt::Key_Return,Qt::Key_Enter,Qt::Key_Space})QTest::keyClick(window,k);
 QTest::mouseClick(window,Qt::LeftButton,Qt::NoModifier,era->mapToScene(QPointF(era->width()/2,era->height()/2)).toPoint());
 verify(spy.count()==count&&era->property("checked").toBool()==state,"disabled checkbox rejects A keyboard and mouse");
 foreverBox->forceActiveFocus();QTest::keyClick(window,Qt::Key_Down);verify(window->activeFocusItem()!=era,"direction navigation skips disabled checkbox");
 era->setEnabled(true);mouse("mouse works after direct controller input");key(Qt::Key_Space,"keyboard Space works after controller input");
 SDL_JoystickClose(joystick);SDL_JoystickDetachVirtual(device);input.setDirectInput(false);
 SDL_version version;SDL_GetVersion(&version);std::cout<<"SDL runtime "<<int(version.major)<<"."<<int(version.minor)<<"."<<int(version.patch)<<" revision "<<SDL_GetRevision()<<std::endl;
 std::cout<<"SIMULATED_INPUT_ONLY failures="<<failures<<std::endl;return failures?32:0;
}
