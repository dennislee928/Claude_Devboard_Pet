#include "display.h"
#include <TFT_eSPI.h>
#include "sprites_gen.h"

static TFT_eSPI tft;
static TFT_eSprite spr(&tft);
static uint16_t lut565[256]; // RGB332 -> RGB565

static const uint16_t BG = 0x18E3;    // near-black warm gray
static const uint16_t FLOOR_C = 0x2965; // floor line

bool display_init() {
    tft.init();
    tft.setRotation(0);
    tft.fillScreen(TFT_BLACK);

    for (int i = 0; i < 256; i++) {
        uint8_t r = (i >> 5) & 0x07;
        uint8_t g = (i >> 2) & 0x07;
        uint8_t b = i & 0x03;
        lut565[i] = ((r * 255 / 7 & 0xF8) << 8) | ((g * 255 / 7 & 0xFC) << 3) | (b * 255 / 3 >> 3);
    }

    spr.setColorDepth(8);
    if (spr.createSprite(240, 240) == nullptr) {
        return false;
    }
    return true;
}

static void draw_frame(const uint8_t* frame, int x0, int y0, uint8_t scale) {
    for (int y = 0; y < SPR_W; y++) {
        for (int x = 0; x < SPR_W; x++) {
            uint8_t c = pgm_read_byte(frame + y * SPR_W + x);
            if (c == SPR_TRANSPARENT) continue;
            spr.fillRect(x0 + x * scale, y0 + y * scale, scale, scale, lut565[c]);
        }
    }
}

void display_render(const uint8_t* frame, const uint8_t* overlay, int8_t bob, uint8_t scale) {
    spr.fillSprite(BG);
    int size = SPR_W * scale;
    int x0 = (240 - size) / 2;
    // keep feet on a fixed floor line regardless of scale
    int floorY = 210;
    int y0 = floorY - size + 4 * scale; // art leaves ~4px empty at the bottom
    if (y0 < 0) y0 = 0;
    spr.drawFastHLine(0, floorY + 2, 240, FLOOR_C);
    spr.drawFastHLine(0, floorY + 3, 240, FLOOR_C);
    draw_frame(frame, x0, y0, scale);
    if (overlay != nullptr) {
        draw_frame(overlay, x0, y0 + bob * scale, scale);
    }
    spr.pushSprite(0, 0);
}

void display_fatal(const char* msg) {
    tft.fillScreen(TFT_BLACK);
    tft.setTextColor(TFT_RED, TFT_BLACK);
    tft.drawString(msg, 10, 110, 2);
}
