#include "Bridge.h"
#include <QApplication>
#include <QClipboard>
#include <QCoreApplication>
#include <QDir>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLockFile>
#include <QMessageBox>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickItem>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QStandardPaths>
#include <QTimer>
#include <iostream>
#include <memory>
extern "C" {
const char *wow_version();
const char *wow_source();
int wow_developer_tools();
char *wow_request(const char *);
void wow_free(char *);
}
#if WOW_ENABLE_DEVELOPER_TOOLS
int wowDeveloperMain(int argc, char **argv);
#endif
static int refuse(const char *message) {
    std::cout << QJsonDocument(QJsonObject{
        {"ok", false}, {"error", message}, {"results", QJsonArray{}}})
        .toJson(QJsonDocument::Compact).constData() << std::endl;
    return 1;
}
int main(int argc, char **argv) {
    QStringList args;
    for (int n = 1; n < argc; ++n) args.append(QString::fromUtf8(argv[n]));
    constexpr bool developerTools = WOW_ENABLE_DEVELOPER_TOOLS != 0;
    if ((wow_developer_tools() != 0) != developerTools)
        return refuse("The native application and Rust core build modes do not match.");
#if !WOW_ENABLE_DEVELOPER_TOOLS
    const QStringList allowed{"--build-info", "--engine-json", "--self-check", "--demo", "--help", "--version"};
    for (const auto &arg : args)
        if (!allowed.contains(arg))
            return refuse("Developer tools are not included in this public build. Automatic pointer configuration remains unavailable; use Gaming Mode pointer setup for manual instructions.");
#endif
    if (args.contains("--build-info")) {
        std::cout << QJsonDocument(QJsonObject{
            {"version", QString::fromUtf8(wow_version())},
            {"source_sha256", QString::fromUtf8(wow_source())},
            {"developer_tools", developerTools},
            {"build_mode", developerTools ? "developer" : "public"}
        }).toJson(QJsonDocument::Compact).constData() << std::endl;
        return 0;
    }
#if WOW_ENABLE_DEVELOPER_TOOLS
    return wowDeveloperMain(argc, argv);
#else
    if (args.contains("--help")) {
        std::cout << "Deckhand: open normally for setup; --demo previews without changes; --self-check checks the basic interface; --build-info prints build identity.\n";
        return 0;
    }
    if (args.contains("--version")) { std::cout << wow_version() << std::endl; return 0; }
    if (args.contains("--engine-json")) {
        if (args.size() != 1) return refuse("Use --engine-json without other options.");
        QCoreApplication core(argc, argv);
        std::string input((std::istreambuf_iterator<char>(std::cin)), {});
        char *raw = wow_request(input.c_str());
        const QByteArray reply(raw); wow_free(raw);
        std::cout << reply.constData() << std::endl;
        return reply.contains("\"ok\":false") ? 1 : 0;
    }
    QQuickWindow::setSceneGraphBackend("software");
    QApplication app(argc, argv);
    app.setApplicationName("Deckhand");
    app.setOrganizationName("WoWSteamOSRust");
    app.setDesktopFileName("org.wowsteamos.Installer");
    app.setApplicationVersion(QString::fromUtf8(wow_version()));
    const bool check = args.contains("--self-check");
    const bool demo = args.contains("--demo") || check;
    QQuickStyle::setStyle("Basic");
    const QString path = QStandardPaths::writableLocation(QStandardPaths::GenericDataLocation) + "/wow-steamos-rust";
    std::unique_ptr<QLockFile> lock;
    if (!demo) {
        if (!QDir().mkpath(path)) {
            QMessageBox::critical(nullptr, "Cannot start setup", "The local setup data folder could not be created."); return 2;
        }
        lock = std::make_unique<QLockFile>(path + "/window.lock");
        if (!lock->tryLock()) {
            QMessageBox::information(nullptr, "Setup already open", "Another setup window is open. Return to that window."); return 2;
        }
    }
    Bridge bridge(demo, nullptr, check);
    QQmlApplicationEngine engine;
    engine.rootContext()->setContextProperty("backend", &bridge);
    QObject::connect(&engine, &QQmlApplicationEngine::objectCreated, &app,
        [&](QObject *obj, const QUrl &) {
            if (!obj) { std::cerr << "The QML interface could not load.\n"; QCoreApplication::exit(3); }
        }, Qt::QueuedConnection);
    engine.load(QUrl("qrc:/ui/Main.qml"));
    if (engine.rootObjects().isEmpty()) return 3;
    auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
    if (!window) return 3;
    if (!demo && qEnvironmentVariableIsSet("WOW_UPDATE_STARTUP_ACK")) {
        // A replacement is ready only after the normal window lock and QML
        // window have loaded. Rust validates the wrapper's private capability.
        QTimer::singleShot(0, &app, [window, &app, &bridge] {
            if (!window->isVisible()) { app.exit(4); return; }
            char *raw = wow_request("{\"action\":\"updater_ready\"}");
            const QByteArray response(raw); wow_free(raw);
            if (!QJsonDocument::fromJson(response).object().value("ok").toBool()) {
                std::cerr << "The update startup acknowledgment could not be verified.\n";
                app.exit(4); return;
            }
            bridge.completeUpdateStartup();
            QMetaObject::invokeMethod(window, "beginNormalStartup");
        });
    }
    if (check) {
        QVariant guidance;
        if (!QMetaObject::invokeMethod(window, "checkGuidance", Q_RETURN_ARG(QVariant, guidance)) || !guidance.toBool()) {
            std::cerr << "Guided flow checks failed\n"; return 10;
        }
        QTimer::singleShot(1200, &app, [window, &app, &bridge] {
            QVariant signInHint;
            if (!QMetaObject::invokeMethod(window, "checkFirstSignInHint", Q_RETURN_ARG(QVariant, signInHint)) || !signInHint.toBool()) {
                std::cerr << "First-sign-in hint screen check failed\n"; app.exit(11); return;
            }
            if(bridge.publisherPage("curseforge")!="https://www.curseforge.com/download/app" || bridge.publisherPage("gearlever")!="https://flathub.org/apps/it.mijorus.gearlever" || bridge.publisherPage("issues")!="https://github.com/Coaxske/wow-steamos-installer/issues" || !bridge.publisherPage("https://unrelated.invalid/app").isEmpty() || !bridge.publisherPage("curseforge?download=1").isEmpty()) {
                std::cerr << "Publisher page boundary check failed\n"; app.exit(13); return;
            }
            QVariant reportGuide;
            if (!QMetaObject::invokeMethod(window, "checkReportAndGuide", Q_RETURN_ARG(QVariant, reportGuide)) || !reportGuide.toMap().value("ok").toBool() || QGuiApplication::clipboard()->text() != reportGuide.toMap().value("copied").toString()) {
                std::cerr << "Problem report or optional guide screen check failed\n"; app.exit(12); return;
            }
            auto *back = window->findChild<QObject *>("backButton");
            auto *footer = window->findChild<QQuickItem *>("footerBar");
            if (!back || !footer) { app.exit(5); return; }
            if (footer->isVisible() && footer->mapToScene(QPointF(0,0)).y() + footer->height() > window->height()+1) {
                std::cerr << "Footer is outside window\n"; app.exit(8); return;
            }
            std::cout << QJsonDocument(QJsonObject{
                {"version", QString::fromUtf8(wow_version())}, {"application_name", QCoreApplication::applicationName()}, {"window_title", window->title()}, {"qml_loaded", true},
                {"footer_present", true}, {"first_sign_in_hint", true}, {"problem_report_preview_copy", true}, {"optional_guide_default_off_and_skip", true}, {"publisher_links_allowlist", true}, {"classic_guide_mode", window->property("classicMode").toString()}, {"demo", true}, {"hardware_tested", false}
            }).toJson(QJsonDocument::Compact).constData() << std::endl;
            app.quit();
        });
    }
    return app.exec();
#endif
}
