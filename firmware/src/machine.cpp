#include "machine.h"

static const uint32_t ERROR_STICKY_MS = 5000;
static const uint32_t SUCCESS_HOLD_MS = 10000;
static const uint32_t NOTIFY_HOLD_MS = 4000;
static const uint32_t RECENT_ERROR_MS = 60000;
static const uint32_t IDLE_TO_SLEEP_MS = 180000;
static const uint32_t PET_XP_COOLDOWN_MS = 60000;
static const uint32_t FEED_XP_COOLDOWN_MS = 600000;

static uint8_t state = ST_IDLE;
static bool hasRevert = false;
static uint32_t revertAt = 0;
static uint8_t revertTo = ST_IDLE;
static bool hasError = false;
static uint32_t errorAt = 0;
static uint32_t lastActivity = 0;
static uint32_t lastPetXp = 0, lastFeedXp = 0;

static void setState(uint8_t s, bool revert, uint32_t at, uint8_t to) {
    state = s;
    hasRevert = revert;
    revertAt = at;
    revertTo = to;
}

void machine_begin(uint32_t now) {
    state = ST_IDLE;
    hasRevert = false;
    hasError = false;
    lastActivity = now;
    // start the interaction cooldowns already expired
    lastPetXp = now - PET_XP_COOLDOWN_MS - 1;
    lastFeedXp = now - FEED_XP_COOLDOWN_MS - 1;
}

uint8_t machine_state() { return state; }

bool machine_active() {
    return !(state == ST_IDLE || state == ST_SLEEP || state == ST_WAITING);
}

void machine_celebrate(uint32_t now) {
    setState(ST_CELEBRATING, true, now + 4000, ST_THINKING);
}

uint32_t machine_event(uint8_t code, uint8_t arg, uint32_t now) {
    lastActivity = now;
    uint32_t xp = 0;
    switch (code) {
        case EV_PROMPT:
            xp += 5;
            setState(ST_THINKING, false, 0, 0);
            break;
        case EV_SESSION_START:
            // greet the new session, then settle into thinking
            setState(ST_NOTIFY, true, now + NOTIFY_HOLD_MS, ST_THINKING);
            break;
        case EV_TOOL_START: {
            xp += 1;
            bool recentErr = hasError && (now - errorAt) < RECENT_ERROR_MS;
            uint8_t s;
            switch (arg) {
                case TK_EDIT:   s = recentErr ? ST_DEBUGGING : ST_CODING; break;
                case TK_TEST:   s = ST_TESTING; break;
                case TK_BUILD:  s = recentErr ? ST_DEBUGGING : ST_BUILDING; break;
                case TK_SEARCH: s = ST_SEARCHING; break;
                case TK_AGENT:  s = ST_THINKING; break;
                default:        s = ST_BUILDING; break;
            }
            setState(s, false, 0, 0);
            break;
        }
        case EV_TOOL_ERR:
            hasError = true;
            errorAt = now;
            setState(ST_ERROR, true, now + ERROR_STICKY_MS, ST_THINKING);
            break;
        case EV_TOOL_OK:
            if (hasError) {
                hasError = false;
                xp += 3; // recovered from an error
            }
            if (state != ST_ERROR || !hasRevert || (int32_t)(now - revertAt) >= 0) {
                setState(ST_THINKING, false, 0, 0);
            }
            break;
        case EV_STOPPED:
            hasError = false;
            setState(ST_SUCCESS, true, now + SUCCESS_HOLD_MS, ST_IDLE);
            break;
        case EV_PERMISSION:
            setState(ST_NOTIFY, true, now + NOTIFY_HOLD_MS, ST_WAITING);
            break;
        case EV_SUBAGENT_DONE:
            xp += 2;
            setState(ST_SUCCESS, true, now + 3000, ST_THINKING);
            break;
        case EV_PETTED:
            if (now - lastPetXp > PET_XP_COOLDOWN_MS) {
                lastPetXp = now;
                xp += 1;
            }
            setState(ST_CELEBRATING, true, now + 2500, ST_IDLE);
            break;
        case EV_FEED:
            if (now - lastFeedXp > FEED_XP_COOLDOWN_MS) {
                lastFeedXp = now;
                xp += 5;
            }
            setState(ST_CELEBRATING, true, now + 4000, ST_IDLE);
            break;
        case EV_TOGGLE_SLEEP:
            setState(state == ST_SLEEP ? ST_IDLE : ST_SLEEP, false, 0, 0);
            break;
        case EV_FORCE_STATE:
            if (arg < ST_COUNT) setState(arg, false, 0, 0);
            break;
        default:
            break;
    }
    return xp;
}

bool machine_tick(uint32_t now) {
    if (hasRevert && (int32_t)(now - revertAt) >= 0) {
        hasRevert = false;
        if (state != revertTo) {
            state = revertTo;
            return true;
        }
    }
    if (state == ST_IDLE && now - lastActivity > IDLE_TO_SLEEP_MS) {
        state = ST_SLEEP;
        return true;
    }
    return false;
}
