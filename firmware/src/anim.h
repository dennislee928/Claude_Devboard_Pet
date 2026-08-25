#pragma once
#include <Arduino.h>

// State indices match asset-gen STATE_NAMES order.
enum PetState : uint8_t {
    ST_IDLE = 0, ST_CODING, ST_THINKING, ST_SEARCHING, ST_TESTING, ST_BUILDING,
    ST_DEBUGGING, ST_ERROR, ST_SUCCESS, ST_WAITING, ST_NOTIFY, ST_CELEBRATING,
    ST_SLEEP, ST_COUNT
};

// chr: 0=clawd 1=beemo. level 1..5 (level 1 renders the egg regardless of chr).
void anim_set(uint8_t chr, uint8_t state, uint8_t level);
void anim_tick(uint32_t now);

int8_t anim_state_index(const char* name); // -1 if unknown
