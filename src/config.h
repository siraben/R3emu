#pragma once


// Default values for CLI-configurable options.

#define DEFAULT_allowsmul 1
// if S units may be able to multiply. they can either always multiply or never.

#define DEFAULT_maketracedump 0
#define DEFAULT_tracesize 100000


// main
#define DEFAULT_memrows 64 // amount of memory rows get emulated.
#define DEFAULT_coreamount 10 // amount of cores the emulator emulates. can go up to an max of 50 by standard, check the main function. all cores are multiply capable by standard.
#define DEFAULT_memdump 0 // if 1, dumps the memory after emulation
#define DEFAULT_targetfps 60 // target fps. multiply this by core amount and you got your target ips.
#define DEFAULT_fpslimiter 1 // if set to 1, tries to limit the fps to targetfps.
#define DEFAULT_updxframes 10 // only updates the SDL window every <this value>th frame.


// memory
#define DEFAULT_rowsize 128

// terminal
#define DEFAULT_charsnh 12
#define DEFAULT_charsnv 8
#define DEFAULT_haspixplot 1
