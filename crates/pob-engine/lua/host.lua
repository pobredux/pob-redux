-- Headless SimpleGraphic host for pob-engine.
--
-- Path of Building is written against "SimpleGraphic", a small C++ host that
-- provides rendering, input, file search, compression and a few process
-- services as Lua globals. This file provides the same globals for a headless
-- process: rendering and input are no-ops, everything else delegates to Rust
-- through the `__native` table. Mirrors src/_SimpleGraphic.def.lua upstream.
--
-- Rust sets `__pob_root`, `__user_dir` and `__native` before running this.

local root, userDir, native = __pob_root, __user_dir, __native
local cache = native.cache

local function isAbsolute(p)
	local c = p:sub(1, 1)
	return c == "/" or c == "\\" or p:match("^%a:[/\\]") ~= nil
end

local function resolve(p)
	if type(p) ~= "string" or p == "" or isAbsolute(p) then
		return p
	end
	return root .. "/" .. p
end

-- PoB addresses its own files relative to the runtime's working directory.
-- Redirect relative paths to the vendored PoB root so the host process keeps
-- its own CWD. The vendored tree is read-only once installed, so writes into
-- it (PoB's runtime caches, e.g. Data/QueryMods.lua) land in a per-user
-- overlay directory instead; reads fall back to the overlay.
do
	local io_open, io_lines, os_remove, os_rename = io.open, io.lines, os.remove, os.rename
	local l_loadfile, l_dofile = loadfile, dofile
	local cacheDir = userDir .. "/pob-redux-cache"
	local function underRoot(abs)
		return abs:sub(1, #root) == root
	end
	local function cachePath(abs)
		return cacheDir .. abs:sub(#root + 1)
	end
	-- The C runtime words an open failure in the process locale (GTK sets it
	-- from the desktop), and PoB matches the English text to tell a missing
	-- Settings.xml from a broken one.
	local function openError(abs, err)
		if not native.file_exists(abs) then
			return abs .. ": No such file or directory"
		end
		return err
	end
	io.open = function(p, mode, ...)
		local abs = resolve(p)
		mode = mode or "r"
		if underRoot(abs) then
			if mode:find("[wa]") then
				local cp = cachePath(abs)
				native.make_dir(cp:match("^(.*)[/\\][^/\\]*$") or cacheDir)
				return io_open(cp, mode, ...)
			end
			local f, err = io_open(abs, mode, ...)
			if f then return f end
			local cf = io_open(cachePath(abs), mode, ...)
			if cf then return cf end
			return nil, openError(abs, err)
		end
		local f, err = io_open(abs, mode, ...)
		if f then return f end
		return nil, openError(abs, err)
	end
	io.lines = function(p, ...)
		local abs = resolve(p)
		if underRoot(abs) then
			local ok, it = pcall(io_lines, abs, ...)
			if ok then return it end
			return io_lines(cachePath(abs), ...)
		end
		return io_lines(abs, ...)
	end
	os.remove = function(p)
		local abs = resolve(p)
		local ok, err = os_remove(abs)
		if not ok and underRoot(abs) then
			return os_remove(cachePath(abs))
		end
		return ok, err
	end
	os.rename = function(a, b) return os_rename(resolve(a), resolve(b)) end
	local l_loadstring, dump = loadstring, string.dump
	local function compile(abs, ...)
		if not cache or not underRoot(abs) or select("#", ...) > 0 then
			return l_loadfile(abs, ...)
		end
		local code = cache.lookup(abs)
		local f = code and l_loadstring(code)
		if f then return f end
		local err
		f, err = l_loadfile(abs)
		if f then cache.store(abs, dump(f)) end
		return f, err
	end
	loadfile = function(p, ...)
		local abs = resolve(p)
		local f, err = compile(abs, ...)
		if f == nil and underRoot(abs) then
			local cf = l_loadfile(cachePath(abs), ...)
			if cf then return cf end
		end
		return f, err
	end
	dofile = function(p)
		local f, err = loadfile(p)
		if not f then error(err, 2) end
		return f()
	end
	package.path = table.concat({
		root .. "/?.lua", root .. "/?/init.lua",
		root .. "/lua/?.lua", root .. "/lua/?/init.lua",
	}, ";")
	package.cpath = ""
	-- require compiles through C; this loader sends PoB's modules through the cache.
	if cache then
		table.insert(package.loaders, 2, function(name)
			local path = package.searchpath(name, package.path)
			if not path then return end
			local f, err = loadfile(path)
			if not f then
				error(("error loading module '%s' from file '%s':\n\t%s"):format(name, path, err), 0)
			end
			return f
		end)
	end
end

-- Native modules shipped with the SimpleGraphic runtime. Network and profiling
-- libraries are simply absent; lua-utf8 gets a byte-wise shim because PoB only
-- uses it with ASCII separators in number formatting. lcurl.safe gets a
-- GET-only shim over native.http_get — enough for PoB's synchronous cache
-- fills (trade-stats download); everything else stays unsupported.
do
	local absent = {
		lcurl = true, lzip = true, socket = true, ["lua-profiler"] = true,
	}
	package.preload["lua-utf8"] = function()
		return setmetatable({}, { __index = string })
	end
	package.preload["lcurl.safe"] = function()
		local M = {}
		function M.easy()
			local h = { _post = false }
			local function noop() end
			return setmetatable({}, { __index = function(t, k)
				if k == "setopt_url" then return function(self, u) h._url = u end end
				if k == "setopt_useragent" then return function(self, ua) h._ua = ua end end
				if k == "setopt_writefunction" then return function(self, f) h._write = f end end
				if k == "perform" then
					return function(self)
						if h._post or not h._url then return nil, "only GET is supported headless" end
						local body, err = native.http_get(h._url, h._ua)
						if not body then return nil, err or "download failed" end
						if h._write then h._write(body) end
						return self
					end
				end
				if k == "close" then return noop end
				if k:match("^setopt_post") or k == "setopt_httppost" then
					return function(self, ...) h._post = true end
				end
				if k:match("^setopt") or k:match("^getinfo") then return noop end
				return nil
			end })
		end
		return M
	end
	local l_require = require
	function require(name)
		if absent[name] then
			return nil
		end
		return l_require(name)
	end
end

-- The lua CLI defines `arg`; Main.lua indexes it unguarded.
arg = arg or {}

-- ---------------------------------------------------------------------------
-- Callbacks / main object
-- ---------------------------------------------------------------------------
local callbackTable, mainObject = {}, nil
function runCallback(name, ...)
	if callbackTable[name] then
		return callbackTable[name](...)
	elseif mainObject and mainObject[name] then
		return mainObject[name](mainObject, ...)
	end
end
function SetCallback(name, func) callbackTable[name] = func end
function GetCallback(name) return callbackTable[name] end
function SetMainObject(obj) mainObject = obj end

-- ---------------------------------------------------------------------------
-- Console / logging
-- ---------------------------------------------------------------------------
function ConPrintf(fmt, ...)
	local ok, msg = pcall(string.format, tostring(fmt), ...)
	native.log("debug", ok and msg or tostring(fmt))
end
function ConPrintTable() end
function ConExecute() end
function ConClear() end
function print(...)
	local parts = {}
	for i = 1, select("#", ...) do
		parts[i] = tostring((select(i, ...)))
	end
	native.log("debug", table.concat(parts, "\t"))
end

-- ---------------------------------------------------------------------------
-- Rendering (no-ops) and images
-- ---------------------------------------------------------------------------
local imageHandleClass = {}
imageHandleClass.__index = imageHandleClass
function NewImageHandle() return setmetatable({}, imageHandleClass) end
function imageHandleClass:Load() self.valid = true end
function imageHandleClass:LoadArtRectangle() self.valid = true end
function imageHandleClass:LoadArtArcBand() self.valid = true end
function imageHandleClass:Unload() self.valid = false end
function imageHandleClass:IsValid() return self.valid == true end
function imageHandleClass:IsLoading() return false end
function imageHandleClass:SetLoadingPriority() end
function imageHandleClass:ImageSize() return 1, 1 end
function NewArtHandle() return { Size = function() return 1, 1 end } end

function RenderInit() end
function GetScreenSize() return 1920, 1080 end
function GetVirtualScreenSize() return 1920, 1080 end
function GetScreenScale() return 1 end
function GetDPIScaleOverridePercent() return 0 end
function SetDPIScaleOverridePercent() end
function SetClearColor() end
function SetDrawLayer() end
function GetDrawLayer() return 0 end
function SetViewport() end
function SetBlendMode() end
function SetDrawColor() end
function GetDrawColor() return 1, 1, 1, 1 end
function DrawImage() end
function DrawImageQuad() end
function DrawString() end
function DrawStringWidth(height, font, text) return 1 end
function DrawStringCursorIndex() return 0 end
function StripEscapes(text)
	local s = text:gsub("%^%d", ""):gsub("%^x%x%x%x%x%x%x", "")
	return s
end
function GetAsyncCount() return 0 end
function SetProfiling() end
function TakeScreenshot() end

-- ---------------------------------------------------------------------------
-- Input / window
-- ---------------------------------------------------------------------------
__window_title = ""
function SetWindowTitle(title) __window_title = title end
function GetCursorPos() return 0, 0 end
function SetCursorPos() end
function ShowCursor() end
function IsKeyDown() return false end
function SetForeground() end

-- Clipboard is owned by the UI; PoB's Copy/Paste go through these buffers.
__clipboard, __paste = nil, nil
function Copy(text) __clipboard = text end
function Paste()
	local t = __paste
	__paste = nil
	return t
end

-- ---------------------------------------------------------------------------
-- Compression
-- ---------------------------------------------------------------------------
function Deflate(data, isGzip)
	local r = native.deflate(data, isGzip and true or false)
	if r then return r end
	return nil, "deflate failed"
end
function Inflate(data)
	local r = native.inflate(data)
	if r then return r end
	return nil, "inflate failed"
end

-- ---------------------------------------------------------------------------
-- Time / process
-- ---------------------------------------------------------------------------
function GetTime() return native.time() end
function SpawnProcess() end
function OpenURL(url) __last_url = url end
function Restart() end
function Exit() end

-- ---------------------------------------------------------------------------
-- Filesystem
-- ---------------------------------------------------------------------------
function GetScriptPath() return root end
-- Differs from the script path on purpose: Main.lua treats equal paths as a
-- portable install and would put user data inside the resources directory.
function GetRuntimePath() return root .. "/runtime" end
function GetUserPath() return userDir end
function GetWorkDir() return root end
function SetWorkDir() end
function GetCloudProvider() return nil, nil, nil end
function MakeDir(path)
	local ok, err = native.make_dir(resolve(path))
	if ok then return true end
	return nil, err
end
function RemoveDir(path, recurse) native.remove_dir(resolve(path), recurse and true or false) end

local fileSearchClass = {}
fileSearchClass.__index = fileSearchClass
function fileSearchClass:NextFile()
	self.i = self.i + 1
	return self.i <= #self.list
end
function fileSearchClass:GetFileName() return self.list[self.i].name end
function fileSearchClass:GetFileSize() return self.list[self.i].size end
function fileSearchClass:GetFileModifiedTime() return self.list[self.i].mtime end
function NewFileSearch(spec, findDirectories)
	local list = native.file_search(resolve(spec), findDirectories and true or false)
	if not list then return nil end
	return setmetatable({ list = list, i = 1 }, fileSearchClass)
end

-- ---------------------------------------------------------------------------
-- Sub-scripts (PoB uses these for network I/O and update checks; none here)
-- ---------------------------------------------------------------------------
function LaunchSubScript() return nil end
function AbortSubScript() end
function IsSubScriptRunning() return false end

-- ---------------------------------------------------------------------------
-- Module loading
-- ---------------------------------------------------------------------------
-- Data/ModCache.lua lacks some lines every boot parses (rune mods, mastery effects), so their parses are kept between runs.
local bootMods
local function withBootMods(base)
	bootMods = { base = base, sources = cache.fingerprint(), lines = { } }
	local code = cache.read("mods.lua")
	local f = code and loadstring(code, "=mods")
	if f then
		local ok, sources, lines = pcall(f)
		if ok and sources == bootMods.sources then
			bootMods.lines = lines
			for line, entry in pairs(lines) do
				if entry then base[line] = entry end
			end
		end
	end
	return base
end
local function serialise(v, out, depth)
	local t = type(v)
	if t == "table" and depth < 16 then
		out[#out + 1] = "{"
		for k, x in pairs(v) do
			out[#out + 1] = "["
			serialise(k, out, depth + 1)
			out[#out + 1] = "]="
			serialise(x, out, depth + 1)
			out[#out + 1] = ","
		end
		out[#out + 1] = "}"
	elseif t == "string" then
		out[#out + 1] = ("%q"):format(v)
	elseif t == "number" and v == v and v ~= math.huge and v ~= -math.huge then
		out[#out + 1] = ("%.17g"):format(v)
	elseif t == "boolean" then
		out[#out + 1] = tostring(v)
	else
		error("not plain data")
	end
end
local function saveBootMods()
	local st = bootMods
	bootMods = nil
	if not st or not modLib or not modLib.parseModCache then return end
	local base, lines, found = st.base, st.lines, false
	for line, entry in pairs(modLib.parseModCache) do
		if base[line] == nil and lines[line] == nil then
			lines[line] = entry
			found = true
		end
	end
	if not found then return end
	local out, count = { "local c={}\n(function()\n" }, 0
	for line, entry in pairs(lines) do
		local part = { }
		-- A parse that holds a function cannot be written; false stops it counting as new next boot.
		local text = entry and pcall(serialise, entry, part, 0) and table.concat(part) or "false"
		count = count + 1
		-- LuaJIT limits the constants in one function.
		if count % 500 == 0 then out[#out + 1] = "end)()\n(function()\n" end
		out[#out + 1] = ("c[%q]=%s\n"):format(line, text)
	end
	out[#out + 1] = ("end)()\nreturn %q,c\n"):format(st.sources)
	cache.write("mods.lua", table.concat(out))
end

local function replacePlain(s, from, to)
	local i, j = s:find(from, 1, true)
	return i and (s:sub(1, i - 1) .. to .. s:sub(j + 1)) or s
end

-- pob-sync can add trees (trees/<game>/) that GameVersions.lua does not list yet. Each copies the newest listed entry with the same suffix.
local function registerAddedTrees()
	local added = { }
	local search = NewFileSearch("TreeData/*", true)
	while search do
		local name = search:GetFileName()
		local major, minor, variant = name:match("^(%d+)_(%d+)(.*)$")
		local file = major and not treeVersions[name] and io.open("TreeData/" .. name .. "/tree.lua", "r")
		if file then
			file:close()
			table.insert(added, { name = name, major = tonumber(major), minor = tonumber(minor), variant = variant })
		end
		if not search:NextFile() then break end
	end
	table.sort(added, function(a, b)
		if a.major ~= b.major then return a.major < b.major end
		if a.minor ~= b.minor then return a.minor < b.minor end
		if (a.variant == "") ~= (b.variant == "") then return b.variant == "" end
		return a.variant < b.variant
	end)
	for _, tree in ipairs(added) do
		local entry = { display = tree.name, num = tree.major + tree.minor / 100 }
		for i = #treeVersionList, 1, -1 do
			local prevName = treeVersionList[i]
			local pMajor, pMinor, pVariant = prevName:match("^(%d+)_(%d+)(.*)$")
			local prev = treeVersions[prevName]
			if pVariant == tree.variant and prev and tonumber(pMinor) > 0 then
				local step = (prev.num - tonumber(pMajor)) / tonumber(pMinor)
				entry.num = math.floor((tree.major + tree.minor * step) * 1e6 + 0.5) / 1e6
				local oldDot, newDot = pMajor .. "." .. pMinor, tree.major .. "." .. tree.minor
				entry.display = replacePlain(replacePlain(prev.display, pMajor .. "_" .. pMinor, tree.major .. "_" .. tree.minor), oldDot, newDot)
				entry.url = prev.url and replacePlain(prev.url, oldDot, newDot)
				break
			end
		end
		treeVersions[tree.name] = entry
		table.insert(treeVersionList, tree.name)
	end
	latestTreeVersion = treeVersionList[#treeVersionList]
end

function LoadModule(name, ...)
	if not name:match("%.lua$") then
		name = name .. ".lua"
	end
	local func, err = loadfile(name)
	if not func then
		error("LoadModule() error loading '" .. name .. "': " .. tostring(err))
	end
	if cache and name == "Data/ModCache.lua" then
		return withBootMods(func(...))
	end
	if name == "GameVersions.lua" then
		func(...)
		return registerAddedTrees()
	end
	return func(...)
end
function PLoadModule(name, ...)
	if not name:match("%.lua$") then
		name = name .. ".lua"
	end
	local func, err = loadfile(name)
	if not func then
		error("PLoadModule() error loading '" .. name .. "': " .. tostring(err))
	end
	return PCall(func, ...)
end
function PCall(func, ...)
	local ret = { xpcall(func, function(e) return debug.traceback(tostring(e), 2) end, ...) }
	if ret[1] then
		table.remove(ret, 1)
		return nil, unpack(ret)
	end
	return ret[2]
end

-- ---------------------------------------------------------------------------
-- Upstream fixes
-- ---------------------------------------------------------------------------
-- PoB's copyTableSafe sets the copy's metatable before filling it. A class
-- object's parent proxy (ModList.ModStore) is its own metatable with
-- `__newindex` aimed at the object, so filling the proxy's copy writes the
-- copy's fields into the ORIGINAL object instead. SwitchAttributeNode deep
-- copies a shared tree node on every build load, so the tree's ModLists end up
-- pointing at each previous copy and the next copy carries all of them: an
-- engine that reloads builds grows without bound. Fill first, then set the
-- metatable; a self-metatable table's copy becomes its own metatable.
local function fixCopyTableSafe()
	local subTableMap = {}
	local function copy(tbl, noRecurse, preserveMeta, isSubTable)
		local out = {}
		if not noRecurse then
			subTableMap[tbl] = out
		end
		for k, v in pairs(tbl) do
			if not noRecurse and type(v) == "table" then
				out[k] = subTableMap[v] or copy(v, false, preserveMeta, true)
			else
				out[k] = v
			end
		end
		if preserveMeta then
			local mt = getmetatable(tbl)
			if mt ~= nil then
				setmetatable(out, mt == tbl and out or mt)
			end
		end
		if not noRecurse and not isSubTable then
			for k in pairs(subTableMap) do
				subTableMap[k] = nil
			end
		end
		return out
	end
	copyTableSafe = copy
end

-- ---------------------------------------------------------------------------
-- Boot
-- ---------------------------------------------------------------------------
function __pob_boot()
	dofile("Launch.lua")
	runCallback("OnInit")
	if launch.promptMsg then
		error("PoB startup failed: " .. tostring(launch.promptMsg), 0)
	end
	fixCopyTableSafe()
	-- Settings.xml may queue "reopen the last build"; the app manages builds itself.
	launch.main:SetMode("BUILD", false, "Unnamed build")
	-- PoB parses the item database a slice per frame; boot leaves every slice for later.
	local frameFuncs = launch.main.onFrameFuncs
	local loadItems = frameFuncs.LoadItems
	frameFuncs.LoadItems = nil
	runCallback("OnFrame")
	frameFuncs.LoadItems = loadItems
	if launch.promptMsg then
		error("PoB startup failed: " .. tostring(launch.promptMsg), 0)
	end
	if cache then
		saveBootMods()
		cache.finish()
		cache = nil
	end
end
