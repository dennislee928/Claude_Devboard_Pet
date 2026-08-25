#pragma once
#include <Arduino.h>
#include "anim.h"

// Work-state machine, ported from pc/petd/src/state.rs. In the firmware
// edition the PC only forwards raw events; every decision below happens here.

// Event codes on the wire (must match pc/petd/src/board.rs::event_code).
enum PetEvent : uint8_t {
    EV_PROMPT = 1,
    EV_SESSION_START = 2,
    EV_TOOL_START = 3,   // arg = tool kind
    EV_TOOL_OK = 4,
    EV_TOOL_ERR = 5,
    EV_STOPPED = 6,
    EV_PERMISSION = 7,
    EV_PETTED = 8,
    EV_FEED = 9,
    EV_TOGGLE_SLEEP = 10,
    EV_SUBAGENT_DONE = 11,
    EV_FORCE_STATE = 12, // arg = state index
    EV_SET_CHAR = 20,
};

enum ToolKind : uint8_t { TK_EDIT = 0, TK_TEST, TK_BUILD, TK_SEARCH, TK_AGENT, TK_OTHER };

void machine_begin(uint32_t now);
// Applies an event; returns the XP it earned (interaction XP is rate limited).
uint32_t machine_event(uint8_t code, uint8_t arg, uint32_t now);
// Advances timers; returns true when the state changed.
bool machine_tick(uint32_t now);
void machine_celebrate(uint32_t now);
uint8_t machine_state();
bool machine_active();
