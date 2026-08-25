# Host-compiles the board's decision logic and runs it. No hardware needed.
CXXFLAGS ?= -std=c++17 -Wall -Wextra -Wno-unused-parameter -I stub -I ../src

all: run

fwtest: test_machine.cpp ../src/machine.cpp ../src/growth.cpp
	$(CXX) $(CXXFLAGS) -o $@ $^

prototest: test_proto.cpp ../src/proto.cpp
	$(CXX) $(CXXFLAGS) -o $@ $^

run: fwtest prototest
	./fwtest
	./prototest

clean:
	rm -f fwtest prototest

.PHONY: all run clean
