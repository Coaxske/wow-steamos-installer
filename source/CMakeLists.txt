cmake_minimum_required(VERSION 3.21)
project(WoWSteamOS VERSION 0.2.0 LANGUAGES CXX)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)
set(CMAKE_SKIP_RPATH TRUE)
find_package(Qt6 6.4 REQUIRED COMPONENTS Core Gui Qml Quick QuickControls2 Concurrent Widgets Test DBus)
find_package(SDL2 REQUIRED CONFIG)
find_package(PkgConfig REQUIRED)
pkg_check_modules(CONTROLLER_DEPS REQUIRED IMPORTED_TARGET gstreamer-1.0 gstreamer-app-1.0 gstreamer-video-1.0 tesseract)
find_program(CARGO_EXECUTABLE cargo REQUIRED)
set(RUST_LIB ${CMAKE_CURRENT_SOURCE_DIR}/target/release/libwow_steamos_core.a)
file(GLOB_RECURSE RUST_SOURCES CONFIGURE_DEPENDS src/*.rs ui/* scripts/* packaging/* tests/* vendor/* .cargo/*)
add_custom_command(OUTPUT ${RUST_LIB}
  COMMAND ${CARGO_EXECUTABLE} build --release --locked --offline
  WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
  DEPENDS ${RUST_SOURCES} Cargo.toml Cargo.lock build.rs build_support.rs CMakeLists.txt LICENSE README.md FEATURES.md VALIDATION.md TEST-ON-DEVICE.md THIRD-PARTY.md
  COMMENT "Building new Rust core")
add_custom_target(rust-core DEPENDS ${RUST_LIB})
qt_add_executable(wow-steamos ui/main.cpp ui/Bridge.cpp ui/Bridge.h ui/ControllerAutomation.cpp ui/ControllerAutomation.h ui/ControllerFrameSafetyChecks.cpp)
qt_add_resources(wow-steamos "ui" PREFIX "/" FILES ui/Main.qml ui/ActionButton.qml ui/InstallerCheckBox.qml ui/InfoText.qml ui/Guidance.js ui/TestCenter.qml ui/LiveTests.qml)
add_dependencies(wow-steamos rust-core)
target_link_libraries(wow-steamos PRIVATE ${RUST_LIB} Qt6::Core Qt6::Gui Qt6::Qml Qt6::Quick Qt6::QuickControls2 Qt6::Concurrent Qt6::Widgets Qt6::Test Qt6::DBus PkgConfig::CONTROLLER_DEPS SDL2::SDL2 pthread dl)
install(TARGETS wow-steamos RUNTIME DESTINATION bin)


# Focused native geometry policy + actual generated compositor-script checks.
add_executable(controller-geometry-checks ui/ControllerGeometryChecks.cpp)
target_link_libraries(controller-geometry-checks PRIVATE Qt6::Core Qt6::Qml)
