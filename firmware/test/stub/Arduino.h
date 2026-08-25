#pragma once
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <cstdlib>
struct SerialStub {
  void begin(long){} void setRxBufferSize(int){}
  int available(){return 0;} int read(){return -1;}
  void println(const char*){} int printf(const char*, ...){return 0;}
};
extern SerialStub Serial;
uint32_t millis();
void delay(uint32_t);
#define PROGMEM
inline uint8_t pgm_read_byte(const uint8_t* p){return *p;}
