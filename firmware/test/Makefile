# Host-compiles the board's decision logic and runs it. No hardware needed.
CXXFLAGS ?= -std=c++17 -Wall -Wextra -Wno-unused-parameter -I stub -I ../src

all: run

fwtest: test_machine.cpp ../src/machine.cpp ../src/growth.cpp
	$(CXX) $(CXXFLAGS) -o $@ $^

run: fwtest
	./fwtest

clean:
	rm -f fwtest

.PHONY: all run clean
