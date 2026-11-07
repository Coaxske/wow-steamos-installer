#include "ControllerFrameSafety.h"
#include "ControllerAutomation.h"
#include <gst/app/gstappsrc.h>
#include <gst/app/gstappsink.h>
#include <QJsonDocument>
#include <QJsonObject>
#include <iostream>
#include <stdexcept>
#include <memory>
#include <QThread>
#include <QElapsedTimer>

int controllerSafety39Checks(){
 using namespace ControllerFrameSafety;int checks=0;
 auto require=[&](bool condition,const char *name){if(!condition)throw std::runtime_error(name);++checks;};
 try{
  ControllerPortal packagedBackend;
  require(knownDisplayOn("dpms mode for screen eDP-1: on\n"),"known internal on");
  require(knownDisplayOn("dpms mode for screen HDMI-A-1: on\ndpms mode for screen eDP-1: on\n"),"reported displays on");
  for(const auto &text:QStringList{"","on","dpms mode for screen eDP-1: off","dpms mode for screen eDP-1: unknown","dpms mode for screen   : on","dpms mode for screen a:b: on","dpms mode for screen a\tb: on","dpms mode for screen eDP-1: on\ndpms mode for screen HDMI-A-1: off","dpms mode for screen eDP-1: on\nunexpected output","dpms mode for screen eDP-1: on\ndpms mode for screen eDP-1: on"})require(!knownDisplayOn(text),"off or ambiguous display rejected");
  QImage readable(64,64,QImage::Format_RGB888);for(int y=0;y<64;++y)for(int x=0;x<64;++x)readable.setPixelColor(x,y,QColor(x*4,y*4,(x+y)*2));
  QImage black(64,64,QImage::Format_RGB888);black.fill(Qt::black);QImage flat=black;flat.fill(QColor(70,70,70));
  QImage nearBlack(64,64,QImage::Format_RGB888);for(int y=0;y<64;++y)for(int x=0;x<64;++x)nearBlack.setPixelColor(x,y,(x+y)%2?QColor(13,13,13):QColor(0,0,0));QImage transparent=readable.convertToFormat(QImage::Format_ARGB32);for(int y=0;y<64;++y)for(int x=0;x<64;++x){auto c=transparent.pixelColor(x,y);c.setAlpha(0);transparent.setPixelColor(x,y,c);}require(!frameReadable(nearBlack)&&!frameReadable(transparent),"near-black and transparent capture rejected");
  require(frameReadable(readable),"nonblack varied frame accepted");require(!frameReadable(black)&&!frameReadable(flat)&&!frameReadable(QImage()),"black flat missing rejected");
  TimestampSequence good(1000);require(good.observe(10,1001)==SampleDecision::DiscardFirst&&good.observe(11,1002)==SampleDecision::Accept,"source progression");
  for(auto pts:{quint64(9),std::numeric_limits<quint64>::max()}){TimestampSequence s(1000);s.observe(10,1001);require(s.observe(pts,1002)==SampleDecision::Reject&&s.observe(12,1003)==SampleDecision::Reject,"bad PTS latches refusal");}
  TimestampSequence missing(1000);require(missing.observe(std::numeric_limits<quint64>::max(),1001)==SampleDecision::Reject,"missing first timestamp");
  TimestampSequence late(1000);late.observe(10,1001);require(late.observe(11,6001)==SampleDecision::Reject,"late second source sample");
  TimestampSequence reversed(1000);reversed.observe(10,1002);require(reversed.observe(11,1001)==SampleDecision::Reject,"reversed receipt rejected");
  TimestampSequence repeat(1000);require(repeat.observe(10,1001)==SampleDecision::DiscardFirst&&repeat.observe(10,1002)==SampleDecision::Wait&&repeat.observe(11,1003)==SampleDecision::Accept,"duplicate waits without authorization");
  TimestampSequence deadline(1000);deadline.observe(10,1001);require(deadline.observe(10,5999)==SampleDecision::Wait&&deadline.observe(11,6001)==SampleDecision::Reject,"repeat cannot extend original deadline");
  GstSegment segment;gst_segment_init(&segment,GST_FORMAT_TIME);CaptureEpoch epoch;
  require(epoch.bindClock(1,100,0)&&epoch.bindSegment(segment)&&epoch.bindClock(1,100,1)&&epoch.bindSegment(segment),"stable capture epoch");
  GstSegment changedSegment=segment;changedSegment.base=1;require(!epoch.bindSegment(changedSegment)&&!epoch.bindClock(1,100,2),"segment change latches stop");
  for(int kind=0;kind<3;++kind){CaptureEpoch e;e.bindClock(1,100,10);e.bindSegment(segment);require(!e.bindClock(kind==0?2:1,kind==1?101:100,kind==2?9:11),"clock identity base or reversal stops");}
  require(sourceWindow(segment,137271636,300*GST_MSECOND,0,0).decision==SourceDecision::Eligible&&sourceWindow(segment,153976636,300*GST_MSECOND,0,0).decision==SourceDecision::Eligible,"retained recent probe pair eligible");
  require(sourceWindow(segment,137271636,2*GST_SECOND,0,0).decision==SourceDecision::TooOld,"new dequeue cannot freshen old source");
  require(sourceWindow(segment,137271636,300*GST_MSECOND,140*GST_MSECOND,0).decision==SourceDecision::BeforeBoundary,"pre-action sample excluded");
  require(sourceWindow(segment,153976636,300*GST_MSECOND,0,160*GST_MSECOND).decision==SourceDecision::BeforeBoundary,"published watermark excludes cached sample");
  require(sourceWindow(segment,320*GST_MSECOND,300*GST_MSECOND,0,0).decision==SourceDecision::Future&&sourceWindow(segment,320*GST_MSECOND,320*GST_MSECOND,0,0).decision==SourceDecision::Eligible,"future waits until real clock catches up");
  require(sourceWindow(segment,400*GST_MSECOND,300*GST_MSECOND,0,0).decision==SourceDecision::Invalid,"large future timestamp refused");
  changedSegment=segment;changedSegment.rate=2;require(sourceWindow(changedSegment,1,2,0,0).decision==SourceDecision::Invalid,"unknown segment rate refused");
  require(sourceWindow(segment,GST_CLOCK_TIME_NONE,2,0,0).decision==SourceDecision::Invalid,"missing source timestamp refused");
  const auto before=sourceWindow(segment,821993285,949895937,865906870,238539285);
  const auto after=sourceWindow(segment,905238285,949895937,865906870,238539285);
  SourceSequence causal(1000);require(causal.observe(before,1001)==SampleDecision::Wait&&causal.observe(after,1002)==SampleDecision::Accept,"recent pre-action reference permits only new post-action image");
  SourceSequence single(1000);require(single.observe(after,1001)==SampleDecision::Wait&&single.observe(after,1002)==SampleDecision::Wait,"single or repeated sample does not prove progression");
  SourceSequence oldOnly(1000);require(oldOnly.observe(before,1001)==SampleDecision::Wait&&oldOnly.observe(sourceWindow(segment,850000000,949895937,865906870,238539285),1002)==SampleDecision::Wait,"two pre-action images never publish");
  SourceSequence staleRef(1000);staleRef.observe(sourceWindow(segment,100*GST_MSECOND,150*GST_MSECOND,0,0),1001);require(staleRef.observe(sourceWindow(segment,4*GST_SECOND,4*GST_SECOND+10*GST_MSECOND,0,0),4900)==SampleDecision::Wait&&staleRef.observe(sourceWindow(segment,4*GST_SECOND+20*GST_MSECOND,4*GST_SECOND+30*GST_MSECOND,0,0),4920)==SampleDecision::Accept,"reference must still be recent at acceptance");
  for(auto excluded:{sourceWindow(segment,10*GST_MSECOND,2*GST_SECOND,0,0),sourceWindow(segment,320*GST_MSECOND,300*GST_MSECOND,0,0)}){SourceSequence seq(1000);require(seq.observe(excluded,1001)==SampleDecision::Wait&&seq.observe(sourceWindow(segment,400*GST_MSECOND,450*GST_MSECOND,0,0),1002)==SampleDecision::Wait,"stale or future sample cannot seed progression");}
  SourceSequence published(1000);published.observe(before,1001);require(published.observe(sourceWindow(segment,905238285,949895937,865906870,905238285),1002)==SampleDecision::Wait,"published watermark cannot authorize even with recent reference");
  SourceSequence causalReverse(1000);causalReverse.observe(after,1001);require(causalReverse.observe(before,1002)==SampleDecision::Reject&&causalReverse.observe(after,1003)==SampleDecision::Reject,"source reversal is fatal across exclusions");
  SourceSequence causalLate(1000);causalLate.observe(before,1001);require(causalLate.observe(after,6001)==SampleDecision::Reject,"causal pairing retains original deadline");
  SourceSequence causalMissing(1000);require(causalMissing.observe(sourceWindow(segment,GST_CLOCK_TIME_NONE,2,0,0),1001)==SampleDecision::Reject,"causal pairing rejects missing timestamp");
  DistinctSourceBuffers dedup;dedup.streamStart();dedup.setSegment(&segment);require(dedup.observe(10,GST_BUFFER_FLAG_DISCONT)==BufferDecision::Keep&&dedup.observe(10)==BufferDecision::Duplicate&&dedup.observe(11)==BufferDecision::Keep&&dedup.observe(11)==BufferDecision::Duplicate&&dedup.healthy(),"only consecutive duplicates excluded before queue");
  require(dedup.observe(10)==BufferDecision::Reject&&!dedup.healthy()&&dedup.observe(12)==BufferDecision::Reject,"reversal latches before queue can hide it");
  for(auto flags:{guint(GST_BUFFER_FLAG_CORRUPTED),guint(GST_BUFFER_FLAG_GAP),guint(GST_BUFFER_FLAG_RESYNC),guint(GST_BUFFER_FLAG_DECODE_ONLY),guint(GST_BUFFER_FLAG_DISCONT)}){DistinctSourceBuffers gate;gate.streamStart();gate.setSegment(&segment);gate.observe(10);require(gate.observe(10,flags)==BufferDecision::Reject&&!gate.healthy(),"semantic duplicate fault cannot hide as repeat");}
  auto faultRecord=dedup.diagnostic();require(faultRecord["fault"]==4&&faultRecord["previous_pts"]==QString("11")&&faultRecord["observed_pts"]==QString("10"),"first numeric fault survives later refused samples");
  DistinctSourceBuffers missingBuffer;missingBuffer.streamStart();missingBuffer.setSegment(&segment);require(missingBuffer.observe(GST_CLOCK_TIME_NONE)==BufferDecision::Reject&&!missingBuffer.healthy(),"missing source timestamp latches");
  DistinctSourceBuffers resetStream;resetStream.streamStart();resetStream.setSegment(&segment);resetStream.streamStart();require(!resetStream.healthy(),"stream restart latches");
  DistinctSourceBuffers resetSegment;resetSegment.streamStart();resetSegment.setSegment(&segment);changedSegment=segment;changedSegment.base=1;resetSegment.setSegment(&changedSegment);require(!resetSegment.healthy(),"changed source segment latches before queue");
  DistinctSourceBuffers earlyBuffer;require(earlyBuffer.observe(1)==BufferDecision::Reject&&!earlyBuffer.healthy(),"buffer without source segment refuses");
  FrameLease aged(5000);require(aged.publish(readable,153976636,853)&&aged.valid(5853)&&!aged.valid(5854),"source age remains in lease expiry");
  FrameLease lease(3000);require(lease.publish(readable,11,1002)&&lease.valid(1002)&&lease.valid(4002)&&!lease.valid(4003)&&!lease.valid(1001),"bounded monotonic lease");
  auto original=lease.image(1002).pixelColor(0,0);readable.fill(Qt::white);require(lease.image(1002).pixelColor(0,0)==original,"sample memory owned");
  QImage same=lease.image(1002);require(lease.publish(same,12,1100),"identical pixels independently fresh");
  FrameLease action=lease;lease.invalidate();require(!lease.valid(1101)&&action.valid(1101)&&!action.valid(4101),"transaction retains original expiry only");
  require(!lease.publish(same,12,1102)&&!lease.valid(1102),"invalidating cannot replay old PTS");
  require(!lease.publish(black,13,1103)&&!lease.valid(1103),"black invalidates prior lease");
  require(lease.publish(same,14,1104),"new independently verified capture can be published");
  GError *error=nullptr;GstElement *pipeline=gst_parse_launch("appsrc name=source is-live=true format=time caps=video/x-raw,format=RGB,width=8,height=8,framerate=30/1 ! appsink name=sink sync=false",&error);
  require(!error&&pipeline,"real GStreamer sample pipeline");
  std::unique_ptr<GstElement,void(*)(GstElement*)> pipelineOwner(pipeline,[](GstElement *p){gst_element_set_state(p,GST_STATE_NULL);gst_object_unref(p);});
  auto *source=GST_APP_SRC(gst_bin_get_by_name(GST_BIN(pipeline),"source"));auto *sink=GST_APP_SINK(gst_bin_get_by_name(GST_BIN(pipeline),"sink"));
  std::unique_ptr<GstAppSrc,decltype(&gst_object_unref)> sourceOwner(source,gst_object_unref);std::unique_ptr<GstAppSink,decltype(&gst_object_unref)> sinkOwner(sink,gst_object_unref);
  gst_element_set_state(pipeline,GST_STATE_PLAYING);TimestampSequence real(2000);
  for(int n=0;n<2;++n){auto *buffer=gst_buffer_new_allocate(nullptr,192,nullptr);GST_BUFFER_PTS(buffer)=(n+1)*GST_MSECOND;GST_BUFFER_DURATION(buffer)=GST_MSECOND;require(gst_app_src_push_buffer(source,buffer)==GST_FLOW_OK,"sample pushed");auto *sample=gst_app_sink_try_pull_sample(sink,GST_SECOND);require(sample,"sample delivered");auto decision=real.observe(GST_BUFFER_PTS(gst_sample_get_buffer(sample)),2001+n);gst_sample_unref(sample);require(decision==(n==0?SampleDecision::DiscardFirst:SampleDecision::Accept),"actual timestamp path");}
  require(!streamFailed(pipeline),"healthy stream accepted");
  gst_element_post_message(pipeline,gst_message_new_eos(GST_OBJECT(pipeline)));require(streamFailed(pipeline),"actual EOS bus message rejected");
  auto *failure=g_error_new_literal(g_quark_from_static_string("wow-safety-check"),1,"fixture failure");gst_element_post_message(pipeline,gst_message_new_error(GST_OBJECT(pipeline),failure,"fixture only"));g_error_free(failure);require(streamFailed(pipeline),"actual error bus message rejected");
  gst_element_set_state(pipeline,GST_STATE_NULL);
  GError *queueError=nullptr;auto *queued=gst_parse_launch("appsrc name=source is-live=true format=time caps=video/x-raw,format=RGB,width=8,height=8,framerate=60/1 ! appsink name=sink sync=false max-buffers=2 drop=true wait-on-eos=false",&queueError);require(!queueError&&queued,"bounded two-frame pipeline");
  std::unique_ptr<GstElement,void(*)(GstElement*)> queueOwner(queued,[](GstElement *p){gst_element_set_state(p,GST_STATE_NULL);gst_object_unref(p);});auto *qs=GST_APP_SRC(gst_bin_get_by_name(GST_BIN(queued),"source"));auto *qk=GST_APP_SINK(gst_bin_get_by_name(GST_BIN(queued),"sink"));std::unique_ptr<GstAppSrc,decltype(&gst_object_unref)> qsOwner(qs,gst_object_unref);std::unique_ptr<GstAppSink,decltype(&gst_object_unref)> qkOwner(qk,gst_object_unref);gst_element_set_state(queued,GST_STATE_PLAYING);
  const quint64 times[]={0,3953636,53939636,70597636,120609636,137271636,153976636};
  for(auto pts:times){auto *buffer=gst_buffer_new_allocate(nullptr,192,nullptr);GST_BUFFER_PTS(buffer)=pts;GST_BUFFER_DURATION(buffer)=GST_MSECOND;require(gst_app_src_push_buffer(qs,buffer)==GST_FLOW_OK,"bounded queue sample push");}
  QElapsedTimer renderedWait;renderedWait.start();guint64 rendered=0;
  do{GstStructure *stats=nullptr;g_object_get(qk,"stats",&stats,nullptr);if(stats){gst_structure_get_uint64(stats,"rendered",&rendered);gst_structure_free(stats);}if(rendered>=7)break;QThread::msleep(2);}while(renderedWait.elapsed()<2000);require(rendered>=7,"all source samples rendered before observation");
  TimestampSequence pair(1000);for(int i=0;i<2;++i){auto *sample=gst_app_sink_try_pull_sample(qk,GST_SECOND);require(sample,"recent queued sample retained without drain");auto pts=GST_BUFFER_PTS(gst_sample_get_buffer(sample));auto window=sourceWindow(*gst_sample_get_segment(sample),pts,300*GST_MSECOND,0,0);auto choice=pair.observe(window.running,1001+i);require(pts==times[5+i]&&window.decision==SourceDecision::Eligible&&choice==(i==0?SampleDecision::DiscardFirst:SampleDecision::Accept),"actual bounded queue retains recent progressing pair");gst_sample_unref(sample);}
  require(!gst_app_sink_try_pull_sample(qk,0),"no manufactured extra frame");
  for(auto pts:{quint64(821993285),quint64(905238285)}){auto *buffer=gst_buffer_new_allocate(nullptr,192,nullptr);GST_BUFFER_PTS(buffer)=pts;require(gst_app_src_push_buffer(qs,buffer)==GST_FLOW_OK,"causal queue sample push");}
  SourceSequence queuedCausal(1000);quint64 selected=GST_CLOCK_TIME_NONE;
  for(int i=0;i<2;++i){auto *sample=gst_app_sink_try_pull_sample(qk,GST_SECOND);require(sample,"causal source sample delivered");auto pts=GST_BUFFER_PTS(gst_sample_get_buffer(sample));auto choice=queuedCausal.observe(sourceWindow(*gst_sample_get_segment(sample),pts,949895937,865906870,238539285),1001+i);if(choice==SampleDecision::Accept)selected=pts;require(choice==(i==0?SampleDecision::Wait:SampleDecision::Accept),"real queue applies causal policy");gst_sample_unref(sample);}
  require(selected==905238285&&!gst_app_sink_try_pull_sample(qk,0),"only new post-release sample selected without extra static damage");

  for(int i=0;i<2;++i){auto *buffer=gst_buffer_new_allocate(nullptr,192,nullptr);GST_BUFFER_PTS(buffer)=(1000+i*16)*GST_MSECOND;require(gst_app_src_push_buffer(qs,buffer)==GST_FLOW_OK,"EOS queue sample push");}
  renderedWait.restart();do{GstStructure *stats=nullptr;g_object_get(qk,"stats",&stats,nullptr);if(stats){gst_structure_get_uint64(stats,"rendered",&rendered);gst_structure_free(stats);}if(rendered>=11)break;QThread::msleep(2);}while(renderedWait.elapsed()<2000);require(rendered>=11,"EOS test has queued samples");
  require(gst_app_src_end_of_stream(qs)==GST_FLOW_OK,"actual source EOS delivered");bool ended=false;renderedWait.restart();while(!ended&&renderedWait.elapsed()<1000){ended=streamFailed(queued);if(!ended)QThread::msleep(2);}require(ended,"actual EOS exposed without draining queued buffers");auto *retained=gst_app_sink_try_pull_sample(qk,0);require(retained,"EOS did not require consuming queued pixels");gst_sample_unref(retained);gst_element_set_state(queued,GST_STATE_NULL);
  GError *burstError=nullptr;auto *burst=gst_parse_launch("appsrc name=source is-live=true format=time caps=video/x-raw,format=RGB,width=8,height=8,framerate=60/1 ! appsink name=sink sync=false max-buffers=2 drop=true wait-on-eos=false",&burstError);require(!burstError&&burst,"duplicate burst pipeline");
  std::unique_ptr<GstElement,void(*)(GstElement*)> burstOwner(burst,[](GstElement *p){gst_element_set_state(p,GST_STATE_NULL);gst_object_unref(p);});auto *bs=GST_APP_SRC(gst_bin_get_by_name(GST_BIN(burst),"source"));auto *bk=GST_APP_SINK(gst_bin_get_by_name(GST_BIN(burst),"sink"));std::unique_ptr<GstAppSrc,decltype(&gst_object_unref)> bsOwner(bs,gst_object_unref);std::unique_ptr<GstAppSink,decltype(&gst_object_unref)> bkOwner(bk,gst_object_unref);auto *bp=gst_element_get_static_pad(GST_ELEMENT(bk),"sink");auto ingress=std::make_shared<DistinctSourceBuffers>();auto probeId=installDistinctSourceProbe(bp,ingress);require(probeId,"real source probe installed before playing");gst_element_set_state(burst,GST_STATE_PLAYING);
  const quint64 burstTimes[]={0,13949261,47274261,97283261,147262261,147262261,180588261,180588261};
  for(auto pts:burstTimes){auto *buffer=gst_buffer_new_allocate(nullptr,192,nullptr);GST_BUFFER_PTS(buffer)=pts;require(gst_app_src_push_buffer(bs,buffer)==GST_FLOW_OK,"source duplicate burst push");}
  renderedWait.restart();do{GstStructure *stats=nullptr;g_object_get(bk,"stats",&stats,nullptr);if(stats){gst_structure_get_uint64(stats,"rendered",&rendered);gst_structure_free(stats);}if(rendered>=6)break;QThread::msleep(2);}while(renderedWait.elapsed()<2000);require(rendered==6&&ingress->healthy(),"duplicate filtering leaves six actual distinct source renders");
  SourceSequence burstSequence(1000);for(int i=0;i<2;++i){auto *sample=gst_app_sink_try_pull_sample(bk,GST_SECOND);require(sample,"distinct queued sample delivered");auto pts=GST_BUFFER_PTS(gst_sample_get_buffer(sample));auto choice=burstSequence.observe(sourceWindow(*gst_sample_get_segment(sample),pts,352184869,0,0),1001+i);require(pts==(i==0?147262261:180588261)&&choice==(i==0?SampleDecision::Wait:SampleDecision::Accept),"last pair stays distinct without retimestamping");gst_sample_unref(sample);}
  require(!gst_app_sink_try_pull_sample(bk,0),"duplicate filter manufactures no samples");
  auto *bad=gst_buffer_new_allocate(nullptr,192,nullptr);GST_BUFFER_PTS(bad)=180588260;require(gst_app_src_push_buffer(bs,bad)==GST_FLOW_OK,"upstream reverse sample delivered");renderedWait.restart();while(ingress->healthy()&&renderedWait.elapsed()<1000)QThread::msleep(2);require(!ingress->healthy()&&!gst_app_sink_try_pull_sample(bk,0),"reverse fault latches before queue with no new pixels");
  require(gst_app_src_end_of_stream(bs)==GST_FLOW_OK,"filtered stream EOS delivered");ended=false;renderedWait.restart();while(!ended&&renderedWait.elapsed()<1000){ended=streamFailed(burst);if(!ended)QThread::msleep(2);}require(ended,"EOS propagates through guard for normal shutdown");gst_element_set_state(burst,GST_STATE_NULL);gst_pad_remove_probe(bp,probeId);gst_object_unref(bp);
  for(auto eventType:{GST_EVENT_FLUSH_START,GST_EVENT_FLUSH_STOP,GST_EVENT_EOS}){auto gate=std::make_shared<DistinctSourceBuffers>();auto *context=&gate;GstEvent *event=eventType==GST_EVENT_FLUSH_START?gst_event_new_flush_start():eventType==GST_EVENT_FLUSH_STOP?gst_event_new_flush_stop(TRUE):gst_event_new_eos();GstPadProbeInfo info{};info.type=GstPadProbeType(GST_PAD_PROBE_TYPE_EVENT_DOWNSTREAM|GST_PAD_PROBE_TYPE_EVENT_FLUSH);info.data=event;require(distinctSourceProbe(nullptr,&info,context)==GST_PAD_PROBE_OK&&!gate->healthy(),"lifecycle faults latch but events propagate");gst_event_unref(event);}
  auto listGate=std::make_shared<DistinctSourceBuffers>();auto *list=gst_buffer_list_new();GstPadProbeInfo listInfo{};listInfo.type=GST_PAD_PROBE_TYPE_BUFFER_LIST;listInfo.data=list;require(distinctSourceProbe(nullptr,&listInfo,&listGate)==GST_PAD_PROBE_DROP&&!listGate->healthy(),"unsupported buffer list latches");gst_buffer_list_unref(list);
  std::cout<<QJsonDocument(QJsonObject{{"checks",checks},{"result","PASS"},{"method","native policy and real synthetic GStreamer samples; no desktop, power or input calls"}}).toJson(QJsonDocument::Compact).constData()<<std::endl;return 0;
 }catch(const std::exception &e){std::cerr<<e.what()<<std::endl;return 1;}
}
