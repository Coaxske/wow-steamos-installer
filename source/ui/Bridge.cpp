#include <QSettings>
#include "Bridge.h"
#include <QGuiApplication>
#include <QClipboard>
#include <QDesktopServices>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonArray>
#include <QKeyEvent>
#include <QWindow>
#include <QQuickWindow>
#include <QQuickItem>
#include <limits>
#include <cmath>
#include <functional>
#include <QUrl>
#include <QProcess>
#include <QDir>
#include <QFile>
#include <QProcessEnvironment>
#include <QtConcurrent>
extern "C" {char *wow_request(const char*);void wow_free(char*);void wow_cancel();void wow_reset_cancel();int wow_steam_state();
#if WOW_ENABLE_DEVELOPER_TOOLS
char *wow_steam_snapshot();
#endif
}
static int steamDesktopRunning();
Bridge::Bridge(bool demo,QObject *parent,bool selfCheck):QObject(parent),m_selfCheck(selfCheck),m_demo(demo){
 m_timer.setInterval(16);connect(&m_timer,&QTimer::timeout,this,&Bridge::pollInput);
 connect(&m_steamWorker,&QFutureWatcher<int>::finished,this,[this]{const int state=m_steamWorker.result();m_steamKnown=state>=0;m_steamPresent=state>0;m_steamError=state<0;m_probePending=false;
#if WOW_ENABLE_DEVELOPER_TOOLS
if(m_trace)m_trace->write("probe_applied",{{"sequence",int(m_probeSequence)},{"code",state}});
#endif
});
 qApp->installEventFilter(this);
 if(!demo)setDirectInput(true);
}
Bridge::~Bridge(){m_steamWorker.waitForFinished();wow_cancel();m_worker.waitForFinished();closeController();if(m_direct)SDL_QuitSubSystem(SDL_INIT_GAMECONTROLLER);}
static QVariantMap repairResponse(QVariantMap response,const QVariantMap &arguments){
 if(arguments.contains("_registration_generation"))response["registration_generation"]=arguments.value("_registration_generation");
 if(arguments.contains("_pointer_refresh"))response["pointer_refresh"]=arguments.value("_pointer_refresh");
 if(arguments.contains("_controller_generation"))response["controller_generation"]=arguments.value("_controller_generation");
 if(arguments.contains("_repair_generation"))response["repair_generation"]=arguments.value("_repair_generation");
 return response;
}
void Bridge::completeSelfCheckRequest(const QString &action,const QVariantMap &response,const QVariantMap &arguments){
 if(!m_selfCheck)return;
 m_busy=false;emit busyChanged();emit completed(action,repairResponse(response,arguments));
}
void Bridge::request(const QString &action,const QVariantMap &arguments){
 if(m_busy)return;
 if(m_selfCheck&&m_selfCheckResponder){auto response=m_selfCheckResponder(action,arguments);if(response.isEmpty()){m_busy=true;emit busyChanged();return;}emit completed(action,repairResponse(response,arguments));return;}
 // Preview mode never invokes the filesystem-capable Rust API.
 if(m_demo && action!="catalog" && action!="demo"){
  emit completed(action,{{"ok",false},{"error","Preview mode: no changes are made. Open normally for real setup."}});return;
 }
 auto args=arguments;args["action"]=action;auto bytes=QJsonDocument::fromVariant(args).toJson(QJsonDocument::Compact);
 m_busy=true;emit busyChanged();wow_reset_cancel();
 disconnect(&m_worker,nullptr,this,nullptr);
 connect(&m_worker,&QFutureWatcher<QByteArray>::finished,this,[this,action,arguments]{
  auto result=QJsonDocument::fromJson(m_worker.result()).object().toVariantMap();
  m_busy=false;emit busyChanged();emit completed(action,repairResponse(result,arguments));
 });
 m_worker.setFuture(QtConcurrent::run([bytes]{char *result=wow_request(bytes.constData());QByteArray reply(result);wow_free(result);return reply;}));
}
void Bridge::cancel(){wow_cancel();}
void Bridge::copyText(const QString &text){QGuiApplication::clipboard()->setText(text);}
static void openExternal(const QUrl &url){
 auto env=QProcessEnvironment::systemEnvironment();
 for(const auto &key: {QString("LD_LIBRARY_PATH"),QString("QT_PLUGIN_PATH"),QString("QML2_IMPORT_PATH"),QString("QML_IMPORT_PATH")}) {
  const auto saved="WOW_ORIGINAL_"+key;
  if(env.contains(saved)&&!env.value(saved).isEmpty())env.insert(key,env.value(saved));else env.remove(key);
  env.remove(saved);
 }
 QProcess process;process.setProcessEnvironment(env);process.setProgram("xdg-open");process.setArguments({url.toString()});process.startDetached();
}
void Bridge::openPublisher(const QString &publisher){
 const QMap<QString,QString> urls{{"battle","https://download.battle.net/en-us/desktop"},{"curseforge","https://www.curseforge.com/download/app"},{"wowup_cf","https://wowup.io/"},{"wowup_wago","https://wowup.io/"}};
 if(urls.contains(publisher))openExternal(QUrl(urls[publisher]));
}
void Bridge::openFolder(const QString &path){openExternal(QUrl::fromLocalFile(path));}
QString Bridge::localPath(const QUrl &url){return url.toLocalFile();}
bool Bridge::selectSelfCheckController(SDL_JoystickID id){
 if(!m_selfCheck||id<0)return false;
 closeController();m_active=false;m_waitNeutral=true;m_selfCheckId=id;return true;
}
void Bridge::closeController(){if(m_controller){
#if WOW_ENABLE_DEVELOPER_TOOLS
if(m_trace)m_trace->write("controller_close",{{"instance",int(selfCheckSelectedId())}});
#endif
SDL_GameControllerClose(m_controller);m_controller=nullptr;}m_axis=0;}
void Bridge::setDirectInput(bool value){if(m_direct==value)return;m_direct=value;
 if(value){SDL_SetHint(SDL_HINT_JOYSTICK_HIDAPI_STEAMDECK,"0");SDL_SetHint("SDL_JOYSTICK_HIDAPI_STEAM","1");SDL_SetHint("SDL_JOYSTICK_STEAM","1");if(SDL_InitSubSystem(SDL_INIT_GAMECONTROLLER)!=0){m_direct=false;m_status="Controller access unavailable. Use touchscreen, mouse, keyboard, or KDE Connect.";}else{m_timer.start();m_status="Controller navigation is on. You can also use the mouse.";}}
 else{m_timer.stop();closeController();SDL_QuitSubSystem(SDL_INIT_GAMECONTROLLER);m_status="Controller navigation is off. Mouse and Steam desktop controls still work.";}
 emit directInputChanged();emit controllerStatusChanged();
}
void Bridge::sendKey(int key){auto *window=QGuiApplication::focusWindow();if(!window)return;const auto modifiers=key==Qt::Key_Backtab?Qt::ShiftModifier:Qt::NoModifier;QKeyEvent down(QEvent::KeyPress,key,modifiers);QKeyEvent up(QEvent::KeyRelease,key,modifiers);QCoreApplication::sendEvent(window,&down);QCoreApplication::sendEvent(window,&up);}

