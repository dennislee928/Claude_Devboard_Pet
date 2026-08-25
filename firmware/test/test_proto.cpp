// Wire-contract test: every line pc/petd/src/board.rs emits must be understood
// by firmware/src/proto.cpp, and the status line the board reports back must be
// the shape pc/petd/src/board.rs::parse_status expects. The two sides are
// written in different languages, so this is the only place they meet.
#include <Arduino.h>
#include <cstdio>
#include "proto.h"
#include "machine.h"
#include "anim.h"

SerialStub Serial;
uint32_t millis() { return 0; }
void delay(uint32_t) {}

// proto.cpp only needs these two from anim.cpp, which would drag in the whole
// sprite table and TFT_eSPI; the names must match anim.cpp exactly.
static const char* const STATES[ST_COUNT] = {
    "idle", "coding", "thinking", "searching", "testing", "building",
    "debugging", "error", "success", "waiting", "notify", "celebrating", "sleep"};
static const char* const CHARS[4] = {"clawd", "beemo", "grogu", "egg"};
int8_t anim_state_index(const char* n) {
    for (int i = 0; i < ST_COUNT; i++) if (!strcmp(n, STATES[i])) return i;
    return -1;
}
const char* anim_char_name(uint8_t c) { return CHARS[c < 4 ? c : 0]; }

int fails = 0;
#define CHECK(c) do { if (!(c)) { printf("FAIL %s:%d %s\n", __FILE__, __LINE__, #c); fails++; } } while (0)

static PetMsg feed(const char* line) {
    PetMsg m{};
    Serial.feed(std::string(line) + "\n");
    CHECK(proto_poll(m));
    return m;
}

int main() {
    // --- firmware edition: raw events, exactly as board.rs::event_code emits
    PetMsg m = feed("{\"e\":3,\"t\":1}");            // ToolStart(RunTest)
    CHECK(m.event == EV_TOOL_START);
    CHECK(m.arg == TK_TEST);

    m = feed("{\"e\":6,\"t\":0}");                   // Stopped
    CHECK(m.event == EV_STOPPED);

    m = feed("{\"e\":12,\"t\":11}");                 // ForceState(celebrating)
    CHECK(m.event == EV_FORCE_STATE);
    CHECK(m.arg == ST_CELEBRATING);

    // --- character selection
    m = feed("{\"e\":20,\"c\":\"grogu\"}");
    CHECK(m.chr == 2);
    m = feed("{\"e\":20,\"c\":\"clawd\"}");
    CHECK(m.chr == 0);
    m = feed("{\"e\":20,\"c\":\"nobody\"}");
    CHECK(m.chr == -1);                              // unknown name is ignored

    // --- standalone edition: the PC already decided the state
    m = feed("{\"s\":\"debugging\",\"c\":\"beemo\",\"lv\":4}");
    CHECK(m.state == ST_DEBUGGING);
    CHECK(m.chr == 1);
    CHECK(m.level == 4);
    CHECK(m.event == -1);

    // --- Claude Code status strip
    m = feed("{\"m\":\"Opus 5\",\"a\":\"Editing main.rs\",\"n\":2,\"tk\":12345}");
    CHECK(m.status);
    const PetStatus& st = proto_status();
    CHECK(!strcmp(st.model, "Opus 5"));
    CHECK(!strcmp(st.action, "Editing main.rs"));
    CHECK(st.sessions == 2);
    CHECK(st.tokens == 12345);

    // an over-long action must not overflow the fixed buffer
    m = feed("{\"m\":\"Sonnet 4.5\",\"a\":\"012345678901234567890123456789012345678901234567890\",\"n\":1,\"tk\":0}");
    CHECK(strlen(proto_status().action) == sizeof(st.action) - 1);

    // --- junk and partial lines are survivable
    PetMsg junk{};
    Serial.feed("not json at all\n");
    CHECK(!proto_poll(junk));

    // --- what we report back must match board.rs::parse_status
    Serial.out.clear();
    proto_report("coding", "grogu", 3, 432, 1200);
    CHECK(Serial.out == "{\"s\":\"coding\",\"c\":\"grogu\",\"lv\":3,\"xp\":432,\"nx\":1200}\n");

    printf(fails ? "%d FAILURES\n" : "proto wire contract: all checks passed\n", fails);
    return fails != 0;
}
