#pragma once

#include "memory.h"
#include <stdint.h>

typedef struct {
    uint8_t cores[50];
    /*
    types:
        - 0: f
        - 1: s
        - 2: m
    */
    uint8_t coreamount;
    VM_memory memory;
    VM_registers regs;
    VM_flags flags;
    VM_word IP;
    uint8_t halted;

    uint16_t sch_addr; // schedule addr
    uint8_t sch_mode; // schedule mode
    uint8_t sch_reg; // schedule reg

    uint8_t allowsmul;
    uint8_t maketracedump;
    uint64_t maxtracesize;

    VM_word* backtrace;
    VM_word* backtraceaddrs;
    VM_word (*backtraceop)[3];
    uint64_t tracesize;
} VM_vminstance;

VM_vminstance VM_newinstance(uint8_t memsize, uint8_t coreamount, uint8_t* coretypes, uint16_t rowsize, uint8_t allowsmul, uint8_t maketracedump, uint64_t tracesize);
void VM_delinstance(VM_vminstance inst);
void VM_instcycle(VM_vminstance* _inst);
