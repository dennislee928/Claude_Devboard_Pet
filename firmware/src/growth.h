#pragma once
#include <Arduino.h>

// XP + level engine, mirrored from pc/petd/src/growth.rs so the firmware
// edition grows the pet exactly like the standalone edition does.
// Levels: 1 Egg, 2 Baby, 3 Junior, 4 Senior, 5 Legend.

void growth_begin();                 // loads XP from NVS
bool growth_add(uint32_t amount);    // true when a level boundary was crossed
uint8_t growth_level();
uint32_t growth_xp();
uint32_t growth_next();              // XP needed for the next level, 0 at max
void growth_reset();
void growth_flush(uint32_t now);     // persist to NVS at most once a minute
