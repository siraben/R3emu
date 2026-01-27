#include "common.h"
#pragma once
typedef VM_word(*VM_mrhook)(uint16_t);
typedef void(*VM_mwhook)(VM_word, uint16_t);

typedef struct {
	uint16_t rows;
	uint16_t rowsize;
	VM_word* content;

	// hooks
	uint16_t rha;
	uint16_t wha;
	VM_mrhook rhooks[32];
	VM_mwhook whooks[32];
	uint16_t rhaddrf[32];
	uint16_t whaddrf[32];
	uint16_t rhaddrt[32];
	uint16_t whaddrt[32];
} VM_memory;



VM_word VM_memread(VM_memory memory, uint16_t addr);
void VM_memwrite(VM_memory memory, uint16_t addr, VM_word newval);
void VM_addrhook(VM_memory* memory, uint16_t addr, VM_mrhook hook, uint16_t length);
void VM_addwhook(VM_memory* memory, uint16_t addr, VM_mwhook hook, uint16_t length);
uint16_t VM_getsize(uint8_t rows, uint16_t rowsize);
VM_memory VM_newmemory(uint8_t rows, uint16_t rowsize);
