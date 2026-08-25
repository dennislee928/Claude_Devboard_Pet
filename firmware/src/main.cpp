// Desk-pet firmware: ESP-WROOM-32 DevKit V1 + ST7789 240x240.
// Receives {"s","c","lv"} JSON lines from the petd daemon over USB serial,
// plays the matching pet animation, and caches character/level in NVS so a
// standalone (unplugged-from-daemon) boot still shows the grown pet.

#include <Arduino.h>
#include <Preferences.h>
#include "display.h"
#include "anim.h"
#include "proto.h"

static Preferences prefs;
static uint8_t chr = 0, level = 1, state = ST_IDLE;
static uint32_t lastMsgAt = 0;

// With no daemon traffic the pet dozes off on its own.
static const uint32_t AUTOSLEEP_MS = 8UL * 60UL * 1000UL;

void setup() {
    Serial.begin(115200);
    Serial.setRxBufferSize(1024);

    prefs.begin("pet", false);
    chr = prefs.getUChar("chr", 0);
    level = prefs.getUChar("lv", 1);

    if (!display_init()) {
        display_fatal("sprite alloc failed (RAM)");
        while (true) delay(1000);
    }
    anim_set(chr, ST_IDLE, level);
    lastMsgAt = millis();
}

void loop() {
    uint32_t now = millis();

    PetMsg msg;
    if (proto_poll(msg)) {
        lastMsgAt = now;
        if (msg.chr >= 0 && msg.chr != chr) {
            chr = msg.chr;
            prefs.putUChar("chr", chr);
        }
        if (msg.level >= 1 && msg.level != level) {
            prefs.putUChar("lv", msg.level);
            level = msg.level;
        }
        if (msg.state >= 0) state = msg.state;
        anim_set(chr, state, level);
    }

    if (state != ST_SLEEP && now - lastMsgAt > AUTOSLEEP_MS) {
        state = ST_SLEEP;
        anim_set(chr, state, level);
    }

    anim_tick(now);
    delay(5);
}
