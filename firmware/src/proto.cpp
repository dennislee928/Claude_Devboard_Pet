#include "proto.h"
#include "anim.h"
#include "machine.h"

static char buf[256];
static size_t len = 0;
static PetStatus status = {"", "", 0, 0, -1, false};

// Extract "key":"value" (string) into out; returns false if key absent.
static bool json_str(const char* line, const char* key, char* out, size_t outlen) {
    char pat[16];
    snprintf(pat, sizeof(pat), "\"%s\":\"", key);
    const char* p = strstr(line, pat);
    if (!p) return false;
    p += strlen(pat);
    size_t i = 0;
    while (*p && *p != '"' && i + 1 < outlen) out[i++] = *p++;
    out[i] = 0;
    return true;
}

static bool json_int(const char* line, const char* key, long& out) {
    char pat[16];
    snprintf(pat, sizeof(pat), "\"%s\":", key);
    const char* p = strstr(line, pat);
    if (!p) return false;
    out = atol(p + strlen(pat));
    return true;
}

static int8_t char_index(const char* name) {
    for (uint8_t i = 0; i < SPR_PICKABLE_CHARS; i++) {
        if (strcmp(name, anim_char_name(i)) == 0) return (int8_t)i;
    }
    return -1;
}

static bool parse_line(const char* line, PetMsg& out) {
    if (!strchr(line, '{')) return false;
    out.state = out.chr = out.level = -1;
    out.event = -1;
    out.arg = 0;
    out.status = false;

    char sval[32];
    long n;

    // firmware edition: a raw event for our own state machine
    if (json_int(line, "e", n)) {
        out.event = (int16_t)n;
        long t;
        if (json_int(line, "t", t)) out.arg = (uint8_t)t;
    }
    // character choice travels with both protocols
    if (json_str(line, "c", sval, sizeof(sval))) {
        out.chr = char_index(sval);
    }
    // standalone edition: the PC already decided the state
    if (json_str(line, "s", sval, sizeof(sval))) {
        out.state = anim_state_index(sval);
    }
    if (json_int(line, "lv", n) && n >= 1 && n <= 5) {
        out.level = (int8_t)n;
    }
    // Claude Code status strip
    if (json_str(line, "m", status.model, sizeof(status.model))) {
        out.status = true;
        status.valid = true;
    }
    if (json_str(line, "a", status.action, sizeof(status.action))) {
        out.status = true;
        status.valid = true;
    }
    if (json_int(line, "n", n)) {
        status.sessions = (uint16_t)n;
        out.status = true;
    }
    if (json_int(line, "tk", n)) {
        status.tokens = (uint32_t)n;
        out.status = true;
    }
    if (json_int(line, "pc", n)) {
        status.percent = (int16_t)n;
        out.status = true;
    }

    Serial.println("{\"ok\":1}");
    return true;
}

bool proto_poll(PetMsg& out) {
    while (Serial.available()) {
        char ch = (char)Serial.read();
        if (ch == '\n' || ch == '\r') {
            if (len == 0) continue;
            buf[len] = 0;
            len = 0;
            if (parse_line(buf, out)) return true;
        } else if (len + 1 < sizeof(buf)) {
            buf[len++] = ch;
        } else {
            len = 0; // overflow: drop garbage line
        }
    }
    return false;
}

const PetStatus& proto_status() { return status; }

void proto_report(const char* state, const char* chr, uint8_t level, uint32_t xp, uint32_t next) {
    Serial.printf("{\"s\":\"%s\",\"c\":\"%s\",\"lv\":%u,\"xp\":%lu,\"nx\":%lu}\n",
                  state, chr, (unsigned)level, (unsigned long)xp, (unsigned long)next);
}
