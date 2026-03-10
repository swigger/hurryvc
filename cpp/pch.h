#pragma once

#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <Windows.h>
#include <WinSock2.h>
#else
#include <signal.h>
#endif

// common C headers
#include <stdio.h>
#include <stdlib.h>
#include <assert.h>
#include <stdint.h>

// well known C++ headers
#include <string>
#include <vector>
#include <memory>
#include <optional>
#include <algorithm>

using std::string;
using std::vector;
