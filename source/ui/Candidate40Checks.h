#pragma once
inline int candidate40Checks(QQuickWindow *window,Bridge &bridge){
 int result=candidate20Checks(window,bridge);
 int failures=0;
 for(const char *name:{"configureNewPointerTiles","automaticPointerSetup","pointerReplace","applyControllerSetup"}){
  const bool absent=window->findChild<QObject*>(name)==nullptr;
  std::cout<<(absent?"PASS ":"FAIL ")<<"unavailable automatic pointer control absent: "<<name<<std::endl;
  if(!absent)++failures;
 }
 return result?result:(failures?67:0);
}
