#include <iostream>
#include <fstream>
#include <filesystem>
#include <string>

#include "argh.h"
#include <memory.h>
#include <SDL.h>

#include <unistd.h>

#include <cmath>
extern "C" {
#include "terminal.h"
#include "keyboard.h"
#include "common.h"
#include "disassembler.h"
#include "config.h"
#include "cores.h"
#include "memory.h"
#include "cores.h"
}
VM_keyboard* HOOK_keyboard;
VM_term* HOOK_term;
SDL_Renderer* renderer;
uint8_t HOOK_haspixplot;

VM_word hook_getkey(uint16_t) {
    return VM_getkey(HOOK_keyboard);
}
void hook_colreg(VM_word newval, uint16_t) {
    HOOK_term->colors = newval;
}
void hook_hrangereg(VM_word newval, uint16_t) {
    HOOK_term->hrange = newval;
}
void hook_vrangereg(VM_word newval, uint16_t) {
    HOOK_term->vrange = newval;
}
void hook_cursorreg(VM_word newval, uint16_t) {
    HOOK_term->cursor = newval;
}
void hook_nlcharreg(VM_word newval, uint16_t) {
    HOOK_term->nlchar = newval;
}
void hook_scrollmaskreg(VM_word newval, uint16_t) {
    HOOK_term->scrollmask = newval;
}
void hook_char0oddreg(VM_word newval, uint16_t) {
    HOOK_term->char0odd = newval;
}
void hook_char0evenreg(VM_word newval, uint16_t) {
    HOOK_term->char0even = newval;
}
void hook_scrollprint(VM_word newval, uint16_t addr) {
    uint8_t nlchar = addr >> 5 & 1;
    uint8_t tmscroll = addr >> 4 & 1;
    //uint8_t scrollm = addr >> 3 & 1;
    uint8_t roprint = addr >> 2 & 1;
    uint8_t cfdata = addr >> 1 & 1;
    uint8_t etmode = addr & 1;
    uint8_t column = HOOK_term->cursor & 0b11111;
    uint8_t row = (HOOK_term->cursor>>5) & 0b11111;
    uint8_t* pdir = roprint ? &column : &row;
    uint8_t* sdir = !roprint ? &column : &row;
    uint16_t* prange = roprint ? &HOOK_term->hrange : &HOOK_term->vrange;
    uint16_t* srange = !roprint ? &HOOK_term->hrange : &HOOK_term->vrange;
    uint8_t forecolor = !cfdata ? HOOK_term->colors & 0b1111 : newval>>8&0b1111;
    uint8_t backcolor = !cfdata ? (HOOK_term->colors >> 4) & 0b1111 : newval>>13&0b1111;
    uint8_t charindex = newval & 0b11111111;

    if (nlchar && charindex == HOOK_term->nlchar) {
        (*pdir) = ((*prange)>>5&0b11111)+1;
    }

    if (etmode == 1) {
        if (*pdir > ((*prange)>>5&0b11111)) {
            *pdir = 0;
            (*sdir) ++;
        }
        if (*sdir > ((*srange)>>5&0b11111)) {
            if (tmscroll) {
                // copy prev lines aka. scroll
                for (uint8_t y=(*srange)&0b11111;y<=((*srange)>>5&0b11111);y++) {
                    for (uint8_t x=(*prange)&0b11111;x<=((*prange)>>5&0b11111);x++) {
                        VM_copycharpix(HOOK_term, x, y+1, x, y, renderer);
                    }
                }
                // fill up space
                for (uint8_t x=(*prange)&0b11111;x<=((*prange)>>5&0b11111);x++) {
                    VM_setchar(HOOK_term, forecolor, backcolor, HOOK_term->nlchar, x, ((*srange)>>5&0b11111), renderer);
                }
                (*sdir) --;
            } else {
                (*sdir) = (*srange)&0b11111;
            }
        }

        if (!(nlchar && charindex == HOOK_term->nlchar)) {
            VM_setchar(HOOK_term, forecolor, backcolor, charindex, column, row, renderer);
            (*pdir) ++;
        }
    } else {
        if (1) {
            for (uint8_t y=(*srange)&0b11111;y<((*srange)>>5&0b11111);y++) {
                for (uint8_t x=(*prange)&0b11111;x<=((*prange)>>5&0b11111);x++) {
                    VM_copycharpix(HOOK_term, x, y+1, x, y, renderer);
                }
            }
        }
        //VM_setchar(HOOK_term, forecolor, backcolor, charindex, column, row);
        // fill up space
        for (uint8_t x=(*prange)&0b11111;x<=((*prange)>>5&0b11111);x++) {
            VM_setchar(HOOK_term, forecolor, backcolor, charindex, x, ((*srange)>>5&0b11111), renderer);
        }
    }

    HOOK_term->cursor = column+(row<<5);
}
void hook_plotpix(VM_word newval, uint16_t addr) {
    if (!HOOK_haspixplot) {return;} // pixel plotter not available

    uint8_t colorindex = addr & 0b1111;
    uint8_t row = (newval>>8) & 0b11111111;
    uint8_t column = newval & 0b11111111;

    VM_setpix(HOOK_term, column, row, colorindex, renderer);
}
uint64_t smolmin(uint64_t x, uint64_t y) {
	return (x < y) ? x : y;
}
void calcmemcol(VM_word value, uint8_t* out) { // taken from an R2 emulator at https://github.com/catsoften/r2_emulator/blob/master/src/memory.js, thanks! (was too lazy to implement on ma own)
	uint64_t wl = value | 0x20000000;
	uint64_t x, r = 0, g = 0, b = 0, a = 127;
	for (x = 0; x < 12; x++) {
		r += (wl >> (x + 18)) & 1;
		b += (wl >> x) & 1;
	}
	for (x = 0; x < 12; x++) {
		g += (wl >> (x + 9)) & 1;
	}
	x = trunc(624 / (r + g + b + 1));
	r = trunc(a * smolmin(r * x, 255) / 0xFF);
	g = trunc(a * smolmin(g * x, 255) / 0xFF);
	b = trunc(a * smolmin(b * x, 255) / 0xFF);
	out[0] = r;
	out[1] = g;
	out[2] = b;
}
void rendermem(VM_memory memory, uint8_t memrows, uint8_t charsnv) {
	for (uint64_t row=0;row<memrows;row++) {
		for (uint64_t column=0;column<128;column++) {
			VM_word val = VM_memread(memory, column+(128*row));
			uint8_t color[3];
			calcmemcol(val, color);
			SDL_SetRenderDrawColor(renderer, color[0], color[1], color[2], 255);
    		SDL_RenderDrawPoint(renderer, column, row+(8*charsnv));
			//VM_setrawpix(HOOK_term, column, row+8*charsnv, renderer, color);
			//VM_setpix(HOOK_term, column, row+8*charsnv, )
		}
	}
}

