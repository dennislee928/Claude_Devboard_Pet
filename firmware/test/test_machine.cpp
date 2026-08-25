#include <Arduino.h>
#include <cassert>
#include "machine.h"
#include "growth.h"
SerialStub Serial;
uint32_t millis(){return 0;}
void delay(uint32_t){}
#define CHECK(c) do{ if(!(c)){ printf("FAIL %s:%d %s\n",__FILE__,__LINE__,#c); fails++; } }while(0)
int fails=0;
int main(){
  uint32_t t=100000;
  machine_begin(t);
  CHECK(machine_state()==ST_IDLE);
  CHECK(machine_event(EV_PROMPT,0,t)==5);
  CHECK(machine_state()==ST_THINKING);
  CHECK(machine_event(EV_TOOL_START,TK_EDIT,t)==1);
  CHECK(machine_state()==ST_CODING);
  machine_event(EV_TOOL_ERR,0,t);
  CHECK(machine_state()==ST_ERROR);
  machine_event(EV_TOOL_START,TK_EDIT,t+1000);
  CHECK(machine_state()==ST_DEBUGGING);           // error makes edits "debugging"
  CHECK(machine_event(EV_TOOL_OK,0,t+2000)==3); // recovery bonus only, same as the Rust engine
  machine_event(EV_STOPPED,0,t+3000);
  CHECK(machine_state()==ST_SUCCESS);
  CHECK(!machine_tick(t+8000));
  CHECK(machine_tick(t+14000)); CHECK(machine_state()==ST_IDLE);
  CHECK(machine_tick(t+200000)); CHECK(machine_state()==ST_SLEEP);
  machine_event(EV_SESSION_START,0,t+200000);
  CHECK(machine_state()==ST_NOTIFY);
  machine_tick(t+205000);
  CHECK(machine_state()==ST_THINKING);
  // interaction xp is rate limited exactly like the PC edition
  uint32_t a=machine_event(EV_PETTED,0,t+205000);
  uint32_t b=machine_event(EV_PETTED,0,t+206000);
  CHECK(a==1); CHECK(b==0);
  // growth thresholds
  growth_begin();
  CHECK(growth_level()==1);
  CHECK(!growth_add(99)); CHECK(growth_level()==1);
  CHECK(growth_add(1));   CHECK(growth_level()==2);
  CHECK(growth_next()==400);
  CHECK(growth_add(300)); CHECK(growth_level()==3);
  growth_add(2600);       CHECK(growth_level()==5); CHECK(growth_next()==0);
  growth_reset();         CHECK(growth_xp()==0);
  printf(fails? "%d FAILURES\n":"firmware logic: all checks passed\n", fails);
  return fails!=0;
}
