#include "display.h"
#include <TFT_eSPI.h>
#include "sprites_gen.h"

static TFT_eSPI tft;
static TFT_eSprite spr(&tft);
static uint16_t lut565[256]; // RGB332 -> RGB565

static const uint16_t BG = 0x18E3;       // near-black warm gray
static const uint16_t FLOOR_C = 0x2965;  // floor line
static const uint16_t BAR_BG = 0x2124;
static const uint16_t BAR_FG = 0x3E4D;   // green
static const uint16_t TEXT_C = 0xBDF7;
static const uint16_t DIM_C = 0x7BEF;

// The pet stands on this line; everything below is the status HUD.
static const int FLOOR_Y = 180;

static const char* LEVEL_NAMES[5] = {"Egg", "Baby", "Junior", "Senior", "Legend"};

static uint8_t hudLevel = 1;
static uint32_t hudXp = 0, hudNext = 100;
static PetStatus hudStatus = {"", "", 0, 0, false};

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
    spr.setTextFont(1);
    return true;
}

void display_set_hud(uint8_t level, uint32_t xp, uint32_t next, const PetStatus* status) {
    hudLevel = level < 1 ? 1 : (level > 5 ? 5 : level);
    hudXp = xp;
    hudNext = next;
    if (status) hudStatus = *status;
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

// 12345 -> "12.3k", 2500000 -> "2.5M"
static void human(uint32_t n, char* out, size_t len) {
    if (n < 1000) snprintf(out, len, "%lu", (unsigned long)n);
    else if (n < 1000000) snprintf(out, len, "%lu.%luk", (unsigned long)(n / 1000), (unsigned long)((n % 1000) / 100));
    else snprintf(out, len, "%lu.%luM", (unsigned long)(n / 1000000), (unsigned long)((n % 1000000) / 100000));
}

static void draw_hud() {
    char line[56];

    // XP bar
    const int barX = 20, barW = 200, barY = 186, barH = 6;
    spr.fillRect(barX, barY, barW, barH, BAR_BG);
    uint32_t base = 0;
    static const uint32_t TH[5] = {0, 100, 400, 1200, 3000};
    for (int i = 0; i < 5; i++) {
        if (hudXp >= TH[i]) base = TH[i];
    }
    int w = barW;
    if (hudNext > base) {
        uint32_t span = hudNext - base;
        uint32_t got = hudXp > base ? hudXp - base : 0;
        w = (int)((uint64_t)barW * got / span);
        if (w > barW) w = barW;
    }
    spr.fillRect(barX, barY, w, barH, BAR_FG);

    spr.setTextColor(TEXT_C, BG);
    if (hudNext > 0) {
        snprintf(line, sizeof(line), "Lv%u %s   %lu/%lu", (unsigned)hudLevel, LEVEL_NAMES[hudLevel - 1],
                 (unsigned long)hudXp, (unsigned long)hudNext);
    } else {
        snprintf(line, sizeof(line), "Lv%u %s   %lu XP (max)", (unsigned)hudLevel, LEVEL_NAMES[hudLevel - 1],
                 (unsigned long)hudXp);
    }
    spr.drawString(line, barX, 196, 1);

    if (!hudStatus.valid) return;

    char tk[12];
    human(hudStatus.tokens, tk, sizeof(tk));
    spr.setTextColor(DIM_C, BG);
    snprintf(line, sizeof(line), "%s  %u sess  %s tok",
             hudStatus.model[0] ? hudStatus.model : "-", (unsigned)hudStatus.sessions, tk);
    spr.drawString(line, barX, 208, 1);

    spr.setTextColor(TEXT_C, BG);
    spr.drawString(hudStatus.action, barX, 222, 1);
}

void display_render(const uint8_t* frame, const uint8_t* overlay, int8_t bob, uint8_t scale) {
    spr.fillSprite(BG);
    int size = SPR_W * scale;
    int x0 = (240 - size) / 2;
    // keep feet on a fixed floor line regardless of scale
    int y0 = FLOOR_Y - size + 4 * scale; // art leaves ~4px empty at the bottom
    if (y0 < 0) y0 = 0;
    spr.drawFastHLine(0, FLOOR_Y + 2, 240, FLOOR_C);
    spr.drawFastHLine(0, FLOOR_Y + 3, 240, FLOOR_C);
    draw_frame(frame, x0, y0, scale);
    if (overlay != nullptr) {
        draw_frame(overlay, x0, y0 + bob * scale, scale);
    }
    draw_hud();
    spr.pushSprite(0, 0);
}

void display_fatal(const char* msg) {
    tft.fillScreen(TFT_BLACK);
    tft.setTextColor(TFT_RED, TFT_BLACK);
    tft.drawString(msg, 10, 110, 2);
}
