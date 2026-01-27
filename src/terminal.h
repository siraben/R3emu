#pragma once
#include <stdint.h>
#include <SDL.h>
typedef uint8_t VM_pixel;
extern VM_pixel VM_colortable[16][3];

typedef struct {
    uint8_t charsnh;
    uint8_t charsnv;
    uint8_t* pixbuf;
    uint8_t colors;
    uint16_t hrange;
    uint16_t vrange;
    uint16_t cursor;
    uint8_t nlchar;
    uint32_t scrollmask;
    uint32_t char0even;
    uint32_t char0odd;
} VM_term;

void VM_setchar(VM_term* term, uint8_t fcolor, uint8_t bcolor, uint8_t charindex, uint8_t column, uint8_t row, SDL_Renderer* renderer);
void VM_copycharpix(VM_term* term, uint8_t sx, uint8_t sy, uint8_t dx, uint8_t dy, SDL_Renderer* renderer);
void VM_copypix(VM_term* term, uint32_t sx, uint32_t sy, uint32_t dx, uint32_t dy, SDL_Renderer* renderer);
void VM_setpix(VM_term* term, uint32_t x, uint32_t y, uint8_t colorindex, SDL_Renderer* renderer);
VM_term VM_newterm(uint8_t charsnh, uint8_t charsnv);
void VM_delterm(VM_term* term);
