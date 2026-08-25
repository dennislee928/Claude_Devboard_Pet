#pragma once
#include <Arduino.h>

// Initialise TFT + 8-bpp full-screen sprite. Returns false if the sprite
// buffer (57.6KB) could not be allocated.
bool display_init();

// Render one 40x40 RGB332 frame (PROGMEM) scaled `scale`x, centred, with an
// optional overlay frame shifted down by `bob` source pixels.
void display_render(const uint8_t* frame, const uint8_t* overlay, int8_t bob, uint8_t scale);

// Small status text at the bottom of the screen (used for fatal errors).
void display_fatal(const char* msg);
