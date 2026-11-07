# Candidate 3 SDL pin

Alpha10 embedded SDL reports SDL-release-2.30.0-0-g859844eae (Ubuntu 2.30.0+dfsg-1ubuntu3.1). Candidate2 used libsdl2-dev from Debian bookworm in the fallback build container and therefore bundled 2.26.5. Candidate3 restores upstream version 2.30.0 using https://www.libsdl.org/release/SDL2-2.30.0.tar.gz (official release https://github.com/libsdl-org/SDL/releases/tag/release-2.30.0).

Archive SHA256: 36e2e41557e0fa4a1519315c0f5958a87ccb27e25c51776beb6f1239526447b0. Download recorded over HTTPS; no separate publisher signature/checksum was supplied or verified. Unmodified archive is included here. build-sdl230.sh stages it only in a disposable build container. The build uses the staged /usr tree inside the container, not host libraries. Packaging indexes the upstream library to its own included license notice.

This retains the baseline upstream SDL version, not the original Ubuntu binary, patches or exact build configuration. Linux joystick, HIDAPI, virtual joystick and dynamic udev are enabled in generated SDL_config.h. Physical controller / Steam Input desktop mapping compatibility remains untested. No SDL version upgrade or host dependency installation is implied.