bool Bridge::moveFocus(int key){
 auto *window=qobject_cast<QQuickWindow*>(QGuiApplication::focusWindow());
 if(!window)return false;
 auto *focus=window->activeFocusItem();
 // Text editing and open choice lists retain their normal arrow-key behavior.
 for(auto *p=focus;p;p=p->parentItem()) {
  const QByteArray type=p->metaObject()->className();
  if(type.contains("TextInput")||type.contains("TextEdit")||type.contains("ListView"))return false;
  auto *popup=qvariant_cast<QObject*>(p->property("popup"));
  if(popup&&popup->property("visible").toBool())return false;
 }
 QList<QQuickItem*> choices;
 std::function<void(QQuickItem*)> visit=[&](QQuickItem *item){
  if(!item->isVisible()||!item->isEnabled())return;
  if(item->activeFocusOnTab()&&item->width()>0&&item->height()>0)choices.append(item);
  for(auto *child:item->childItems())visit(child);
 };
 QQuickItem *scope=window->contentItem();
 for(auto *p=focus;p;p=p->parentItem()){
  if(QByteArray(p->metaObject()->className()).contains("PopupItem")){scope=p;break;}
  if(p->objectName()=="footerBar"&&(key==Qt::Key_Left||key==Qt::Key_Right)){scope=p;break;}
 }
 visit(scope);
 if(choices.isEmpty())return false;
 if(!focus){choices.first()->forceActiveFocus(Qt::TabFocusReason);return true;}
 const auto origin=focus->mapToScene(QPointF(focus->width()/2,focus->height()/2));
 QQuickItem *best=nullptr;double score=std::numeric_limits<double>::max();
 for(auto *item:choices){
  if(item==focus)continue;
  bool ancestor=false;for(auto *p=focus->parentItem();p;p=p->parentItem())if(p==item)ancestor=true;
  if(ancestor)continue;
  const auto center=item->mapToScene(QPointF(item->width()/2,item->height()/2));
  const double dx=center.x()-origin.x(),dy=center.y()-origin.y();
  const bool vertical=key==Qt::Key_Up||key==Qt::Key_Down;
  const double forward=(key==Qt::Key_Left?-dx:key==Qt::Key_Right?dx:key==Qt::Key_Up?-dy:dy);
  if(forward<2)continue;
  const double cross=std::abs(vertical?dx:dy);
  // Left/right should stay within the current row, never jump to a distant footer.
  if(!vertical&&cross>(item->height()+focus->height())/2+8)continue;
  const double overlap=(vertical?(item->width()+focus->width()):(item->height()+focus->height()))/2;
  const double candidate=forward+std::max(0.0,cross-overlap)*4+cross*0.15;
  if(candidate<score){score=candidate;best=item;}
 }
 if(best)best->forceActiveFocus(Qt::TabFocusReason);
 return true;
}
bool Bridge::eventFilter(QObject *object,QEvent *event){
 if(object==QGuiApplication::focusWindow()&&event->type()==QEvent::KeyRelease){
  auto *key=static_cast<QKeyEvent*>(event);
  if(m_comboReturnKey==key->key()){if(!key->isAutoRepeat())m_comboReturnKey=0;return true;}
 }
 if(event->type()==QEvent::KeyPress&&object==QGuiApplication::focusWindow()){
  auto *key=static_cast<QKeyEvent*>(event);
  // Standard ComboBox shortcut: never reinterpret Alt+Down as spatial navigation.
  if(key->modifiers()==Qt::AltModifier&&key->key()==Qt::Key_Down){
   auto *window=qobject_cast<QQuickWindow*>(QGuiApplication::focusWindow());
   for(auto *item=window?window->activeFocusItem():nullptr;item;item=item->parentItem()){
    if(QByteArray(item->metaObject()->className()).contains("ComboBox")&&item->isEnabled()){
     auto *popup=qvariant_cast<QObject*>(item->property("popup"));
     if(popup){if(!popup->property("visible").toBool()&&!key->isAutoRepeat())QMetaObject::invokeMethod(popup,"open");return true;}
    }
   }
  }
  if(key->modifiers()!=Qt::NoModifier)return QObject::eventFilter(object,event);
  if(key->key()==Qt::Key_Return||key->key()==Qt::Key_Enter){
   if(m_comboReturnKey==key->key())return true;
   auto *window=qobject_cast<QQuickWindow*>(QGuiApplication::focusWindow());
   for(auto *item=window?window->activeFocusItem():nullptr;item;item=item->parentItem()){
    if(QByteArray(item->metaObject()->className()).contains("ComboBox")&&item->isEnabled()){
     auto *popup=qvariant_cast<QObject*>(item->property("popup"));
     // Closed ComboBox accepts Space, but not Return/Steam-mapped A.
     // Leave popup selection to Qt; consume only the opening key's release.
     if(popup&&!popup->property("visible").toBool()){
      if(!key->isAutoRepeat()){m_comboReturnKey=key->key();sendKey(Qt::Key_Space);}
      return true;
     }
     break;
    }
   }
  }
  if(key->key()==Qt::Key_Up||key->key()==Qt::Key_Down||key->key()==Qt::Key_Left||key->key()==Qt::Key_Right)
   if(moveFocus(key->key()))return true;
 }
 return QObject::eventFilter(object,event);
}
static int steamDesktopRunning(){
 // The engine and input ownership must agree on UID scope and unknown states.
 return wow_steam_state();
}
void Bridge::pollInput(){
 // One input source at a time prevents Steam desktop emulation + SDL doubles.
 if(!m_demo){
  const auto now=SDL_GetTicks64();
  if(now>=m_steamCheck&&!m_probePending){
   // A finished future can still have an unapplied queued completion. Never replace it.
   m_probePending=true;++m_probeSequence;auto probe=m_selfCheck&&m_probe?m_probe:std::function<int()>(steamDesktopRunning);
#if WOW_ENABLE_DEVELOPER_TOOLS
   const auto sequence=m_probeSequence;auto trace=m_trace;
   if(trace)trace->write("probe_scheduled",{{"sequence",int(sequence)}});
   m_steamWorker.setFuture(QtConcurrent::run([trace,probe,sequence]{if(trace)trace->write("probe_started",{{"sequence",int(sequence)}});const int state=probe();if(trace)trace->write("probe_returned",{{"sequence",int(sequence)},{"code",state}});return state;}));
#else
   m_steamWorker.setFuture(QtConcurrent::run([probe]{return probe();}));
#endif
   m_steamCheck=now+1000;
  }
  if(!m_steamKnown||m_steamPresent){
   closeController();m_active=false;m_waitNeutral=true;
   const QString status=m_steamError?"Steam ownership could not be verified. Direct input is paused; mouse and keyboard remain available.":m_steamKnown?"Steam desktop controls are active. Direct input will resume after Steam closes.":"Checking Steam input ownership…";
   if(m_status!=status){m_status=status;emit controllerStatusChanged();}return;
  }
 }

 if(QGuiApplication::applicationState()!=Qt::ApplicationActive){
  if(m_active)closeController();m_active=false;m_waitNeutral=true;
  const QString status="Direct controller navigation is paused until this installer window is active.";if(m_status!=status){m_status=status;emit controllerStatusChanged();}return;
 }
 SDL_PumpEvents();
 if(m_controller&&!SDL_GameControllerGetAttached(m_controller))closeController();
 if(!m_controller){
  for(int n=0;n<SDL_NumJoysticks();n++)if(SDL_IsGameController(n)&&(!m_selfCheck||m_selfCheckId<0||SDL_JoystickGetDeviceInstanceID(n)==m_selfCheckId)){m_controller=SDL_GameControllerOpen(n);
#if WOW_ENABLE_DEVELOPER_TOOLS
if(m_trace)m_trace->write("controller_open",{{"slot",n},{"instance",SDL_JoystickGetDeviceInstanceID(n)},{"success",m_controller!=nullptr}});
#endif
if(m_controller)break;}
  if(!m_controller){
   const QString status="No accessible direct controller was detected. Keep Steam open for built-in controls; setup closes and reopens it automatically.";
   if(m_status!=status){m_status=status;emit controllerStatusChanged();}
   return;
  }
  auto *selected=SDL_GameControllerGetJoystick(m_controller);
  m_status=QString("Controller connected. Use D-pad, A select, B back; mouse stays available. [VID %1 PID %2 instance %3]").arg(SDL_JoystickGetVendor(selected),4,16,QChar('0')).arg(SDL_JoystickGetProduct(selected),4,16,QChar('0')).arg(SDL_JoystickInstanceID(selected));
  emit controllerStatusChanged();m_active=false;
 }
 if(!m_active){
  for(int b=0;b<SDL_CONTROLLER_BUTTON_MAX;b++)m_buttons[b]=SDL_GameControllerGetButton(m_controller,SDL_GameControllerButton(b));
  m_axis=0;m_waitNeutral=true;m_active=true;return;
 }
 for(int b=0;b<SDL_CONTROLLER_BUTTON_MAX;b++){
  auto now=SDL_GameControllerGetButton(m_controller,SDL_GameControllerButton(b));
  if(now&&!m_buttons[b]){
   if(b==SDL_CONTROLLER_BUTTON_A)sendKey(Qt::Key_Space);
   if(b==SDL_CONTROLLER_BUTTON_B)sendKey(Qt::Key_Escape);
  }
  m_buttons[b]=now;
 }
 ++m_controllerSamples;
 const int x=SDL_GameControllerGetAxis(m_controller,SDL_CONTROLLER_AXIS_LEFTX);
 const int y=SDL_GameControllerGetAxis(m_controller,SDL_CONTROLLER_AXIS_LEFTY);
 int direction=0;
 if(m_buttons[SDL_CONTROLLER_BUTTON_DPAD_UP])direction=Qt::Key_Up;
 else if(m_buttons[SDL_CONTROLLER_BUTTON_DPAD_DOWN])direction=Qt::Key_Down;
 else if(m_buttons[SDL_CONTROLLER_BUTTON_DPAD_LEFT])direction=Qt::Key_Left;
 else if(m_buttons[SDL_CONTROLLER_BUTTON_DPAD_RIGHT])direction=Qt::Key_Right;
 else if(std::max(std::abs(x),std::abs(y))>18000)
  direction=std::abs(x)>std::abs(y)?(x>0?Qt::Key_Right:Qt::Key_Left):(y>0?Qt::Key_Down:Qt::Key_Up);
 if(direction)++m_directionSamples;
 if(m_waitNeutral){if(!direction)m_waitNeutral=false;return;}
 const auto now=SDL_GetTicks64();
 if(direction&&(direction!=m_axis||now>=m_repeatAt)){
  ++m_navigationDispatches;sendKey(direction);m_repeatAt=now+(direction!=m_axis?280:80);
 }
 m_axis=direction;
}

