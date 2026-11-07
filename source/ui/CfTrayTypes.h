#pragma once
#include <QDBusArgument>
#include <QDBusVariant>
#include <QVariantMap>
// Standard DBusMenu (ia{sv}av), requested properties only; no icons/tooltips.
struct CfMenuNode {int id=0;QVariantMap properties;QList<QDBusVariant> children;};
Q_DECLARE_METATYPE(CfMenuNode)
inline QDBusArgument&operator<<(QDBusArgument&a,const CfMenuNode&n){a.beginStructure();a<<n.id<<n.properties<<n.children;a.endStructure();return a;}
inline const QDBusArgument&operator>>(const QDBusArgument&a,CfMenuNode&n){a.beginStructure();a>>n.id>>n.properties>>n.children;a.endStructure();return a;}