static void print_usage(const char* prog) {
    std::cout << "Usage: " << prog << " [options] <input.bin>" << std::endl;
    std::cout << "Options:" << std::endl;
    std::cout << "  -h, --help              Show this help and exit" << std::endl;
    std::cout << "  --memrows N             Memory rows (default: " << DEFAULT_memrows << ")" << std::endl;
    std::cout << "  --cores N               Number of cores (default: " << DEFAULT_coreamount << ")" << std::endl;
    std::cout << "  --targetfps N           Target FPS (default: " << DEFAULT_targetfps << ")" << std::endl;
    std::cout << "  --updxframes N          SDL update interval in frames (default: " << DEFAULT_updxframes << ")" << std::endl;
    std::cout << "  --tracesize N           Trace buffer size (default: " << DEFAULT_tracesize << ")" << std::endl;
    std::cout << "  --term-cols N           Terminal character columns (default: " << DEFAULT_charsnh << ")" << std::endl;
    std::cout << "  --term-rows N           Terminal character rows (default: " << DEFAULT_charsnv << ")" << std::endl;
    std::cout << "  --rowsize N             Memory row size in words (default: " << DEFAULT_rowsize << ")" << std::endl;
    std::cout << "  --memdump               Dump memory after emulation" << std::endl;
    std::cout << "  --tracedump             Enable execution trace dump" << std::endl;
    std::cout << "  --no-fpslimiter         Disable FPS limiter" << std::endl;
    std::cout << "  --no-smul               Disallow S-type core multiplication" << std::endl;
    std::cout << "  --no-pixplot            Disable pixel plotting" << std::endl;
}

