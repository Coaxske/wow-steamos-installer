#pragma once
#include <QObject>
#include <QVariantMap>
#include <QEventLoop>
#include <QImage>
#include <QJsonObject>
#include <QRect>
#include <functional>
#include <QDBusObjectPath>
#include <QDBusUnixFileDescriptor>
#include <QStringList>
#include <QDBusContext>
#include "CfTrayTypes.h"
#include "ControllerFrameSafety.h"
class ControllerPortal : public QObject {
 friend int controllerPermissionProbe();
 friend int controllerPortalFixtureCheck();
 friend int addonRegistrationClipboardCheck();
 friend int addonLifecycle35Check();
 Q_OBJECT
 Q_CLASSINFO("D-Bus Interface", "org.wowinstaller.ControllerMetadata")
public:
 explicit ControllerPortal(QObject *parent=nullptr);
 ~ControllerPortal();
 QVariantMap request(const QString &interface,const QString &method,const QVariantList &args,QVariantMap options);
 void start();
 QImage frame(const QRect &region);
 void click(const QPoint &global);
 void move(const QPoint &global);
 void escape();
 void keyChord(const QList<int>&keys);
 void typeText(const QString&text);
 bool clipboardRequested=false;
 bool addonOperation=false;
 std::function<void()> operationScope;
 quint64 clipboardRevision();
 QString readPublicClipboard(quint64 after);
 void close();
 void checkpoint();
 void invalidateFrames();
 void verifyVisible();
 void verifyCaptureStream();
 void requireInputFrame();
 quint64 captureRunningTime();
 void sceneChanged();
 bool hasFreshObservation();
 void verifyInputProgress();
 QString cancelFile;
 QString permissionFile;
 QJsonObject metadata(const QString &activate={},const QString &closeWindow={},const QJsonObject &prepare={});
private:
 void watchSession();
 QString session,requestPath,metadataText;
 uint node=0;
 QVariantMap response;
 bool received=false;
 bool revoked=false;
 bool clipboardEnabled=false,clipboardOwnerKnown=false,clipboardSessionOwner=false;
 quint64 clipboardSequence=0;
 QStringList clipboardMimeTypes;
 QSize logicalSize;
 ControllerFrameSafety::FrameLease frameLease{5000};
 ControllerFrameSafety::FrameLease actionFrameLease{5000};
 ControllerFrameSafety::CaptureEpoch captureEpoch;
 std::shared_ptr<ControllerFrameSafety::DistinctSourceBuffers> captureIngress=std::make_shared<ControllerFrameSafety::DistinctSourceBuffers>();
 GstPad *capturePad=nullptr;
 gulong captureProbe=0;
 GstClock *captureClock=nullptr;
 quint64 sceneBoundary=0;

 QEventLoop *waiting=nullptr;
 qint64 started=0;
 int pipefd=-1;
 void *pipeline=nullptr;
public slots:
 void portalResponse(uint code,const QVariantMap &data);
 void record(const QString &json);
 void sessionClosed(const QVariantMap &details);
 void clipboardOwnerChanged(const QDBusObjectPath&handle,const QVariantMap&options);
};
QByteArray runControllerDriver(const QString &jobPath);
int controllerDriverCheck();
class ControllerPortalFixture : public QObject {
 Q_OBJECT
 Q_CLASSINFO("D-Bus Interface", "org.freedesktop.portal.RemoteDesktop")
public:
 bool silent=false;
public slots:
 QDBusObjectPath CreateSession(const QVariantMap &options);
 QVariantMap StreamReply();
};
int controllerPortalFixtureCheck();

int controllerInputProbe();

int controllerPermissionProbe();

int addonRegistrationCheck();
int addonFolderObserve34();

class RegistrationClipboardFixture:public QObject {
 Q_OBJECT
 Q_CLASSINFO("D-Bus Interface","org.freedesktop.portal.Clipboard")
public:
 using QObject::QObject;
 int reads=0;
public slots:
 void RequestClipboard(const QDBusObjectPath&,const QVariantMap&){}
 QDBusUnixFileDescriptor SelectionRead(const QDBusObjectPath&,const QString&);
};
int addonRegistrationClipboardCheck();

class CfTrayWatcherFixture:public QObject {
 Q_OBJECT
 Q_CLASSINFO("D-Bus Interface","org.kde.StatusNotifierWatcher")
 Q_PROPERTY(QStringList RegisteredStatusNotifierItems READ items)
public: using QObject::QObject;QStringList entries;QStringList items()const{return entries;}
};
class CfTrayItemFixture:public QObject {
 Q_OBJECT
 Q_CLASSINFO("D-Bus Interface","org.kde.StatusNotifierItem")
 Q_PROPERTY(QString Id READ id)
 Q_PROPERTY(QDBusObjectPath Menu READ menu)
public: using QObject::QObject;QString ident="CurseForge_status_icon_1";QString menuPath="/fixture/menu";QString id()const{return ident;}QDBusObjectPath menu()const{return QDBusObjectPath(menuPath);}
};
class CfTrayMenuFixture:public QObject,protected QDBusContext {
 Q_OBJECT
 Q_CLASSINFO("D-Bus Interface","com.canonical.dbusmenu")
public: using QObject::QObject;CfMenuNode tree;uint revision=1;int reads=0,events=0;bool changeRevision=false,reject=false,noReply=false;
public slots:
 uint GetLayout(int,int,const QStringList&,CfMenuNode&root);
 void Event(int,const QString&,const QDBusVariant&,uint);
};
int addonExit33Check();

int addonLifecycle35Check();
