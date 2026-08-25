#pragma once
#include <Arduino.h>

// Parsed message from the PC daemon:
//   {"s":"coding","c":"clawd","lv":3}
// Any field may be absent. `ping`-only messages are acked but not applied.
struct PetMsg {
    int8_t state;  // -1 = not present
    int8_t chr;    // -1 = not present, else 0/1
    int8_t level;  // -1 = not present, else 1..5
};

// Polls Serial. Returns true when a full message was parsed (ack is sent).
bool proto_poll(PetMsg& out);
