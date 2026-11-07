#pragma once
#include <QFile>
#include <QDateTime>
#include <QElapsedTimer>
#include <QJsonDocument>
#include <QJsonObject>
#include <QMutex>
#include <QMutexLocker>
// Opt-in metadata only. Exclusive private file; no window titles or input values.
class InputTrace {
 QFile file; QString pause; QElapsedTimer clock; QMutex mutex; qint64 limit;
public:
 InputTrace(const QString &path,const QString &pausePath,int seconds):file(path),pause(pausePath),limit(seconds*1000){clock.start();if(file.open(QIODevice::WriteOnly|QIODevice::NewOnly,QFileDevice::ReadOwner|QFileDevice::WriteOwner))file.setPermissions(QFileDevice::ReadOwner|QFileDevice::WriteOwner);}
 bool valid(){return file.isOpen();}
 void write(const QString &event,QJsonObject data={}){QMutexLocker lock(&mutex);if(!file.isOpen())return;if(clock.elapsed()>limit){file.close();return;}if(!pause.isEmpty()&&QFile::exists(pause))return;data["event"]=event;data["utc"]=QDateTime::currentDateTimeUtc().toString(Qt::ISODateWithMs);data["elapsed_ms"]=clock.elapsed();file.write(QJsonDocument(data).toJson(QJsonDocument::Compact)+"\n");file.flush();}
};
