#include "growth.h"
#include <Preferences.h>

static const uint32_t THRESHOLDS[5] = {0, 100, 400, 1200, 3000};

static Preferences prefs;
static uint32_t xp = 0;
static bool dirty = false;
static uint32_t lastSave = 0;

// Flash has a limited write budget, so XP is only committed once a minute
// (and immediately on level-up, which is rare).
static const uint32_t SAVE_INTERVAL_MS = 60UL * 1000UL;

void growth_begin() {
    prefs.begin("petxp", false);
    xp = prefs.getUInt("xp", 0);
}

uint8_t growth_level() {
    uint8_t lv = 1;
    for (uint8_t i = 0; i < 5; i++) {
        if (xp >= THRESHOLDS[i]) lv = i + 1;
    }
    return lv;
}

uint32_t growth_xp() { return xp; }

uint32_t growth_next() {
    for (uint8_t i = 0; i < 5; i++) {
        if (THRESHOLDS[i] > xp) return THRESHOLDS[i];
    }
    return 0; // max level
}

bool growth_add(uint32_t amount) {
    if (amount == 0) return false;
    uint8_t before = growth_level();
    xp += amount;
    dirty = true;
    if (growth_level() > before) {
        prefs.putUInt("xp", xp);
        dirty = false;
        return true;
    }
    return false;
}

void growth_reset() {
    xp = 0;
    prefs.putUInt("xp", 0);
    dirty = false;
}

void growth_flush(uint32_t now) {
    if (!dirty || now - lastSave < SAVE_INTERVAL_MS) return;
    lastSave = now;
    dirty = false;
    prefs.putUInt("xp", xp);
}
