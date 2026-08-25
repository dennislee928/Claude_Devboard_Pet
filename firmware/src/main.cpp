// Desk-pet firmware: ESP-WROOM-32 DevKit V1 + ST7789 240x240.
//
// This firmware runs in one of two modes, decided by what the PC sends:
//
//   * **firmware edition** (default) — the board is the brain. The PC only
//     forwards raw Claude Code events ({"e":..}); the state machine, the XP /
//     growth engine and the pet's memory all live here, and the board
//     broadcasts its decisions back so a PC window can mirror them.
//   * **standalone edition** — the PC already decided everything and sends
//     {"s":..,"c":..,"lv":..}; the board just draws it.
//
// Unplugged from any PC the board keeps animating on its own, with the pet it
// last grew (character, level and XP survive in NVS).

#include <Arduino.h>
#include <Preferences.h>
#include "display.h"
#include "anim.h"
#include "proto.h"
#include "machine.h"
#include "growth.h"

static Preferences prefs;
static uint8_t chr = 0;
static uint8_t pcLevel = 1, pcState = ST_IDLE;
static bool brain = true;  // false once a PC drives us in standalone edition
static uint32_t lastMsgAt = 0;
static uint32_t lastMinute = 0;
static uint32_t lastReport = 0;

// With no daemon traffic the pet dozes off on its own.
static const uint32_t AUTOSLEEP_MS = 8UL * 60UL * 1000UL;
static const uint32_t REPORT_MS = 2000;

static uint8_t current_state() { return brain ? machine_state() : pcState; }
static uint8_t current_level() { return brain ? growth_level() : pcLevel; }

static void report(uint32_t now) {
    lastReport = now;
    proto_report(anim_state_name(current_state()), anim_char_name(chr), current_level(),
                 growth_xp(), growth_next());
}

void setup() {
    Serial.begin(115200);
    Serial.setRxBufferSize(1024);

    prefs.begin("pet", false);
    chr = prefs.getUChar("chr", 0);
    if (chr >= SPR_PICKABLE_CHARS) chr = 0;
    pcLevel = prefs.getUChar("lv", 1);
    growth_begin();

    if (!display_init()) {
        display_fatal("sprite alloc failed (RAM)");
        while (true) delay(1000);
    }
    uint32_t now = millis();
    machine_begin(now);
    display_set_hud(current_level(), growth_xp(), growth_next(), nullptr);
    anim_set(chr, ST_IDLE, current_level());
    lastMsgAt = now;
    lastMinute = now;
}

void loop() {
    uint32_t now = millis();
    bool dirty = false;

    PetMsg msg;
    if (proto_poll(msg)) {
        lastMsgAt = now;
        if (msg.chr >= 0 && (uint8_t)msg.chr != chr) {
            chr = (uint8_t)msg.chr;
            prefs.putUChar("chr", chr);
        }
        if (msg.event >= 0 && msg.event != EV_SET_CHAR) {
            brain = true;
            uint32_t gained = machine_event((uint8_t)msg.event, msg.arg, now);
            if (growth_add(gained)) {
                machine_celebrate(now);
            }
            dirty = true;
        } else if (msg.state >= 0) {
            // the PC is the brain in this edition
            brain = false;
            pcState = (uint8_t)msg.state;
            if (msg.level >= 1) {
                if ((uint8_t)msg.level != pcLevel) prefs.putUChar("lv", (uint8_t)msg.level);
                pcLevel = (uint8_t)msg.level;
            }
            dirty = true;
        }
        if (msg.status) dirty = true;
    }

    if (brain) {
        if (machine_tick(now)) dirty = true;
        // a minute of real work is worth one XP, same rule as the PC edition
        if (machine_active() && now - lastMinute >= 60000UL) {
            lastMinute = now;
            if (growth_add(1)) machine_celebrate(now);
            dirty = true;
        }
        growth_flush(now);
    }

    // nothing from the PC for a long while: doze off
    if (now - lastMsgAt > AUTOSLEEP_MS && current_state() != ST_SLEEP) {
        if (brain) {
            machine_event(EV_TOGGLE_SLEEP, 0, now); // the != ST_SLEEP guard above
                                                    // keeps this from re-firing
        } else {
            pcState = ST_SLEEP;
        }
        dirty = true;
    }

    if (dirty) {
        display_set_hud(current_level(), growth_xp(), growth_next(), &proto_status());
        anim_set(chr, current_state(), current_level());
        anim_force_redraw();
    }
    anim_tick(now);

    if (brain && (dirty || now - lastReport > REPORT_MS)) {
        report(now);
    }
    delay(5);
}
