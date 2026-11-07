#pragma once
#include <atomic>
#include <QThread>
#include <QTemporaryDir>
inline int candidate18Checks(QQuickWindow *window){
 int failures=0;auto verify=[&](bool ok,const char *name){std::cout<<(ok?"PASS ":"FAIL ")<<name<<std::endl;if(!ok)++failures;};
 QTemporaryDir traces;QString output=traces.path()+"/trace.jsonl",pause=traces.path()+"/private-phase";
 {InputTrace trace(output,pause,1);verify(trace.valid(),"opt-in trace creates new private metadata file");trace.write("before");QFile latch(pause);latch.open(QIODevice::WriteOnly);latch.close();trace.write("MUST_NOT_LOG");latch.remove();trace.write("after");}
 QFile log(output);log.open(QIODevice::ReadOnly);auto bytes=log.readAll();verify(bytes.contains("before")&&bytes.contains("after")&&!bytes.contains("MUST_NOT_LOG"),"private-phase latch drops records without replay");InputTrace collision(output,pause,1);verify(!collision.valid(),"trace refuses to overwrite existing evidence");verify(!(QFile::permissions(output)&(QFileDevice::ReadGroup|QFileDevice::ReadOther|QFileDevice::WriteGroup|QFileDevice::WriteOther)),"trace file has owner-only permissions");
 Bridge production(false,nullptr,true);std::atomic<int> calls{0},code{1};
 production.selfCheckProbe([&]{++calls;return code.load();});production.selfCheckPollNow();
 // Block only this fixture GUI thread until worker has returned, then poll before queued completion.
 QElapsedTimer deadline;deadline.start();while(production.selfCheckFutureRunning()&&deadline.elapsed()<2000)QThread::msleep(1);auto sequence=production.selfCheckProbeSequence();production.selfCheckPollNow();
 verify(!production.selfCheckFutureRunning(),"old isRunning-only guard would admit replacement before queued completion");
 verify(calls==1&&production.selfCheckProbeSequence()==sequence&&production.selfCheckProbePending(),"finished-but-unapplied ownership result cannot be replaced");
 verify(diagnosticWait("ownership completion",[&]{return production.selfCheckSteamKnown()&&production.selfCheckSteamPresent()&&!production.selfCheckProbePending();}),"queued result applied before next ownership probe");
 SDL_JoystickID device=SDL_JoystickAttachVirtual(SDL_JOYSTICK_TYPE_GAMECONTROLLER,SDL_CONTROLLER_AXIS_MAX,SDL_CONTROLLER_BUTTON_MAX,0);auto *joystick=SDL_JoystickOpen(device);if(!joystick)return 33;auto id=SDL_JoystickInstanceID(joystick);production.selectSelfCheckController(id);window->requestActivate();code=0;production.selfCheckPollNow();
 verify(diagnosticWait("closed handoff",[&]{return !production.selfCheckSteamPresent()&&production.selfCheckReady();}),"closed completion rediscovery opens matching fixture and reaches neutral readiness");verify(production.selfCheckSelectedId()==id,"opened instance matches injected fixture instance");
 code=-1;production.selfCheckPollNow();verify(diagnosticWait("uncertain ownership",[&]{return !production.selfCheckSteamKnown()&&production.selfCheckSelectedId()<0;}),"uncertain ownership closes direct device and does not assume Steam absent");
 code=0;production.selfCheckPollNow();verify(diagnosticWait("closed again",[&]{return production.selfCheckReady();}),"valid closed result recovers direct readiness");code=1;production.selfCheckPollNow();verify(diagnosticWait("reopened ownership",[&]{return production.selfCheckSteamPresent()&&production.selfCheckSelectedId()<0;}),"reopened Steam completion releases direct device");
 production.setDirectInput(false);SDL_JoystickClose(joystick);SDL_JoystickDetachVirtual(device);return failures?46:0;
}