// Qt 6.4 Basic FolderBreadcrumbBar leaves Rectangle.color at white even with
// a dark inherited text palette. Correct only this fallback dialog background.
bool Bridge::styleFolderDialog(QObject *object){
 auto *window=qobject_cast<QQuickWindow*>(object);if(!window)return false;
 bool styled=false;
 std::function<void(QQuickItem*)> visit=[&](QQuickItem *item){
  if(QString::fromLatin1(item->metaObject()->className()).contains("FolderBreadcrumbBar")){
   auto *background=qvariant_cast<QQuickItem*>(item->property("background"));
   if(background)styled=background->setProperty("color",QColor("#1a2638"))||styled;
  }
  for(auto *child:item->childItems())visit(child);
 };
 visit(window->contentItem());return styled;
}

#if WOW_ENABLE_DEVELOPER_TOOLS
bool Bridge::startInputTrace(const QString &path,const QString &pause,int seconds){
 if(m_trace||seconds<1||seconds>90||!QDir::isAbsolutePath(path)||!QDir::isAbsolutePath(pause))return false;
 auto trace=std::make_shared<InputTrace>(path,pause,seconds);if(!trace->valid())return false;m_trace=trace;
 trace->write("trace_started",{{"duration_seconds",seconds},{"deck_hidapi",QString::fromUtf8(SDL_GetHint(SDL_HINT_JOYSTICK_HIDAPI_STEAMDECK)?SDL_GetHint(SDL_HINT_JOYSTICK_HIDAPI_STEAMDECK):"")}});
 connect(&m_traceTimer,&QTimer::timeout,this,&Bridge::traceState);m_traceTimer.start(500);QTimer::singleShot(seconds*1000,this,[this]{m_traceTimer.stop();if(m_trace)m_trace->write("trace_finished");});traceState();return true;
}
void Bridge::traceState(){
 if(!m_trace)return;
 QJsonArray devices;if(m_direct){for(int n=0;n<SDL_NumJoysticks();++n)devices.append(QJsonObject{{"slot",n},{"instance",SDL_JoystickGetDeviceInstanceID(n)},{"vid",SDL_JoystickGetDeviceVendor(n)},{"pid",SDL_JoystickGetDeviceProduct(n)},{"eligible",SDL_IsGameController(n)==SDL_TRUE}});}
 char *raw=wow_steam_snapshot();const auto processes=QJsonDocument::fromJson(QByteArray(raw)).object();wow_free(raw);
 m_trace->write("state",{{"application_active",QGuiApplication::applicationState()==Qt::ApplicationActive},{"focus_window",QGuiApplication::focusWindow()!=nullptr},{"direct_enabled",m_direct},{"probe_pending",m_probePending},{"probe_sequence",int(m_probeSequence)},{"known",m_steamKnown},{"present",m_steamPresent},{"error",m_steamError},{"status",m_status},{"controller_instance",int(selfCheckSelectedId())},{"controller_attached",m_controller&&SDL_GameControllerGetAttached(m_controller)},{"active",m_active},{"wait_neutral",m_waitNeutral},{"controller_samples",int(m_controllerSamples)},{"direction_activity_samples",int(m_directionSamples)},{"navigation_dispatches",int(m_navigationDispatches)},{"devices",devices},{"process_snapshot",processes}});
}

#endif

// Presentation preference only: never forwarded to installation/Steam/launcher state.
QString Bridge::classicGuideMode(){
 const auto mode=QSettings().value("guides/classicMode", "era").toString();
 return QStringList{"era","hardcore","sod"}.contains(mode)?mode:QStringLiteral("era");
}
void Bridge::setClassicGuideMode(const QString &mode){
 if(QStringList{"era","hardcore","sod"}.contains(mode))QSettings().setValue("guides/classicMode",mode);
}