int main(int argc, char* argv[]) {
    std::cout << "R3 emulator" << std::endl;
    std::cout << "Written by Justus Wolff in very late 2025-2026" << std::endl << "With help from LBPHacker, to fix alot of arithmetic bugs, who also made the original R3" << std::endl << "Also credit to siraben due to finding bugs and patching them by implementing haskell for the R3." << std::endl;
    std::cout << "Also, if you read this, I might do an complete rewrite soon since this is quite the mess." << std::endl;

	argh::parser cmdl(argc, argv);

    if (cmdl[{ "-h", "--help" }]) {
        print_usage(argv[0]);
        return 0;
    }

    if (cmdl.size() < 2) {
        print_usage(argv[0]);
        return 1;
    }

    const std::string input_path = cmdl[1];
    if (!std::filesystem::exists(input_path)) {
        std::cout << "File doesnt exist!" << std::endl;
        return 2;
    }

    // Parse CLI options with defaults from config.h
    int memrows;
    cmdl("--memrows", DEFAULT_memrows) >> memrows;

    int coreamount;
    cmdl("--cores", DEFAULT_coreamount) >> coreamount;

    int targetfps;
    cmdl("--targetfps", DEFAULT_targetfps) >> targetfps;

    int updxframes;
    cmdl("--updxframes", DEFAULT_updxframes) >> updxframes;

    uint64_t tracesize;
    cmdl("--tracesize", DEFAULT_tracesize) >> tracesize;

    int charsnh;
    cmdl("--term-cols", DEFAULT_charsnh) >> charsnh;

    int charsnv;
    cmdl("--term-rows", DEFAULT_charsnv) >> charsnv;

    int rowsize;
    cmdl("--rowsize", DEFAULT_rowsize) >> rowsize;

    bool memdump = cmdl["--memdump"];
    bool tracedump = cmdl["--tracedump"];
    bool fpslimiter = !cmdl["--no-fpslimiter"];
    bool allowsmul = !cmdl["--no-smul"];
    bool haspixplot = !cmdl["--no-pixplot"];

    HOOK_haspixplot = haspixplot ? 1 : 0;

    VM_vminstance instance = VM_newinstance(
        (uint8_t)memrows,
        (uint8_t)coreamount,
        (uint8_t[]){2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2},
        (uint16_t)rowsize,
        allowsmul ? 1 : 0,
        tracedump ? 1 : 0,
        tracesize
    );
    uint16_t memsize_words = VM_getsize((uint8_t)memrows, (uint16_t)rowsize);
    memset(instance.memory.content, 0x00, memsize_words*sizeof(VM_word));

    std::ifstream file(input_path, std::ios_base::binary);
    file.seekg(0, std::ios::end);
    size_t length = file.tellg();
    file.seekg(0, std::ios::beg);
    if (length > memsize_words*sizeof(VM_word)) {
        length = memsize_words*sizeof(VM_word);
    }
    char* buf = new char[memsize_words*sizeof(VM_word)];
	memset(buf, 0x00, memsize_words*sizeof(VM_word));
    file.read(buf, length);
    std::cout << "Reading into memory..." << std::endl;
    memcpy(instance.memory.content, buf, memsize_words*sizeof(VM_word));
    delete[] buf;

    VM_keyboard keyboard = VM_newkeyboard();
    VM_term terminal = VM_newterm((uint8_t)charsnh, (uint8_t)charsnv);
    HOOK_term = &terminal;
    HOOK_keyboard = &keyboard;
    uint16_t baseaddr = 0x9F80;
    // setup hooks for terminal/keyboard
    VM_addrhook(&instance.memory, baseaddr, hook_getkey, 0); // input register
    VM_addwhook(&instance.memory, baseaddr+0x46, hook_colreg, 0); // color register
    VM_addwhook(&instance.memory, baseaddr+0x42, hook_hrangereg, 0); // hrange register
    VM_addwhook(&instance.memory, baseaddr+0x43, hook_vrangereg, 0); // vrange register
    VM_addwhook(&instance.memory, baseaddr+0x44, hook_cursorreg, 0); // cursor register
    VM_addwhook(&instance.memory, baseaddr+0x45, hook_nlcharreg, 0); // nlchar register
    VM_addwhook(&instance.memory, baseaddr+0x47, hook_scrollmaskreg, 0); // scrollmask register
    VM_addwhook(&instance.memory, baseaddr+0x40, hook_char0oddreg, 0); // char0odd register
    VM_addwhook(&instance.memory, baseaddr+0x41, hook_char0evenreg, 0); // char0even register
    VM_addwhook(&instance.memory, baseaddr, hook_scrollprint, 0x3F); // scrollprint
    VM_addwhook(&instance.memory, baseaddr+0x60, hook_plotpix, 0x1F); // plotpix

    std::cout << "Target fps: " << targetfps << std::endl;
    std::cout << "Target ips: " << targetfps*coreamount << std::endl;

    SDL_Event event;
    SDL_Renderer* _renderer;
    SDL_Window* window;

    SDL_Init(SDL_INIT_VIDEO);
    if (SDL_CreateWindowAndRenderer(300, 500, 0, &window, &_renderer) == -1) {
        std::cout << "Failed to create window! '" << SDL_GetError() << "'" << std::endl;
    }
    renderer = _renderer;
    SDL_RenderSetLogicalSize(renderer, 8*charsnh, 8*charsnv+64);
    SDL_SetRenderDrawColor(renderer, 0, 0, 0, 0);
    SDL_RenderClear(renderer);

    std::cout << "Emulation started." << std::endl;
    uint64_t frame=0;
    float frameLimit = 1.f / targetfps;
    while (!instance.halted) {
        VM_instcycle(&instance);

		SDL_PollEvent(&event);
		if (event.type == SDL_QUIT) {
			instance.halted = 1;
		}
		if (event.type == SDL_KEYDOWN) {
			SDL_Keycode key = event.key.keysym.sym;

			char ch = 0;

			if (key >= SDLK_a && key <= SDLK_z) {
				ch = 'a' + (key - SDLK_a);
			}
			else if (key >= SDLK_0 && key <= SDLK_9) {
				ch = '0' + (key - SDLK_0);
			}
			else {
				switch (key) {
					case SDLK_RETURN:    ch = '\n'; break;
					case SDLK_BACKSPACE: ch = '\b'; break;
					case SDLK_TAB:       ch = '\t'; break;
					case SDLK_SPACE:     ch = ' ';  break;
					case SDLK_ESCAPE:    ch = 27;   break;
				}
			}
			if (ch != 0) {
				VM_registerkeypress(&keyboard, ch);
			}
		}

        frame ++;
        if (frame >= (uint64_t)updxframes) {
			rendermem(instance.memory, (uint8_t)memrows, (uint8_t)charsnv); // render memory

			for (uint16_t x=0;x<(8*charsnh);x++) { // render main screen
				for (uint16_t y=0;y<(8*charsnv);y++) {
					uint8_t color = terminal.pixbuf[x+(y*charsnh*8)];
					uint8_t r,g,b;
					r = VM_colortable[color][0];
					g = VM_colortable[color][1];
					b = VM_colortable[color][2];
					SDL_SetRenderDrawColor(renderer, r, g, b, 255);
    				SDL_RenderDrawPoint(renderer, x, y);
				}
			}

            frame = 0;
            SDL_RenderPresent(renderer);
        }

        if (fpslimiter) {
            usleep(frameLimit*1000000);
        }
    }
    std::cout << "Emulation finished at IP '" << instance.IP << "'" << std::endl;

    if (memdump) {
        std::cout << "Dumping memory..." << std::endl;
        std::ofstream dumpfile("memdump.bin");
        for (uint16_t index=0;index<memsize_words;index++) {
            dumpfile << instance.memory.content[index] << std::endl;
        }
        dumpfile.close();

        std::cout << "Dumping disassembled memory..." << std::endl;
        std::ofstream dumpfile2("memdumpdisasm.asm");
        dumpfile2 << "%include \"common\"" << std::endl;
        for (uint16_t index=0;index<memsize_words;index++) {
            char* temp = VM_disasminstruction(instance.memory.content[index]);
            dumpfile2 << temp << std::endl;
            free(temp);
        }
        dumpfile2.close();
    }
    if (tracedump) {
        std::cout << "Dumping trace..." << std::endl;
        std::ofstream dumpfile("tracedump.bin");
        for (uint16_t index=0;index<instance.tracesize;index++) {
            dumpfile << instance.backtrace[index] << std::endl;
        }
        dumpfile.close();

        std::cout << "Dumping disassembled trace..." << std::endl;
        std::ofstream dumpfile2("tracedumpdisasm.asm");
        dumpfile2 << "%include \"common\"" << std::endl;
        for (uint16_t index=0;index<instance.tracesize;index++) {
            char* temp = VM_disasminstruction(instance.backtrace[index]);
            dumpfile2 << instance.backtraceaddrs[index] << ": " << temp << "    "
            << instance.backtraceop[index][0]
            << "/"
            << instance.backtraceop[index][1]
            << "/"
            << instance.backtraceop[index][2] << std::endl;
            free(temp);
        }
        dumpfile2.close();
    }

    VM_delterm(&terminal);
    VM_delinstance(instance);
    SDL_DestroyRenderer(renderer);
    SDL_DestroyWindow(window);
    SDL_Quit();
}
