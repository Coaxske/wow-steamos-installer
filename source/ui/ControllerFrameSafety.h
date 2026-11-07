#pragma once
#include <QImage>
#include <QColor>
#include <QStringList>
#include <QRegularExpression>
#include <QSet>
#include <QString>
#include <QtGlobal>
#include <gst/gst.h>
#include <cmath>
#include <limits>
#include <mutex>
#include <atomic>
#include <memory>
#include <QJsonObject>

namespace ControllerFrameSafety {
inline bool knownDisplayOn(const QString &output){
 static const QRegularExpression line(QStringLiteral("^dpms mode for screen (.+): (on|off)$"));
 QSet<QString> screens;
 for(const auto &raw:output.split('\n')){
  auto text=raw.trimmed();if(text.isEmpty())continue;auto m=line.match(text);
  auto screen=m.captured(1).trimmed();
  if(!m.hasMatch()||m.captured(2)!="on"||screen.isEmpty()||screen.contains(QRegularExpression(QStringLiteral("[\\s\\p{Cc}:]")))||screens.contains(screen))return false;
  screens.insert(screen);
 }
 return !screens.isEmpty();
}
inline bool frameReadable(const QImage &image){
 if(image.isNull()||image.width()<2||image.height()<2)return false;
 double sum=0,squares=0;int count=0,visible=0,low=255,high=0;
 const int dx=qMax(1,image.width()/64),dy=qMax(1,image.height()/64);
 for(int y=0;y<image.height();y+=dy)for(int x=0;x<image.width();x+=dx){
  auto p=image.pixelColor(x,y);int v=p.alpha()<16?0:(299*p.red()+587*p.green()+114*p.blue())/1000;
  if(v>=24)++visible;
  sum+=v;squares+=v*v;++count;low=qMin(low,v);high=qMax(high,v);
 }
 double mean=sum/count,variance=squares/count-mean*mean;
 return high-low>12&&variance>16&&visible>=qMax(2,count/200);
}
inline bool streamFailed(GstElement *pipeline){
 if(!pipeline)return true;auto *bus=gst_element_get_bus(pipeline);if(!bus)return true;
 auto *message=gst_bus_pop_filtered(bus,GstMessageType(GST_MESSAGE_ERROR|GST_MESSAGE_EOS));gst_object_unref(bus);
 if(!message)return false;gst_message_unref(message);return true;
}
inline bool knownTimeSegment(const GstSegment &s){return s.format==GST_FORMAT_TIME&&s.rate==1.0&&s.applied_rate==1.0;}
inline bool sameSegmentEpoch(const GstSegment &a,const GstSegment &b){return a.format==b.format&&a.flags==b.flags&&a.rate==b.rate&&a.applied_rate==b.applied_rate&&a.base==b.base&&a.offset==b.offset&&a.start==b.start&&a.stop==b.stop&&a.time==b.time;}
class CaptureEpoch {
 quintptr clock=0;quint64 base=GST_CLOCK_TIME_NONE,lastRunning=GST_CLOCK_TIME_NONE;GstSegment segment{};bool hasSegment=false,failed=false;
public:
 bool bindClock(quintptr identity,quint64 baseTime,quint64 running){
  if(failed||!identity||baseTime==GST_CLOCK_TIME_NONE||running==GST_CLOCK_TIME_NONE||(clock&&(clock!=identity||base!=baseTime))||(lastRunning!=GST_CLOCK_TIME_NONE&&running<lastRunning)){failed=true;return false;}
  clock=identity;base=baseTime;lastRunning=running;return true;
 }
 bool bindSegment(const GstSegment &value){if(failed||!clock||!knownTimeSegment(value)||(hasSegment&&!sameSegmentEpoch(segment,value))){failed=true;return false;}segment=value;hasSegment=true;return true;}
 bool segmentKnown()const{return hasSegment;}
};
// Runs before the appsink queue: duplicates cannot displace its last two
// distinct timestamps. Faults latch even when downstream would discard a buffer.
enum class BufferDecision { Keep,Duplicate,Reject };
class DistinctSourceBuffers {
 std::mutex mutex;std::atomic_int fault{0};bool stream=false,segmentKnown=false,seen=false;GstSegment segment{};quint64 last=GST_CLOCK_TIME_NONE,observed=GST_CLOCK_TIME_NONE,observedOffset=GST_CLOCK_TIME_NONE;guint previousFlags=0,observedFlags=0;
public:
 void stop(int reason=1)noexcept{int zero=0;fault.compare_exchange_strong(zero,reason);}
 bool healthy()const noexcept{return fault.load()==0;}
 QJsonObject diagnostic(){std::lock_guard<std::mutex> lock(mutex);return {{"fault",fault.load()},{"previous_pts",QString::number(last)},{"observed_pts",QString::number(observed)},{"offset",QString::number(observedOffset)},{"flags",int(observedFlags)},{"previous_flags",int(previousFlags)}};}
 void streamStart(){std::lock_guard<std::mutex> lock(mutex);if(stream||seen||segmentKnown)stop(7);stream=true;}
 void setSegment(const GstSegment *value){std::lock_guard<std::mutex> lock(mutex);if(!stream||!value||!knownTimeSegment(*value)){stop(8);return;}if(segmentKnown&&!sameSegmentEpoch(segment,*value)){stop(9);return;}segment=*value;segmentKnown=true;}
 BufferDecision observe(quint64 pts,guint flags=0,quint64 offset=GST_CLOCK_TIME_NONE){
  std::lock_guard<std::mutex> lock(mutex);if(!healthy())return BufferDecision::Reject;
  observed=pts;observedFlags=flags;observedOffset=offset;
  int reason=!stream||!segmentKnown?2:pts==GST_CLOCK_TIME_NONE?3:seen&&pts<last?4:(flags&(GST_BUFFER_FLAG_CORRUPTED|GST_BUFFER_FLAG_GAP|GST_BUFFER_FLAG_RESYNC|GST_BUFFER_FLAG_DECODE_ONLY))?5:seen&&(flags&GST_BUFFER_FLAG_DISCONT)?6:0;
  if(reason){stop(reason);return BufferDecision::Reject;}
  if(seen&&pts==last)return BufferDecision::Duplicate;
  seen=true;last=pts;previousFlags=flags;return BufferDecision::Keep;
 }
};
inline GstPadProbeReturn distinctSourceProbe(GstPad *,GstPadProbeInfo *info,gpointer data)noexcept{
 auto &gate=**static_cast<std::shared_ptr<DistinctSourceBuffers>*>(data);
 try{
  auto type=GST_PAD_PROBE_INFO_TYPE(info);
  if(type&GST_PAD_PROBE_TYPE_EVENT_DOWNSTREAM){
   auto *event=GST_PAD_PROBE_INFO_EVENT(info);
   switch(GST_EVENT_TYPE(event)){
    case GST_EVENT_STREAM_START:gate.streamStart();break;
    case GST_EVENT_SEGMENT:{const GstSegment *segment=nullptr;gst_event_parse_segment(event,&segment);gate.setSegment(segment);break;}
    case GST_EVENT_FLUSH_START:case GST_EVENT_FLUSH_STOP:gate.stop(10);break;
    case GST_EVENT_EOS:gate.stop(11);break;
    default:break;
   }
   return GST_PAD_PROBE_OK; // Lifecycle events must propagate for normal shutdown.
  }
  if(type&GST_PAD_PROBE_TYPE_BUFFER_LIST){gate.stop(12);return GST_PAD_PROBE_DROP;}
  if(type&GST_PAD_PROBE_TYPE_BUFFER){auto *buffer=GST_PAD_PROBE_INFO_BUFFER(info);if(!buffer){gate.stop(13);return GST_PAD_PROBE_DROP;}return gate.observe(GST_BUFFER_PTS(buffer),GST_BUFFER_FLAGS(buffer),GST_BUFFER_OFFSET(buffer))==BufferDecision::Keep?GST_PAD_PROBE_OK:GST_PAD_PROBE_DROP;}
  return GST_PAD_PROBE_OK;
 }catch(...){gate.stop(14);return (GST_PAD_PROBE_INFO_TYPE(info)&GST_PAD_PROBE_TYPE_EVENT_DOWNSTREAM)?GST_PAD_PROBE_OK:GST_PAD_PROBE_DROP;}
}
inline gulong installDistinctSourceProbe(GstPad *pad,const std::shared_ptr<DistinctSourceBuffers> &gate){
 auto *context=new std::shared_ptr<DistinctSourceBuffers>(gate);
 auto id=gst_pad_add_probe(pad,GstPadProbeType(GST_PAD_PROBE_TYPE_BUFFER|GST_PAD_PROBE_TYPE_BUFFER_LIST|GST_PAD_PROBE_TYPE_EVENT_DOWNSTREAM|GST_PAD_PROBE_TYPE_EVENT_FLUSH),distinctSourceProbe,context,[](gpointer data){delete static_cast<std::shared_ptr<DistinctSourceBuffers>*>(data);});
 if(!id)delete context;return id;
}
enum class SourceDecision { Eligible,BeforeBoundary,TooOld,Future,Invalid };
struct SourceWindow { SourceDecision decision=SourceDecision::Invalid;quint64 running=GST_CLOCK_TIME_NONE,age=0; };
inline SourceWindow sourceWindow(const GstSegment &segment,quint64 pts,quint64 now,quint64 boundary,quint64 published){
 if(!knownTimeSegment(segment)||pts==GST_CLOCK_TIME_NONE||now==GST_CLOCK_TIME_NONE)return {};
 auto running=gst_segment_to_running_time(&segment,GST_FORMAT_TIME,pts);if(running==GST_CLOCK_TIME_NONE)return {};
 if(running>now)return {running-now<=50*GST_MSECOND?SourceDecision::Future:SourceDecision::Invalid,running,0};
 auto age=now-running;if(age>GST_SECOND)return {SourceDecision::TooOld,running,age};
 if(running<=boundary||running<=published)return {SourceDecision::BeforeBoundary,running,age};
 return {SourceDecision::Eligible,running,age};
}
enum class SampleDecision { DiscardFirst,Accept,Wait,Reject };
class TimestampSequence {
 qint64 start,wait,lastReceipt=-1;quint64 lastPts=0;bool seen=false,failed=false;
public:
 explicit TimestampSequence(qint64 startedAtMs,qint64 maxWaitMs=5000):start(startedAtMs),wait(maxWaitMs){}
 SampleDecision observe(quint64 pts,qint64 receivedAtMs){
  if(failed||start<0||wait<=0||receivedAtMs<start||receivedAtMs-start>wait||receivedAtMs<lastReceipt||pts==std::numeric_limits<quint64>::max()||(seen&&pts<lastPts)){
   failed=true;return SampleDecision::Reject;
  }
  lastReceipt=receivedAtMs;if(seen&&pts==lastPts)return SampleDecision::Wait;lastPts=pts;if(!seen){seen=true;return SampleDecision::DiscardFirst;}
  return SampleDecision::Accept;
 }
};
// A pre-boundary sample may establish recent source progression, but its pixels
// never authorize input. Only a newer eligible post-boundary sample can publish.
class SourceSequence {
 TimestampSequence allSamples;quint64 reference=GST_CLOCK_TIME_NONE;bool failed=false;
public:
 explicit SourceSequence(qint64 startedAtMs):allSamples(startedAtMs){}
 SampleDecision observe(const SourceWindow &window,qint64 receivedAtMs){
  if(failed||window.decision==SourceDecision::Invalid||allSamples.observe(window.running,receivedAtMs)==SampleDecision::Reject){failed=true;return SampleDecision::Reject;}
  if(window.decision!=SourceDecision::Eligible&&window.decision!=SourceDecision::BeforeBoundary)return SampleDecision::Wait;
  const auto now=window.running+window.age;
  if(reference!=GST_CLOCK_TIME_NONE&&now-reference>GST_SECOND)reference=GST_CLOCK_TIME_NONE;
  if(reference==GST_CLOCK_TIME_NONE){reference=window.running;return SampleDecision::Wait;}
  if(window.running==reference)return SampleDecision::Wait;
  reference=window.running;
  return window.decision==SourceDecision::Eligible?SampleDecision::Accept:SampleDecision::Wait;
 }
};
class FrameLease {
 QImage pixels;quint64 sourcePts=0;bool hasPts=false; qint64 receipt=-1,maxAge;
public:
 explicit FrameLease(qint64 maxAgeMs=3000):maxAge(maxAgeMs){}
 void invalidate(){pixels={};receipt=-1;}
 bool publish(const QImage &image,quint64 pts,qint64 receivedAtMs){
  if(receivedAtMs<0||maxAge<=0||pts==std::numeric_limits<quint64>::max()||(hasPts&&pts<=sourcePts)||!frameReadable(image)){invalidate();return false;}
  pixels=image.copy();if(pixels.isNull()){invalidate();return false;}sourcePts=pts;hasPts=true;receipt=receivedAtMs;return true;
 }
 bool valid(qint64 nowMs)const{return !pixels.isNull()&&receipt>=0&&nowMs>=receipt&&nowMs-receipt<=maxAge;}
 QImage image(qint64 nowMs)const{return valid(nowMs)?pixels:QImage();}
 quint64 pts()const{return sourcePts;}
};
}
