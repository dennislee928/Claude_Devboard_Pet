#pragma once
#include <Arduino.h>

// Line protocol with the PC (see pc/petd/src/board.rs for the other side).
//
//   {"e":3,"t":1}                       raw event      -> firmware edition
//   {"e":20,"c":"grogu"}                set character
//   {"m":"Opus 5","a":"…","n":2,"tk":9,"pc":17}  agent status strip
//   {"s":"coding","c":"clawd","lv":3}   fully rendered state -> standalone edition
//
// Anything the board decides itself is broadcast back as
//   {"s":"coding","c":"grogu","lv":3,"xp":432,"nx":1200}

struct PetMsg {
    int8_t state;   // -1 = absent
    int8_t chr;     // -1 = absent, else 0..2
    int8_t level;   // -1 = absent, else 1..5
    int16_t event;  // -1 = absent, else a PetEvent code
    uint8_t arg;    // event argument (tool kind / state index)
    bool status;    // true when the line carried a Claude Code status strip
};

// Agent status as reported by the PC, drawn under the pet. `percent` is the
// tightest usage window across the watched providers (Claude Code, Codex), or
// -1 when nothing reports a limit.
struct PetStatus {
    char model[20];
    char action[32];
    uint16_t sessions;
    uint32_t tokens;
    int16_t percent;
    bool valid;
};

bool proto_poll(PetMsg& out);
const PetStatus& proto_status();

// Broadcast what the board decided, so a PC-side window can mirror it.
void proto_report(const char* state, const char* chr, uint8_t level, uint32_t xp, uint32_t next);
