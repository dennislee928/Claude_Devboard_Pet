#include "proto.h"
#include "anim.h"

static char buf[192];
static size_t len = 0;

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

static bool json_int(const char* line, const char* key, int& out) {
    char pat[16];
    snprintf(pat, sizeof(pat), "\"%s\":", key);
    const char* p = strstr(line, pat);
    if (!p) return false;
    out = atoi(p + strlen(pat));
    return true;
}

static bool parse_line(const char* line, PetMsg& out) {
    if (!strchr(line, '{')) return false;
    out.state = out.chr = out.level = -1;
    char sval[16];
    if (json_str(line, "s", sval, sizeof(sval))) {
        out.state = anim_state_index(sval);
    }
    if (json_str(line, "c", sval, sizeof(sval))) {
        if (strcmp(sval, "clawd") == 0) out.chr = 0;
        else if (strcmp(sval, "beemo") == 0) out.chr = 1;
    }
    int lv;
    if (json_int(line, "lv", lv) && lv >= 1 && lv <= 5) {
        out.level = (int8_t)lv;
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
