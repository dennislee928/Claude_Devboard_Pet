#include "anim.h"
#include "display.h"
#include "sprites_gen.h"

static const char* const STATE_NAMES[ST_COUNT] = {
    "idle", "coding", "thinking", "searching", "testing", "building",
    "debugging", "error", "success", "waiting", "notify", "celebrating", "sleep"
};
static const char* const CHAR_NAMES[SPR_NUM_CHARS] = {"clawd", "beemo", "grogu", "egg"};

static const uint8_t SCALE_BY_LEVEL[5] = {3, 3, 4, 4, 5};

static uint8_t curChar = 0, curState = ST_IDLE, curLevel = 1;
static uint8_t frameIdx = 0;
static uint32_t nextFrameAt = 0;

int8_t anim_state_index(const char* name) {
    for (int i = 0; i < ST_COUNT; i++) {
        if (strcmp(name, STATE_NAMES[i]) == 0) return i;
    }
    return -1;
}

const char* anim_state_name(uint8_t state) {
    return STATE_NAMES[state < ST_COUNT ? state : ST_IDLE];
}

const char* anim_char_name(uint8_t chr) {
    return CHAR_NAMES[chr < SPR_NUM_CHARS ? chr : 0];
}

void anim_force_redraw() { nextFrameAt = 0; }

void anim_set(uint8_t chr, uint8_t state, uint8_t level) {
    if (chr >= SPR_PICKABLE_CHARS) chr = 0;
    if (state >= ST_COUNT) state = ST_IDLE;
    if (level < 1) level = 1;
    if (level > 5) level = 5;
    if (chr == curChar && state == curState && level == curLevel) return;
    curChar = chr;
    curState = state;
    curLevel = level;
    frameIdx = 0;
    nextFrameAt = 0; // render immediately on next tick
}

void anim_tick(uint32_t now) {
    if (now < nextFrameAt) return;

    uint8_t drawChar = (curLevel == 1) ? SPR_EGG_CHAR : curChar;
    const SprAnim& a = SPR_ANIMS[drawChar][curState];
    if (frameIdx >= a.count) frameIdx = 0;

    const uint8_t* overlay = nullptr;
    if (curLevel >= 3 && drawChar < SPR_PICKABLE_CHARS) {
        overlay = SPR_OVERLAYS[drawChar][curLevel - 3];
    }
    int8_t bob = (int8_t)pgm_read_byte((const uint8_t*)a.bob + frameIdx);
    display_render(a.frames[frameIdx], overlay, bob, SCALE_BY_LEVEL[curLevel - 1]);

    frameIdx = (frameIdx + 1) % a.count;
    nextFrameAt = now + a.dur_ms;
}
