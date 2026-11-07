#pragma once
#include <QQmlProperty>
inline int candidate17Checks(QQuickWindow *window,Bridge &bridge){
 int failures=0;auto verify=[&](bool ok,const char*n){std::cout<<(ok?"PASS ":"FAIL ")<<n<<std::endl;if(!ok)++failures;};
 QVariant result;QMetaObject::invokeMethod(window,"seedC12Checks",Q_RETURN_ARG(QVariant,result));verify(result.toBool(),"public synthetic setup state");window->setProperty("page","home");window->requestActivate();if(!diagnosticWait("active workflow window",[&]{return QGuiApplication::focusWindow()==window&&QGuiApplication::applicationState()==Qt::ApplicationActive;}))return 33;QTest::qWait(100);
 auto *first=window->findChild<QQuickItem*>("homeContinue");auto *other=window->findChild<QQuickItem*>("homeAdvanced");auto *back=window->findChild<QQuickItem*>("backButton");auto *home=window->findChild<QQuickItem*>("footerHome");
 first->forceActiveFocus();QTest::keyClick(window,Qt::Key_Left);QTest::qWait(60);verify(window->activeFocusItem()==first,"Left on stacked main control cannot jump down into footer");
 window->setProperty("page","help");QTest::qWait(60);back->forceActiveFocus();QTest::keyClick(window,Qt::Key_Right);verify(window->activeFocusItem()==home,"Left/right within footer row preserved");
 window->setProperty("page","home");QTest::qWait(60);first->forceActiveFocus();QTest::mouseMove(window,other->mapToScene(QPointF(other->width()/2,other->height()/2)).toPoint());QTest::qWait(80);
 auto *background=qvariant_cast<QObject*>(other->property("background"));verify(other->property("hovered").toBool()&&background&&QQmlProperty(background,"border.color").read().value<QColor>()==QColor("#6aa8bd"),"mouse hover has distinct visible action-button border");verify(window->activeFocusItem()==first,"hover does not steal controller focus");
 for(auto op:{QString("recover"),QString("undo"),QString("abandon_cleanup")}){
  QMetaObject::invokeMethod(window,"openMaintenance",Q_ARG(QVariant,QVariant(op)));QTest::qWait(70);auto *dialog=window->findChild<QObject*>("steamMaintenanceDialog");auto *consent=window->findChild<QQuickItem*>("maintenanceConsent");auto *next=window->findChild<QQuickItem*>("maintenanceContinue");verify(dialog&&dialog->property("visible").toBool()&&!consent->property("checked").toBool()&&!next->isEnabled(),"saved-operation Steam consent shown BEFORE any shutdown");
  consent->forceActiveFocus();QTest::keyClick(window,Qt::Key_Space);verify(next->isEnabled(),"explicit Steam consent enables maintenance request");QTest::keyClick(window,Qt::Key_Escape);QTest::qWait(50);verify(!dialog->property("visible").toBool(),"B/Escape cancels maintenance without operation");
 }
 bridge.setSelfCheckResponder({});return failures?42:0;
}
