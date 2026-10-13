-- Native TPT Lua instrumentation. Reads particle state without changing logic.
local cfg = dofile("capture-config.lua")
local function run()
	sim.clearSim()
	assert(sim.loadStamp(cfg.save, 0, 0))
	sim.paused(true)
	tpt.setfpscap(2)
	local ax, ay
	for id in sim.parts() do
		if sim.partProperty(id, "type") == elem.DEFAULT_PT_QRTZ and sim.partProperty(id, "ctype") == 0x1864A205 then
			ax, ay = sim.partPosition(id)
			break
		end
	end
	assert(ax, "R3 anchor missing")
	local function prop(x, y, p)
		local id = sim.partID(x, y)
		assert(id, "missing particle " .. x .. "," .. y)
		return sim.partProperty(id, p or "ctype")
	end
	local function u32(v)
		return v < 0 and v + 4294967296 or v
	end
	local model = ""
	for i = 1, 20 do
		local c = prop(ax + i, ay)
		if c == 0 then
			break
		end
		model = model .. string.char(c)
	end
	local rows, cores = model:match("R3A(%d%d)(%d%d)")
	rows, cores = tonumber(rows), tonumber(cores)
	local meta = assert(io.open(cfg.out .. "/metadata.txt", "w"))
	meta:write("model=", model, "\nanchor=", ax, ",", ay, "\nrows=", rows, "\ncores=", cores, "\n")
	local types = ""
	for i = 0, cores - 1 do
		types = types .. string.char(prop(ax + 6, ay - cores * 6 - 4 + i * 6))
	end
	meta:write("core_types=", types, "\n")
	meta:close()
	assert(model == "R3A1610" and types == "msfffmsfff", "unexpected showcase topology")
	local function ramxy(i)
		return ax - 41 + i % 128, ay - 13 - rows - cores * 6 + math.floor(i / 128)
	end
	local function dump_ram(name)
		local f = assert(io.open(cfg.out .. "/" .. name, "wb"))
		for i = 0, rows * 128 - 1 do
			local x, y = ramxy(i)
			local w = u32(prop(x, y))
			f:write(
				string.char(
					w % 256,
					math.floor(w / 256) % 256,
					math.floor(w / 65536) % 256,
					math.floor(w / 16777216) % 256
				)
			)
		end
		f:close()
	end
	local function write_screen(p)
		for y = 138, 201 do
			for x = 342, 437 do
				local w = u32(prop(x, y, "dcolour"))
				p:write(string.char(math.floor(w / 65536) % 256, math.floor(w / 256) % 256, w % 256))
			end
		end
	end
	local function dump_screen(name)
		local p = assert(io.open(cfg.out .. "/" .. name, "wb"))
		p:write("P6\n96 64\n255\n")
		write_screen(p)
		p:close()
	end
	local screen_trace = cfg.screen_trace and assert(io.open(cfg.out .. "/screen-trace.rgb", "wb"))
	dump_ram("initial.bin")
	if cfg.rom then
		local f = assert(io.open(cfg.rom, "rb"))
		local data = f:read("*a")
		f:close()
		assert(#data <= rows * 128 * 4 and #data % 4 == 0)
		for i = 0, rows * 128 - 1 do
			local a, b, c, d = data:byte(i * 4 + 1, i * 4 + 4)
			local w = a and (a + b * 256 + c * 65536 + d * 16777216) or 0x20000000
			if w % 0x40000000 == 0 then
				w = w + 0x20000000
			end
			local x, y = ramxy(i)
			sim.partProperty(sim.partID(x, y), "ctype", w)
		end
	end
	dump_ram("loaded.bin")
	local f = assert(io.open(cfg.out .. "/trace.csv", "w"))
	f:write("frame,core,pc,state,flags,ram_addr,io_state")
	for j = 1, 31 do
		f:write(",r" .. j)
	end
	f:write("\n")
	local function capture(frame)
		for i = 0, cores - 1 do
			local y = ay - cores * 6 + i * 6
			f:write(
				frame,
				",",
				i,
				",",
				u32(prop(ax + 15, y)),
				",",
				u32(prop(ax + 11, y)),
				",",
				u32(prop(ax + 17, y)),
				",",
				u32(prop(ax + 87, y - 1)),
				",",
				u32(prop(ax + 85, y - 1))
			)
			for j = 1, 31 do
				f:write(",", u32(prop(ax - 3 - 2 * j, y - 1)))
			end
			f:write("\n")
		end
		f:flush()
	end
	local parts = assert(io.open(cfg.out .. "/initial-particles.csv", "w"))
	parts:write("id,x,y,type,ctype,life,tmp,tmp2,dcolour\n")
	for id in sim.parts() do
		local x, y = sim.partPosition(id)
		parts:write(id, ",", x, ",", y)
		for _, p in ipairs({ "type", "ctype", "life", "tmp", "tmp2", "dcolour" }) do
			parts:write(",", u32(sim.partProperty(id, p)))
		end
		parts:write("\n")
	end
	parts:close()
	capture(0)
	if screen_trace then
		write_screen(screen_trace)
	end
	local frame = 0
	local shots = {}
	local pending_shot, pending_start, finished
	for _, n in ipairs(cfg.shots or {}) do
		shots[n] = true
	end
	event.register(event.TICK, function()
		if pending_start then
			local x, y = ax + 35, ay + 1
			assert(sim.partProperty(sim.partID(x, y), "type") == elem.DEFAULT_PT_INST, "start button not INST")
			assert(sim.partCreate(-2, x, y, elem.DEFAULT_PT_SPRK) >= 0, "failed to spark start button")
			pending_start = false
		end
		if pending_shot then
			local path = tpt.screenshot(0, 0)
			if path then
				os.rename(path, cfg.out .. "/frame-" .. pending_shot .. ".png")
			end
			pending_shot = nil
		end
		if finished then
			sim.paused(true)
			os.exit(0)
		end
	end)
	event.register(event.AFTERSIM, function()
		if finished then
			return
		end
		local ok, err = pcall(function()
			frame = frame + 1
			capture(frame)
			if screen_trace then
				write_screen(screen_trace)
			end
			if frame == cfg.start then
				pending_start = true
			end
			if shots[frame] then
				pending_shot = frame
				dump_ram("ram-" .. frame .. ".bin")
				dump_screen("screen-" .. frame .. ".ppm")
			end
			if frame >= cfg.frames then
				dump_ram("final.bin")
				dump_screen("screen-final.ppm")
				f:close()
				if screen_trace then
					screen_trace:close()
				end
				finished = true
				local done = assert(io.open(cfg.out .. "/DONE", "w"))
				done:write(frame)
				done:close()
			end
		end)
		if not ok then
			local e = io.open(cfg.out .. "/ERROR", "w")
			e:write(tostring(err))
			e:close()
			os.exit(1)
		end
	end)
	sim.paused(false)
end
local ok, err = pcall(run)
if not ok then
	local e = io.open(cfg.out .. "/ERROR", "w")
	e:write(tostring(err))
	e:close()
	os.exit(1)
end
