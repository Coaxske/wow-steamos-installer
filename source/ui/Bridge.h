#pragma once
#include <QObject>
#include <QVariantMap>
#include <QTimer>
#include <QFutureWatcher>
#include <SDL.h>
#include <functional>
#include <memory>
#if WOW_ENABLE_DEVELOPER_TOOLS
#include "InputTrace.h"
#endif
class Bridge : public QObject {
 Q_OBJECT
 Q_PROPERTY(bool busy READ busy NOTIFY busyChanged)
 Q_PROPERTY(bool updateBusy READ updateBusy NOTIFY updateBusyChanged)
 Q_PROPERTY(bool startupPending READ startupPending NOTIFY busyChanged)
 Q_PROPERTY(bool demo READ demo CONSTANT)
 Q_PROPERTY(bool directInput READ directInput WRITE setDirectInput NOTIFY directInputChanged)
 Q_PROPERTY(QString controllerStatus READ controllerStatus NOTIFY controllerStatusChanged)
public:
 explicit Bridge(bool demo, QObject *parent=nullptr, bool selfCheck=false);
 ~Bridge() override;
 bool busy()const{return m_busy||m_updateInstalling||m_startupPending;}
 bool startupPending()const{return m_startupPending;}
 void completeUpdateStartup(){if(m_startupPending){m_startupPending=false;emit busyChanged();}}
 bool updateBusy()const{return m_updateBusy;}
 bool demo()const{return m_demo;}
 bool directInput()const{return m_direct;}
 QString controllerStatus()const{return m_status;}
 void setDirectInput(bool value);
 void completeSelfCheckRequest(const QString &action,const QVariantMap &response,const QVariantMap &arguments);
 bool setSelfCheckResponder(std::function<QVariantMap(const QString&,const QVariantMap&)> responder){if(!m_selfCheck)return false;m_selfCheckResponder=std::move(responder);return true;}
 // C++ self-check API only; never exposed to QML or enabled in normal mode.
 bool selectSelfCheckController(SDL_JoystickID id);
 SDL_JoystickID selfCheckSelectedId()const{return m_controller?SDL_JoystickInstanceID(SDL_GameControllerGetJoystick(m_controller)):-1;}
 bool selfCheckReady()const{return m_selfCheck&&m_active&&!m_waitNeutral&&(m_demo||(m_steamKnown&&!m_steamPresent));}
 bool selfCheckSteamKnown()const{return m_steamKnown;}
 bool selfCheckSteamPresent()const{return m_steamPresent;}

#if WOW_ENABLE_DEVELOPER_TOOLS
 bool startInputTrace(const QString &path,const QString &pause,int seconds);
#endif
 bool selfCheckProbe(std::function<int()> probe){if(!m_selfCheck)return false;m_probe=std::move(probe);return true;}
 void selfCheckPollNow(){if(m_selfCheck){m_steamCheck=0;pollInput();}}
 bool selfCheckFutureRunning()const{return m_steamWorker.isRunning();}
 bool selfCheckProbePending()const{return m_probePending;}
 unsigned selfCheckProbeSequence()const{return m_probeSequence;}
 Q_INVOKABLE void request(const QString &action,const QVariantMap &arguments=QVariantMap());
 Q_INVOKABLE bool styleFolderDialog(QObject *window);
 Q_INVOKABLE void requestUpdate(const QString &action,const QVariantMap &arguments=QVariantMap());
 Q_INVOKABLE void openReleasePage(const QString &page=QString());
 Q_INVOKABLE bool cancel();
 Q_INVOKABLE QString classicGuideMode();
 Q_INVOKABLE void setClassicGuideMode(const QString &mode);
 Q_INVOKABLE void copyText(const QString &text);
 Q_INVOKABLE void openPublisher(const QString &publisher);
 Q_INVOKABLE void openFolder(const QString &path);
 Q_INVOKABLE QString localPath(const QUrl &url);
signals:
 void busyChanged();
 void updateBusyChanged();
 void updateCompleted(const QString &action,const QVariantMap &response);
 void directInputChanged();
 void controllerStatusChanged();
 void completed(const QString &action,const QVariantMap &response);
 void backRequested();
private:
 bool eventFilter(QObject *object,QEvent *event) override;
 bool moveFocus(int key);
 void pollInput();
 std::function<QVariantMap(const QString&,const QVariantMap&)> m_selfCheckResponder;
 bool m_selfCheck=false;
 SDL_JoystickID m_selfCheckId=-1;
 bool m_busy=false,m_demo=false,m_direct=false,m_active=false;
 QString m_status="Direct input is off. Steam desktop controls can be used.";
 QFutureWatcher<QByteArray> m_worker;
 QFutureWatcher<QByteArray> m_updateWorker;
 bool m_updateBusy=false,m_updateInstalling=false,m_startupPending=false;
 QFutureWatcher<int> m_steamWorker;
 bool m_steamKnown=false,m_steamError=false,m_probePending=false;
 unsigned m_probeSequence=0;
 unsigned m_controllerSamples=0,m_directionSamples=0,m_navigationDispatches=0;
 std::function<int()> m_probe;
#if WOW_ENABLE_DEVELOPER_TOOLS
 std::shared_ptr<InputTrace> m_trace;
 QTimer m_traceTimer;
 void traceState();
#endif
 QTimer m_timer;
 SDL_GameController *m_controller=nullptr;
 Uint8 m_buttons[SDL_CONTROLLER_BUTTON_MAX]{};
 int m_comboReturnKey=0;
 int m_axis=0;
 Uint64 m_repeatAt=0, m_steamCheck=0;
 bool m_steamPresent=false;
 bool m_waitNeutral=true;
 void closeController();
 void sendKey(int key);
};

