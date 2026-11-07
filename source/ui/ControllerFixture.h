#pragma once
#include <QElapsedTimer>
#include <QTest>
#include <QGuiApplication>
#include <QQuickWindow>
#include <SDL.h>
#include <functional>
#include <iostream>
// All helpers are invoked only from --self-check paths.
inline bool diagnosticWait(const char *name,const std::function<bool()> &ready,int timeout=5000,bool prerequisite=true){
 QElapsedTimer elapsed;elapsed.start();
 while(!ready()&&elapsed.elapsed()<timeout)QTest::qWait(10);
 bool ok=ready();std::cout<<(prerequisite?(ok?"PREREQUISITE PASS ":"PREREQUISITE BLOCKED "):(ok?"INPUT PASS ":"INPUT FAIL "))<<name<<std::endl;return ok;
}
struct DiagnosticExtraController {
 int device=-1;bool initialized=false;
 explicit DiagnosticExtraController(bool enabled){if(enabled){
  SDL_SetHint(SDL_HINT_JOYSTICK_HIDAPI_STEAMDECK,"0");
  initialized=SDL_InitSubSystem(SDL_INIT_GAMECONTROLLER)==0;
  if(initialized)device=SDL_JoystickAttachVirtual(SDL_JOYSTICK_TYPE_GAMECONTROLLER,SDL_CONTROLLER_AXIS_MAX,SDL_CONTROLLER_BUTTON_MAX,0);
  std::cout<<"SIMULATED extra controller index="<<device<<std::endl;
 }}
 ~DiagnosticExtraController(){if(device>=0)SDL_JoystickDetachVirtual(device);if(initialized)SDL_QuitSubSystem(SDL_INIT_GAMECONTROLLER);}
};
inline bool diagnosticReady(Bridge &input,QQuickWindow *window,SDL_Joystick *joystick,bool productionProbe){
 window->requestActivate();
 if(!diagnosticWait("Qt window active",[&]{return QGuiApplication::applicationState()==Qt::ApplicationActive&&QGuiApplication::focusWindow()==window;}))return false;
 if(productionProbe&&!diagnosticWait("Steam probe completed and Steam closed",[&]{return input.selfCheckSteamKnown()&&!input.selfCheckSteamPresent();}))return false;
 const auto injected=SDL_JoystickInstanceID(joystick);
 if(!diagnosticWait("controller selected and neutral",[&]{return input.selfCheckSelectedId()==injected&&input.selfCheckReady();}))return false;
 std::cout<<"SIMULATED selected_instance="<<input.selfCheckSelectedId()<<" injected_instance="<<injected<<" MATCH="<<(input.selfCheckSelectedId()==injected)<<std::endl;
 return input.selfCheckSelectedId()==injected;
}
