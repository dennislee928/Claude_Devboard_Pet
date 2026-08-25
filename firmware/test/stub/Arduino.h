#pragma once
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <cstdlib>
#include <cstdarg>
#include <string>
// Enough of Serial to drive proto.cpp from a host test: feed() queues bytes
// as if the PC had sent them, and everything written back is captured in out.
struct SerialStub {
  std::string in, out;
  size_t pos = 0;
  void begin(long){} void setRxBufferSize(int){}
  void feed(const std::string& s){ in += s; }
  int available(){ return (int)(in.size() - pos); }
  int read(){ return pos < in.size() ? (unsigned char)in[pos++] : -1; }
  void println(const char* s){ out += s; out += "\n"; }
  int printf(const char* fmt, ...){
    char buf[256];
    va_list ap; va_start(ap, fmt);
    int n = vsnprintf(buf, sizeof(buf), fmt, ap);
    va_end(ap);
    out += buf;
    return n;
  }
};
extern SerialStub Serial;
uint32_t millis();
void delay(uint32_t);
#define PROGMEM
inline uint8_t pgm_read_byte(const uint8_t* p){return *p;}
