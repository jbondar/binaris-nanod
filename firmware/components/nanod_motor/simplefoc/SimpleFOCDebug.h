#pragma once
// Stub — debug disabled via SIMPLEFOC_DISABLE_DEBUG
#define SIMPLEFOC_DEBUG(...)
class SimpleFOCDebug {
public:
    static void enable(void* p = nullptr) {}
    template<typename T> static void print(T) {}
    template<typename T> static void println(T) {}
};
