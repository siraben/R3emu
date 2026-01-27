/*
Written by Justus Wolff in very late 2025.
*/

#include <stdlib.h>
#include <memory.h>
#include <string.h>

#include "memory.h"
#include "common.h"
#include "cores.h"


void VM_addrhook(VM_memory* memory, uint16_t addr, VM_mrhook hook, uint16_t length) {
	memory->rhooks[memory->rha++] = hook;
	memory->rhaddrf[memory->rha-1] = addr;
	memory->rhaddrt[memory->rha-1] = addr+length;
}
void VM_addwhook(VM_memory* memory, uint16_t addr, VM_mwhook hook, uint16_t length) {
	memory->whooks[memory->wha++] = hook;
	memory->whaddrf[memory->wha-1] = addr;
	memory->whaddrt[memory->wha-1] = addr+length;
}
VM_mrhook VM_callrhooks(VM_memory memory, uint16_t addr) {
	for (uint16_t i=0;i<memory.rha;i++) {
		if (addr >= memory.rhaddrf[i] && addr <= memory.rhaddrt[i]) {return memory.rhooks[i];}
	}
	return NULL;
}
uint8_t VM_callwhooks(VM_memory memory, uint16_t addr, VM_word val) {
	uint8_t called = 0;
	for (uint16_t i=0;i<memory.wha;i++) {
		if (addr >= memory.whaddrf[i] && addr <= memory.whaddrt[i]) {
			memory.whooks[i](val, addr-memory.whaddrf[i]);
			called = 1;
		}
	}
	return called;
}
uint16_t VM_getsize(uint8_t rows, uint16_t rowsize) {
	return rowsize*rows;
}
VM_memory VM_newmemory(uint8_t rows, uint16_t rowsize) {
	VM_memory out;
	out.rows = rows;
	out.rowsize = rowsize;
	out.content = (VM_word*)malloc(sizeof(VM_word)*VM_getsize(rows, rowsize));
	memset(out.content, 0xAA, sizeof(VM_word)*VM_getsize(rows, rowsize));
	out.rha = 0;
	out.wha = 0;
	return out;
}
VM_word VM_memread(VM_memory memory, uint16_t addr) {
	VM_mrhook hook = VM_callrhooks(memory, addr);
	if (hook != NULL) {
		return hook(addr);
	}
	if (addr >= VM_getsize(memory.rows, memory.rowsize)) {return VM_nullword;}
	VM_word temp = memory.content[addr];
	patchword(&temp);
	return temp;
}
void VM_memwrite(VM_memory memory, uint16_t addr, VM_word newval) {
	if (VM_callwhooks(memory, addr, newval)) {return;}
	if (addr >= VM_getsize(memory.rows, memory.rowsize)) {return;}
	patchword(&newval);
	memory.content[addr] = newval;
}


