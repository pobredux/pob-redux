-- JSON-facing method table over the live Path of Building state.
--
-- Every method takes one table of params (or nil) and returns a JSON-friendly
-- table. Arrays must be marked with `array()` so empty ones serialise as [].
-- Methods raise Lua errors for bad input; Rust turns those into error strings.
--
-- The method set started from pob-mcp's lua/pob_bridge.lua (same author, MIT).

local native = __native
local array, null = native.array, native.null
local dkjson = require("dkjson")

local main = launch.main
local build = main.modes["BUILD"]
main.__reduxBuildGeneration = 0

-- PoB's sidebar rows carry only text. Feeding AddDisplayStatList one entry at
-- a time tags every row it appends with the entry's stat key and actor, which
-- is what the UI groups on; PoB's own spacer logic is unchanged by the split.
do
	local addStats = build.AddDisplayStatList
	build.AddDisplayStatList = function(self, statList, actor, actorName)
		local list = self.controls.statBox.list
		for _, statData in ipairs(statList) do
			local before = #list
			addStats(self, { statData }, actor, actorName)
			for i = before + 1, #list do
				list[i].stat = statData.stat or statData.labelStat
				list[i].actor = actorName
			end
		end
	end
end

local function frame()
	runCallback("OnFrame")
end

-- PoB's Calcs tab keeps its own copy of every skill selection (`input.skill_number`
-- and the `...Calcs` fields) and the CALCS pass reads only those. The app has no
-- separate Calcs selector, so the copies follow the main selections.
local CALCS_TWINS = {
	skillPart = "skillPartCalcs",
	skillStageCount = "skillStageCountCalcs",
	skillMineCount = "skillMineCountCalcs",
	skillMinion = "skillMinionCalcs",
	skillMinionItemSet = "skillMinionItemSetCalcs",
	skillMinionSkill = "skillMinionSkillCalcs",
	skillMinionSkillStatSetIndexLookup = "skillMinionSkillStatSetIndexLookupCalcs",
	statSet = "statSetCalcs",
}

local function syncCalcsSelection()
	if not build or not build.calcsTab or not build.skillsTab then return false end
	local changed = false
	-- PoB reloads the table-valued twins in another shape, so only a stat set
	-- choice that is not the default counts as a change for them.
	local function set(t, key, value)
		if t[key] == value then return end
		if type(value) == "table" then
			if key == "statSetCalcs" then
				for id, idx in pairs(value) do
					if idx ~= 1 and (type(t[key]) ~= "table" or t[key][id] ~= idx) then changed = true end
				end
			end
		else
			changed = true
		end
		t[key] = value
	end
	set(build.calcsTab.input, "skill_number", build.mainSocketGroup or 1)
	for _, group in ipairs(build.skillsTab.socketGroupList or {}) do
		set(group, "mainActiveSkillCalcs", group.mainActiveSkill or 1)
		for _, gem in ipairs(group.gemList or {}) do
			for main, twin in pairs(CALCS_TWINS) do set(gem, twin, gem[main]) end
		end
	end
	return changed
end

-- Mark the build dirty and run one frame: PoB rebuilds calc output, the
-- sidebar stat list and dependent tab state inside OnFrame.
local function refresh()
	syncCalcsSelection()
	build.buildFlag = true
	build.modFlag = true
	frame()
end

local function ensureBuild(p)
	if p and p.generation ~= nil and p.generation ~= main.__reduxBuildGeneration then
		error("the build changed; paste the item again", 0)
	end
	if not build or not build.calcsTab or not build.calcsTab.mainOutput then
		error("no build is loaded; call new_build, load_build_xml or load_build_code first", 0)
	end
end

-- PoB's calculator repeats the Full DPS pass (one calc per included skill)
-- for every candidate while the build is in tree view, whatever the caller
-- asked for. Scoring that reads any other stat runs with the view switched
-- away for the duration.
local function withoutFullDPS(fn, ...)
	local mode = build.viewMode
	build.viewMode = "CALCS"
	local ok, a, b = pcall(fn, ...)
	build.viewMode = mode
	if not ok then
		error(a, 0)
	end
	return a, b
end

local function isScalar(v)
	local t = type(v)
	return t == "number" or t == "string" or t == "boolean"
end

local function opt(v)
	if v == nil then return null end
	return v
end

local function strArray(t)
	local out = array({})
	for i, v in ipairs(t or {}) do out[i] = tostring(v) end
	return out
end

-- A PoB tooltip as sized, colour-coded lines. `header` names the rarity art
-- PoB would frame it with (UNIQUE, RARE, MAGIC, NORMAL, RELIC, GEM); `font`
-- is set on the lines PoB draws in the game font ("FONTIN SC").
local function tooltipPayload(tt)
	local lines = array({})
	for _, l in ipairs(tt.lines) do
		lines[#lines + 1] = {
			size = l.size or 14,
			text = l.text or "",
			center = l.center == true,
			sep = (l.separatorImage ~= nil or l.text == nil) and true or false,
			font = opt(l.font),
		}
	end
	return {
		lines = lines,
		header = tt.tooltipHeader and tostring(tt.tooltipHeader):upper() or null,
		runic = tt.runicItem ~= nil,
		uniqueGem = tt.isUniqueGem == true,
	}
end

-- Which game this PoB is for. The two forks share their class and tab layout;
-- the differences the bridge has to bracket are keyed on this.
local GAME = (tostring(APP_NAME or ""):find("PoE2", 1, true) or tostring(liveTargetVersion or ""):match("^0_")) and "poe2" or "poe1"
local IS_POE2 = GAME == "poe2"

-- PoE1 keeps set copy/rename/delete inside its list controls and has no
-- loadout API; PoE2's methods are supplied here so the rest of the bridge is
-- the same for both games. Mirrors ItemSetListControl.lua,
-- SkillSetListControl.lua, ConfigSetListControl.lua, TreeTab.lua and the
-- loadout dropdown in Build.lua (a loadout is the sets that share a name).
if not IS_POE2 then
	local classes = common.classes
	local function freeId(sets)
		local id = 1
		while sets[id] do id = id + 1 end
		return id
	end
	local function orderIndex(list, id)
		for i, v in ipairs(list) do if v == id then return i end end
	end
	local function copyTitle(set, title)
		return title or ((set.title or "Default") .. " (Copy)")
	end
	local ItemsTab = classes.ItemsTab
	if ItemsTab and not ItemsTab.CopyItemSet then
		function ItemsTab:CopyItemSet(sourceId, title)
			local src = self.itemSets[sourceId]
			local newSet = copyTable(src)
			newSet.id = freeId(self.itemSets)
			newSet.title = copyTitle(src, title)
			self.itemSets[newSet.id] = newSet
			table.insert(self.itemSetOrderList, newSet.id)
			self.modFlag = true
			self.build:SyncLoadouts()
			return newSet
		end
		function ItemsTab:RenameItemSet(id, title)
			self.itemSets[id].title = title
			self.modFlag = true
			self.build:SyncLoadouts()
		end
		function ItemsTab:DeleteItemSet(id, index)
			index = index or orderIndex(self.itemSetOrderList, id)
			table.remove(self.itemSetOrderList, index)
			self.itemSets[id] = nil
			if id == self.activeItemSetId then
				self:SetActiveItemSet(self.itemSetOrderList[math.max(1, index - 1)])
			end
			self:AddUndoState()
			self.build:SyncLoadouts()
		end
	end
	local SkillsTab = classes.SkillsTab
	if SkillsTab and not SkillsTab.CopySkillSet then
		function SkillsTab:CopySkillSet(sourceId, title)
			local src = self.skillSets[sourceId]
			local newSet = copyTable(src, true)
			newSet.socketGroupList = {}
			for _, group in ipairs(src.socketGroupList) do
				local newGroup = copyTable(group, true)
				newGroup.gemList = {}
				for gi, gem in ipairs(group.gemList) do newGroup.gemList[gi] = copyTable(gem, true) end
				table.insert(newSet.socketGroupList, newGroup)
			end
			newSet.id = freeId(self.skillSets)
			newSet.title = copyTitle(src, title)
			self.skillSets[newSet.id] = newSet
			table.insert(self.skillSetOrderList, newSet.id)
			self.modFlag = true
			self.build:SyncLoadouts()
			return newSet
		end
		function SkillsTab:RenameSkillSet(id, title)
			self.skillSets[id].title = title
			self.modFlag = true
			self.build:SyncLoadouts()
		end
		function SkillsTab:DeleteSkillSet(id, index)
			index = index or orderIndex(self.skillSetOrderList, id)
			table.remove(self.skillSetOrderList, index)
			self.skillSets[id] = nil
			if id == self.activeSkillSetId then
				self:SetActiveSkillSet(self.skillSetOrderList[math.max(1, index - 1)])
			end
			self:AddUndoState()
			self.build:SyncLoadouts()
		end
	end
	local ConfigTab = classes.ConfigTab
	if ConfigTab and not ConfigTab.CopyConfigSet then
		function ConfigTab:CopyConfigSet(sourceId, title)
			local src = self.configSets[sourceId]
			local newSet = copyTable(src)
			newSet.id = freeId(self.configSets)
			newSet.title = copyTitle(src, title)
			self.configSets[newSet.id] = newSet
			table.insert(self.configSetOrderList, newSet.id)
			self.modFlag = true
			self.build:SyncLoadouts()
			return newSet
		end
		function ConfigTab:RenameConfigSet(id, title)
			self.configSets[id].title = title
			self.modFlag = true
			self.build:SyncLoadouts()
		end
		function ConfigTab:DeleteConfigSet(id, index)
			index = index or orderIndex(self.configSetOrderList, id)
			table.remove(self.configSetOrderList, index)
			self.configSets[id] = nil
			if id == self.activeConfigSetId then
				self:SetActiveConfigSet(self.configSetOrderList[math.max(1, index - 1)])
			end
			self:AddUndoState()
			self.build:SyncLoadouts()
		end
	end
	local TreeTab = classes.TreeTab
	if TreeTab and not TreeTab.CopyTree then
		function TreeTab:CopyTree(sourceId, title)
			local src = self.specList[sourceId]
			local newSpec = new("PassiveSpec"):PassiveSpec(self.build, src.treeVersion)
			newSpec.title = copyTitle(src, title)
			newSpec.jewels = copyTable(src.jewels)
			newSpec:RestoreUndoState(src:CreateUndoState())
			newSpec:BuildClusterJewelGraphs()
			table.insert(self.specList, newSpec)
			self.modFlag = true
			self.build:SyncLoadouts()
			return newSpec
		end
	end
	if not build.GetLoadoutByName then
		local function setByTitle(orderList, sets, name)
			for _, id in ipairs(orderList) do
				if (sets[id].title or "Default") == name then return id end
			end
		end
		local function loadoutNames(self)
			local names = {}
			for _, entry in ipairs(self.controls.buildLoadouts.list) do
				if type(entry) == "string" and not entry:match("^%^7%^7") and entry ~= "No Loadouts" then names[#names + 1] = entry end
			end
			return names
		end
		function build:GetLoadoutByName(name)
			local specId
			for i, spec in ipairs(self.treeTab.specList) do
				if (spec.title or "Default") == name then specId = i break end
			end
			if not specId then return nil end
			return {
				name = name,
				specId = specId,
				itemSetId = setByTitle(self.itemsTab.itemSetOrderList, self.itemsTab.itemSets, name),
				skillSetId = setByTitle(self.skillsTab.skillSetOrderList, self.skillsTab.skillSets, name),
				configSetId = setByTitle(self.configTab.configSetOrderList, self.configTab.configSets, name),
			}
		end
		function build:SetActiveLoadout(lo)
			if not lo or not lo.specId then return end
			if lo.specId ~= self.treeTab.activeSpec then self.treeTab:SetActiveSpec(lo.specId) end
			if lo.itemSetId and lo.itemSetId ~= self.itemsTab.activeItemSetId then self.itemsTab:SetActiveItemSet(lo.itemSetId) end
			if lo.skillSetId and lo.skillSetId ~= self.skillsTab.activeSkillSetId then self.skillsTab:SetActiveSkillSet(lo.skillSetId) end
			if lo.configSetId and lo.configSetId ~= self.configTab.activeConfigSetId then self.configTab:SetActiveConfigSet(lo.configSetId) end
			self:SyncLoadouts()
			self.activeLoadout = nil
			for i, n in ipairs(loadoutNames(self)) do
				if n == lo.name then self.activeLoadout = i end
			end
		end
		function build:NewLoadout(name)
			local newSpec = new("PassiveSpec"):PassiveSpec(self, latestTreeVersion)
			newSpec.title = name
			table.insert(self.treeTab.specList, newSpec)
			local itemSet = self.itemsTab:NewItemSet()
			itemSet.title = name
			table.insert(self.itemsTab.itemSetOrderList, itemSet.id)
			local skillSet = self.skillsTab:NewSkillSet()
			skillSet.title = name
			table.insert(self.skillsTab.skillSetOrderList, skillSet.id)
			local configSet = self.configTab:NewConfigSet(nil, name)
			table.insert(self.configTab.configSetOrderList, configSet.id)
			self:SyncLoadouts()
			self:SetActiveLoadout(self:GetLoadoutByName(name))
			self.modFlag = true
		end
		function build:CopyLoadout(sourceName, name)
			local lo = self:GetLoadoutByName(sourceName)
			if not lo then return end
			self.treeTab:CopyTree(lo.specId, name)
			self.itemsTab:CopyItemSet(lo.itemSetId or self.itemsTab.itemSetOrderList[1], name)
			self.skillsTab:CopySkillSet(lo.skillSetId or self.skillsTab.skillSetOrderList[1], name)
			self.configTab:CopyConfigSet(lo.configSetId or self.configTab.configSetOrderList[1], name)
			self:SetActiveLoadout(self:GetLoadoutByName(name))
			self.modFlag = true
		end
		function build:RenameLoadout(oldName, newName)
			local lo = self:GetLoadoutByName(oldName)
			if not lo then return end
			self.treeTab.specList[lo.specId].title = newName
			if lo.itemSetId then self.itemsTab:RenameItemSet(lo.itemSetId, newName) end
			if lo.skillSetId then self.skillsTab:RenameSkillSet(lo.skillSetId, newName) end
			if lo.configSetId then self.configTab:RenameConfigSet(lo.configSetId, newName) end
			self.modFlag = true
		end
		function build:DeleteLoadout(name, nextName)
			local lo = self:GetLoadoutByName(name)
			if not lo then return end
			if #self.treeTab.specList > 1 then
				table.remove(self.treeTab.specList, lo.specId)
				if self.treeTab.activeSpec > #self.treeTab.specList then self.treeTab:SetActiveSpec(#self.treeTab.specList) end
			end
			if lo.itemSetId and #self.itemsTab.itemSetOrderList > 1 then self.itemsTab:DeleteItemSet(lo.itemSetId) end
			if lo.skillSetId and #self.skillsTab.skillSetOrderList > 1 then self.skillsTab:DeleteSkillSet(lo.skillSetId) end
			if lo.configSetId and #self.configTab.configSetOrderList > 1 then self.configTab:DeleteConfigSet(lo.configSetId) end
			self.modFlag = true
			self:SetActiveLoadout(self:GetLoadoutByName(nextName))
		end
	end
end

-- PoE1's CountAllocNodes stops at the socket count; the weapon-set counts are PoE2's.
local function countAllocNodes(spec)
	local used, ascUsed, secAscUsed, sockets, ws1, ws2 = spec:CountAllocNodes()
	return used or 0, ascUsed or 0, secAscUsed or 0, sockets or 0, ws1 or 0, ws2 or 0
end

-- PoB's gem data has no character level requirement (grantedEffect levels all
-- report levelRequirement 0), only a `Tier`. This is the tier -> level ladder
-- and the base support socket count that comes with it.
local GEM_TIER_LEVEL = { 1, 3, 6, 10, 14, 18, 22, 26, 31, 36, 41, 46, 52, 58, 64, 66, 72, 78, 84, 90 }

local function gemReqLevel(tier)
	return tier and GEM_TIER_LEVEL[tier] or nil
end

local function gemBaseSockets(tier)
	if not tier or tier < 1 then return nil end
	if tier >= 20 then return 5 end
	if tier >= 15 then return 4 end
	if tier >= 10 then return 3 end
	return 2
end

local ATTR_BY_COLOR = { [1] = "Str", [2] = "Dex", [3] = "Int" }

-- The attribute a gem's colour stands for. Supports have no requirement of
-- their own; each one socketed adds 5 to a build-wide source for its colour.
local function gemAttr(gemData)
	local ge = gemData and gemData.grantedEffect
	return ge and ATTR_BY_COLOR[ge.color] or nil
end

-- The highest gem level the character level allows: gem levels carry their
-- own character level requirement (grantedEffect.levels[i].levelRequirement).
local function usableGemLevel(gemData, charLevel)
	local ge = gemData and gemData.grantedEffect
	local max = gemData and gemData.naturalMaxLevel or 1
	if not ge or not ge.levels then return math.max(1, max) end
	local best = 1
	for i = 1, math.max(1, max) do
		local lv = ge.levels[i]
		if lv and (lv.levelRequirement or 0) <= (charLevel or 100) then best = i end
	end
	return best
end

-- What a gem asks of the character at one gem level, by PoB's own formula.
-- gemData.reqStr/reqDex/reqInt are attribute weightings (100 = pure), not
-- requirements; the requirement comes from the gem level's level requirement.
-- PoE1's calcLib takes (level, isSupport, multi); PoE2's (level, multi, isSupport).
local function gemStatRequirement(level, multi, isSupport)
	if IS_POE2 then return calcLib.getGemStatRequirement(level, multi, isSupport) end
	return calcLib.getGemStatRequirement(level, isSupport, multi)
end

local function gemRequirements(gemData, gemLevel)
	local ge = gemData and gemData.grantedEffect
	local lv = ge and ge.levels and ge.levels[gemLevel]
	local charLevel = lv and lv.levelRequirement or 0
	local isSupport = ge and ge.support or false
	return {
		level = charLevel,
		str = gemStatRequirement(charLevel, gemData.reqStr or 0, isSupport),
		dex = gemStatRequirement(charLevel, gemData.reqDex or 0, isSupport),
		int = gemStatRequirement(charLevel, gemData.reqInt or 0, isSupport),
	}
end

-- Skills the game hands out rather than sells as gems: default weapon attacks
-- (Mace Strike, Bow Shot), Raise Shield, and skills unique items grant. PoB
-- gives them tier 0 because no uncut gem makes them. They can still carry
-- supports, so the game lists them among the character's skills.
local function grantedGemNote(gemData)
	if not gemData or gemData.Tier ~= 0 then return nil end
	local ge = gemData.grantedEffect
	if ge and ge.support then return nil end
	local variant = gemData.variantId or ""
	if variant:find("^PlayerDefault") then
		return "default attack of " .. tostring(gemData.weaponRequirements or "the weapon") .. "; comes with the weapon, not a gem"
	end
	if gemData.weaponRequirements then
		return "comes with " .. tostring(gemData.weaponRequirements) .. ", not a gem"
	end
	return "granted by an item or effect, not a gem"
end

-- Where an item-granted socket group comes from. Such a group cannot be
-- removed or re-socketed; it leaves with the item.
local function grantedBy(group)
	if not group.source then return null end
	local src = tostring(group.source)
	local item = group.sourceItem
	local itemName = item and (item.name or item.title) or src:match("^Item:%d+:(.+)$")
	local node = group.sourceNode
	local nodeName = node and node.dn or node and node.name
	if not nodeName and src:match("^Tree:") then
		local n = build.spec and build.spec.nodes[tonumber(src:match("^Tree:(%d+)"))]
		nodeName = n and (n.dn or n.name)
	end
	-- PoB also synthesises groups for mechanics ("Thorns", "Explode") so it can
	-- show their damage; those have no item or node behind them.
	local kind = itemName and "item" or nodeName and "node" or "mechanic"
	return { kind = kind, item = opt(itemName), node = opt(nodeName), slot = opt(group.slot), source = src }
end

local function requirementSummary()
	local o = build.calcsTab.mainOutput or {}
	local out = {}
	for _, attr in ipairs({ "Str", "Dex", "Int" }) do
		local need = o["Req" .. attr] or 0
		local have = o[attr] or 0
		local src = o["Req" .. attr .. "Item"]
		local from = null
		if type(src) == "table" then
			if src.source == "Item" and src.sourceItem then
				from = string.format("item %s (%s)", src.sourceItem.name or "?", src.sourceSlot or "")
			elseif src.source == "Gem" and src.sourceGem then
				from = string.format("gem %s %d", src.sourceGem.nameSpec or "?", src.sourceGem.level or 0)
			elseif src.source == "Support Gems" then
				from = string.format("%d %s support gems (5 each, one shared source)", math.floor((src[attr] or 0) / 5), attr)
			else
				from = tostring(src.source)
			end
		end
		out[attr:lower()] = { need = need, have = have, met = have >= need, from = from }
	end
	return out
end

-- Skills that need no keypress once set up: persistent buffs stay on, triggers
-- and meta gems fire from their own condition. Warcries carry PoB's `trigger`
-- tag because they exert attacks, but the player still presses them.
local function gemPressClass(gemData)
	local tags = gemData and gemData.tags
	if not tags then return "active" end
	if tags.warcry then return "active" end
	if tags.meta then return "meta" end
	if tags.trigger then return "trigger" end
	if tags.persistent then return "persistent" end
	return "active"
end

-- Quest passive points are campaign progress, not level, so this is the total
-- available once an act is finished. PoE2 0.5.5: 4 per act plus 8 across the
-- interludes; acts 5 and 6 replace the interludes at 1.0. PoE1: PoB's own act
-- table (Build.lua), 23 points over ten acts, the bandit reward included.
local QUEST_POINTS_BY_ACT = { 4, 8, 12, 16 }
local QUEST_POINTS_MAX = 24
local POE1_ACTS = {
	{ level = 1, questPoints = 0 }, { level = 12, questPoints = 2 }, { level = 22, questPoints = 4 },
	{ level = 32, questPoints = 6 }, { level = 40, questPoints = 7 }, { level = 44, questPoints = 9 },
	{ level = 50, questPoints = 12 }, { level = 54, questPoints = 15 }, { level = 60, questPoints = 18 },
	{ level = 64, questPoints = 20 }, { level = 67, questPoints = 23 },
}

local function questPointsForLevel(level)
	if not IS_POE2 then
		local act = 1
		while POE1_ACTS[act + 1] and level >= POE1_ACTS[act + 1].level do act = act + 1 end
		return POE1_ACTS[act].questPoints, POE1_ACTS[math.min(act + 1, #POE1_ACTS)].questPoints
	end
	-- Act boundaries by level, used only to bracket the budget when the caller
	-- has not said how far through the campaign they are.
	local act = 0
	if level >= 60 then return QUEST_POINTS_MAX, QUEST_POINTS_MAX end
	if level >= 45 then act = 4 elseif level >= 32 then act = 3 elseif level >= 20 then act = 2 elseif level >= 12 then act = 1 end
	local low = act > 0 and QUEST_POINTS_BY_ACT[act] or 0
	local high = QUEST_POINTS_BY_ACT[math.min(act + 1, 4)] or QUEST_POINTS_MAX
	return low, high
end

local function decodeCode(code)
	code = code:gsub("%s+", ""):gsub("-", "+"):gsub("_", "/")
	local ok, decoded = pcall(common.base64.decode, code)
	if not ok or not decoded then
		error("failed to base64-decode build code", 0)
	end
	local xml = Inflate(decoded)
	if not xml or xml == "" then
		error("build code did not inflate to XML", 0)
	end
	return xml
end

local function encodeCode(xml)
	local deflated = Deflate(xml)
	if not deflated or deflated == "" then
		error("deflate failed", 0)
	end
	return common.base64.encode(deflated):gsub("+", "-"):gsub("/", "_")
end

local M = {}

-- ---------------------------------------------------------------------------
-- Meta
-- ---------------------------------------------------------------------------

M.ping = function()
	return { ok = true, buildLoaded = (build.calcsTab ~= nil and build.calcsTab.mainOutput ~= nil) }
end

-- Lua heap in MB, as it stands; `gc` runs a full collection first.
M.mem = function()
	return { mb = math.floor(collectgarbage("count") / 1024) }
end

M.gc = function()
	collectgarbage("collect")
	return M.mem()
end

M.version = function()
	return {
		game = GAME,
		pobVersion = launch.versionNumber,
		pobBranch = opt(launch.versionBranch),
		treeVersions = strArray(treeVersionList),
		latestTreeVersion = latestTreeVersion,
		liveTargetVersion = liveTargetVersion,
		userPath = main.userPath,
		buildPath = main.buildPath,
	}
end

M.take_clipboard = function()
	local t = __clipboard
	__clipboard = nil
	return { text = opt(t) }
end

M.set_paste = function(p)
	__paste = p and p.text or nil
	return { ok = true }
end

M.refresh = function()
	ensureBuild()
	refresh()
	return { rev = build.outputRevision }
end

-- ---------------------------------------------------------------------------
-- Build lifecycle
-- ---------------------------------------------------------------------------

-- A file saved by PoB carries its own Calcs selections; one more pass brings
-- them in line before anything reads the CALCS output.
local function loaded()
	main.__reduxBuildGeneration = main.__reduxBuildGeneration + 1
	build = main.modes["BUILD"]
	ensureBuild()
	if syncCalcsSelection() then refresh() end
	return M.get_build()
end

M.new_build = function(p)
	main:SetMode("BUILD", false, (p and p.name) or "Unnamed build")
	frame()
	return loaded()
end

M.load_build_xml = function(p)
	if not p or type(p.xml) ~= "string" or p.xml == "" then
		error("params.xml is required", 0)
	end
	-- `path` keeps the build attached to its file, for a snapshot of a saved
	-- build restored at startup.
	local path = type(p.path) == "string" and p.path ~= "" and p.path or false
	main:SetMode("BUILD", path, p.name or "Imported build", p.xml)
	frame()
	return loaded()
end

M.load_build_code = function(p)
	if not p or type(p.code) ~= "string" or p.code == "" then
		error("params.code is required", 0)
	end
	return M.load_build_xml({ xml = decodeCode(p.code), name = p.name })
end

-- Which game a share code is for, without loading it, so the app can switch
-- game first when a code from the other one is pasted.
M.code_game = function(p)
	if not p or type(p.code) ~= "string" or p.code == "" then error("params.code is required", 0) end
	local xml = decodeCode(p.code)
	local game = null
	if xml then
		if xml:find("<PathOfBuilding2[%s>]") then game = "poe2"
		elseif xml:find("<PathOfBuilding[%s>]") then game = "poe1" end
	end
	return { game = game }
end

M.load_build_file = function(p)
	if not p or type(p.path) ~= "string" then
		error("params.path is required", 0)
	end
	local name = p.path:match("([^/\\]+)%.xml$") or p.path:match("([^/\\]+)$")
	main:SetMode("BUILD", p.path, name)
	frame()
	return loaded()
end

M.save_build_xml = function()
	ensureBuild()
	return { xml = build:SaveDB("code") }
end

-- Replace the passive trees from a `<Tree>` section of build XML, leaving
-- items, skills and config as they are. Worker engines take this instead of
-- a full reload when a save differs from what they hold only in that
-- section; PoB loads the section the same way inside Build:Init.
M.sync_tree = function(p)
	ensureBuild()
	if not p or type(p.xml) ~= "string" or p.xml == "" then
		error("params.xml is required", 0)
	end
	local doc, err = common.xml.ParseXML(p.xml)
	if err or not doc or not doc[1] or doc[1].elem ~= "Tree" then
		error("sync_tree: expected a <Tree> section" .. (err and (": " .. tostring(err)) or ""), 0)
	end
	if build.treeTab:Load(doc[1], build.dbFileName) then
		error("sync_tree: PoB rejected the tree section", 0)
	end
	build.treeTab:PostLoad()
	refresh()
	return { rev = build.outputRevision }
end

M.save_build_code = function()
	ensureBuild()
	return { code = encodeCode(build:SaveDB("code")) }
end

M.save_build_file = function(p)
	ensureBuild()
	if p and p.path then
		build.dbFileName = p.path
		build.buildName = p.path:match("([^/\\]+)%.xml$") or build.buildName
	end
	if not build.dbFileName then
		error("build has no file name; pass params.path", 0)
	end
	if build:SaveDBFile() then
		error("could not write " .. tostring(build.dbFileName), 0)
	end
	-- SaveDBFile resets the mod flags; `unsaved` is only recomputed in OnFrame.
	frame()
	return { ok = true, path = build.dbFileName, unsaved = build.unsaved == true }
end

-- Rename the open build. PoB ties the name to the file, so a saved build's
-- file moves with it (the same folder, the new name); unsaved changes stay
-- in memory and the next Save writes to the new path.
M.set_build_name = function(p)
	ensureBuild()
	local name = p and type(p.name) == "string" and p.name:gsub("^%s+", ""):gsub("%s+$", "") or ""
	if name == "" then error("params.name is required", 0) end
	if name:find("[\\/:%*%?\"<>|%c]") then error("a build name cannot contain \\ / : * ? \" < > |", 0) end
	local old = build.dbFileName
	if old then
		local dir = old:match("^(.*[/\\])[^/\\]+$") or ""
		local new = dir .. name .. ".xml"
		if new ~= old then
			local exists = io.open(new, "r")
			if exists then
				exists:close()
				error("a build named " .. name .. " already exists in that folder", 0)
			end
			local f = io.open(old, "r")
			if f then
				f:close()
				local ok, err = os.rename(old, new)
				if not ok then error("could not rename the build file: " .. tostring(err), 0) end
			end
			build.dbFileName = new
		end
	end
	build.buildName = name
	frame()
	return M.get_build()
end

M.get_build = function()
	ensureBuild()
	local spec = build.spec
	local out = build.calcsTab.mainOutput
	-- Same arithmetic as buildMode:EstimatePlayerProgress.
	local used, ascUsed, secAscUsed, socketsUsed, ws1, ws2 = countAllocNodes(spec)
	local extra = out and out.ExtraPoints or 0
	local extraWs = out and out.PassivePointsToWeaponSetPoints or 0
	local points = {
		used = used - math.min(ws1, ws2),
		max = IS_POE2 and (99 + (build.maxWeaponSets or 0) + extra) or (99 + 23 + extra),
		ascUsed = ascUsed,
		ascMax = 8,
		weaponSet1Used = ws1 or 0,
		weaponSet2Used = ws2 or 0,
		weaponSetMax = (build.maxWeaponSets or 0) + extraWs,
		socketsUsed = socketsUsed or 0,
		requiredLevelText = opt(build.controls.pointDisplay and build.controls.pointDisplay.req),
		act = opt(build.Act),
	}
	return {
		points = points,
		name = build.buildName,
		file = type(build.dbFileName) == "string" and build.dbFileName or null,
		level = build.characterLevel,
		levelAuto = build.characterLevelAutoMode == true,
		classId = spec.curClassId,
		className = spec.curClassName,
		ascendClassId = spec.curAscendClassId,
		ascendClassName = opt(spec.curAscendClassName),
		-- PoE1 only; PoE2 trees carry no alternate ascendancies.
		secondaryAscendClassId = spec.curSecondaryAscendClassId or 0,
		secondaryAscendClassName = opt(spec.curSecondaryAscendClassName),
		mainSocketGroup = build.mainSocketGroup,
		treeVersion = spec.treeVersion,
		rev = build.outputRevision,
		generation = main.__reduxBuildGeneration,
		unsaved = build.unsaved == true,
		title = __window_title,
		targetVersion = build.targetVersion,
	}
end

-- ---------------------------------------------------------------------------
-- Stats
-- ---------------------------------------------------------------------------

-- Stat keys as callers tend to write them: "life", "fire_resist",
-- "lightning resistance". Matched against the real keys with case, spaces and
-- underscores ignored, and a few common words mapped onto PoB's spelling.
local STAT_ALIASES = { resistance = "resist", res = "resist", dps = "dps", ehp = "totalehp", hp = "life", strength = "str", dexterity = "dex", intelligence = "int" }
local function resolveStatKey(output, key)
	if output[key] ~= nil then return key end
	local words = {}
	for w in tostring(key):lower():gmatch("[^%s_%-]+") do words[#words + 1] = STAT_ALIASES[w] or w end
	local norm = table.concat(words)
	if norm == "resist" then return nil end
	local best
	for k in pairs(output) do
		if type(k) == "string" and isScalar(output[k]) then
			local kn = k:lower()
			if kn == norm then return k end
			if not best and (kn == norm .. "resist" or kn == "total" .. norm or kn == norm .. "mod") then best = k end
		end
	end
	return best
end

M.get_stats = function(p)
	ensureBuild()
	local output = build.calcsTab.mainOutput
	local stats = {}
	if p and p.fields then
		local unknown = {}
		for _, key in ipairs(p.fields) do
			local real = resolveStatKey(output, key)
			if real then
				stats[key] = isScalar(output[real]) and output[real] or null
				if real ~= key then stats[real] = stats[key] end
			else
				stats[key] = null
				unknown[#unknown + 1] = tostring(key)
			end
		end
		if #unknown > 0 then
			return {
				stats = stats,
				rev = build.outputRevision,
				unknown = strArray(unknown),
				hint = "these keys do not exist; call list_stat_keys for the real names (Life, FireResist, ColdResist, LightningResist, ChaosResist, TotalEHP, CombinedDPS, Str, Dex, Int)",
			}
		end
	else
		for key, value in pairs(output) do
			if isScalar(value) then
				stats[key] = value
			end
		end
	end
	return { stats = stats, rev = build.outputRevision }
end

M.list_stat_keys = function()
	ensureBuild()
	local keys = array({})
	for key, value in pairs(build.calcsTab.mainOutput) do
		if isScalar(value) then
			keys[#keys + 1] = key
		end
	end
	table.sort(keys)
	return { keys = keys }
end

-- The sidebar exactly as PoB renders it: rows carry PoB colour escapes
-- (^7, ^xRRGGBB) which the UI parses. `h` is PoB's row height (6 = spacer).
-- Rows with `hasBreakdown` accept `sidebar_breakdown { rowIndex }`.
M.get_sidebar = function()
	ensureBuild()
	local rows = array({})
	for _, row in ipairs(build.controls.statBox.list) do
		rows[#rows + 1] = {
			h = row.height or 16,
			lhs = opt(row[1]),
			rhs = opt(row[2]),
			breakdown = opt(row.breakdown),
			hasBreakdown = (row.breakdown ~= nil or row.modNames ~= nil) and true or false,
			align = opt(row.align),
			stat = opt(row.stat),
			actor = opt(row.actor),
		}
	end
	local warnings = strArray(build.controls.warnings and build.controls.warnings.lines or {})
	return { rows = rows, warnings = warnings, rev = build.outputRevision }
end

-- Serialise the typed sections a CalcBreakdownControl built (TEXT lines,
-- TABLE with preformatted coloured cells, RADIUS marker).
local function breakdownSections(ctl)
	local sections = array({})
	for _, s in ipairs(ctl.sectionList or {}) do
		if s.type == "TEXT" then
			sections[#sections + 1] = { type = "text", size = s.textSize or 16, lines = strArray(s.lines) }
		elseif s.type == "TABLE" then
			local cols = array({})
			for _, c in ipairs(s.colList) do
				cols[#cols + 1] = { label = c.label or "", key = c.key, right = c.right == true }
			end
			local rows = array({})
			for _, r in ipairs(s.rowList) do
				local rr = {}
				for _, c in ipairs(s.colList) do
					local v = r[c.key]
					if isScalar(v) then rr[c.key] = tostring(v) end
				end
				rows[#rows + 1] = rr
			end
			sections[#sections + 1] = { type = "table", label = opt(s.label), footer = opt(s.footer), cols = cols, rows = rows }
		elseif s.type == "RADIUS" then
			sections[#sections + 1] = { type = "radius", radius = s.radius }
		end
	end
	return sections
end

-- Breakdown popup for one sidebar row, via Build's own breakdown control
-- (GetSidebarBreakdown → CalcBreakdownControl against mainEnv).
M.sidebar_breakdown = function(p)
	ensureBuild()
	local line = build.controls.statBox.list[tonumber(p and p.rowIndex) or -1]
	if not line then error("unknown sidebar row " .. tostring(p and p.rowIndex), 0) end
	if not line.breakdown and not line.modNames then
		return { sections = array({}), rev = build.outputRevision }
	end
	local displayData = build:GetSidebarBreakdown(line.breakdown, line.modNames, line.ignoredSections, line.actorName)
	local ctl = build.controls.breakdown
	ctl:SetBreakdownData(displayData, false, line.actorName == "minion" and "minion" or nil)
	local sections = breakdownSections(ctl)
	ctl:SetBreakdownData()
	return { sections = sections, rev = build.outputRevision }
end

-- PoE2's CalcSectionControl exposes its cell formatter as FormatStr; PoE1
-- keeps the same code file-local, so it is repeated here for that game.
local function formatVal(val, p)
	return formatNumSep(tostring(round(val, p)))
end

local function formatCalcStr(section, str, actor, colData)
	if section.FormatStr then return section:FormatStr(str, actor, colData) end
	str = str:gsub("{output:([%a%.:]+)}", function(c)
		local ns, var = c:match("^(%a+)%.(%a+)$")
		if ns then
			return actor.output[ns] and actor.output[ns][var] or ""
		end
		return actor.output[c] or ""
	end)
	str = str:gsub("{(%d+):output:([%a%.:]+)}", function(p, c)
		local ns, var = c:match("^(%a+)%.(%a+)$")
		if ns then
			return formatVal(actor.output[ns] and actor.output[ns][var] or 0, tonumber(p))
		end
		return formatVal(actor.output[c] or 0, tonumber(p))
	end)
	str = str:gsub("{(%d+):mod:([%d,]+)}", function(p, n)
		local numList = {}
		for num in n:gmatch("%d+") do
			numList[#numList + 1] = tonumber(num)
		end
		local modType = colData[numList[1]].modType
		local modTotal = modType == "MORE" and 1 or 0
		for _, num in ipairs(numList) do
			local sectionData = colData[num]
			local modCfg = (sectionData.cfg and actor.mainSkill[sectionData.cfg .. "Cfg"]) or {}
			if sectionData.modSource then
				modCfg.source = sectionData.modSource
				modCfg.ignoreSourceInCheckConditions = true
			end
			if sectionData.actor then
				modCfg.actor = sectionData.actor
			end
			local modStore = (sectionData.enemy and actor.enemy.modDB) or (sectionData.cfg and actor.mainSkill.skillModList) or actor.modDB
			local modVal
			if type(sectionData.modName) == "table" then
				modVal = modStore:Combine(sectionData.modType, modCfg, unpack(sectionData.modName))
			else
				modVal = modStore:Combine(sectionData.modType, modCfg, sectionData.modName)
			end
			if modType == "MORE" then
				modTotal = modTotal * modVal
			else
				modTotal = modTotal + modVal
			end
		end
		if modType == "MORE" then
			modTotal = (modTotal - 1) * 100
		end
		return formatVal(modTotal, tonumber(p))
	end)
	return str
end

-- The Calcs tab grid: PoB's own section controls with every cell's format
-- string resolved against the requested actor.
local BUFF_MODES = { UNBUFFED = true, BUFFED = true, COMBAT = true, EFFECTIVE = true }

-- Which buffs the Calcs tab assumes. The sidebar is always EFFECTIVE, as in PoB.
M.calc_mode = function(p)
	ensureBuild()
	local input = build.calcsTab.input
	if p and p.mode ~= nil then
		local mode = string.upper(tostring(p.mode))
		if not BUFF_MODES[mode] then error("mode must be UNBUFFED, BUFFED, COMBAT or EFFECTIVE", 0) end
		input.misc_buffMode = mode
		pcall(function() build.calcsTab.controls.mode:SelByValue(mode, "buffMode") end)
		build.calcsTab:AddUndoState()
		refresh()
	end
	return { mode = input.misc_buffMode or "EFFECTIVE", modes = array({ "UNBUFFED", "BUFFED", "COMBAT", "EFFECTIVE" }) }
end

local BUFF_LABELS = { UNBUFFED = "Unbuffed", BUFFED = "Buffed", COMBAT = "In combat", EFFECTIVE = "Effective DPS" }

-- The View Skill Details rows are PoB's selectors; each becomes the value the
-- CALCS pass used. Nil drops the row (checkboxes and library buttons).
local function controlText(name, env)
	local skill = env.player and env.player.mainSkill
	local ae = skill and skill.activeEffect
	local ge = ae and ae.grantedEffect
	if name == "mainSocketGroup" then
		local n = build.calcsTab.input.skill_number or 1
		local group = build.skillsTab.socketGroupList[n]
		if not group then return nil end
		local label = group.displayLabel or group.label
		if not label or label == "" then label = "Group " .. n end
		local ok, ws = pcall(build.skillsTab.GetSocketGroupWeaponSetLabel, build.skillsTab, group)
		if ok and type(ws) == "string" and ws ~= "" and ws ~= "Both" then label = label .. " (" .. ws .. ")" end
		return label
	elseif name == "mainSkill" then
		local ok, nm = pcall(build.calcsTab.calcs.getActiveSkillDisplayName, skill)
		return (ok and nm) or (ge and ge.name)
	elseif name == "statSet" then
		local sets = ge and ge.statSets
		if not sets or #sets < 2 then return nil end
		local idx = (ae.statSetCalcs and ae.statSetCalcs.index) or (ae.statSet and ae.statSet.index) or 1
		return sets[idx] and tostring(sets[idx].label) or nil
	elseif name == "mainSkillPart" then
		return skill and skill.skillPartName
	elseif name == "mainSkillStageCount" then
		return skill and skill.activeStageCount and tostring(skill.activeStageCount)
	elseif name == "mainSkillMineCount" then
		return skill and skill.activeMineCount and tostring(skill.activeMineCount)
	elseif name == "mainSkillMinion" then
		return env.minion and env.minion.minionData and env.minion.minionData.name
	elseif name == "mainSkillMinionSkill" then
		local ms = env.minion and env.minion.mainSkill
		return ms and ms.activeEffect and ms.activeEffect.grantedEffect and ms.activeEffect.grantedEffect.name
	elseif name == "mode" then
		return BUFF_LABELS[build.calcsTab.input.misc_buffMode or "EFFECTIVE"]
	end
	return nil
end

M.calc_sections = function(p)
	ensureBuild()
	local calcsTab = build.calcsTab
	local env = calcsTab.calcsEnv or calcsTab.mainEnv
	local actor = (p and p.actor == "minion" and env.minion) or env.player
	local out = array({})
	for sIndex, section in ipairs(calcsTab.sectionList) do
		if section.subSection then
			local enabled = calcsTab:CheckFlag(section)
			local colour = section.colour
			local hex = null
			if type(colour) == "string" then
				hex = colour:match("^%^x(%x%x%x%x%x%x)$") and ("#" .. colour:sub(3)) or null
			elseif type(colour) == "table" then
				hex = string.format("#%02x%02x%02x", (colour[1] or 1) * 255, (colour[2] or 1) * 255, (colour[3] or 1) * 255)
			end
			local secOut = {
				index = sIndex,
				group = opt(section.group),
				colour = hex,
				enabled = enabled and true or false,
				subSections = array({}),
			}
			if enabled then
				for si, subSec in ipairs(section.subSection) do
					local sub = { index = si, label = subSec.label or "", colWidth = opt(subSec.data.colWidth), rows = array({}) }
					local okExtra, extra = pcall(function()
						return subSec.data.extra and formatCalcStr(section, subSec.data.extra, actor)
					end)
					sub.extra = (okExtra and extra) and extra or null
					for ri, rowData in ipairs(subSec.data) do
						if calcsTab:CheckFlag(rowData) then
							local label = rowData.label
							if type(label) == "string" and label:find("^Socket Group") then label = "Socket Group" end
							local row = { index = ri, label = opt(label), textSize = opt(rowData.textSize), cells = array({}) }
							local keep = true
							for ci, colData in ipairs(rowData) do
								local text = ""
								if colData.control then
									local ok, value = pcall(controlText, colData.controlName, env)
									if ok and value then text = tostring(value) else keep = false end
								elseif colData.format then
									local okF, formatted = pcall(formatCalcStr, section, colData.format, actor, colData)
									text = okF and formatted or "?"
								end
								row.cells[#row.cells + 1] = {
									index = ci,
									text = text,
									hasBreakdown = #colData > 0,
								}
							end
							if keep then sub.rows[#sub.rows + 1] = row end
						end
					end
					secOut.subSections[#secOut.subSections + 1] = sub
				end
			end
			out[#out + 1] = secOut
		end
	end
	return { sections = out, rev = build.outputRevision }
end

-- Breakdown for one Calcs-grid cell (the cell's entry list drives
-- CalcBreakdownControl exactly as clicking it does in PoB).
M.calc_cell_breakdown = function(p)
	ensureBuild()
	local section = build.calcsTab.sectionList[tonumber(p and p.section) or -1]
	local subSec = section and section.subSection and section.subSection[tonumber(p.sub) or -1]
	local rowData = subSec and subSec.data[tonumber(p.row) or -1]
	local colData = rowData and rowData[tonumber(p.col) or -1]
	if not colData then error("unknown calc cell", 0) end
	local ctl = build.controls.breakdown
	ctl:SetBreakdownData(colData, false, p.actor == "minion" and "minion" or nil)
	local sections = breakdownSections(ctl)
	ctl:SetBreakdownData()
	return { sections = sections, rev = build.outputRevision }
end

-- Which config options PoB would show for the current build (each control's
-- `shown` closure evaluates ifSkill/ifFlag/ifCond/... against the live env).
M.config_visibility = function()
	ensureBuild()
	local vis = {}
	for var, control in pairs(build.configTab.varControls) do
		local ok, shown = pcall(control.IsShown, control)
		vis[var] = (ok and shown) and true or false
	end
	return { visibility = vis, rev = build.outputRevision }
end

M.set_level = function(p)
	ensureBuild()
	local level = tonumber(p and p.level)
	if not level then error("params.level is required", 0) end
	build.characterLevel = math.max(1, math.min(100, math.floor(level)))
	build.characterLevelAutoMode = false
	refresh()
	return M.get_build()
end

-- PoB's Auto/Manual level button: in auto mode EstimatePlayerProgress sets
-- the level from the points spent on every stat refresh.
M.set_level_auto = function(p)
	ensureBuild()
	build.characterLevelAutoMode = p and p.auto == true
	refresh()
	return M.get_build()
end

-- ---------------------------------------------------------------------------
-- Class / ascendancy
-- ---------------------------------------------------------------------------

M.list_classes = function()
	ensureBuild()
	local classes = array({})
	for classId, classData in pairs(build.spec.tree.classes) do
		local ascendancies = array({})
		for ascendId, ascendData in pairs(classData.classes or {}) do
			if ascendId ~= 0 then
				ascendancies[#ascendancies + 1] = { id = ascendId, name = ascendData.name, internalId = opt(ascendData.internalId) }
			end
		end
		table.sort(ascendancies, function(a, b) return a.id < b.id end)
		classes[#classes + 1] = { id = classId, name = classData.name, ascendancies = ascendancies }
	end
	table.sort(classes, function(a, b) return a.id < b.id end)
	local secondary = array({})
	for id, data in pairs(build.spec.tree.alternate_ascendancies or {}) do
		if id ~= 0 and data.name then secondary[#secondary + 1] = { id = id, name = data.name } end
	end
	table.sort(secondary, function(a, b) return a.id < b.id end)
	return { classes = classes, secondaryAscendancies = secondary }
end

M.select_class = function(p)
	ensureBuild()
	if not p then error("params.classId or params.ascendClassId is required", 0) end
	-- SelectClass mutates before validating; snapshot so a bad id never leaves
	-- a half-applied state.
	local snapshot = build:SaveDB("snapshot")
	local ok, err = pcall(function()
		if p.classId ~= nil and tonumber(p.classId) ~= build.spec.curClassId then
			build.spec:SelectClass(tonumber(p.classId))
		end
		if p.ascendClassId ~= nil then
			build.spec:SelectAscendClass(tonumber(p.ascendClassId))
		end
		if p.secondaryAscendClassId ~= nil and build.spec.SelectSecondaryAscendClass then
			build.spec:SelectSecondaryAscendClass(tonumber(p.secondaryAscendClassId))
		end
	end)
	if not ok then
		M.load_build_xml({ xml = snapshot, name = build.buildName })
		error("select_class failed and was rolled back: " .. tostring(err), 0)
	end
	build.spec:AddUndoState()
	refresh()
	return M.get_build()
end

-- ---------------------------------------------------------------------------
-- Passive tree
-- ---------------------------------------------------------------------------

local function requireNode(p)
	local id = tonumber(p and p.id)
	if not id then error("params.id (node id) is required", 0) end
	local node = build.spec.nodes[id]
	if not node then error("unknown node id " .. tostring(id), 0) end
	return node
end

local function nodeSummary(id, node)
	return {
		id = id,
		name = opt(node.dn or node.name),
		type = opt(node.type),
		stats = strArray(node.sd),
		allocated = node.alloc == true,
		ascendancyName = opt(node.ascendancyName),
		pathCost = node.path and #node.path or null,
		reminder = node.reminderText and strArray(node.reminderText) or null,
	}
end

-- PoE1 masteries: the effects a mastery can take. An effect is unique on the
-- tree, so one held by another mastery of the same kind is marked taken.
local function masteryOptions(node)
	local spec = build.spec
	local out = array({})
	if IS_POE2 or node.type ~= "Mastery" or not node.masteryEffects then return out end
	for _, effect in ipairs(node.masteryEffects) do
		local taken = nil
		for nid, eid in pairs(spec.masterySelections or {}) do
			if eid == effect.effect and nid ~= node.id then taken = nid break end
		end
		local data = spec.tree.masteryEffects and spec.tree.masteryEffects[effect.effect]
		out[#out + 1] = { effect = effect.effect, stats = strArray(data and data.sd or effect.stats), takenBy = opt(taken) }
	end
	return out
end

-- Two uniques move their radius onto a keystone they name instead of using
-- their socket: From Nothing on PoE2, Impossible Escape on PoE1. Item.lua fills
-- these tables only for those jewels, and PassiveSpec:NodeInKeystoneRadius
-- branches on the table rather than the item title, so this does too.
local function radiusKeystoneNames(jewel)
	local jd = jewel and jewel.jewelData
	if not jd then return nil end
	local names = IS_POE2 and jd.fromNothingKeystones or jd.impossibleEscapeKeystones
	if type(names) ~= "table" or not next(names) then return nil end
	return names
end

--- The keystone node ids such a jewel's radius follows, or nil for an ordinary
--- jewel. keystoneMap is keyed by display name and its lowercase, which is the
--- form the parsed mod stores.
local function radiusKeystoneIds(jewel, tree)
	local names = radiusKeystoneNames(jewel)
	if not names then return nil end
	local ids = array({})
	for name in pairs(names) do
		local keystone = tree.keystoneMap and tree.keystoneMap[name]
		if keystone and keystone.x and keystone.y then ids[#ids + 1] = keystone.id end
	end
	if #ids == 0 then return nil end
	table.sort(ids)
	return ids
end

M.get_tree_state = function()
	ensureBuild()
	local spec = build.spec
	local alloc = array({})
	local weaponSetNodes = { array({}), array({}) }
	-- Nodes whose content differs from tree.json: attribute nodes switched to
	-- Str/Dex/Int, and `isSwitchable` ascendancy variants (e.g. Abyssal Lich).
	-- The renderer overlays these onto its static model.
	local overrides = {}
	local function override(id, node)
		overrides[tostring(id)] = {
			name = opt(node.dn),
			icon = opt(node.type == "Mastery" and node.activeIcon or node.icon),
			effect = opt(node.activeEffectImage),
			stats = strArray(node.sd),
			overlay = node.overlay and node.overlay.alloc and {
				alloc = node.overlay.alloc,
				path = node.overlay.path,
				unalloc = node.overlay.unalloc,
			} or null,
		}
	end
	for id, node in pairs(spec.nodes) do
		local tnode = spec.tree.nodes[id]
		if node.alloc then
			alloc[#alloc + 1] = id
			local set = weaponSetNodes[node.allocMode or 0]
			if set then set[#set + 1] = id end
			if node.isAttribute and node.dn and node.dn ~= "Attribute" then
				override(id, node)
			end
			-- An allocated PoE1 mastery shows its chosen effect, not the option list.
			if not IS_POE2 and node.type == "Mastery" and spec.masterySelections and spec.masterySelections[id] then
				override(id, node)
			end
		end
		if tnode and tnode.isSwitchable then
			override(id, node)
		end
		-- Timeless jewels and tattoos rewrite a node in place (PassiveSpec:ReplaceNode);
		-- the renderer's static model still holds the original.
		if tnode and not overrides[tostring(id)] and (node.conqueredBy or (spec.hashOverrides and spec.hashOverrides[id]) or node.dn ~= tnode.dn) then
			override(id, node)
		end
	end
	table.sort(alloc)
	table.sort(weaponSetNodes[1])
	table.sort(weaponSetNodes[2])
	local sockets = array({})
	-- Every socket in the spec, cluster jewel sub-sockets included; tree.sockets
	-- has only the base tree's.
	local socketIds = {}
	for nodeId, snode in pairs(spec.nodes) do
		if snode.type == "Socket" then socketIds[#socketIds + 1] = nodeId end
	end
	table.sort(socketIds)
	for _, nodeId in ipairs(socketIds) do
		local ok, _, jewel = pcall(build.itemsTab.GetSocketAndJewelForNodeID, build.itemsTab, nodeId)
		if ok and jewel then
			sockets[#sockets + 1] = {
				nodeId = nodeId,
				itemId = jewel.id,
				name = jewel.name,
				title = opt(jewel.title),
				baseName = opt(jewel.baseName),
				rarity = opt(jewel.rarity),
				radiusIndex = opt(jewel.jewelRadiusIndex),
				radiusLabel = opt(jewel.jewelRadiusLabel),
				-- Timeless-style jewels: which legion's ring pair to draw.
				conqueror = opt(jewel.jewelData and jewel.jewelData.conqueredBy and jewel.jewelData.conqueredBy.conqueror and jewel.jewelData.conqueredBy.conqueror.type),
				-- Where this jewel's ring really belongs (PassiveTreeView.drawJewelRadius).
				radiusKeystones = opt(radiusKeystoneIds(jewel, spec.tree)),
			}
		end
	end
	-- Cluster jewel subgraphs (PoE1): nodes PoB generates from the socketed
	-- jewel, absent from tree.json. Their ids start at 65536.
	local dynamic = array({})
	if not IS_POE2 then
		-- Everything a subgraph places: the jewel's own passives (ids from
		-- 65536) and the tree's sub-socket nodes PoB moves into the cluster.
		local seen = {}
		for _, sg in pairs(spec.subGraphs or {}) do
			for _, node in pairs(sg.nodes or {}) do
				local id = node.id
				if type(id) == "number" and node.x and node.y and not seen[id] then
					seen[id] = true
					local links = array({})
					for _, other in ipairs(node.linked or {}) do links[#links + 1] = other.id end
					dynamic[#dynamic + 1] = {
						id = id,
						name = opt(node.dn),
						type = opt(node.type),
						stats = strArray(node.sd),
						x = node.x,
						y = node.y,
						icon = opt(node.icon),
						links = links,
						expansion = node.expansionJewel ~= nil,
						allocated = node.alloc == true,
					}
				end
			end
		end
	end
	-- One entry per cluster subgraph: its centre and the orbits it uses, which
	-- pick the ring art (PassiveTreeView.lua renderGroup with isExpansion).
	local dynamicGroups = array({})
	if not IS_POE2 then
		for _, sg in pairs(spec.subGraphs or {}) do
			local g = sg.group
			if g and g.x and g.y then
				local orbits = array({})
				for orbit in pairs(g.oo or {}) do orbits[#orbits + 1] = orbit end
				table.sort(orbits)
				dynamicGroups[#dynamicGroups + 1] = { x = g.x, y = g.y, orbits = orbits }
			end
		end
	end
	local used, ascUsed, secondaryAscUsed, socketCount, ws1Used, ws2Used = countAllocNodes(spec)
	local level = build.characterLevel or 1
	local questLow, questHigh = questPointsForLevel(level)
	local extra = (build.calcsTab.mainOutput or {}).ExtraPoints or 0
	return {
		treeVersion = spec.treeVersion,
		classId = spec.curClassId,
		className = spec.curClassName,
		ascendClassId = spec.curAscendClassId,
		ascendClassName = opt(spec.curAscendClassName),
		allocatedNodes = alloc,
		allocatedNodeCount = #alloc,
		weaponSet1Nodes = weaponSetNodes[1],
		weaponSet2Nodes = weaponSetNodes[2],
		-- A point buys a node in either weapon set, so PoB charges only the larger set.
		pointsUsed = used,
		passivePointsSpent = used - math.min(ws1Used, ws2Used),
		mainTreePointsUsed = used - ws1Used - ws2Used,
		ascendancyPointsUsed = ascUsed,
		secondaryAscendancyPointsUsed = secondaryAscUsed,
		jewelSocketsUsed = socketCount,
		weaponSet1PointsUsed = ws1Used,
		weaponSet2PointsUsed = ws2Used,
		weaponSetPointsAvailablePerSet = IS_POE2 and 24 or 0,
		-- PoB tracks points spent but not the budget. Levels give 1 point each
		-- after the first; the rest are campaign quest rewards, which depend on
		-- progress rather than level, hence the range.
		characterLevel = level,
		pointsFromLevels = math.max(0, level - 1),
		questPointsMin = questLow,
		questPointsMax = questHigh,
		extraPoints = extra,
		pointsAvailableMin = math.max(0, level - 1) + questLow + extra,
		pointsAvailableMax = math.max(0, level - 1) + questHigh + extra,
		ascendancyPointsAvailable = 8,
		overrides = overrides,
		sockets = sockets,
		dynamicNodes = dynamic,
		dynamicGroups = dynamicGroups,
		rev = build.outputRevision,
	}
end

-- Allocated node ids of any spec, without switching to it (compare overlay).
M.spec_alloc = function(p)
	ensureBuild()
	local spec = build.treeTab.specList[tonumber(p and p.index) or -1]
	if not spec then error("unknown spec index " .. tostring(p and p.index), 0) end
	local ids = array({})
	for id, node in pairs(spec.nodes) do
		if node.alloc then ids[#ids + 1] = id end
	end
	table.sort(ids)
	return { index = tonumber(p.index), allocatedNodes = ids }
end

-- Only the tree view sends a weapon set, so every other caller allocates into the main tree.
local function weaponSetParam(p)
	local mode = IS_POE2 and tonumber(p and p.weaponSet) or 0
	return (mode == 1 or mode == 2) and mode or 0
end

local function withAllocMode(mode, fn, ...)
	if not IS_POE2 then return fn(...) end
	local spec = build.spec
	spec.allocMode = mode
	local ok, res = pcall(fn, ...)
	spec.allocMode = 0
	if not ok then error(res, 0) end
	return res
end

local function touchesWeaponSet(node)
	for i = 2, #(node.path or {}) do
		local other = node.path[i]
		if other.alloc and (other.allocMode or 0) > 0 then return true end
	end
	for _, other in ipairs(node.linked or {}) do
		if other.alloc and (other.allocMode or 0) > 0 then return true end
	end
	return false
end

-- PassiveTreeView's rule for keystones and jewel sockets: the reason a click is refused, or nil.
local function weaponSetBlock(node, mode)
	if not IS_POE2 or not (node.type == "Keystone" or node.type == "Socket" or node.containJewelSocket) then return nil end
	local kind = node.type == "Keystone" and "keystones" or "jewel sockets"
	if not node.alloc and node.path then
		if mode > 0 then return "Cannot allocate " .. kind .. " while weapon set " .. mode .. " is selected" end
		if touchesWeaponSet(node) then return "Cannot allocate " .. kind .. " connected to weapon set passives" end
	elseif node.alloc and (node.allocMode or 0) == 0 and mode > 0 then
		return "Cannot remove main tree " .. kind .. " while weapon set " .. mode .. " is selected"
	end
	return nil
end

-- Allocate a traced path (shift-hover in the tree). `ids` must run from the
-- tree side to the target; PoB's AllocNode takes it as the alternate path.
M.alloc_trace = function(p)
	ensureBuild()
	if not p or type(p.ids) ~= "table" or #p.ids == 0 then error("params.ids is required", 0) end
	local spec = build.spec
	local nodes = {}
	for i, id in ipairs(p.ids) do
		local n = spec.nodes[tonumber(id)]
		if not n then error("unknown node id " .. tostring(id), 0) end
		nodes[i] = n
	end
	-- PoB takes an explicit path on trust: GetEffectiveAllocationPath returns
	-- altPath unchecked, and a chain that never reaches the tree is then
	-- dropped by BuildAllDependsAndPaths. The call reports success having
	-- allocated nothing, so connectivity is checked here instead.
	local first = nodes[1]
	if not first.alloc then
		local rooted = false
		for _, other in ipairs(first.linked or {}) do
			if other.alloc then rooted = true break end
		end
		if not rooted then
			error("node " .. first.id .. " does not touch the allocated tree, so this path has no root", 0)
		end
	end
	for i = 2, #nodes do
		local prev, cur = nodes[i - 1], nodes[i]
		local linked = false
		for _, other in ipairs(prev.linked or {}) do
			if other.id == cur.id then linked = true break end
		end
		if not linked then
			error("nodes " .. prev.id .. " and " .. cur.id .. " are not connected", 0)
		end
	end

	local target = nodes[#nodes]
	if not target.path then error("target node cannot be reached", 0) end
	local mode = weaponSetParam(p)
	local blocked = weaponSetBlock(target, mode)
	if blocked then error(blocked, 0) end
	withAllocMode(mode, spec.AllocNode, spec, target, nodes)
	spec:AddUndoState()
	refresh()
	return M.get_tree_state()
end

-- Node ids inside one jewel radius of a socket (tree-space precomputed map).
-- From Nothing and Impossible Escape reach the nodes in radius of the keystones
-- they name instead of the socket's own (PassiveSpec:NodeInKeystoneRadius).
M.socket_nodes = function(p)
	ensureBuild()
	local id = tonumber(p and p.id)
	local ri = tonumber(p and p.radiusIndex)
	local spec = build.spec
	local socket = spec.tree.sockets[id or -1]
	if not socket then error("unknown socket " .. tostring(p and p.id), 0) end
	local set = {}
	local ok, _, jewel = pcall(build.itemsTab.GetSocketAndJewelForNodeID, build.itemsTab, id)
	local names = ok and radiusKeystoneNames(jewel) or nil
	-- A tree that does not carry the named keystone falls back to the socket,
	-- so the ring and the highlight never simply vanish.
	local onKeystone = false
	if names and ri then
		for keystoneName in pairs(names) do
			local keystone = spec.tree.keystoneMap and spec.tree.keystoneMap[keystoneName]
			if keystone then
				onKeystone = true
				local map = keystone.nodesInRadius and keystone.nodesInRadius[ri]
				if map then
					for nid in pairs(map) do set[nid] = true end
				end
			end
		end
	end
	if not onKeystone then
		local map = socket.nodesInRadius and ri and socket.nodesInRadius[ri]
		if map then
			for nid in pairs(map) do set[nid] = true end
		end
	end
	local ids = array({})
	for nid in pairs(set) do ids[#ids + 1] = nid end
	table.sort(ids)
	return { id = id, radiusIndex = opt(ri), nodes = ids }
end

-- Hover preview: the allocation path for an unallocated node, or the nodes
-- that would be removed with an allocated one (PassiveTreeView's hoverPath /
-- hoverDep).
M.node_hover = function(p)
	ensureBuild()
	local node = requireNode(p)
	local mode = weaponSetParam(p)
	local blocked = weaponSetBlock(node, mode)
	local out = { id = node.id, allocated = node.alloc == true, path = array({}), depends = array({}), blocked = opt(blocked) }
	if node.alloc then
		for i, n in ipairs(node.depends or {}) do out.depends[i] = n.id end
	elseif node.path then
		local ok, path = pcall(withAllocMode, mode, build.spec.GetEffectiveAllocationPath, build.spec, node)
		if ok and not path then
			out.blocked = blocked or ("No path reaches this node in weapon set " .. mode)
			return out
		end
		path = (ok and path) or node.path or {}
		for i, n in ipairs(path) do out.path[i] = n.id end
		out.cost = #path
	end
	return out
end

-- PoB's "Mod differences" block from the node tooltip (PassiveTreeView), as
-- sized, colour-coded lines: what allocating or unallocating this node, and
-- then the whole path to it, does to the sidebar stats. `path` overrides the
-- engine's own path so a shift-traced route compares what the UI highlights.
M.node_compare = function(p)
	ensureBuild()
	local node = requireNode(p)
	local mode = weaponSetParam(p)
	local calcsTab = build.calcsTab
	local granted = (calcsTab.mainEnv.grantedPassives or {})[node.id] == true
	local path = {}
	if type(p.path) == "table" and #p.path > 0 then
		for _, pid in ipairs(p.path) do
			local pn = build.spec.nodes[tonumber(pid) or -1]
			if pn then path[#path + 1] = pn end
		end
	elseif node.alloc then
		for _, pn in pairs(node.depends or {}) do path[#path + 1] = pn end
	else
		local ok, eff = pcall(withAllocMode, mode, build.spec.GetEffectiveAllocationPath, build.spec, node)
		for _, pn in pairs((ok and eff) or node.path or {}) do path[#path + 1] = pn end
	end
	local pathNodes = {}
	for _, pn in ipairs(path) do pathNodes[pn] = true end

	local calcFunc, calcBase = calcsTab:GetMiscCalculator(build)
	local nodeOutput, pathOutput
	if node.alloc then
		nodeOutput = calcFunc({ removeNodes = { [node] = true } })
		if #path > 1 then pathOutput = calcFunc({ removeNodes = pathNodes }) end
	elseif granted then
		nodeOutput = calcFunc({ removeNodes = { [node.id] = true } })
	else
		nodeOutput = calcFunc({ addNodes = { [node] = true } })
		if #path > 1 then pathOutput = calcFunc({ addNodes = pathNodes }) end
	end

	local tt = new("Tooltip"):Tooltip()
	local heads = {}
	local function compare(output, header, nodeCount)
		local at = #tt.lines + 1
		local n = build:AddStatComparesToTooltip(tt, calcBase, output, header, nodeCount)
		if n > 0 then heads[at] = true end
		return n
	end
	local count = compare(nodeOutput,
		granted and "^7This node is granted by an item. Removing it will give you:"
		or node.alloc and "^7Unallocating this node will give you:"
		or "^7Allocating this node will give you:")
	if pathOutput and not granted and (#(node.intuitiveLeapLikesAffecting or {}) == 0 or node.alloc) then
		count = count + compare(pathOutput,
			node.alloc and "^7Unallocating this node and all nodes depending on it will give you:"
			or "^7Allocating this node and all nodes leading to it will give you:", #path)
	end

	-- A minion build puts the "Minion:" caption in the header line itself.
	local NL = string.char(10)
	local lines = array({})
	for i, l in ipairs(tooltipPayload(tt).lines) do
		if tostring(l.text):find(NL, 1, true) then
			for piece in tostring(l.text):gmatch("[^" .. NL .. "]+") do
				lines[#lines + 1] = { size = l.size, text = piece, center = l.center, sep = l.sep, font = l.font, head = heads[i] }
			end
		else
			l.head = heads[i]
			lines[#lines + 1] = l
		end
	end
	return {
		id = node.id,
		allocated = node.alloc == true,
		granted = granted,
		pathCount = #path,
		changes = count,
		lines = lines,
		rev = build.outputRevision,
	}
end

-- Left-click on a node, following PassiveTreeView:Draw's click handling:
-- deallocate, switch attribute, switch ascendancy (same class directly,
-- cross-class after confirmation), or allocate along the path.
-- params: { id, attribute = 1|2|3 (Str/Dex/Int), confirm = "reset"|"connect", weaponSet = 0|1|2 }
local function treeClick(p, node)
	local spec = build.spec
	local attr = tonumber(p.attribute)

	-- A PoE1 mastery is allocated with one of its effects; PoB opens a picker.
	if not IS_POE2 and node.type == "Mastery" and node.masteryEffects and not node.alloc then
		if p.effect then return M.select_mastery(p) end
		if node.path then
			return {
				needsMastery = true,
				id = node.id,
				name = opt(node.dn),
				effects = masteryOptions(node),
				selected = opt(spec.masterySelections and spec.masterySelections[node.id]),
			}
		end
	end

	if node.alloc then
		if node.isAttribute and attr then
			spec.attributeIndex = attr
			spec:SwitchAttributeNode(node.id, attr)
			spec:BuildAllDependsAndPaths()
		elseif node.isAttribute then
			spec.hashOverrides[node.id] = nil
			spec:DeallocNode(node)
		else
			spec:DeallocNode(node)
		end
		spec:AddUndoState()
		refresh()
		return M.get_tree_state()
	end

	if node.ascendancyName then
		local cur = spec.curAscendClass and (spec.curAscendClass.replace or spec.curAscendClassBaseName)
		local different = spec.curAscendClassId == 0 or node.ascendancyName ~= cur
		if different and not (spec.curSecondaryAscendClass and node.ascendancyName == spec.curSecondaryAscendClass.id) then
			local targetAscendClassId
			for ascendClassId, ascendClass in pairs(spec.curClass.classes) do
				if ascendClass.id == node.ascendancyName then targetAscendClassId = ascendClassId break end
			end
			if targetAscendClassId then
				spec:SelectAscendClass(targetAscendClassId)
			else
				local targetBaseClassId
				for classId, classData in pairs(spec.tree.classes) do
					for ascendClassId, ascendClass in pairs(classData.classes) do
						if ascendClass.id == node.ascendancyName then
							targetBaseClassId, targetAscendClassId = classId, ascendClassId
							break
						end
					end
					if targetBaseClassId then break end
				end
				if targetBaseClassId then
					local used = spec:CountAllocNodes()
					if used == 0 or spec:IsClassConnected(targetBaseClassId) or p.confirm == "reset" then
						spec:SelectClass(targetBaseClassId)
						spec:SelectAscendClass(targetAscendClassId)
					elseif p.confirm == "connect" then
						if not spec:ConnectToClass(targetBaseClassId) then
							error("no path connects the tree to that class", 0)
						end
						spec:SelectClass(targetBaseClassId)
						spec:SelectAscendClass(targetAscendClassId)
					else
						return {
							needsConfirm = "class_change",
							className = spec.tree.classes[targetBaseClassId].name,
							ascendClassName = node.ascendancyName,
							id = node.id,
						}
					end
				end
			end
			spec:SetWindowTitleWithBuildClass()
		end
	end

	local target = spec.nodes[node.id]
	if target and target.path then
		if target.isAttribute then
			if not attr then
				return { needsAttribute = true, id = target.id }
			end
			spec.attributeIndex = attr
			spec:SwitchAttributeNode(target.id, attr)
			target = spec.nodes[node.id]
		end
		spec:AllocNode(target)
	end
	spec:AddUndoState()
	refresh()
	return M.get_tree_state()
end

M.tree_click = function(p)
	ensureBuild()
	local node = requireNode(p)
	local mode = weaponSetParam(p)
	local blocked = weaponSetBlock(node, mode)
	if blocked then return { blocked = blocked, id = node.id } end
	return withAllocMode(mode, treeClick, p, node)
end

-- Allocate a PoE1 mastery with an effect, or change an allocated one's
-- effect. Mirrors TreeTab:SaveMasteryPopup.
M.select_mastery = function(p)
	ensureBuild()
	local node = requireNode(p)
	local spec = build.spec
	if node.type ~= "Mastery" or not node.masteryEffects then error("params.id must be a mastery node", 0) end
	local effect = spec.tree.masteryEffects and spec.tree.masteryEffects[tonumber(p.effect) or -1]
	if not effect then error("unknown mastery effect " .. tostring(p.effect), 0) end
	for nid, eid in pairs(spec.masterySelections) do
		if eid == effect.id and nid ~= node.id then error("that effect is already taken by mastery " .. tostring(nid), 0) end
	end
	node.sd = effect.sd
	node.allMasteryOptions = false
	node.reminderText = { "Tip: Right click to select a different effect" }
	spec.tree:ProcessStats(node)
	spec.masterySelections[node.id] = effect.id
	if not node.alloc then spec:AllocNode(node) end
	spec:AddUndoState()
	refresh()
	return M.get_tree_state()
end

M.switch_attribute = function(p)
	ensureBuild()
	local node = requireNode(p)
	local attr = tonumber(p.attribute)
	if not node.isAttribute or not attr then error("params.id must be an attribute node and params.attribute 1..3", 0) end
	build.spec.attributeIndex = attr
	build.spec:SwitchAttributeNode(node.id, attr)
	build.spec:BuildAllDependsAndPaths()
	build.spec:AddUndoState()
	refresh()
	return M.get_tree_state()
end

M.export_tree_url = function()
	ensureBuild()
	return { url = build.spec:EncodeURL("https://www.pathofexile.com/passive-skill-tree/") }
end

M.import_tree_url = function(p)
	ensureBuild()
	if not p or type(p.url) ~= "string" or p.url == "" then error("params.url is required", 0) end
	local err = build.spec:DecodeURL(p.url)
	if err then error(err, 0) end
	build.spec:AddUndoState()
	refresh()
	return M.get_tree_state()
end

-- Stats selectable for the node power heat map (Data.lua powerStatList).
M.power_stats = function()
	local out = array({})
	for _, s in ipairs(data.powerStatList) do
		if not s.ignoreForNodes then
			out[#out + 1] = { stat = opt(s.stat), label = s.label }
		end
	end
	return { stats = out }
end

local function findPowerStat(stat)
	if stat then
		for _, s in ipairs(data.powerStatList) do
			if s.stat == stat then return s end
		end
	end
	return data.powerStatList[1]
end

local powerProgress = 0
local powerStarted = 0

-- Chunked node-power build for a responsive UI: start, then step until done,
-- then result. Each step resumes CalcsTab's PowerBuilder coroutine (which
-- yields roughly every 100 ms of work) until the time budget is spent.
-- params: { stat = "Life"|nil (nil = Offence/Defence), maxDepth = 10|nil }
M.tree_power_start = function(p)
	ensureBuild()
	p = p or {}
	local calcsTab = build.calcsTab
	calcsTab.powerStat = findPowerStat(p.stat)
	calcsTab.nodePowerMaxDepth = tonumber(p.maxDepth)
	calcsTab.powerBuildFlag = true
	powerProgress = 0
	powerStarted = GetTime()
	build.powerBuilderProgressCallback = function(percent) powerProgress = percent or 0 end
	calcsTab:BuildPower()
	return { done = calcsTab.powerBuilder == nil, progress = powerProgress }
end

M.tree_power_step = function(p)
	ensureBuild()
	local calcsTab = build.calcsTab
	local budget = tonumber(p and p.budgetMs) or 150
	local t0 = GetTime()
	while calcsTab.powerBuilder and GetTime() - t0 < budget do
		calcsTab:BuildPower()
	end
	return { done = calcsTab.powerBuilder == nil, progress = powerProgress }
end

local function powerResult()
	local calcsTab = build.calcsTab
	local powerStat = calcsTab.powerStat or data.powerStatList[1]
	local nodes = {}
	for id, node in pairs(build.spec.nodes) do
		local pw = node.power
		if pw and (pw.singleStat or pw.offence or pw.defence) then
			nodes[tostring(id)] = {
				s = opt(pw.singleStat),
				o = opt(pw.offence),
				d = opt(pw.defence),
				p = opt(pw.pathPower),
				dist = opt(pw.distance),
			}
		end
	end
	local report = array({})
	local ok, list = pcall(build.treeTab.BuildPowerReportList, build.treeTab, powerStat)
	if ok and type(list) == "table" then
		for i, r in ipairs(list) do
			report[i] = {
				id = r.id,
				name = r.name,
				power = r.power,
				powerStr = r.powerStr,
				pathPower = r.pathPower,
				pathPowerStr = r.pathPowerStr,
				allocated = r.allocated == true,
				pathDist = opt(r.pathDist),
				type = opt(r.type),
			}
		end
	end
	local max = calcsTab.powerMax or {}
	return {
		stat = opt(powerStat.stat),
		label = powerStat.label,
		nodes = nodes,
		max = {
			singleStat = max.singleStat or 0,
			offence = max.offence or 0,
			defence = max.defence or 0,
			offencePerPoint = max.offencePerPoint or 0,
			defencePerPoint = max.defencePerPoint or 0,
		},
		report = report,
		ms = GetTime() - powerStarted,
		rev = build.outputRevision,
	}
end

M.tree_power_result = function()
	ensureBuild()
	return powerResult()
end

-- Blocking convenience for the CLI.
M.tree_power = function(p)
	local r = M.tree_power_start(p)
	while not r.done do
		r = M.tree_power_step({ budgetMs = 1000 })
	end
	return powerResult()
end

-- ---------------------------------------------------------------------------
-- Parallel node power. The main engine lists the eligible nodes
-- (tree_power_partition), pooled worker engines score their share with the
-- same calc PowerBuilder runs (score_nodes), and the scores are written back
-- into node.power (tree_power_apply) so tree_power_result and the power
-- report are unchanged.
-- ---------------------------------------------------------------------------

local function num(v)
	if type(v) == "number" then return v end
	return nil
end

M.tree_power_partition = function(p)
	ensureBuild()
	p = p or {}
	local calcsTab = build.calcsTab
	calcsTab.powerStat = findPowerStat(p.stat)
	calcsTab.nodePowerMaxDepth = tonumber(p.maxDepth)
	powerStarted = GetTime()
	local grantedPassives = calcsTab.mainEnv and calcsTab.mainEnv.grantedPassives or {}
	local list = {}
	for nodeId, node in pairs(build.spec.nodes) do
		if node.power then wipeTable(node.power) else node.power = {} end
		if node.modKey ~= "" and not grantedPassives[nodeId] then
			local hidden = false
			if node.unlockConstraint then
				for _, unlockNodeId in ipairs(node.unlockConstraint.nodes) do
					local unlockNode = build.spec.nodes[unlockNodeId]
					if unlockNode and unlockNode.ascendancyName and not unlockNode.alloc then
						hidden = true
						break
					end
				end
			end
			if not hidden then
				local dist = node.pathDist or 1000
				for _, leap in ipairs(node.intuitiveLeapLikesAffecting or {}) do
					if leap.alloc then dist = math.max(math.min(leap.pathDist or 1000, dist), 1) end
				end
				node.power.distance = dist
				if (not calcsTab.nodePowerMaxDepth) or dist <= calcsTab.nodePowerMaxDepth then
					list[#list + 1] = { id = nodeId, dist = dist, modKey = node.modKey }
				end
			end
		end
	end
	-- identical mod keys share one calc in PowerBuilder's cache; keep them
	-- adjacent so a contiguous slice lands on one worker
	table.sort(list, function(a, b)
		if a.modKey ~= b.modKey then return a.modKey < b.modKey end
		return a.id < b.id
	end)
	-- shortest paths can tie; ship this engine's choice so workers score the
	-- same path PowerBuilder would here (pathPower depends on it)
	local ids, dists, paths = array({}), array({}), array({})
	for i, e in ipairs(list) do
		ids[i] = e.id
		dists[i] = e.dist
		local node = build.spec.nodes[e.id]
		local pathIds = array({})
		if node then
			local src = (not node.alloc) and node.path or node.depends
			for _, pn in pairs(src or {}) do
				if type(pn) == "table" and pn.id then pathIds[#pathIds + 1] = pn.id end
			end
		end
		paths[i] = pathIds
	end
	return { ids = ids, dists = dists, paths = paths, stat = opt(calcsTab.powerStat.stat), rev = build.outputRevision }
end

-- Worker side: PowerBuilder's per-node evaluation for a subset of nodes.
local function scoreNodes(p)
	local calcsTab = build.calcsTab
	local powerStat = findPowerStat(p and p.stat)
	local useFullDPS = powerStat and powerStat.stat == "FullDPS"
	local calcFunc, calcBase = calcsTab:GetMiscCalculator()
	local cache = {}
	local out = array({})
	-- the main engine's path for each node, when supplied; else this state's
	local function pathSet(i, fallback)
		local given = p.paths and p.paths[i]
		if type(given) == "table" and #given > 0 then
			local set = {}
			for _, pid in ipairs(given) do
				local pn = build.spec.nodes[pid]
				if pn then set[pn] = true end
			end
			return set
		end
		local set = {}
		for _, pn in pairs(fallback or {}) do set[pn] = true end
		return set
	end
	for i, id in ipairs(p.ids or {}) do
		local node = build.spec.nodes[id]
		if node then
			local dist = num(p.dists and p.dists[i]) or node.pathDist or 1000
			local r = { id = id, dist = dist }
			if not node.alloc then
				if not cache[node.modKey] then
					cache[node.modKey] = calcFunc({ addNodes = { [node] = true } }, useFullDPS)
				end
				local output = cache[node.modKey]
				if powerStat and powerStat.stat and not powerStat.ignoreForNodes then
					r.s = calcsTab:CalculatePowerStat(powerStat, output, calcBase)
					if node.path and not node.ascendancyName then
						r.rank = true
						r.p = r.s
						if dist > 1 then
							r.p = calcsTab:CalculatePowerStat(powerStat, calcFunc({ addNodes = pathSet(i, node.path) }, useFullDPS), calcBase)
						end
					end
				elseif not powerStat or not powerStat.ignoreForNodes then
					r.o, r.d = calcsTab:CalculateCombinedOffDefStat(output, calcBase)
					r.s = r.o
					if node.path and not node.ascendancyName then r.rank = true end
				end
			else
				local key = node.modKey .. "_remove"
				if not cache[key] then
					cache[key] = calcFunc({ removeNodes = { [node] = true } }, useFullDPS)
				end
				local output = cache[key]
				if powerStat and powerStat.stat and not powerStat.ignoreForNodes then
					r.s = calcsTab:CalculatePowerStat(powerStat, output, calcBase)
					if node.depends and not node.ascendancyName then
						r.p = r.s
						if #node.depends > 1 then
							r.p = calcsTab:CalculatePowerStat(powerStat, calcFunc({ removeNodes = pathSet(i, node.depends) }, useFullDPS), calcBase)
						end
					end
				end
			end
			out[#out + 1] = r
		end
	end
	return { nodes = out }
end

M.score_nodes = function(p)
	ensureBuild()
	local powerStat = findPowerStat(p and p.stat)
	if powerStat and powerStat.stat == "FullDPS" then
		return scoreNodes(p)
	end
	return withoutFullDPS(scoreNodes, p)
end

-- Worker side of the point planner; the main engine's tree is never changed.
M.plan_alloc = function(p)
	ensureBuild()
	local node = requireNode(p)
	local before = {}
	for id in pairs(build.spec.allocNodes) do before[id] = true end
	build.spec:AllocNode(node)
	refresh()
	local added = array({})
	for id in pairs(build.spec.allocNodes) do
		if not before[id] then added[#added + 1] = id end
	end
	return { added = added }
end

M.tree_power_apply = function(p)
	ensureBuild()
	local calcsTab = build.calcsTab
	local max = { singleStat = 0, offence = 0, defence = 0, offencePerPoint = 0, defencePerPoint = 0 }
	for _, r in ipairs(p and p.nodes or {}) do
		local node = build.spec.nodes[r.id]
		if node then
			node.power = node.power or {}
			node.power.singleStat = num(r.s)
			node.power.offence = num(r.o)
			node.power.defence = num(r.d)
			node.power.pathPower = num(r.p)
			node.power.distance = num(r.dist) or node.power.distance
			if r.rank == true then
				if node.power.singleStat then
					max.singleStat = math.max(max.singleStat, node.power.singleStat)
				end
				if node.power.offence then
					local d = node.power.distance or 1
					max.offence = math.max(max.offence, node.power.offence)
					max.defence = math.max(max.defence, node.power.defence or 0)
					max.offencePerPoint = math.max(max.offencePerPoint, node.power.offence / d)
					max.defencePerPoint = math.max(max.defencePerPoint, (node.power.defence or 0) / d)
				end
			end
		end
	end
	calcsTab.powerMax = max
	calcsTab.powerBuilderInitialized = true
	calcsTab.powerBuildFlag = false
	calcsTab.powerBuilder = nil
	return powerResult()
end

-- Jewel radii for the socket hover rings (Data.lua jewelRadii, tree units).
M.jewel_radii = function()
	local out = array({})
	-- PoE1's Data.lua bakes the distance multiplier into the radii already.
	local mult = IS_POE2 and data.gameConstants and data.gameConstants["PassiveTreeJewelDistanceMultiplier"] or 1
	for i, r in ipairs(data.jewelRadius or {}) do
		out[i] = { inner = r.inner * mult, outer = r.outer * mult, color = r.col, label = r.label }
	end
	return { radii = out }
end

M.node_info = function(p)
	ensureBuild()
	local node = requireNode(p)
	local info = nodeSummary(node.id, node)
	local mods = array({})
	if node.modList then
		for _, mod in ipairs(node.modList) do
			mods[#mods + 1] = { name = mod.name, type = mod.type, value = isScalar(mod.value) and mod.value or tostring(mod.value) }
		end
	end
	info.mods = mods
	info.icon = opt(node.icon)
	info.masteryEffects = masteryOptions(node)
	info.masterySelected = opt(build.spec.masterySelections and build.spec.masterySelections[node.id])
	return info
end

M.node_path = function(p)
	ensureBuild()
	local node = requireNode(p)
	local ids = array({})
	for i, n in ipairs(node.path or {}) do ids[i] = n.id end
	return { id = node.id, path = ids, cost = #ids, allocated = node.alloc == true }
end

-- Objectives for path_plan. Matched against a node's stat lines, so a route can
-- be judged by what it grants on the way rather than by length alone. Weights
-- separate what a category is really about from what merely correlates.
local PATH_OBJECTIVES = {
	defence = {
		{ "maximum life", 10 }, { "maximum energy shield", 10 }, { "%% increased life", 8 },
		{ "resistance", 8 }, { "armour", 6 }, { "evasion", 6 }, { "block", 6 },
		{ "suppress", 6 }, { "life regeneration", 4 }, { "recoup", 3 }, { "reduced damage taken", 10 },
		{ "stun threshold", 2 }, { "ailment", 2 },
	},
	damage = {
		{ "increased damage", 10 }, { "critical", 8 }, { "penetration", 8 },
		{ "attack speed", 7 }, { "cast speed", 7 }, { "damage over time", 7 },
		{ "accuracy", 4 }, { "%% increased.*damage", 8 }, { "added.*damage", 6 },
	},
	speed = {
		{ "movement speed", 10 }, { "attack speed", 6 }, { "cast speed", 6 },
	},
	attributes = {
		{ "strength", 8 }, { "dexterity", 8 }, { "intelligence", 8 }, { "all attributes", 12 },
	},
}

-- How much a node is worth for an objective. `objective` is a known category or
-- any substring to match against the node's stat lines.
local function nodeValue(node, objective)
	if not objective or objective == "short" then return 0 end
	local lines = node.sd
	if not lines or #lines == 0 then return 0 end
	local rules = PATH_OBJECTIVES[objective]
	local score = 0
	for _, line in ipairs(lines) do
		local text = line:lower()
		if rules then
			for _, rule in ipairs(rules) do
				if text:find(rule[1]) then score = score + rule[2] end
			end
		elseif text:find(objective:lower(), 1, true) then
			score = score + 10
		end
	end
	-- Notables carry the meaningful passives; keep them ahead of small nodes
	-- that happen to mention the same words.
	if score > 0 and node.type == "Notable" then score = score * 2 end
	return score
end

--- Cheapest route from the allocated tree to a node, preferring routes whose
--- intermediate nodes serve `objective`.
---
--- PoB's own `node.path` is shortest by node count and indifferent to what it
--- passes through. This walks the graph itself so a caller can ask for the
--- shortest route that also picks up life or damage on the way, and can spend
--- up to `max_extra` further points when that buys enough.
---
--- best[len][id] = highest objective score reachable at `id` using exactly
--- `len` unallocated nodes, so length stays a hard budget while score decides
--- between routes that cost the same.
M.path_plan = function(p)
	ensureBuild()
	local node = requireNode(p)
	local spec = build.spec
	local objective = p.objective and tostring(p.objective) or "short"
	local maxExtra = math.max(0, math.min(tonumber(p.max_extra) or 0, 12))

	if node.alloc then
		return { id = node.id, name = opt(node.dn or node.name), already_allocated = true,
			path = array({}), points = 0, shortest = 0, extra = 0, score = 0, attribute_nodes = 0 }
	end

	-- Anointing an amulet grants a notable outright ("Allocates <notable>"),
	-- which reaches the build as a GrantedPassive mod rather than a tree
	-- allocation. The calculation counts it, the spec does not, so pathing to
	-- one would spend points on something already held. Granted notables are
	-- still not walkable: nothing connects them to the tree.
	local granted = build.calcsTab and build.calcsTab.mainEnv and build.calcsTab.mainEnv.grantedPassives
	if granted and granted[node.id] then
		return { id = node.id, name = opt(node.dn or node.name), already_granted = true,
			granted_by = "an anointment or other item that allocates it",
			path = array({}), points = 0, shortest = 0, extra = 0, score = 0, attribute_nodes = 0 }
	end
	if not node.path then error("node " .. node.id .. " cannot be reached from the current tree", 0) end

	local shortest = #node.path
	local budget = shortest + maxExtra

	-- best[len] maps node id -> { score, prev }. Length 0 is the allocated tree.
	local best = { [0] = {} }
	for id in pairs(spec.allocNodes) do best[0][id] = { score = 0, prev = nil } end

	for len = 1, budget do
		best[len] = {}
		for id, entry in pairs(best[len - 1]) do
			local cur = spec.nodes[id]
			for _, other in ipairs(cur and cur.linked or {}) do
				-- Ascendancy and class-start nodes are not walkable filler.
				if not other.alloc and other.id and not other.isAscendancyStart and other.type ~= "ClassStart" then
					local score = entry.score + nodeValue(other, objective)
					local prevBest = best[len][other.id]
					if not prevBest or score > prevBest.score then
						best[len][other.id] = { score = score, prev = id, at = len - 1 }
					end
				end
			end
		end
	end

	-- Shortest wins; score only separates routes of equal length, unless the
	-- caller allowed extra points, in which case take the best score within it.
	local pickLen, pickScore
	for len = 1, budget do
		local hit = best[len] and best[len][node.id]
		if hit and (pickLen == nil or hit.score > pickScore) then
			pickLen, pickScore = len, hit.score
		end
	end
	if not pickLen then error("node " .. node.id .. " is unreachable within " .. budget .. " points", 0) end

	local ids, len, id = {}, pickLen, node.id
	while len > 0 and id do
		table.insert(ids, 1, id)
		local step = best[len][id]
		id, len = step and step.prev, len - 1
	end

	local steps, attrCount = array({}), 0
	for i, nid in ipairs(ids) do
		local n = spec.nodes[nid]
		steps[i] = nodeSummary(nid, n)
		steps[i].value = nodeValue(n, objective)
		steps[i].is_attribute = n.isAttribute == true
		if n.isAttribute then attrCount = attrCount + 1 end
	end

	return {
		id = node.id,
		name = opt(node.dn or node.name),
		objective = objective,
		path = steps,
		points = pickLen,
		shortest = shortest,
		extra = pickLen - shortest,
		score = pickScore,
		attribute_nodes = attrCount,
		attribute_index = spec.attributeIndex,
	}
end

--- What the tree's switchable attribute nodes grant: 1 Str, 2 Dex, 3 Int.
---
--- `attributeIndex` is the default applied to nodes allocated from now on, so
--- set this before pathing. Nodes already allocated keep whatever they were
--- given until `apply_to_allocated` rewrites them.
M.set_attribute_choice = function(p)
	ensureBuild()
	local attr = tonumber(p and p.attribute)
	if not attr or attr < 1 or attr > 3 then
		error("params.attribute must be 1 (Strength), 2 (Dexterity) or 3 (Intelligence)", 0)
	end
	local spec = build.spec
	spec.attributeIndex = attr
	local switched = 0
	if p.apply_to_allocated then
		for id, n in pairs(spec.allocNodes) do
			if n.isAttribute then
				spec:SwitchAttributeNode(id, attr)
				switched = switched + 1
			end
		end
	end
	spec:BuildAllDependsAndPaths()
	spec:AddUndoState()
	refresh()
	local state = M.get_tree_state()
	state.attribute_index = attr
	state.switched = switched
	return state
end

M.search_tree = function(p)
	ensureBuild()
	p = p or {}
	local query = p.query and tostring(p.query):lower() or nil
	local limit = tonumber(p.limit) or 200
	local results = array({})
	for id, node in pairs(build.spec.nodes) do
		local include = true
		if p.type and node.type ~= p.type then include = false end
		if include and p.ascendancyName ~= nil then
			if p.ascendancyName == false or p.ascendancyName == "" then
				include = node.ascendancyName == nil
			else
				include = node.ascendancyName == p.ascendancyName
			end
		end
		if include and query and query ~= "" then
			local hay = (node.dn or node.name or ""):lower()
			if not hay:find(query, 1, true) then
				include = false
				for _, line in ipairs(node.sd or {}) do
					if line:lower():find(query, 1, true) then include = true break end
				end
			end
		end
		if include then
			results[#results + 1] = nodeSummary(id, node)
			if #results >= limit then break end
		end
	end
	table.sort(results, function(a, b) return a.id < b.id end)
	return { nodes = results, truncated = (#results >= limit) }
end

M.alloc_node = function(p)
	ensureBuild()
	local node = requireNode(p)
	build.spec:AllocNode(node)
	build.spec:AddUndoState()
	refresh()
	return M.get_tree_state()
end

M.dealloc_node = function(p)
	ensureBuild()
	local node = requireNode(p)
	build.spec:DeallocNode(node)
	build.spec:AddUndoState()
	refresh()
	return M.get_tree_state()
end

M.tree_undo = function()
	ensureBuild()
	build.spec:Undo()
	refresh()
	return M.get_tree_state()
end

M.tree_redo = function()
	ensureBuild()
	build.spec:Redo()
	refresh()
	return M.get_tree_state()
end

M.reset_tree = function()
	ensureBuild()
	build.spec:ResetNodes()
	build.spec:AddUndoState()
	refresh()
	return M.get_tree_state()
end

-- Convert the active tree (or every tree) to the latest tree version. PoB
-- inserts the converted copy after the original; passives that no longer
-- exist are dropped.
M.convert_tree = function(p)
	ensureBuild()
	local target = (p and p.version) or latestTreeVersion
	if not treeVersions[target] then error("unknown tree version " .. tostring(target), 0) end
	if p and p.all then
		build.treeTab:ConvertAllToVersion(target)
	else
		build.treeTab:ConvertToVersion(target, p and p.replace == true, false)
	end
	build.modFlag = true
	refresh()
	return M.list_specs()
end

M.list_specs = function()
	ensureBuild()
	local specs = array({})
	for index, spec in ipairs(build.treeTab.specList) do
		local count = 0
		for _, node in pairs(spec.nodes) do
			if node.alloc then count = count + 1 end
		end
		local asc = spec.curAscendClassName
		specs[#specs + 1] = {
			index = index,
			title = spec.title or "Default",
			className = spec.curClassName,
			ascendClassName = (asc and asc ~= "None") and asc or null,
			allocatedNodeCount = count,
			treeVersion = spec.treeVersion,
			active = (index == build.treeTab.activeSpec),
		}
	end
	return { specs = specs, activeSpec = build.treeTab.activeSpec }
end

-- A loadout is the sets that share a name (Build.lua SyncLoadouts), so picking
-- one set brings its namesakes in the other tabs along, as the loadout
-- dropdown does. A set with no tree of its name leaves the others alone.
local function followLoadout(title)
	if not build.GetLoadoutByName then return end
	local lo = build:GetLoadoutByName(title or "Default")
	if lo then build:SetActiveLoadout(lo) end
end

M.select_spec = function(p)
	ensureBuild()
	local index = tonumber(p and p.index)
	if not index or not build.treeTab.specList[index] then error("unknown spec index", 0) end
	build.treeTab:SetActiveSpec(index)
	followLoadout(build.treeTab.specList[index].title)
	refresh()
	return M.list_specs()
end

M.create_spec = function(p)
	ensureBuild()
	local spec = new("PassiveSpec"):PassiveSpec(build, build.spec.treeVersion)
	spec.title = (p and p.title) or "New Tree"
	spec:SelectClass(build.spec.curClassId)
	spec:SelectAscendClass(build.spec.curAscendClassId)
	table.insert(build.treeTab.specList, spec)
	build.treeTab:SetActiveSpec(#build.treeTab.specList)
	refresh()
	return M.list_specs()
end

M.copy_spec = function(p)
	ensureBuild()
	local src = tonumber(p and p.index) or build.treeTab.activeSpec
	if not build.treeTab.specList[src] then error("unknown spec index", 0) end
	build.treeTab:CopyTree(src, p and p.title)
	build.treeTab:SetActiveSpec(#build.treeTab.specList)
	refresh()
	return M.list_specs()
end

M.rename_spec = function(p)
	ensureBuild()
	local spec = build.treeTab.specList[tonumber(p and p.index) or -1]
	if not spec then error("unknown spec index", 0) end
	if not p.title then error("params.title is required", 0) end
	spec.title = p.title
	build.modFlag = true
	return M.list_specs()
end

M.delete_spec = function(p)
	ensureBuild()
	local index = tonumber(p and p.index)
	if not index or not build.treeTab.specList[index] then error("unknown spec index", 0) end
	if #build.treeTab.specList <= 1 then error("cannot delete the only tree", 0) end
	table.remove(build.treeTab.specList, index)
	if index == build.treeTab.activeSpec or build.treeTab.activeSpec > #build.treeTab.specList then
		build.treeTab:SetActiveSpec(math.max(1, index - 1))
	end
	refresh()
	return M.list_specs()
end

-- ---------------------------------------------------------------------------
-- Items
-- ---------------------------------------------------------------------------

local PICK_FIELDS = { "variant", "variantAlt", "variantAlt2", "variantAlt3", "variantAlt4", "variantAlt5" }
local PICK_FLAGS = { true, "hasAltVariant", "hasAltVariant2", "hasAltVariant3", "hasAltVariant4", "hasAltVariant5" }

local function variantPickCount(item)
	if not item.variantList then return 0 end
	local n = 0
	for _, flag in ipairs(PICK_FLAGS) do
		if flag == true or item[flag] then n = n + 1 end
	end
	return n
end

local function pickNamesOf(item)
	local names = array({})
	for i, flag in ipairs(PICK_FLAGS) do
		if flag == true or item[flag] then
			local idx = item[PICK_FIELDS[i]]
			if idx and item.variantList[idx] then names[#names + 1] = item.variantList[idx] end
		end
	end
	return names
end

-- A long list (a notable per variant) is cut; `variants` carries the full count.
local function variantNamesOf(item, limit)
	local names = array({})
	for i = 1, math.min(limit, item.variantList and #item.variantList or 0) do names[i] = item.variantList[i] end
	return names
end

local function itemSummary(item)
	local sockets, runes = array({}), array({})
	for _, sock in ipairs(item.sockets or {}) do
		if sock.color then sockets[#sockets + 1] = { colour = sock.color, group = sock.group or 0 } end
	end
	for i = 1, item.itemSocketCount or 0 do
		runes[i] = item.runes and item.runes[i] or "None"
	end
	return {
		id = item.id,
		name = item.name,
		title = opt(item.title),
		baseName = opt(item.baseName),
		sockets = sockets,
		runes = runes,
		type = opt(item.type),
		rarity = opt(item.rarity),
		raw = item.raw,
		corrupted = item.corrupted == true,
		quality = opt(item.quality),
		itemLevel = opt(item.itemLevel),
		variants = item.variantList and #item.variantList or 0,
		variantPicks = variantPickCount(item),
		variantNames = variantNamesOf(item, 40),
		selectedVariants = item.variantList and pickNamesOf(item) or array({}),
		requirements = item.requirements and {
			level = opt(item.requirements.level),
			str = opt(item.requirements.str),
			dex = opt(item.requirements.dex),
			int = opt(item.requirements.int),
		} or null,
	}
end

M.list_slots = function()
	ensureBuild()
	-- PoB flags unallocated tree jewel sockets inactive (and relabels the
	-- active ones "Socket #n") only from its draw path; do it here instead
	pcall(build.itemsTab.UpdateSockets, build.itemsTab)
	-- Populate also decides which abyssal sub-sockets an item actually has,
	-- which PoB otherwise only works out while drawing the slot.
	for _, slot in pairs(build.itemsTab.slots or {}) do
		if slot.Populate then pcall(slot.Populate, slot) end
	end
	local slots = array({})
	for _, slot in ipairs(build.itemsTab.orderedSlots) do
		local item = slot.selItemId and slot.selItemId ~= 0 and build.itemsTab.items[slot.selItemId] or nil
		local shown = true
		if type(slot.shown) == "function" then
			local ok, s = pcall(slot.shown)
			shown = ok and s and true or false
		end
		slots[#slots + 1] = {
			slot = slot.slotName,
			label = opt(slot.label),
			itemId = slot.selItemId or 0,
			itemName = item and item.name or null,
			itemRarity = item and opt(item.rarity) or null,
			nodeId = opt(slot.nodeId),
			weaponSet = opt(slot.weaponSet),
			shown = shown,
			inactive = slot.inactive == true,
		}
	end
	return {
		slots = slots,
		activeItemSet = build.itemsTab.activeItemSetId,
		useSecondWeaponSet = build.itemsTab.activeItemSet.useSecondWeaponSet == true,
	}
end

M.get_items = function()
	ensureBuild()
	local items = array({})
	for _, id in ipairs(build.itemsTab.itemOrderList) do
		local item = build.itemsTab.items[id]
		if item then
			local s = itemSummary(item)
			local ok, slot = pcall(item.GetPrimarySlot, item)
			s.primarySlot = ok and opt(slot) or null
			s.compatibleSlots = array({})
			for _, candidate in ipairs(build.itemsTab.orderedSlots) do
				if build.itemsTab:IsItemValidForSlot(item, candidate.slotName) then
					s.compatibleSlots[#s.compatibleSlots + 1] = candidate.slotName
				end
			end
			-- Returns the slot control object; only its name is serialisable.
			local ok2, equipped = pcall(build.itemsTab.GetEquippedSlotForItem, build.itemsTab, item)
			s.equippedSlot = (ok2 and type(equipped) == "table" and opt(equipped.slotName)) or null
			items[#items + 1] = s
		end
	end
	return { items = items }
end

M.parse_item = function(p)
	if not p or type(p.text) ~= "string" then error("params.text is required", 0) end
	local item = new("Item"):Item(p.text)
	if not item.base then
		return { ok = false, error = "unrecognised item text" }
	end
	local out = itemSummary(item)
	out.ok = true
	return out
end

-- Slot names as callers write them: "belt", "ring1", "Weapon1", "main hand",
-- "body". Resolved against the real slot names with case and spaces ignored,
-- plus a few common synonyms.
local SLOT_SYNONYMS = {
	weapon = "Weapon 1", mainhand = "Weapon 1", weapon1 = "Weapon 1",
	offhand = "Weapon 2", shield = "Weapon 2", weapon2 = "Weapon 2",
	helm = "Helmet", head = "Helmet", chest = "Body Armour", body = "Body Armour", armour = "Body Armour", armor = "Body Armour", bodyarmor = "Body Armour",
	ring = "Ring 1", ring1 = "Ring 1", ring2 = "Ring 2", neck = "Amulet", flask = "Flask 1", charm = "Charm 1",
}
local function resolveSlotName(name)
	if name == nil then return nil end
	local raw = tostring(name)
	if build.itemsTab.slots[raw] then return raw end
	local norm = raw:lower():gsub("[%s_%-]", "")
	if SLOT_SYNONYMS[norm] and build.itemsTab.slots[SLOT_SYNONYMS[norm]] then return SLOT_SYNONYMS[norm] end
	for slotName in pairs(build.itemsTab.slots) do
		if slotName:lower():gsub("[%s_%-]", "") == norm then return slotName end
	end
	local names = {}
	for _, slot in ipairs(build.itemsTab.orderedSlots) do
		if not slot.inactive then names[#names + 1] = slot.slotName end
	end
	error("unknown slot " .. raw .. "; slots: " .. table.concat(names, ", "), 0)
end

M.equip_item_raw = function(p)
	ensureBuild(p)
	if not p or type(p.text) ~= "string" then error("params.text (raw item text) is required", 0) end
	local item = new("Item"):Item(p.text)
	if not item.base then error("could not parse item text (unrecognised base type or format)", 0) end
	local slotName = resolveSlotName(p.slot)
	if slotName and not build.itemsTab:IsItemValidForSlot(item, slotName) then
		error(item.name .. " does not fit " .. slotName, 0)
	end
	if not slotName then
		for _, slot in ipairs(build.itemsTab.orderedSlots) do
			if not slot.inactive and build.itemsTab:IsItemValidForSlot(item, slot.slotName) then
				slotName = slot.slotName
				break
			end
		end
	end
	if not slotName or not build.itemsTab.slots[slotName] then
		error("no compatible slot found for this item; pass params.slot", 0)
	end
	build.itemsTab:AddItem(item, true)
	build.itemsTab.slots[slotName]:SetSelItemId(item.id)
	build.itemsTab:AddUndoState()
	refresh()
	return { ok = true, itemId = item.id, slot = slotName, itemName = item.name }
end

M.equip_item = function(p)
	ensureBuild()
	local slot = build.itemsTab.slots[resolveSlotName(p and p.slot) or ""]
	if not slot then error("unknown slot", 0) end
	local id = tonumber(p.itemId) or 0
	if id ~= 0 and not build.itemsTab.items[id] then error("unknown item id", 0) end
	if id ~= 0 and not build.itemsTab:IsItemValidForSlot(build.itemsTab.items[id], slot.slotName) then
		error(build.itemsTab.items[id].name .. " does not fit " .. slot.slotName, 0)
	end
	slot:SetSelItemId(id)
	build.itemsTab:AddUndoState()
	refresh()
	return M.list_slots()
end

M.unequip_item = function(p)
	return M.equip_item({ slot = p and p.slot, itemId = 0 })
end

M.delete_item = function(p)
	ensureBuild()
	local item = build.itemsTab.items[tonumber(p and p.itemId) or -1]
	if not item then error("unknown item id", 0) end
	build.itemsTab:DeleteItem(item)
	build.itemsTab:AddUndoState()
	refresh()
	return M.get_items()
end

M.list_item_sets = function()
	ensureBuild()
	local sets = array({})
	for _, id in ipairs(build.itemsTab.itemSetOrderList) do
		local set = build.itemsTab.itemSets[id]
		sets[#sets + 1] = { id = id, title = set.title or "Default", active = (id == build.itemsTab.activeItemSetId) }
	end
	return { itemSets = sets, activeItemSet = build.itemsTab.activeItemSetId }
end

M.select_item_set = function(p)
	ensureBuild()
	local id = tonumber(p and p.id)
	if not id or not build.itemsTab.itemSets[id] then error("unknown item set id", 0) end
	build.itemsTab:SetActiveItemSet(id)
	followLoadout(build.itemsTab.itemSets[id].title)
	refresh()
	return M.list_item_sets()
end

M.create_item_set = function(p)
	ensureBuild()
	local set = build.itemsTab:NewItemSet(nil, (p and p.title) or "New Set")
	build.itemsTab:SetActiveItemSet(set.id)
	refresh()
	return M.list_item_sets()
end

M.copy_item_set = function(p)
	ensureBuild()
	local src = tonumber(p and p.id) or build.itemsTab.activeItemSetId
	if not build.itemsTab.itemSets[src] then error("unknown item set id", 0) end
	local set = build.itemsTab:CopyItemSet(src, p and p.title)
	build.itemsTab:SetActiveItemSet(set.id)
	refresh()
	return M.list_item_sets()
end

M.rename_item_set = function(p)
	ensureBuild()
	local id = tonumber(p and p.id)
	if not id or not build.itemsTab.itemSets[id] then error("unknown item set id", 0) end
	if not p.title then error("params.title is required", 0) end
	build.itemsTab:RenameItemSet(id, p.title)
	return M.list_item_sets()
end

M.delete_item_set = function(p)
	ensureBuild()
	local id = tonumber(p and p.id)
	if not id or not build.itemsTab.itemSets[id] then error("unknown item set id", 0) end
	if #build.itemsTab.itemSetOrderList <= 1 then error("cannot delete the only item set", 0) end
	local orderIndex
	for i, sid in ipairs(build.itemsTab.itemSetOrderList) do
		if sid == id then orderIndex = i break end
	end
	build.itemsTab:DeleteItemSet(id, orderIndex)
	build.itemsTab:SetActiveItemSet(build.itemsTab.activeItemSetId)
	refresh()
	return M.list_item_sets()
end

-- ---------------------------------------------------------------------------
-- Skills
-- ---------------------------------------------------------------------------

-- A group by index, or by the name of its skill (case-insensitive, the first
-- active gem or the group label). Callers that guess an index without looking
-- delete the wrong group; a name is what they actually know.
local function findGroupByName(name)
	local want = tostring(name):lower():gsub("^%s+", ""):gsub("%s+$", "")
	if want == "" then return nil end
	local labels = {}
	for i, group in ipairs(build.skillsTab.socketGroupList) do
		local first
		for _, gem in ipairs(group.gemList) do
			local gd = gem.gemData
			if gd and not (gd.grantedEffect and gd.grantedEffect.support) then first = gd.name break end
		end
		local label = first or group.displayLabel or group.label or ""
		labels[#labels + 1] = string.format("%d: %s", i, label)
		if label:lower() == want or (group.displayLabel or ""):lower() == want then return i, group end
	end
	for i, group in ipairs(build.skillsTab.socketGroupList) do
		for _, gem in ipairs(group.gemList) do
			if gem.gemData and (gem.gemData.name or ""):lower() == want then return i, group end
		end
	end
	return nil, nil, labels
end

local function requireGroup(index, name)
	local n = tonumber(index)
	if not n and name ~= nil then
		local i, group, labels = findGroupByName(name)
		if not group then error("no socket group with skill " .. tostring(name) .. "; groups: " .. table.concat(labels or {}, ", "), 0) end
		return group, i
	end
	local group = build.skillsTab.socketGroupList[n or -1]
	if not group then error("unknown socket group index " .. tostring(index), 0) end
	return group, n
end

-- One entry per skill the group grants, with the extra selectors PoB shows on
-- the build screen for the chosen skill (RefreshSkillSelectControls).
local function groupSkills(group)
	local skills = array({})
	for i, activeSkill in ipairs(group.displaySkillList or {}) do
		local ae = activeSkill.activeEffect
		local ge = ae and ae.grantedEffect
		local entry = { index = i }
		local ok, nm = pcall(build.calcsTab.calcs.getActiveSkillDisplayName, activeSkill)
		entry.name = (ok and nm) or (ge and ge.name) or "?"
		if i == (group.mainActiveSkill or 1) and ge and ae.srcInstance then
			local src = ae.srcInstance
			if ge.parts and #ge.parts > 1 then
				local parts = array({})
				for pi, part in ipairs(ge.parts) do
					parts[pi] = { name = tostring(part.name), stages = part.stages and true or false }
				end
				entry.parts = parts
				entry.part = src.skillPart or 1
			end
			if ge.statSets and #ge.statSets > 1 then
				local sets = array({})
				for _, s in ipairs(ge.statSets) do sets[#sets + 1] = tostring(s.label) end
				entry.statSets = sets
				entry.statSet = src.statSet and src.statSet[ge.id] or 1
			end
			local flags = ae.statSet and ae.statSet.skillFlags or {}
			local partStages = ge.parts and #ge.parts > 1 and ge.parts[src.skillPart or 1] and ge.parts[src.skillPart or 1].stages
			if flags.multiStage or partStages then
				entry.hasStages = true
				entry.stageCount = src.skillStageCount or (activeSkill.skillData and activeSkill.skillData.stagesMin) or 1
			end
			if flags.mine then
				entry.hasMines = true
				entry.mineCount = opt(src.skillMineCount)
			end
			local minionList = activeSkill.minionList or ge.minionList
			if not flags.disable and minionList and minionList[1] then
				local minions = array({})
				for _, mid in ipairs(minionList) do
					local m = data.minions[mid]
					if m then minions[#minions + 1] = { id = mid, name = m.name } end
				end
				if #minions > 0 then
					entry.minions = minions
					entry.minion = opt(src.skillMinion)
				end
			end
			-- Raise Spectre and Companion take their minion from the player's
			-- library rather than a fixed list, which upstream spots by the
			-- granted effect carrying an empty one. Only those skills get the
			-- library button beside the minion dropdown (Build.lua).
			if not flags.disable and ge and ge.minionList and not ge.minionList[1] then
				local name = ge.name or ""
				if not IS_POE2 then
					entry.minionLibrary = "spectre"
				elseif name:match("^Spectre:") then
					entry.minionLibrary = "spectre"
				elseif name:match("^Companion:") then
					entry.minionLibrary = "beast"
				end
			end
			if activeSkill.minion and activeSkill.minion.activeSkillList and activeSkill.minion.activeSkillList[1] then
				local ms = array({})
				for _, msk in ipairs(activeSkill.minion.activeSkillList) do
					ms[#ms + 1] = msk.activeEffect.grantedEffect.name
				end
				entry.minionSkills = ms
				entry.minionSkill = src.skillMinionSkill or 1
			end
		end
		skills[#skills + 1] = entry
	end
	return skills
end

M.get_skills = function()
	ensureBuild()
	-- SkillsTab's slot.count.shown; local here because the main chunk is at LuaJIT's 200-local limit.
	local function gemCountable(group, gi, gem)
		if IS_POE2 and gi == 1 and (group.source or group.sourceItem or group.sourceNode) then return false end
		local list = gem.gemData and gem.gemData.grantedEffectList or { gem.grantedEffect }
		for n, ge in ipairs(list) do
			local hidden = IS_POE2 and ge.hideFromSideBar or (not IS_POE2 and ge.unsupported)
			if not ge.support and not hidden and (not ge.hasGlobalEffect or gem["enableGlobal" .. n]) then
				return true
			end
		end
		return false
	end
	local groups = array({})
	for i, group in ipairs(build.skillsTab.socketGroupList) do
		local gems = array({})
		for gi, gem in ipairs(group.gemList) do
			local gd = gem.gemData
			gems[#gems + 1] = {
				index = gi,
				nameSpec = opt(gem.nameSpec),
				name = gd and gd.name or opt(gem.nameSpec),
				gemId = opt(gem.gemId),
				skillId = opt(gem.skillId),
				level = opt(gem.level),
				maxLevel = gd and gd.naturalMaxLevel or 20,
				quality = opt(gem.quality),
				enabled = gem.enabled ~= false,
				support = (gd and gd.grantedEffect and gd.grantedEffect.support) and true or false,
				color = opt(gem.color),
				socketColour = opt(gd and gd.grantedEffect and ({ "R", "G", "B", "W" })[gd.grantedEffect.color]),
				count = opt(gem.count),
				countable = gemCountable(group, gi, gem),
				errMsg = opt(gem.errMsg),
				-- Set for skills the game grants (default weapon attacks, Raise
				-- Shield, unique-granted skills): not a socket the player filled.
				granted = opt(grantedGemNote(gd)),
			}
		end
		groups[#groups + 1] = {
			index = i,
			label = opt(group.label),
			displayLabel = opt(group.displayLabel),
			enabled = group.enabled ~= false,
			includeInFullDPS = group.includeInFullDPS == true,
			slot = opt(group.slot),
			source = opt(group.source),
			groupCount = group.source and math.max(tonumber(group.groupCount) or 1, 1) or null,
			-- Set on groups a weapon, shield or other item grants: the skill
			-- comes with the item and is not a socket the player filled.
			grantedBy = grantedBy(group),
			mainActiveSkill = opt(group.mainActiveSkill),
			gems = gems,
			skills = groupSkills(group),
			isMainSkill = (build.mainSocketGroup == i),
		}
	end
	-- An item's granted copy of a skill the player also has socketed (the
	-- game lists Raise Shield and Mace Strike as skills so supports can go on
	-- them, and PoB adds a support-less copy for the item): point each at the
	-- other so a reader sees one skill, not two.
	local function firstSkillName(group)
		for _, gem in ipairs(group.gemList) do
			local gd = gem.gemData
			if gd and gd.grantedEffect and not gd.grantedEffect.support then return gd.name end
		end
		return nil
	end
	for i, group in ipairs(build.skillsTab.socketGroupList) do
		if group.source and group.sourceItem then
			local name = firstSkillName(group)
			for j, other in ipairs(build.skillsTab.socketGroupList) do
				if j ~= i and not other.source and name and firstSkillName(other) == name then
					groups[i].duplicateOf = j
					groups[j].grantedCopy = i
					break
				end
			end
		end
	end
	local sets = array({})
	for _, id in ipairs(build.skillsTab.skillSetOrderList) do
		local set = build.skillsTab.skillSets[id]
		if set then
			sets[#sets + 1] = { id = id, title = set.title or "Default", active = (id == build.skillsTab.activeSkillSetId) }
		end
	end
	return {
		socketGroups = groups,
		mainSocketGroup = opt(build.mainSocketGroup),
		skillSets = sets,
		activeSkillSet = opt(build.skillsTab.activeSkillSetId),
	}
end

-- Selectors for the chosen skill of a group (part, stat set, stages, minion…).
M.set_main_skill_options = function(p)
	ensureBuild()
	local group = requireGroup(p and p.groupIndex)
	if p.mainActiveSkill ~= nil then group.mainActiveSkill = tonumber(p.mainActiveSkill) end
	local activeSkill = group.displaySkillList and group.displaySkillList[group.mainActiveSkill or 1]
	local ae = activeSkill and activeSkill.activeEffect
	local src = ae and ae.srcInstance
	if src then
		if p.part ~= nil then src.skillPart = tonumber(p.part) end
		if p.statSet ~= nil and ae.grantedEffect then
			src.statSet = src.statSet or {}
			src.statSet[ae.grantedEffect.id] = tonumber(p.statSet)
		end
		if p.stageCount ~= nil then src.skillStageCount = tonumber(p.stageCount) end
		if p.mineCount ~= nil then src.skillMineCount = tonumber(p.mineCount) end
		if p.minionId ~= nil then
			src.skillMinion = p.minionId
			src.skillMinionCalcs = p.minionId
		end
		if p.minionSkill ~= nil then src.skillMinionSkill = tonumber(p.minionSkill) end
	end
	build.modFlag = true
	refresh()
	return M.get_skills()
end

M.move_socket_group = function(p)
	ensureBuild()
	local list = build.skillsTab.socketGroupList
	local from = tonumber(p and p.from)
	local to = tonumber(p and p.to)
	if not from or not to or not list[from] or to < 1 or to > #list then error("bad from/to", 0) end
	local group = table.remove(list, from)
	table.insert(list, to, group)
	local m = build.mainSocketGroup or 1
	if m == from then
		build.mainSocketGroup = to
	elseif from < m and to >= m then
		build.mainSocketGroup = m - 1
	elseif from > m and to <= m then
		build.mainSocketGroup = m + 1
	end
	build.skillsTab:AddUndoState()
	refresh()
	return M.get_skills()
end

M.move_gem = function(p)
	ensureBuild()
	local group = requireGroup(p and p.groupIndex)
	local from = tonumber(p and p.from)
	local to = tonumber(p and p.to)
	if not from or not to or not group.gemList[from] or to < 1 or to > #group.gemList then error("bad from/to", 0) end
	local gem = table.remove(group.gemList, from)
	table.insert(group.gemList, to, gem)
	build.skillsTab:ProcessSocketGroup(group)
	build.skillsTab:AddUndoState()
	refresh()
	return M.get_skills()
end

local gemDpsCache

-- Gems worth scoring for a group: supports valid for one of its active
-- skills, plus actives with a global effect.
local function gemDpsCandidates(group)
	local displaySkills = group.displaySkillList or {}
	local ids = {}
	for gemId, gemData in pairs(data.gems) do
		local ge = gemData.grantedEffect
		if ge and not ge.hidden then
			local score = false
			if ge.support then
				for _, activeSkill in ipairs(displaySkills) do
					local ok, s = pcall(calcLib.canGrantedEffectSupportActiveSkill, ge, activeSkill)
					if ok and s then score = true break end
				end
			elseif ge.hasGlobalEffect then
				score = true
			end
			if score then ids[#ids + 1] = gemId end
		end
	end
	table.sort(ids)
	return ids
end

-- Score gems for a group the way GemSelectControl does: a hypothetical gem
-- instance is appended, the misc calculator runs, the instance is removed.
local function scoreGems(group, gemIds, dpsField)
	local useFullDPS = dpsField == "FullDPS"
	local fastCalcOptions = { nodeAlloc = true, requirementsItems = true, requirementsGems = true, skipEHP = dpsField ~= "TotalEHP", fullDPSOnly = useFullDPS }
	local calcFunc, calcBase = build.calcsTab:GetMiscCalculator(build)
	local function fieldOf(output)
		return (useFullDPS and output[dpsField] ~= nil and output[dpsField])
			or (output.Minion and output.Minion.CombinedDPS)
			or (output[dpsField] ~= nil and output[dpsField])
			or 0
	end
	local dps = {}
	local gemList = group.gemList
	local slotIndex = #gemList + 1
	for _, gemId in ipairs(gemIds) do
		local gemData = data.gems[gemId]
		if gemData then
			gemList[slotIndex] = {
				level = 1,
				quality = build.skillsTab.defaultGemQuality or 0,
				count = 1,
				enabled = true,
				enableGlobal1 = true,
				enableGlobal2 = true,
				gemId = gemData.id,
				nameSpec = gemData.name,
				skillId = gemData.grantedEffectId,
			}
			gemList[slotIndex].level = build.skillsTab:ProcessGemLevel(gemData)
			gemList[slotIndex].gemData = gemData
			local ok, output
			if useFullDPS then
				ok, output = pcall(calcFunc, nil, useFullDPS, fastCalcOptions)
			else
				ok, output = pcall(withoutFullDPS, calcFunc, nil, useFullDPS, fastCalcOptions)
			end
			gemList[slotIndex] = nil
			if ok and output then
				dps[gemId] = fieldOf(output)
			end
		end
	end
	return dps, fieldOf(calcBase)
end

local function gemDpsKey(groupIndex, dpsField)
	return string.format("%d:%d:%s", build.outputRevision, groupIndex, dpsField)
end

-- Cached per (revision, group, field): ~0.7s cold single-threaded, free
-- afterwards; the pool path (gem_dps_candidates / score_gems /
-- gem_dps_apply) fills the same cache from worker engines.
local function gemDpsFor(group, groupIndex, field)
	local dpsField = field or build.skillsTab.sortGemsByDPSField or "FullDPS"
	local key = gemDpsKey(groupIndex, dpsField)
	if gemDpsCache and gemDpsCache.key == key then
		return gemDpsCache
	end
	local dps, base = scoreGems(group, gemDpsCandidates(group), dpsField)
	gemDpsCache = { key = key, base = base, dps = dps }
	return gemDpsCache
end

M.gem_dps_candidates = function(p)
	ensureBuild()
	local groupIndex = tonumber(p and p.groupIndex)
	local group = groupIndex and build.skillsTab.socketGroupList[groupIndex]
	if not group then error("unknown socket group", 0) end
	local dpsField = build.skillsTab.sortGemsByDPSField or "FullDPS"
	local key = gemDpsKey(groupIndex, dpsField)
	if gemDpsCache and gemDpsCache.key == key then
		return { cached = true, key = key, gemIds = array({}), dpsField = dpsField }
	end
	return { cached = false, key = key, gemIds = strArray(gemDpsCandidates(group)), dpsField = dpsField }
end

-- Worker side.
M.score_gems = function(p)
	ensureBuild()
	local groupIndex = tonumber(p and p.groupIndex)
	local group = groupIndex and build.skillsTab.socketGroupList[groupIndex]
	if not group then error("unknown socket group", 0) end
	local dps, base = scoreGems(group, p.gemIds or {}, p.dpsField or "FullDPS")
	return { dps = dps, base = base }
end

M.gem_dps_apply = function(p)
	ensureBuild()
	if not p or not p.key then error("params.key is required", 0) end
	local dps = {}
	for gemId, v in pairs(p.dps or {}) do
		if type(v) == "number" then dps[gemId] = v end
	end
	gemDpsCache = { key = p.key, base = num(p.base) or 0, dps = dps }
	return { ok = true, count = (function() local n = 0 for _ in pairs(dps) do n = n + 1 end return n end)() }
end

-- Gem picker: name/tag search plus per-group support validity, PoB's colours,
-- optional DPS-impact scoring and ordering.
M.gem_search = function(p)
	ensureBuild()
	p = p or {}
	local limit = tonumber(p.limit) or 100
	local q = (p.query or ""):lower()
	local groupIndex = tonumber(p.groupIndex)
	local group = groupIndex and build.skillsTab.socketGroupList[groupIndex]
	local activeSkill = group and group.displaySkillList and group.displaySkillList[group.mainActiveSkill or 1]
	local gemColor = { [1] = colorCodes.STRENGTH, [2] = colorCodes.DEXTERITY, [3] = colorCodes.INTELLIGENCE }
	local dpsCache = (p.sortByDps and group) and gemDpsFor(group, groupIndex) or nil
	local out = array({})
	local total = 0
	for gemId, gemData in pairs(data.gems) do
		local ge = gemData.grantedEffect
		if ge and not ge.hidden then
			local match = q == ""
				or gemData.name:lower():find(q, 1, true) ~= nil
				or (gemData.tagString or ""):lower():find(q, 1, true) ~= nil
			local support = ge.support == true
			if match and (p.onlySupports == nil or support == p.onlySupports) then
				total = total + 1
				local valid = true
				if support and activeSkill then
					local ok, s = pcall(calcLib.canGrantedEffectSupportActiveSkill, ge, activeSkill)
					valid = (ok and s) and true or false
				end
				local row = {
					gemId = gemId,
					name = gemData.name or gemId,
					support = support,
					valid = valid,
					color = gemColor[ge.color] or "^7",
					tags = opt(gemData.tagString),
					family = opt(gemData.gemFamily),
					gemType = opt(gemData.gemType),
					maxLevel = gemData.naturalMaxLevel or 20,
					tier = opt(gemData.Tier),
					legacy = ge.legacy and true or false,
				}
				if dpsCache and dpsCache.dps[gemId] then
					row.dps = dpsCache.dps[gemId]
					row.dpsDiff = dpsCache.dps[gemId] - dpsCache.base
				end
				out[#out + 1] = row
			end
		end
	end
	table.sort(out, function(a, b)
		if a.valid ~= b.valid then return a.valid end
		if dpsCache then
			local da, db = a.dpsDiff, b.dpsDiff
			if (da ~= nil) ~= (db ~= nil) then return da ~= nil end
			if da and db and da ~= db then return da > db end
		end
		if a.support ~= b.support then return b.support end
		return a.name < b.name
	end)
	while #out > limit do table.remove(out) end
	return { gems = out, total = total, truncated = (total > #out), baseDps = dpsCache and dpsCache.base or null }
end

-- Per-build gem defaults; SkillsTab persists them in the build file.
M.get_skills_options = function()
	ensureBuild()
	local tab = build.skillsTab
	return {
		defaultGemLevel = tab.defaultGemLevel or "normalMaximum",
		defaultGemQuality = tab.defaultGemQuality or 0,
		sortGemsByDPS = tab.sortGemsByDPS ~= false,
		sortGemsByDPSField = tab.sortGemsByDPSField or "FullDPS",
		showSupportGemTypes = tab.showSupportGemTypes or "ALL",
		showLegacyGems = tab.showLegacyGems == true,
		sortFields = array({ "FullDPS", "CombinedDPS", "TotalDPS", "AverageDamage", "TotalDot", "BleedDPS", "IgniteDPS", "TotalPoisonDPS", "TotalEHP" }),
		supportTypes = array(IS_POE2 and { "ALL", "LINEAGE", "NORMAL" } or { "ALL", "NORMAL", "EXCEPTIONAL" }),
	}
end

M.set_skills_options = function(p)
	ensureBuild()
	p = p or {}
	if p.defaultGemLevel ~= nil then
		build.skillsTab.defaultGemLevel = p.defaultGemLevel
		pcall(function() build.skillsTab.controls.defaultLevel:SelByValue(p.defaultGemLevel, "gemLevel") end)
	end
	if p.defaultGemQuality ~= nil then
		build.skillsTab.defaultGemQuality = math.max(math.min(tonumber(p.defaultGemQuality) or 0, 23), 0)
		pcall(function() build.skillsTab.controls.defaultQuality:SetText(tostring(build.skillsTab.defaultGemQuality)) end)
	end
	if p.sortGemsByDPSField ~= nil then build.skillsTab.sortGemsByDPSField = p.sortGemsByDPSField end
	if p.sortGemsByDPS ~= nil then build.skillsTab.sortGemsByDPS = p.sortGemsByDPS == true end
	if p.showSupportGemTypes ~= nil then build.skillsTab.showSupportGemTypes = tostring(p.showSupportGemTypes) end
	if p.showLegacyGems ~= nil then build.skillsTab.showLegacyGems = p.showLegacyGems == true end
	build.modFlag = true
	return M.get_skills_options()
end

M.copy_socket_group = function(p)
	ensureBuild()
	local group = requireGroup(p and p.index)
	build.skillsTab:CopySocketGroup(group)
	local text = __clipboard
	__clipboard = nil
	return { text = text or "" }
end

M.paste_socket_group = function(p)
	ensureBuild()
	if not p or type(p.text) ~= "string" or p.text == "" then error("params.text is required", 0) end
	local before = #build.skillsTab.socketGroupList
	build.skillsTab:PasteSocketGroup(p.text)
	if #build.skillsTab.socketGroupList == before then
		error("no valid socket group found in the pasted text", 0)
	end
	refresh()
	return M.get_skills()
end

local gemTooltipModule

-- A gem instance for tooltips: the socketed one, or a temporary instance of
-- any gem id at the build's default level, for a gem not in the build.
local function gemInstanceFor(p)
	p = p or {}
	if p.gemId then
		local gemData = data.gems[p.gemId]
		if not gemData then
			-- A display name works too; the id is what list_gems returns, the
			-- name is what the user said.
			local want = tostring(p.gemId):lower()
			for _, gd in pairs(data.gems) do
				if gd.name and gd.name:lower() == want and gd.grantedEffect and not gd.grantedEffect.hidden then gemData = gd break end
			end
		end
		if not gemData then error("unknown gem " .. tostring(p.gemId) .. "; see list_gems", 0) end
		return {
			level = build.skillsTab:ProcessGemLevel(gemData),
			quality = build.skillsTab.defaultGemQuality or 0,
			count = 1,
			enabled = true,
			enableGlobal1 = true,
			enableGlobal2 = true,
			gemId = gemData.id,
			gemData = gemData,
			nameSpec = gemData.name,
			skillId = gemData.grantedEffectId,
		}
	end
	if p.groupIndex == nil and p.skill == nil then error("group_index, skill or gem_id is required", 0) end
	local group = requireGroup(p.groupIndex, p.skill)
	local gemIndex = tonumber(p.gemIndex)
	if not gemIndex then
		for i, g in ipairs(group.gemList) do
			if g.gemData and not (g.gemData.grantedEffect and g.gemData.grantedEffect.support) then gemIndex = i break end
		end
	end
	local gem = group.gemList[gemIndex or -1]
	if not gem then error("unknown gem index", 0) end
	if not gem.gemData then error("gem is not resolved to any gem data", 0) end
	return gem
end

-- PoB's own gem tooltip (GemTooltip.lua), returned as sized, colour-coded lines.
M.gem_tooltip = function(p)
	ensureBuild()
	local gem = gemInstanceFor(p)
	gemTooltipModule = gemTooltipModule or LoadModule("Classes/GemTooltip")
	local tt = new("Tooltip"):Tooltip()
	local ok, err = pcall(gemTooltipModule.AddGemTooltip, tt, build, gem)
	if not ok then error("tooltip failed: " .. tostring(err), 0) end
	return tooltipPayload(tt)
end

-- Every gem name with what kind of thing it is, for highlighting names in
-- prose. `spirit` covers the gems that are switched on once and reserve:
-- persistent buffs and meta gems.
M.gem_names = function()
	local out = array({})
	for gemId, gemData in pairs(data.gems) do
		local ge = gemData.grantedEffect
		if ge and not ge.hidden and gemData.name and gemData.name ~= "" then
			local kind = "skill"
			if ge.support then
				kind = "support"
			else
				local press = gemPressClass(gemData)
				if press == "persistent" or press == "meta" then kind = "spirit" end
			end
			out[#out + 1] = { name = gemData.name, gemId = gemId, kind = kind }
		end
	end
	table.sort(out, function(a, b) return a.name < b.name end)
	return { gems = out }
end

-- What a skill does, in PoB's own words: the gem tooltip as plain text. Takes a
-- socketed gem (group and gem index) or any gem id, which is rendered at the
-- build's default gem level so a candidate can be read before it is added.
M.skill_info = function(p)
	ensureBuild()
	local gem = gemInstanceFor(p)
	gemTooltipModule = gemTooltipModule or LoadModule("Classes/GemTooltip")
	local tt = new("Tooltip"):Tooltip()
	local ok, err = pcall(gemTooltipModule.AddGemTooltip, tt, build, gem)
	if not ok then error("tooltip failed: " .. tostring(err), 0) end
	local lines = array({})
	for _, l in ipairs(tt.lines) do
		if l.text then
			local text = l.text:gsub("%^x%x%x%x%x%x%x", ""):gsub("%^%d", ""):gsub("^%s+", "")
			if text ~= "" then lines[#lines + 1] = text end
		end
	end
	local gd = gem.gemData
	local tags = {}
	for k, v in pairs(gd.tags or {}) do if v then tags[#tags + 1] = k end end
	table.sort(tags)
	return {
		name = gd.name,
		gemId = gd.id,
		level = gem.level,
		support = (gd.grantedEffect and gd.grantedEffect.support) and true or false,
		press = gemPressClass(gd),
		tags = tags,
		lines = lines,
	}
end

M.select_skill_set = function(p)
	ensureBuild()
	local id = tonumber(p and p.id)
	if not id or not build.skillsTab.skillSets[id] then error("unknown skill set id " .. tostring(p and p.id), 0) end
	build.skillsTab:SetActiveSkillSet(id)
	followLoadout(build.skillsTab.skillSets[id].title)
	refresh()
	return M.get_skills()
end

M.create_skill_set = function(p)
	ensureBuild()
	local set = build.skillsTab:NewSkillSet(nil, (p and p.title) or "New Set")
	build.skillsTab:SetActiveSkillSet(set.id)
	refresh()
	return M.get_skills()
end

M.copy_skill_set = function(p)
	ensureBuild()
	local src = tonumber(p and p.id) or build.skillsTab.activeSkillSetId
	if not build.skillsTab.skillSets[src] then error("unknown skill set id", 0) end
	local set = build.skillsTab:CopySkillSet(src, p and p.title)
	build.skillsTab:SetActiveSkillSet(set.id)
	refresh()
	return M.get_skills()
end

M.rename_skill_set = function(p)
	ensureBuild()
	local id = tonumber(p and p.id)
	if not id or not build.skillsTab.skillSets[id] then error("unknown skill set id", 0) end
	if not p.title then error("params.title is required", 0) end
	build.skillsTab:RenameSkillSet(id, p.title)
	return M.get_skills()
end

M.delete_skill_set = function(p)
	ensureBuild()
	local id = tonumber(p and p.id)
	if not id or not build.skillsTab.skillSets[id] then error("unknown skill set id", 0) end
	if #build.skillsTab.skillSetOrderList <= 1 then error("cannot delete the only skill set", 0) end
	local orderIndex
	for i, sid in ipairs(build.skillsTab.skillSetOrderList) do
		if sid == id then orderIndex = i break end
	end
	build.skillsTab:DeleteSkillSet(id, orderIndex)
	if build.skillsTab.activeSkillSetId == id then
		build.skillsTab:SetActiveSkillSet(build.skillsTab.skillSetOrderList[1])
	end
	refresh()
	return M.get_skills()
end

M.add_socket_group = function(p)
	ensureBuild()
	local group = { label = (p and p.label) or "", enabled = true, gemList = {} }
	if p and p.slot then group.slot = p.slot end
	table.insert(build.skillsTab.socketGroupList, group)
	if not build.mainSocketGroup or build.mainSocketGroup == 0 then
		build.mainSocketGroup = #build.skillsTab.socketGroupList
	end
	build.skillsTab:ProcessSocketGroup(group)
	build.skillsTab:AddUndoState()
	refresh()
	return { ok = true, groupIndex = #build.skillsTab.socketGroupList }
end

M.remove_socket_group = function(p)
	ensureBuild()
	p = p or {}
	local group, index = requireGroup(p.index, p.skill)
	if group.source then error("socket group " .. index .. " is granted by an item; unequip the item instead", 0) end
	table.remove(build.skillsTab.socketGroupList, index)
	-- Same bookkeeping as SkillListControl:OnSelDelete: the main skill and the
	-- calcs tab both hold a group index, and the groups after the removed one
	-- have all moved up.
	if build.mainSocketGroup and build.mainSocketGroup > index then
		build.mainSocketGroup = build.mainSocketGroup - 1
	end
	if build.mainSocketGroup and build.mainSocketGroup > #build.skillsTab.socketGroupList then
		build.mainSocketGroup = math.max(1, #build.skillsTab.socketGroupList)
	end
	local calcsInput = build.calcsTab and build.calcsTab.input
	if calcsInput and calcsInput.skill_number and calcsInput.skill_number > index then
		calcsInput.skill_number = calcsInput.skill_number - 1
	end
	build.skillsTab:AddUndoState()
	refresh()
	return M.get_skills()
end

M.set_socket_group = function(p)
	ensureBuild()
	p = p or {}
	local group = requireGroup(p.index, p.skill)
	if p.enabled ~= nil then group.enabled = p.enabled end
	if p.includeInFullDPS ~= nil then group.includeInFullDPS = p.includeInFullDPS end
	if p.label ~= nil then group.label = p.label end
	if p.slot ~= nil then group.slot = (p.slot ~= "" and p.slot) or nil end
	if p.mainActiveSkill ~= nil then group.mainActiveSkill = tonumber(p.mainActiveSkill) end
	if p.groupCount ~= nil then group.groupCount = math.max(tonumber(p.groupCount) or 1, 1) end
	build.skillsTab:ProcessSocketGroup(group)
	build.skillsTab:AddUndoState()
	refresh()
	return M.get_skills()
end

M.set_main_skill = function(p)
	ensureBuild()
	p = p or {}
	local _, index = requireGroup(p.index, p.skill)
	build.mainSocketGroup = index
	refresh()
	return M.get_skills()
end

M.add_gem = function(p)
	ensureBuild()
	if not p or not p.groupIndex or not (p.gemId or p.skillId or p.nameSpec) then
		error("params.groupIndex and params.gemId (or skillId/nameSpec) are required", 0)
	end
	local group = requireGroup(p.groupIndex)
	local gemData = p.gemId and data.gems[p.gemId]
	local level = tonumber(p.level)
	if not level then
		-- The level the character can use, so a new gem never arrives at level
		-- 20 in a level 30 build with a requirement to match.
		level = gemData and usableGemLevel(gemData, build.characterLevel) or 1
	end
	local gem = {
		nameSpec = p.nameSpec or "",
		gemId = p.gemId,
		skillId = p.skillId,
		level = level,
		quality = tonumber(p.quality) or build.skillsTab.defaultGemQuality or 0,
		enabled = true,
		count = 1,
		enableGlobal1 = true,
		enableGlobal2 = true,
		corrupted = build.skillsTab.defaultCorruptionState or false,
		corruptLevel = build.skillsTab.defaultCorruptionLevel or 0,
	}
	table.insert(group.gemList, gem)
	build.skillsTab:ProcessSocketGroup(group)
	-- A gem named by nameSpec is only resolved by ProcessSocketGroup; give it
	-- the usable level now that its data is known.
	if not tonumber(p.level) and not gemData and gem.gemData then
		gem.level = usableGemLevel(gem.gemData, build.characterLevel)
		build.skillsTab:ProcessSocketGroup(group)
	end
	build.skillsTab:AddUndoState()
	refresh()
	return M.get_skills()
end

M.set_gem = function(p)
	ensureBuild()
	if not p or not p.groupIndex or not p.gemIndex then
		error("params.groupIndex and params.gemIndex are required", 0)
	end
	local group = requireGroup(p.groupIndex)
	local gem = group.gemList[tonumber(p.gemIndex)]
	if not gem then error("unknown gem index", 0) end
	if p.level ~= nil then gem.level = tonumber(p.level) end
	if p.quality ~= nil then gem.quality = tonumber(p.quality) end
	if p.enabled ~= nil then gem.enabled = p.enabled end
	if p.count ~= nil then
		local count = math.max(0, tonumber(p.count) or 1)
		gem.count = IS_POE2 and count or math.floor(count)
	end
	if p.gemId ~= nil then gem.gemId = p.gemId; gem.skillId = nil end
	if p.nameSpec ~= nil then gem.nameSpec = p.nameSpec end
	build.skillsTab:ProcessSocketGroup(group)
	build.skillsTab:AddUndoState()
	refresh()
	return M.get_skills()
end

M.remove_gem = function(p)
	ensureBuild()
	local group = requireGroup(p and p.groupIndex)
	if not group.gemList[tonumber(p.gemIndex) or -1] then error("unknown gem index", 0) end
	table.remove(group.gemList, tonumber(p.gemIndex))
	build.skillsTab:ProcessSocketGroup(group)
	build.skillsTab:AddUndoState()
	refresh()
	return M.get_skills()
end

M.list_gems = function(p)
	p = p or {}
	local query = p.query and tostring(p.query):lower() or nil
	local limit = tonumber(p.limit) or 100
	-- Default the level cap to the open build so a caller cannot be handed gems
	-- the character has no way to socket. `maxLevel = 0` lifts the cap.
	local maxLevel = tonumber(p.maxLevel)
	if maxLevel == nil and build and build.characterLevel then maxLevel = build.characterLevel end
	if maxLevel == 0 then maxLevel = nil end
	local attrs = nil
	if build and build.calcsTab and build.calcsTab.mainOutput then
		local o = build.calcsTab.mainOutput
		attrs = { str = o.Str or 0, dex = o.Dex or 0, int = o.Int or 0 }
	end
	local out = array({})
	local total, hiddenByLevel = 0, 0
	for gemId, gemData in pairs(data.gems) do
		local isSupport = (gemData.grantedEffect and gemData.grantedEffect.support) and true or false
		local include = true
		if p.onlySupports ~= nil then include = (isSupport == p.onlySupports) end
		if include and query and query ~= "" then
			include = (gemData.name and gemData.name:lower():find(query, 1, true)) ~= nil
				or gemId:lower():find(query, 1, true) ~= nil
		end
		local tier = gemData.Tier and gemData.Tier > 0 and gemData.Tier or nil
		local reqLevel = gemReqLevel(tier)
		if include and maxLevel and reqLevel and reqLevel > maxLevel then
			hiddenByLevel = hiddenByLevel + 1
			include = false
		end
		if include then
			total = total + 1
			if #out < limit then
				local gemLevel = usableGemLevel(gemData, maxLevel or (build and build.characterLevel) or 100)
				local req = gemRequirements(gemData, gemLevel)
				local rs = req.str > 0 and req.str or nil
				local rd = req.dex > 0 and req.dex or nil
				local ri = req.int > 0 and req.int or nil
				local shortBy = null
				if attrs then
					local miss = {}
					if rs and attrs.str < rs then miss[#miss + 1] = string.format("Str %d/%d", attrs.str, rs) end
					if rd and attrs.dex < rd then miss[#miss + 1] = string.format("Dex %d/%d", attrs.dex, rd) end
					if ri and attrs.int < ri then miss[#miss + 1] = string.format("Int %d/%d", attrs.int, ri) end
					if #miss > 0 then shortBy = table.concat(miss, ", ") end
				end
				out[#out + 1] = {
					gemId = gemId,
					name = gemData.name or gemId,
					support = isSupport,
					-- Damage type and mechanics live here (e.g. "AoE, Projectile,
					-- Lightning, Chaining"), which is what makes a set coherent.
					tags = gemData.tagString and opt(gemData.tagString) or null,
					color = opt(gemData.color),
					tier = opt(tier),
					-- The character level the gem needs. Not in PoB's data; derived
					-- from the tier ladder.
					req_level = opt(reqLevel),
					-- Support sockets the gem has before Jeweller's Orbs. Scales with
					-- tier, so a low-tier gem cannot hold five supports.
					base_sockets = opt(gemBaseSockets(tier)),
					-- Nil means any weapon; "Bow" means the skill is dead without one.
					weapon = opt(gemData.weaponRequirements),
					attr = opt(gemAttr(gemData)),
					-- The gem level the character level allows, and what that level
					-- asks of the character. Supports have no requirement of their own.
					gem_level = gemLevel,
					req_str = opt(rs),
					req_dex = opt(rd),
					req_int = opt(ri),
					-- Set when the open build does not meet the attribute cost.
					short_by = shortBy,
				}
			end
		end
	end
	-- Earliest first: a caller building for a given level wants the gems that
	-- exist by then, not an alphabetical mix of tier 1 and tier 14. An untiered
	-- gem carries the `null` sentinel rather than nil, so sort on type.
	table.sort(out, function(a, b)
		local at = type(a.tier) == "number" and a.tier or 99
		local bt = type(b.tier) == "number" and b.tier or 99
		if at ~= bt then return at < bt end
		return (a.name or "") < (b.name or "")
	end)
	return {
		gems = out,
		total = total,
		truncated = (total > #out),
		levelCap = opt(maxLevel),
		hiddenByLevel = hiddenByLevel,
		-- Each support socketed adds 5 to one build-wide requirement source for
		-- its colour, compared against the other sources rather than added.
		supportAttributeCost = 5,
	}
end

M.list_valid_supports = function(p)
	ensureBuild()
	p = p or {}
	local group, groupIndex = requireGroup(p.groupIndex, p.skill)
	local activeSkill = group.displaySkillList and group.displaySkillList[group.mainActiveSkill or 1]
	local results = array({})
	if not activeSkill then
		return { supports = results }
	end
	local level = build.characterLevel or 1
	-- Scoring appends each candidate to the group and reruns the calc, so the
	-- delta is what that support would add on top of what is socketed now. The
	-- calc reports the main skill's damage, so the group is made main for the
	-- duration when it is not already.
	local dpsCache, dpsField = nil, nil
	if p.sortByDps then
		dpsField = "CombinedDPS"
		local prevMain = build.mainSocketGroup
		if groupIndex ~= prevMain then
			build.mainSocketGroup = groupIndex
			refresh()
		end
		local ok, res = pcall(gemDpsFor, group, groupIndex, dpsField)
		if groupIndex ~= prevMain then
			build.mainSocketGroup = prevMain
			refresh()
		end
		if not ok then error(res, 0) end
		dpsCache = res
	end
	local socketed = {}
	for _, gem in ipairs(group.gemList) do
		if gem.gemData then socketed[gem.gemData.id] = true end
	end
	local limit = tonumber(p.limit) or 0
	for gemId, gemData in pairs(data.gems) do
		if gemData.grantedEffect and gemData.grantedEffect.support then
			local ok, supports = pcall(calcLib.canGrantedEffectSupportActiveSkill, gemData.grantedEffect, activeSkill)
			if ok and supports then
				local tier = gemData.Tier and gemData.Tier > 0 and gemData.Tier or nil
				local reqLevel = gemReqLevel(tier)
				if not (reqLevel and reqLevel > level) then
					local row = {
						gemId = gemId,
						name = gemData.name or gemId,
						tier = opt(tier),
						req_level = opt(reqLevel),
						-- A support has no requirement of its own; its colour adds 5
						-- to the build-wide "Support Gems" source for that attribute.
						attr = opt(gemAttr(gemData)),
						socketed = socketed[gemId] == true,
					}
					if dpsCache and dpsCache.dps[gemId] then
						row.dps_delta = dpsCache.dps[gemId] - dpsCache.base
					end
					results[#results + 1] = row
				end
			end
		end
	end
	table.sort(results, function(a, b)
		if dpsCache then
			local da, db = a.dps_delta, b.dps_delta
			if (da ~= nil) ~= (db ~= nil) then return da ~= nil end
			if da and db and da ~= db then return da > db end
		end
		return a.name < b.name
	end)
	if limit > 0 then
		while #results > limit do table.remove(results) end
	end
	-- Requirements are the highest single source, never a sum: all supports of
	-- one colour across the build form one source of 5 each, which only binds
	-- when it exceeds every item and skill gem requirement for that attribute.
	return {
		supports = results,
		attributeCostPerSupport = 5,
		requirements = requirementSummary(),
		characterLevel = level,
		dpsField = opt(dpsField),
		baseDps = dpsCache and dpsCache.base or null,
	}
end

-- ---------------------------------------------------------------------------
-- Item database, tooltips, editing, weapon sets
-- ---------------------------------------------------------------------------

-- The unique/rare DBs parse on a coroutine that PoB resumes once per frame;
-- pump frames until they finish (one-time cost on first use).
local function ensureItemDb()
	local guard = 0
	while (main.uniqueDB.loading or main.rareDB.loading) and guard < 2000 do
		frame()
		guard = guard + 1
	end
	if main.uniqueDB.loading or main.rareDB.loading then
		error("item databases are still loading", 0)
	end
end

local function dbFor(name)
	if name == "rare" then return main.rareDB end
	return main.uniqueDB
end

-- Missing picks repeat the first, so the chosen lines appear once.
local function setPicks(item, picks)
	if not item.variantList then return end
	local n = #item.variantList
	local first = math.max(1, math.min(n, picks[1] or item.variant or n))
	for i, flag in ipairs(PICK_FLAGS) do
		if flag == true or item[flag] then
			item[PICK_FIELDS[i]] = math.max(1, math.min(n, picks[i] or first))
		end
	end
end

-- A variant given as its index, its exact name, or a substring of the name.
local function resolveVariant(item, spec)
	local list = item.variantList
	if not list then error(item.name .. " has no variants", 0) end
	local n = tonumber(spec)
	if n then
		if n < 1 or n > #list then error(item.name .. " has " .. #list .. " variants; " .. n .. " is out of range", 0) end
		return n
	end
	local q = tostring(spec):lower():gsub("^%s+", ""):gsub("%s+$", "")
	for i, name in ipairs(list) do
		if name:lower() == q then return i end
	end
	local hits = {}
	for i, name in ipairs(list) do
		if name:lower():find(q, 1, true) then hits[#hits + 1] = i end
	end
	if #hits == 1 then return hits[1] end
	if #hits == 0 then error("no variant of " .. item.name .. " matches " .. tostring(spec), 0) end
	local names = {}
	for i = 1, math.min(8, #hits) do names[#names + 1] = list[hits[i]] end
	error(tostring(spec) .. " matches " .. #hits .. " variants of " .. item.name .. ": " .. table.concat(names, "; "), 0)
end

local function activeModLines(item)
	local lines = array({})
	for _, ml in ipairs(item.explicitModLines or {}) do
		if item:CheckModLineVariant(ml) then lines[#lines + 1] = ml.line end
	end
	return lines
end

M.item_db_list = function(p)
	ensureBuild()
	p = p or {}
	ensureItemDb()
	local db = dbFor(p.db)
	local q = (p.query or ""):lower()
	local wantType = p.type
	local rows = {}
	local types = {}
	for name, item in pairs(db.list) do
		local itype = item.type or "?"
		types[itype] = (types[itype] or 0) + 1
		local match = (q == "" or name:lower():find(q, 1, true) ~= nil or (item.baseName or ""):lower():find(q, 1, true) ~= nil)
			and (not wantType or itype == wantType)
		if match then
			local implicits = array({})
			for _, ml in ipairs(item.implicitModLines or {}) do
				if item:CheckModLineVariant(ml) then implicits[#implicits + 1] = ml.line end
			end
			-- The first variant names give the model something to pick by; a
			-- long list (a notable per variant) is cut and counted.
			local variantNames = array({})
			for i = 1, math.min(40, item.variantList and #item.variantList or 0) do variantNames[i] = item.variantList[i] end
			rows[#rows + 1] = {
				name = name,
				rarity = opt(item.rarity),
				type = itype,
				baseName = opt(item.baseName),
				slot = opt(item:GetPrimarySlot()),
				league = opt(item.league),
				implicits = implicits,
				mods = activeModLines(item),
				variants = item.variantList and #item.variantList or 0,
				variantPicks = variantPickCount(item),
				variantNames = variantNames,
				selectedVariants = item.variantList and pickNamesOf(item) or array({}),
				upgrade = item.upgradePaths and true or false,
			}
		end
	end
	table.sort(rows, function(a, b) return a.name < b.name end)
	local total = #rows
	local offset = tonumber(p.offset) or 0
	local limit = tonumber(p.limit) or 100
	local page = array({})
	for i = offset + 1, math.min(offset + limit, total) do
		page[#page + 1] = rows[i]
	end
	local typeList = array({})
	for t, n in pairs(types) do typeList[#typeList + 1] = { type = t, count = n } end
	table.sort(typeList, function(a, b) return a.type < b.type end)
	return { items = page, total = total, offset = offset, types = typeList }
end

local function resolveItem(p)
	if p.itemId then
		local item = build.itemsTab.items[tonumber(p.itemId)]
		if not item then error("unknown item id " .. tostring(p.itemId), 0) end
		return item, false
	end
	if p.db and p.name then
		ensureItemDb()
		local item = dbFor(p.db).list[p.name]
		if not item then error("unknown database item " .. tostring(p.name), 0) end
		return item, true
	end
	if p.raw then
		local item = new("Item"):Item(p.raw)
		if not item.base then error("unrecognised item text", 0) end
		return item, true
	end
	error("params.itemId, params.db+name or params.raw is required", 0)
end

-- PoB's full item tooltip, including the "Equipping this item in X will give
-- you" comparison sections when a slot is given (or the item's primary slot).
M.item_tooltip = function(p)
	ensureBuild()
	p = p or {}
	local item, dbMode = resolveItem(p)
	local slot
	if p.slotName ~= false then
		local slotName = p.slotName or item:GetPrimarySlot()
		slot = slotName and build.itemsTab.slots[slotName] or nil
	end
	local tt = new("Tooltip"):Tooltip()
	build.itemsTab:AddItemTooltip(tt, item, slot, dbMode and p.itemId == nil and p.dbMode ~= false)
	local r = tooltipPayload(tt)
	-- PoB's Shift-hover tip describes its own window; there is no such hover here.
	local kept = array({})
	for _, l in ipairs(r.lines) do
		if not (type(l.text) == "string" and l.text:find("Tip: Hold Shift", 1, true)) then kept[#kept + 1] = l end
	end
	r.lines = kept
	r.rarity = opt(item.rarity)
	r.itemArt = { game = GAME, name = opt(item.title or item.name), baseName = opt(item.baseName), rarity = opt(item.rarity) }
	return r
end

M.item_prepare_preview = function(p)
	ensureBuild(p)
	if not p or type(p.raw) ~= "string" then
		error("item text is required", 0)
	end
	local item
	-- Capture PoB's candidate without updating the legacy display controls.
	local tab = setmetatable({ SetDisplayItem = function(_, candidate) item = candidate end }, { __index = build.itemsTab })
	tab:CreateDisplayItemFromRaw(p.raw, p.normalise ~= false)
	return { raw = item and item:BuildRaw() }
end

M.item_preview = function(p)
	ensureBuild(p)
	if not p or type(p.raw) ~= "string" or not p.raw:match("%S") then
		error("item text is required", 0)
	end
	local item = resolveItem({ raw = p.raw })
	local slots = array({})
	local tab = build.itemsTab
	-- Jewel/socket availability is normally updated by the legacy draw path.
	M.list_slots()
	local weaponSet = build.calcsTab.mainEnv.weaponSet or (tab.activeItemSet.useSecondWeaponSet and 2 or 1)
	for _, slot in ipairs(tab.orderedSlots) do
		if not slot.inactive and (not slot.weaponSet or slot.weaponSet == weaponSet)
			and (slot.weaponSet or slot.shown()) and tab:IsItemValidForSlot(item, slot.slotName) then
			slots[#slots + 1] = { slot = slot.slotName, label = slot.label or slot.slotName }
		end
	end
	return { tooltip = M.item_tooltip({ raw = p.raw, slotName = false, dbMode = false }), slots = slots,
		generation = main.__reduxBuildGeneration, rev = build.outputRevision }
end

-- PoB's Ctrl+D: whether item tooltips carry the "removing this item" lines.
-- With no `show` it only reports the current state.
M.stat_differences = function(p)
	ensureBuild()
	if p and p.show ~= nil then
		build.itemsTab.showStatDifferences = p.show == true
	end
	return { show = build.itemsTab.showStatDifferences == true }
end

M.item_db_equip = function(p)
	ensureBuild()
	if not p or not p.name then error("params.db and params.name are required", 0) end
	ensureItemDb()
	local dbItem = dbFor(p.db).list[p.name]
	if not dbItem then error("unknown database item " .. tostring(p.name), 0) end
	local item = new("Item"):Item(dbItem:BuildRaw())
	local picks = {}
	if p.variant ~= nil then picks[1] = resolveVariant(item, p.variant) end
	if type(p.variants) == "table" then
		for i, v in ipairs(p.variants) do picks[i] = resolveVariant(item, v) end
	end
	if #picks > 0 then
		local n = variantPickCount(item)
		if #picks > n then error(item.name .. " takes " .. n .. " variant pick" .. (n == 1 and "" or "s") .. ", not " .. #picks, 0) end
		setPicks(item, picks)
		item:BuildAndParseRaw()
	end
	local r = M.equip_item_raw({ text = item.raw, slot = p.slotName })
	local equipped = build.itemsTab.items[r.itemId]
	r.variants = equipped and equipped.variantList and pickNamesOf(equipped) or array({})
	r.mods = equipped and activeModLines(equipped) or array({})
	return r
end

-- Canonical raw text for the edit dialog.
M.item_raw = function(p)
	ensureBuild()
	local item = build.itemsTab.items[tonumber(p and p.itemId) or -1]
	if not item then error("unknown item id", 0) end
	return { raw = item:BuildRaw() }
end

-- Create a new item from raw text, or replace an existing one (same id keeps
-- every slot assignment pointing at the edited item).
M.item_edit = function(p)
	ensureBuild(p)
	if not p or type(p.text) ~= "string" or p.text == "" then error("params.text is required", 0) end
	local item = new("Item"):Item(p.text)
	if not item.base then error("unrecognised item text (check the base type line)", 0) end
	if p.itemId then
		if not build.itemsTab.items[tonumber(p.itemId)] then error("unknown item id", 0) end
		item.id = tonumber(p.itemId)
	end
	build.itemsTab:AddItem(item, true)
	build.itemsTab:PopulateSlots()
	build.itemsTab:AddUndoState()
	refresh()
	return { ok = true, itemId = item.id, name = item.name }
end

local requireItem, commitItemEdit
do
	local requests = setmetatable({}, { __mode = "k" })
	local drafts = setmetatable({}, { __mode = "k" })
	requireItem = function(p)
		ensureBuild(p)
		if p and p.raw ~= nil then
			if p.itemId ~= nil then error("provide raw or itemId, not both", 0) end
			if not requests[p] then
				if type(p.raw) ~= "string" then error("raw item text is required", 0) end
				requests[p] = resolveItem({ raw = p.raw })
				drafts[requests[p]] = true
			end
			return requests[p]
		end
		local item = build.itemsTab.items[tonumber(p and p.itemId) or -1]
		if not item then error("unknown item id " .. tostring(p and p.itemId), 0) end
		return item
	end
	commitItemEdit = function(item)
		item:BuildAndParseRaw()
		if drafts[item] then return end
		build.itemsTab:PopulateSlots()
		build.itemsTab:AddUndoState()
		refresh()
	end
end

-- ---------------------------------------------------------------------------
-- Crafting: bases, affixes, runes (ItemsTab:CraftItem / UpdateAffixControl)
-- ---------------------------------------------------------------------------

M.craft_bases = function()
	local types = strArray(data.itemBaseTypeList)
	local bases = {}
	for _, t in ipairs(data.itemBaseTypeList) do
		local list = array({})
		for _, e in ipairs(data.itemBaseLists[t] or {}) do
			list[#list + 1] = {
				name = e.name,
				label = opt(e.label),
				subType = e.base and opt(e.base.subType) or null,
			}
		end
		bases[t] = list
	end
	return { types = types, bases = bases }
end

-- `itemType` may be a typed list ("Boots: Armour") or its family ("Boots"),
-- which searches every typed list of that family.
local function findBase(itemType, baseName)
	if not itemType or not baseName then error("params.type and params.baseName are required", 0) end
	local want = tostring(baseName):lower()
	local known = false
	for _, t in ipairs(data.itemBaseTypeList) do
		if t == itemType or t:match("^(.-):") == itemType then
			known = true
			for _, e in ipairs(data.itemBaseLists[t] or {}) do
				if e.name:lower() == want then return e, t end
			end
		end
	end
	if not known then error("unknown item type " .. tostring(itemType) .. "; see list_bases", 0) end
	error("unknown base " .. tostring(baseName) .. " in type " .. tostring(itemType) .. "; see list_bases", 0)
end

-- Same construction as CraftItem's makeItem. The item is not added to the
-- build; callers do that, or drop it after reading its affix pool.
-- `range` also sets the roll of ranged implicits (a belt's charm slots), so a
-- perfect item is perfect throughout.
local function makeCraftedItem(base, rarity, title, range)
	rarity = rarity or "RARE"
	local item = new("Item"):Item()
	item.name = base.name
	item.base = base.base
	item.baseName = base.name
	item.charmLimit = base.charmLimit
	item.spiritValue = base.spiritValue
	item.buffModLines = {}
	item.enchantModLines = {}
	item.runeModLines = {}
	item.classRequirementModLines = {}
	item.implicitModLines = {}
	item.explicitModLines = {}
	item.scourgeModLines = {}
	item.crucibleModLines = {}
	item.sockets = {}
	item.runes = {}
	item.quality = base.base.quality and 0 or nil
	-- PoE2 sockets hold runes and have no colour; PoE1's come from the socket editor.
	if IS_POE2 and base.base.socketLimit and (base.base.weapon or base.base.armour or base.base.tags.wand or base.base.tags.staff or base.base.tags.sceptre) then
		for _ = 1, base.base.socketLimit do
			table.insert(item.sockets, { group = 0 })
		end
		item.itemSocketCount = #item.sockets
	end
	if (base.base.flask or (base.base.type == "Jewel" and base.base.subType == "Charm") or base.base.type == "Charm") and rarity == "RARE" then
		rarity = "MAGIC"
	end
	if base.base.type == "Transcendent Limb" then
		rarity = "NORMAL"
	end
	if rarity == "MAGIC" or rarity == "RARE" then
		item.crafted = true
	end
	item.rarity = rarity
	if rarity == "RARE" or rarity == "UNIQUE" then
		item.title = (title and title:match("%S")) and title or "New Item"
	end
	if base.base.implicit then
		local implicitIndex = 1
		for line in base.base.implicit:gmatch("[^\n]+") do
			local modList, extra = modLib.parseMod(line)
			table.insert(item.implicitModLines, { line = line, extra = extra, modList = modList or {}, range = range, modTags = base.base.implicitModTypes and base.base.implicitModTypes[implicitIndex] or {} })
			implicitIndex = implicitIndex + 1
		end
	end
	if base.base.variantList then
		item.variantList = copyTable(base.base.variantList, true)
		item.variant = 1
		item.baseLines = {}
	end
	if base.base.type == "Jewel" and base.base.subType == "Radius" then
		item.jewelRadiusLabel = "Small"
	end
	item:NormaliseQuality()
	item:BuildAndParseRaw()
	return item
end

local function equipFirstValid(item, slotName)
	slotName = resolveSlotName(slotName)
	if slotName then
		local slot = build.itemsTab.slots[slotName]
		if not slot then error("unknown slot " .. tostring(slotName), 0) end
		if not build.itemsTab:IsItemValidForSlot(item, slotName) then
			error(item.baseName .. " does not fit slot " .. slotName, 0)
		end
		slot:SetSelItemId(item.id)
		return slotName
	end
	for _, slot in ipairs(build.itemsTab.orderedSlots) do
		if not slot.inactive and build.itemsTab:IsItemValidForSlot(item, slot.slotName) then
			slot:SetSelItemId(item.id)
			return slot.slotName
		end
	end
	return nil
end

M.craft_item = function(p)
	ensureBuild()
	if not p or not p.type or not p.baseName then error("params.type and params.baseName are required", 0) end
	local entry = findBase(p.type, p.baseName)
	local item = makeCraftedItem(entry, p.rarity, p.title)
	build.itemsTab:AddItem(item, true)
	if p.equip then equipFirstValid(item) end
	build.itemsTab:PopulateSlots()
	build.itemsTab:AddUndoState()
	refresh()
	return { ok = true, itemId = item.id, name = item.name, crafted = item.crafted == true }
end

-- Bases with the numbers that decide between them. `type` may be a typed list
-- ("Boots: Armour") or a family ("Boots"), which covers every typed list of it.
M.list_bases = function(p)
	p = p or {}
	local want = p.type and tostring(p.type) or nil
	local query = p.query and tostring(p.query):lower() or nil
	local limit = tonumber(p.limit) or 60
	if not want then
		return { types = strArray(data.itemBaseTypeList), bases = array({}), total = 0 }
	end
	local out, total = array({}), 0
	for _, t in ipairs(data.itemBaseTypeList) do
		if t == want or t:match("^(.-):") == want then
			for _, e in ipairs(data.itemBaseLists[t] or {}) do
				if not query or e.name:lower():find(query, 1, true) then
					total = total + 1
					if #out < limit then
						local b = e.base
						local row = {
							type = t,
							name = e.name,
							req = b.req and { level = opt(b.req.level), str = opt(b.req.str), dex = opt(b.req.dex), int = opt(b.req.int) } or null,
							implicit = opt(b.implicit),
							sockets = opt(b.socketLimit),
						}
						if b.weapon then
							row.weapon = {
								physMin = b.weapon.PhysicalMin,
								physMax = b.weapon.PhysicalMax,
								attackRate = b.weapon.AttackRateBase,
								critChance = b.weapon.CritChanceBase,
								range = opt(b.weapon.Range),
							}
						end
						if b.armour then
							row.armour = { armour = opt(b.armour.Armour), evasion = opt(b.armour.Evasion), energyShield = opt(b.armour.EnergyShield) }
						end
						if e.charmLimit then row.charmSlots = e.charmLimit end
						if e.spiritValue then row.spirit = e.spiritValue end
						out[#out + 1] = row
					end
				end
			end
		end
	end
	if total == 0 and not data.itemBaseLists[want] then
		error("unknown item type " .. want .. "; valid types: " .. table.concat(data.itemBaseTypeList, ", "), 0)
	end
	return { bases = out, total = total, truncated = total > #out }
end

-- The affix pool for a base at an item level, one row per mod family with the
-- best tier that can roll. Built on a throwaway item so the spawn-weight and
-- level rules are PoB's own.
local function affixPool(item, itemLevel, affixType, query)
	local best = {}
	for modId, mod in pairs(item.affixes) do
		if mod.type == affixType and item:GetModSpawnWeight(mod) > 0 and (mod.level or 1) <= itemLevel then
			local label = table.concat(mod, "/")
			if not query or label:lower():find(query, 1, true) or (mod.group or ""):lower():find(query, 1, true) then
				local g = mod.group or modId
				local cur = best[g]
				if not cur or (mod.level or 0) > cur.level then
					best[g] = { group = g, modId = modId, affix = mod.affix, label = label, level = mod.level or 0, tiers = (cur and cur.tiers or 0) + 1 }
				else
					cur.tiers = cur.tiers + 1
				end
			end
		end
	end
	local out = array({})
	for _, v in pairs(best) do out[#out + 1] = v end
	table.sort(out, function(a, b) return a.group < b.group end)
	return out
end

M.list_affixes = function(p)
	p = p or {}
	local entry = findBase(p.type, p.baseName)
	local item = makeCraftedItem(entry, "RARE", "Pool")
	local itemLevel = tonumber(p.itemLevel) or 82
	item.itemLevel = itemLevel
	local query = p.query and tostring(p.query):lower() or nil
	if not item.affixes then
		return { base = entry.name, itemLevel = itemLevel, prefixes = array({}), suffixes = array({}), prefixLimit = 0, suffixLimit = 0 }
	end
	local prefixes = affixPool(item, itemLevel, "Prefix", query)
	local suffixes = affixPool(item, itemLevel, "Suffix", query)
	-- A filtered view says so, with the full family counts, so a short list is
	-- not mistaken for a small pool.
	return {
		base = entry.name,
		type = entry.base.type,
		itemLevel = itemLevel,
		query = opt(query),
		prefixLimit = item.prefixes and item.prefixes.limit or (item.affixLimit or 6) / 2,
		suffixLimit = item.suffixes and item.suffixes.limit or (item.affixLimit or 6) / 2,
		prefixFamiliesTotal = query and #affixPool(item, itemLevel, "Prefix", nil) or #prefixes,
		suffixFamiliesTotal = query and #affixPool(item, itemLevel, "Suffix", nil) or #suffixes,
		prefixes = prefixes,
		suffixes = suffixes,
	}
end

-- Resolve one requested affix to a mod id: an exact id, a family name
-- (Strength, IncreasedLife, LocalIncreasedPhysicalDamagePercent), or a
-- substring of the mod text. Family and text matches take the best tier the
-- item level allows.
local function resolveAffix(item, itemLevel, affixType, want, usedGroups)
	want = tostring(want)
	local exact = item.affixes[want]
	if exact and exact.type == affixType then return want, exact end
	local lw = want:lower()
	-- An affix name ("Merciless", "of the Vampire") names a tier; the family
	-- it belongs to is what the caller wants, at the best tier that rolls.
	local wantGroup
	for _, mod in pairs(item.affixes) do
		if mod.type == affixType and (mod.affix or ""):lower() == lw then wantGroup = mod.group break end
	end
	local bestId, bestMod
	local families = {}
	for modId, mod in pairs(item.affixes) do
		if mod.type == affixType and item:GetModSpawnWeight(mod) > 0 and (mod.level or 1) <= itemLevel then
			local g = mod.group or modId
			families[g] = true
			if not usedGroups[g] then
				local label = table.concat(mod, "/")
				local hit = g:lower() == lw or g == wantGroup or label:lower():find(lw, 1, true) ~= nil
				if hit and (not bestMod or (mod.level or 0) > (bestMod.level or 0)) then
					bestId, bestMod = modId, mod
				end
			end
		end
	end
	if not bestMod then
		local names = {}
		for g in pairs(families) do names[#names + 1] = g end
		table.sort(names)
		local used = usedGroups[wantGroup or lw] and " (that family is already on the item)" or ""
		error(string.format("no %s matching %q rolls on %s at item level %d%s. %s families that do: %s",
			affixType:lower(), want, item.baseName, itemLevel, used, affixType, table.concat(names, ", ")), 0)
	end
	return bestId, bestMod
end

-- One call builds a rare from a base and a list of wanted mods, using PoB's
-- own affix tables so every line is a real mod at a real tier. `range` is the
-- roll within each tier: 1 is perfect, 0.5 is the middle.
M.craft_rare = function(p)
	ensureBuild()
	p = p or {}
	local entry = findBase(p.type, p.baseName)
	local range = tonumber(p.range)
	if range == nil then range = 1 end
	range = math.max(0, math.min(1, range))
	local item = makeCraftedItem(entry, "RARE", p.title, range)
	if not item.affixes then error(entry.name .. " cannot carry affixes", 0) end
	local itemLevel = tonumber(p.itemLevel) or 82
	item.itemLevel = itemLevel
	local chosen = array({})
	local usedGroups = {}
	for _, spec in ipairs({ { "prefixes", "Prefix" }, { "suffixes", "Suffix" } }) do
		local tableName, affixType = spec[1], spec[2]
		local wants = p[tableName] or {}
		local limit = item[tableName].limit or (item.affixLimit / 2)
		if #wants > limit then
			error(string.format("%s takes at most %d %s", entry.name, limit, tableName), 0)
		end
		for i, want in ipairs(wants) do
			local modId, mod = resolveAffix(item, itemLevel, affixType, want, usedGroups)
			usedGroups[mod.group or modId] = true
			item[tableName][i] = { modId = modId, range = range }
			chosen[#chosen + 1] = { slot = affixType, modId = modId, group = opt(mod.group), level = opt(mod.level), text = table.concat(mod, "/") }
		end
	end
	item:Craft()
	if type(p.runes) == "table" and item.itemSocketCount and item.itemSocketCount > 0 then
		local valid = {}
		for _, r in ipairs(build.itemsTab:GetValidRunesForItem(item)) do valid[r.name:lower()] = r.name end
		for i, name in ipairs(p.runes) do
			if i > item.itemSocketCount then break end
			local real = valid[tostring(name):lower()]
			if not real then error("rune " .. tostring(name) .. " does not fit " .. entry.name, 0) end
			item.runes[i] = real
		end
		item:UpdateRunes()
	end
	item:BuildAndParseRaw()
	build.itemsTab:AddItem(item, true)
	local slotName
	if p.equip ~= false then slotName = equipFirstValid(item, p.slot) end
	build.itemsTab:PopulateSlots()
	build.itemsTab:AddUndoState()
	refresh()
	local lines = array({})
	for _, m in ipairs(item.explicitModLines) do lines[#lines + 1] = m.line end
	return {
		ok = true,
		itemId = item.id,
		name = item.name,
		base = entry.name,
		slot = opt(slotName),
		itemLevel = itemLevel,
		requirements = { level = opt(item.requirements.level), str = opt(item.requirements.str), dex = opt(item.requirements.dex), int = opt(item.requirements.int) },
		affixes = chosen,
		mods = lines,
		raw = item.raw,
	}
end

local function affixSlotOptions(item, affixType, tableName, outputIndex)
	local extraTags, excludeGroups = {}, {}
	for _, tbl in ipairs({ "prefixes", "suffixes" }) do
		for index = 1, (item[tbl].limit or (item.affixLimit / 2)) do
			if index ~= outputIndex or tbl ~= tableName then
				local mod = item.affixes[item[tbl][index] and item[tbl][index].modId]
				if mod then
					if mod.group then excludeGroups[mod.group] = true end
					for _, tag in ipairs(mod.tags or {}) do extraTags[tag] = true end
				end
			end
		end
	end
	local affixList = {}
	for modId, mod in pairs(item.affixes) do
		if mod.type == affixType and not excludeGroups[mod.group] and item:GetModSpawnWeight(mod, extraTags) > 0 then
			affixList[#affixList + 1] = modId
		end
	end
	table.sort(affixList, function(a, b)
		local modA = item.affixes[a]
		local modB = item.affixes[b]
		for i = 1, math.max(#modA, #modB) do
			if not modA[i] then
				return true
			elseif not modB[i] then
				return false
			elseif modA.statOrder[i] ~= modB.statOrder[i] then
				return modA.statOrder[i] < modB.statOrder[i]
			end
		end
		return modA.level > modB.level
	end)
	local opts = array({})
	for _, modId in ipairs(affixList) do
		local mod = item.affixes[modId]
		local modString = table.concat(mod, "/")
		opts[#opts + 1] = {
			modId = modId,
			affix = opt(mod.affix),
			label = modString,
			level = opt(mod.level),
			haveRange = modString:match("%(%-?[%d%.]+%-%-?[%d%.]+%)") and true or false,
		}
	end
	return opts
end

M.item_affixes = function(p)
	ensureBuild()
	local item = requireItem(p)
	if not item.crafted or not item.affixes then
		return { crafted = false, prefixes = array({}), suffixes = array({}) }
	end
	local function slots(tableName, affixType)
		local limit = item[tableName].limit or (item.affixLimit / 2)
		local out = array({})
		for i = 1, limit do
			local cur = item[tableName][i] or { modId = "None" }
			local curMod = item.affixes[cur.modId]
			out[#out + 1] = {
				index = i,
				modId = cur.modId,
				range = type(cur.range) == "table" and null or (cur.range or (main.defaultItemAffixQuality or 0.5)),
				label = curMod and table.concat(curMod, "/") or null,
				affix = curMod and opt(curMod.affix) or null,
				options = affixSlotOptions(item, affixType, tableName, i),
			}
		end
		return out
	end
	return {
		crafted = true,
		affixLimit = item.affixLimit,
		prefixes = slots("prefixes", "Prefix"),
		suffixes = slots("suffixes", "Suffix"),
	}
end

M.set_item_affix = function(p)
	ensureBuild()
	local item = requireItem(p)
	if not item.crafted or not item.affixes then error("only crafted magic/rare items expose affixes", 0) end
	local tableName = p.table == "suffixes" and "suffixes" or "prefixes"
	local index = tonumber(p.index) or 1
	local limit = item[tableName].limit or (item.affixLimit / 2)
	if index % 1 ~= 0 or index < 1 or index > limit then error("affix index out of range", 0) end
	local modId = p.modId or "None"
	local valid = modId == "None"
	for _, option in ipairs(affixSlotOptions(item, tableName == "suffixes" and "Suffix" or "Prefix", tableName, index)) do
		if option.modId == modId then valid = true; break end
	end
	if not valid then error("affix is not compatible with this item", 0) end
	if p.range ~= nil and (type(p.range) ~= "number" or p.range < 0 or p.range > 1) then error("invalid affix roll", 0) end
	item[tableName][index] = {
		modId = modId,
		range = tonumber(p.range) or (main.defaultItemAffixQuality or 0.5),
	}
	item:Craft()
	commitItemEdit(item)
	return M.item_affixes(p)
end

M.item_runes = function(p)
	ensureBuild()
	local item = requireItem(p)
	local sockets = item.itemSocketCount or 0
	local runes = array({})
	for i = 1, sockets do
		runes[i] = item.runes and item.runes[i] or "None"
	end
	local options = array({})
	if sockets > 0 then
		for _, r in ipairs(build.itemsTab:GetValidRunesForItem(item)) do
			options[#options + 1] = {
				name = r.name,
				label = opt(r.label),
				lines = strArray(r.lines),
				req = opt(r.req),
				type = opt(r.type),
				limit = opt(r.limit),
			}
		end
	end
	return { socketCount = sockets, runes = runes, options = options }
end

M.set_item_rune = function(p)
	ensureBuild()
	local item = requireItem(p)
	local index = tonumber(p and p.index)
	if not index or index % 1 ~= 0 or index < 1 or index > (item.itemSocketCount or 0) then error("rune index out of range", 0) end
	local valid = false
	for _, rune in ipairs(build.itemsTab:GetValidRunesForItem(item)) do
		if rune.name == (p.name or "None") then valid = true; break end
	end
	if not valid then error("rune is not compatible with this item", 0) end
	item.runes[index] = p.name or "None"
	item:UpdateRunes()
	commitItemEdit(item)
	return M.item_runes(p)
end

-- Catalysts (rings/amulets): same list and defaults as ItemsTab's dropdown.
local catalystNames = {
	"Flesh (Life)", "Neural (Mana)", "Carapace (Defense)", "Uul-Netol's (Physical)",
	"Xoph's (Fire)", "Tul's (Cold)", "Esh's (Lightning)", "Chayula's (Chaos)",
	"Reaver (Attack)", "Sibilant (Caster)", "Skittering (Speed)", "Adaptive (Attribute)",
	"Necrotic (Minion)",
}

M.set_item_props = function(p)
	ensureBuild()
	local item = requireItem(p)
	if p.quality ~= nil and item.base and (item.base.quality or (not IS_POE2 and (item.base.weapon or item.base.armour or item.base.flask or item.base.tincture))) then
		item.quality = math.max(0, math.min(tonumber(p.quality) or 0, 50))
	end
	if p.itemLevel ~= nil then
		item.itemLevel = math.max(1, math.min(tonumber(p.itemLevel) or 1, 100))
	end
	if p.corrupted ~= nil then
		item.corrupted = p.corrupted == true
	end
	if p.catalyst ~= nil then
		item.catalyst = math.max(0, math.min(tonumber(p.catalyst) or 0, #catalystNames))
		if item.catalyst > 0 and not item.catalystQuality then
			item.catalystQuality = item.name:match("Breach Ring") and 50 or 20
		end
	end
	if p.catalystQuality ~= nil then
		item.catalystQuality = math.max(0, math.min(tonumber(p.catalystQuality) or 0, 100))
	end
	commitItemEdit(item)
	return { ok = true }
end

local function variantInfo(item)
	local picks = array({})
	if item.variantList then
		for i, flag in ipairs(PICK_FLAGS) do
			if flag == true or item[flag] then picks[#picks + 1] = item[PICK_FIELDS[i]] or 1 end
		end
	end
	return { names = strArray(item.variantList or {}), picks = picks }
end

M.item_variants = function(p)
	ensureBuild()
	return variantInfo(requireItem(p))
end

-- params: { itemId, picks = { one variant per pick: its index, name or a substring } }
M.set_item_variant = function(p)
	ensureBuild()
	local item = requireItem(p)
	if not item.variantList then error(item.name .. " has no variants", 0) end
	local wanted = type(p.picks) == "table" and p.picks or {}
	local picks, ordinal = {}, 0
	for i, flag in ipairs(PICK_FLAGS) do
		if flag == true or item[flag] then
			ordinal = ordinal + 1
			picks[i] = wanted[ordinal] ~= nil and resolveVariant(item, wanted[ordinal]) or item[PICK_FIELDS[i]] or 1
		end
	end
	setPicks(item, picks)
	commitItemEdit(item)
	return variantInfo(item)
end

-- ---------------------------------------------------------------------------
-- PoE1 item shape: influence, sockets and links, and cluster jewel crafting.
-- PoE2 items have none of these, so every reader reports what the item can
-- take and the UI shows only what comes back.
-- ---------------------------------------------------------------------------

local INFLUENCES = {
	{ key = "shaper", name = "Shaper" },
	{ key = "elder", name = "Elder" },
	{ key = "adjudicator", name = "Warlord" },
	{ key = "basilisk", name = "Hunter" },
	{ key = "crusader", name = "Crusader" },
	{ key = "eyrie", name = "Redeemer" },
	{ key = "cleansing", name = "Searing Exarch" },
	{ key = "tangle", name = "Eater of Worlds" },
}

local SOCKET_COLOURS = { "R", "G", "B", "W", "A" }

M.item_shape = function(p)
	ensureBuild()
	local item = requireItem(p)
	local influences = array({})
	for _, inf in ipairs(INFLUENCES) do
		influences[#influences + 1] = { key = inf.key, name = inf.name, on = item[inf.key] == true }
	end
	local sockets = array({})
	for _, sock in ipairs(item.sockets or {}) do
		sockets[#sockets + 1] = { colour = sock.color, group = sock.group or 0 }
	end
	local cluster = null
	if item.clusterJewel then
		local skills = array({})
		for id, skill in pairs(item.clusterJewel.skills or {}) do
			skills[#skills + 1] = { id = id, name = skill.name or id }
		end
		table.sort(skills, function(a, b) return a.name < b.name end)
		cluster = {
			skills = skills,
			skill = opt(item.clusterJewelSkill),
			nodeCount = item.clusterJewelNodeCount or item.clusterJewel.maxNodes,
			minNodes = item.clusterJewel.minNodes,
			maxNodes = item.clusterJewel.maxNodes,
		}
	end
	return {
		canBeInfluenced = item.canBeInfluenced == true,
		influences = influences,
		sockets = sockets,
		socketLimit = not IS_POE2 and (item.base and item.base.socketLimit) or 0,
		colours = strArray(SOCKET_COLOURS),
		abyssalSocketCount = item.abyssalSocketCount or 0,
		cluster = cluster,
	}
end

M.set_item_shape = function(p)
	ensureBuild()
	local item = requireItem(p)
	if p.influences ~= nil then
		if item.ResetInfluence then item:ResetInfluence() end
		local byKey = {}
		for _, inf in ipairs(INFLUENCES) do byKey[inf.key] = true end
		local applied = 0
		for _, key in ipairs(p.influences) do
			-- PoB allows two, the same as its pair of dropdowns.
			if byKey[key] and applied < 2 then
				item[key] = true
				applied = applied + 1
			end
		end
	end
	if p.sockets ~= nil then
		local limit = (item.base and item.base.socketLimit) or 0
		local ok = {}
		for _, c in ipairs(SOCKET_COLOURS) do ok[c] = true end
		local next_ = {}
		for _, sock in ipairs(p.sockets) do
			if #next_ >= limit then break end
			local colour = tostring(sock.colour or "W"):upper()
			if not ok[colour] then colour = "W" end
			next_[#next_ + 1] = { color = colour, group = math.max(tonumber(sock.group) or 0, 0) }
		end
		item.sockets = next_
	end
	if p.clusterSkill ~= nil and item.clusterJewel then
		local skill = tostring(p.clusterSkill)
		item.clusterJewelSkill = (skill ~= "" and item.clusterJewel.skills[skill]) and skill or nil
	end
	if p.clusterNodeCount ~= nil and item.clusterJewel then
		local n = tonumber(p.clusterNodeCount) or item.clusterJewel.maxNodes
		item.clusterJewelNodeCount = math.max(math.min(n, item.clusterJewel.maxNodes), item.clusterJewel.minNodes)
	end
	commitItemEdit(item)
	return M.item_shape(p)
end

-- ---------------------------------------------------------------------------
-- Enchantments (PoE1): ItemsTab:EnchantDisplayItem. An item's enchantments
-- table is keyed either by source (lab, Heist, Harvest) or, for helmets, by
-- skill and then by source.
-- ---------------------------------------------------------------------------

M.item_enchants = function(p)
	ensureBuild()
	local item = requireItem(p)
	local ench = item.enchantments
	if not ench then
		return { available = false, bySkill = false, skills = array({}), sources = array({}), lines = array({}), current = array({}), slots = 1 }
	end
	-- If any top-level key is a known source then this item is not per-skill.
	local bySkill = true
	for _, source in ipairs(build.data.enchantmentSource or {}) do
		if ench[source.name] then
			bySkill = false
			break
		end
	end
	local skills = array({})
	if bySkill then
		for name in pairs(ench) do skills[#skills + 1] = name end
		table.sort(skills)
	end
	local skill = p and p.skill
	if bySkill and (skill == nil or ench[skill] == nil) then skill = skills[1] end
	local scope = bySkill and (skill and ench[skill] or {}) or ench
	local sources = array({})
	for _, source in ipairs(build.data.enchantmentSource or {}) do
		if scope[source.name] then sources[#sources + 1] = source.name end
	end
	local source = p and p.source
	if source == nil or scope[source] == nil then source = sources[1] end
	local lines = array({})
	for _, line in ipairs((source and scope[source]) or {}) do lines[#lines + 1] = line end
	local current = array({})
	for _, mod in ipairs(item.enchantModLines or {}) do current[#current + 1] = mod.line end
	local slots = 1
	if item.canHaveTwoEnchants then slots = 2 end
	if item.canHaveThreeEnchants then slots = 3 end
	if item.canHaveFourEnchants then slots = 4 end
	return {
		available = #sources > 0 or #skills > 0,
		bySkill = bySkill,
		skills = skills,
		skill = opt(skill),
		sources = sources,
		source = opt(source),
		lines = lines,
		current = current,
		slots = slots,
	}
end

M.set_item_enchant = function(p)
	ensureBuild()
	local item = requireItem(p)
	local info = M.item_enchants(p)
	if not info.available then error("this item has no enchantments", 0) end
	local slot = tonumber(p.slot) or 1
	if slot % 1 ~= 0 or slot < 1 or slot > (p.remove and #item.enchantModLines or info.slots) then
		error("invalid enchantment slot", 0)
	end
	if not p.remove then
		local allowed = false
		for _, line in ipairs(info.lines) do if line == p.line then allowed = true; break end end
		if not allowed then error("invalid enchantment", 0) end
		slot = math.min(slot, #item.enchantModLines + 1)
	end
	item.enchantModLines = item.enchantModLines or {}
	if p.remove then
		table.remove(item.enchantModLines, slot)
	elseif p.line ~= nil then
		local line = tostring(p.line)
		-- PoB writes a pair when the entry carries two lines separated by "/".
		local first, second = line:match("([^/]+)/([^/]+)")
		if first then
			item.enchantModLines = { { crafted = true, line = first }, { crafted = true, line = second } }
		else
			if info.slots == 1 and #item.enchantModLines > 1 then
				item.enchantModLines = { item.enchantModLines[1] }
			end
			if #item.enchantModLines >= slot then table.remove(item.enchantModLines, slot) end
			table.insert(item.enchantModLines, slot, { crafted = true, line = line })
		end
	else
		error("params.line or params.remove is required", 0)
	end
	commitItemEdit(item)
	return M.item_enchants(p)
end

-- ---------------------------------------------------------------------------
-- Crucible trees (PoE1): ItemsTab:AddCrucibleModifierToDisplayItem. A weapon
-- carries up to five nodes, each holding one mod from the crucible pool that
-- can sit in that position.
-- ---------------------------------------------------------------------------

local CRUCIBLE_NODES = 5

-- "Allocates 12345" reads as the node's name once the tree is known.
local function crucibleLine(line)
	if line and line:match("Allocates") then
		local nodeId = tonumber(line:match("%d+"))
		local node = nodeId and build.spec.nodes[nodeId]
		if node then return "Allocates " .. node.name end
	end
	return line
end

local function crucibleLabel(mod)
	local parts = {}
	for _, line in ipairs(mod) do parts[#parts + 1] = crucibleLine(line) end
	return table.concat(parts, " / ")
end

M.item_crucible = function(p)
	ensureBuild()
	local item = requireItem(p)
	local pool = build.data.crucible
	if not pool then
		return { available = false, nodes = array({}), selected = array({}) }
	end
	local nodes = array({})
	for i = 1, CRUCIBLE_NODES do nodes[i] = array({}) end
	for order, mod in pairs(pool) do
		if item:CanHaveMod(mod) then
			for _, location in ipairs(mod.nodeLocation or {}) do
				if nodes[location] then
					nodes[location][#nodes[location] + 1] = {
						id = order,
						label = crucibleLabel(mod),
						tier = mod.tier,
						type = opt(mod.type),
					}
				end
			end
		end
	end
	for _, list in ipairs(nodes) do
		table.sort(list, function(a, b)
			if a.type ~= b.type then return a.type == "Spawn" end
			return a.id < b.id
		end)
	end
	-- Work out which option each node is showing by matching the lines already
	-- on the item, the way PoB seeds its dropdowns.
	local onItem = {}
	for _, mod in ipairs(item.crucibleModLines or {}) do onItem[mod.line] = true end
	-- An empty string means the node is clear, so the array keeps its shape.
	local selected = array({})
	for i = 1, CRUCIBLE_NODES do selected[i] = "" end
	for order, mod in pairs(pool) do
		if item:CanHaveMod(mod) and onItem[crucibleLine(mod[1])] and (not mod[2] or onItem[crucibleLine(mod[2])]) then
			local loc = mod.nodeLocation or {}
			if loc[1] and selected[loc[1]] ~= "" and loc[2] then
				selected[loc[2]] = order
			elseif loc[1] then
				selected[loc[1]] = order
			end
		end
	end
	return {
		available = item.base ~= nil and item.base.weapon ~= nil,
		nodes = nodes,
		selected = selected,
		count = CRUCIBLE_NODES,
	}
end

M.set_item_crucible = function(p)
	ensureBuild()
	local item = requireItem(p)
	local pool = build.data.crucible
	if not pool then error("this game has no crucible mods", 0) end
	if type(p.selected) ~= "table" then error("params.selected is required", 0) end
	if not item.base.weapon or #p.selected > CRUCIBLE_NODES then error("invalid crucible tree", 0) end
	for i, id in ipairs(p.selected) do
		if id ~= "" then
			local mod = pool[id]
			local allowed = false
			if mod and item:CanHaveMod(mod) then
				for _, location in ipairs(mod.nodeLocation or {}) do if location == i then allowed = true end end
			end
			if not allowed then error("invalid crucible node", 0) end
		end
	end
	item.crucibleModLines = {}
	for i = 1, CRUCIBLE_NODES do
		local order = p.selected[i]
		if order == "" then order = nil end
		local mod = order ~= nil and pool[order] or nil
		if mod then
			for _, line in ipairs(mod) do
				-- The line is tagged {crucible} on the way out, which is how a
				-- re-parse knows to put it back on the crucible tree.
				local entry = { line = crucibleLine(line), modTags = mod.modTags, crucible = true }
				item.crucibleModLines[#item.crucibleModLines + 1] = entry
			end
		end
	end
	commitItemEdit(item)
	return M.item_crucible(p)
end

M.catalyst_info = function(p)
	ensureBuild()
	local item = requireItem(p)
	local usable = (item.crafted or item.hasModTags) and item.base and (item.base.type == "Amulet" or item.base.type == "Ring")
	return {
		usable = usable and true or false,
		names = strArray(catalystNames),
		catalyst = item.catalyst or 0,
		quality = item.catalystQuality or 0,
	}
end

do
	local lineTables = { explicit = "explicitModLines", implicit = "implicitModLines", enchant = "enchantModLines" }
	local function editableModifier(item, section, line)
		-- Craft() overwrites non-custom explicit lines from affix definitions.
		return not line.rune and not (item.crafted and section == "explicit" and not line.custom)
	end
	local function ranged(line)
		return not line.extra and type(line.range) ~= "table" and line.line:match("%(%-?[%d%.]+%-%-?[%d%.]+%)") ~= nil
	end
	local function singleLine(text)
		if type(text) ~= "string" or not text:match("%S") or text:find("[\r\n{}]") then
			error("enter one modifier line without item-text metadata", 0)
		end
		return text
	end

	M.item_customization = function(p)
		local item = requireItem(p)
		local lines = array({})
		for _, section in ipairs({ "implicit", "enchant", "explicit" }) do
			for index, line in ipairs(item[lineTables[section]] or {}) do
				if editableModifier(item, section, line) then
					lines[#lines + 1] = { section = section, index = index, text = line.line,
						disabled = line.disabled == true, range = ranged(line) and (line.range or main.defaultItemAffixQuality or 1) or null,
						parsed = not line.extra and line.modList and #line.modList > 0 or false }
				end
			end
		end
		return { raw = item:BuildRaw(), quality = item.quality or 0,
			canQuality = not not (item.base.quality or (not IS_POE2 and (item.base.weapon or item.base.armour or item.base.flask or item.base.tincture))),
			itemLevel = item.itemLevel or 1, corrupted = item.corrupted == true,
			runeSocketLimit = IS_POE2 and (item.base.socketLimit or 0) or 0,
			affixes = M.item_affixes(p), runes = M.item_runes(p), variants = M.item_variants(p),
			catalyst = M.catalyst_info(p), modifiers = lines,
			shape = M.item_shape(p), crucible = M.item_crucible(p),
			anoints = M.item_anoints(p), corruptions = M.item_corruptions(p),
			enchantable = M.item_enchants(p).available,
			canCopyAnoints = item.canBeAnointed == true or item.base.type == "Amulet",
			canCopyAugments = IS_POE2 and (item.base.socketLimit or 0) > 0 }
	end

	M.item_modifier_options = function(p)
		local item = requireItem(p)
		local result = array({})
		local source = p.source == "Suffix" and "Suffix" or "Prefix"
		local query = tostring(p.query or ""):lower()
		for id, mod in pairs(item.affixes or {}) do
			if mod.type == source and item:GetModSpawnWeight(mod) > 0 then
				local label = table.concat(mod, " / ")
				local match = true
				for word in query:gmatch("%S+") do
					if not label:lower():find(word, 1, true) then match = false; break end
				end
				if match then result[#result + 1] = { id = id, label = label, level = mod.level or 0 } end
			end
		end
		table.sort(result, function(a, b) return a.label == b.label and a.id < b.id or a.label < b.label end)
		local total = #result
		while #result > 100 do table.remove(result) end
		return { options = result, total = total }
	end

	M.item_customize = function(p)
		local item = requireItem(p)
		local setters = { props = M.set_item_props, affix = M.set_item_affix, rune = M.set_item_rune, variant = M.set_item_variant,
			shape = M.set_item_shape, crucible = M.set_item_crucible, enchant = M.set_item_enchant,
			anoint = M.set_item_anoint, corruption = M.corrupt_item }
		if setters[p.operation] then
			setters[p.operation](p)
		else
			if p.operation == "normalize" then
				item:NormaliseQuality()
			elseif p.operation == "copy_anoints" or p.operation == "copy_augments" then
				local copy = build.itemsTab.CopyAnointsAndAugments or build.itemsTab.CopyAnointsAndEldritchImplicits
				if not copy or (p.operation == "copy_augments" and not IS_POE2) then error("copy is not available for this game", 0) end
				local slotName = p.sourceSlot or item:GetPrimarySlot()
				local slot = build.itemsTab.slots[slotName]
				if not slot or not build.itemsTab.items[slot.selItemId] then error("no equipped item in the source slot", 0) end
				if not build.itemsTab:IsItemValidForSlot(item, slotName) then error("source slot is not compatible with this item", 0) end
				local enchants = copyTable(item.enchantModLines, true)
				copy(build.itemsTab, item, p.operation == "copy_augments", true, slotName)
				if p.operation == "copy_augments" then item.enchantModLines = enchants end
			elseif p.operation == "rune_sockets" then
				local limit = IS_POE2 and (item.base.socketLimit or 0) or 0
				local count = tonumber(p.count)
				if not count or count % 1 ~= 0 or count < 0 or count > limit then error("invalid rune socket count", 0) end
				item.itemSocketCount = count
				item:UpdateRunes()
			elseif p.operation == "add_modifier" then
				if p.modId then
					local mod = item.affixes and item.affixes[p.modId]
					if not mod or (mod.type ~= "Prefix" and mod.type ~= "Suffix") or item:GetModSpawnWeight(mod) <= 0 then
						error("modifier is not compatible with this item", 0)
					end
					for _, line in ipairs(mod) do
						table.insert(item.explicitModLines, { line = line, range = main.defaultItemAffixQuality or 0.5,
							modTags = mod.modTags, [mod.type:lower()] = true, custom = item.crafted or nil })
					end
				else
					table.insert(item.explicitModLines, { line = singleLine(p.text), custom = true, range = main.defaultItemAffixQuality or 0.5 })
				end
			elseif p.operation == "modifier" then
				local list = item[lineTables[p.section] or ""]
				local index = tonumber(p.index)
				local line = list and index and list[index]
				if not line or line.rune then error("unknown modifier", 0) end
				if not editableModifier(item, p.section, line) then error("edit generated modifiers through the affix controls", 0) end
				if p.text ~= nil then singleLine(p.text) end
				if p.range ~= nil and (not ranged(line) or type(p.range) ~= "number" or p.range < 0 or p.range > 1) then error("invalid modifier roll", 0) end
				if p.remove then table.remove(list, index)
				else
					if p.text ~= nil then line.line = p.text end
					if p.disabled ~= nil then line.disabled = p.disabled == true end
					if p.range ~= nil then line.range = p.range end
				end
			else error("unknown customization operation", 0) end
			commitItemEdit(item)
		end
		return M.item_customization(p)
	end
end

-- ---------------------------------------------------------------------------
-- Anoints: Distilled Emotion recipes allocating a notable (enchant line
-- "Allocates <name>"), mirroring ItemsTab:AnointDisplayItem/anointItem.
-- ---------------------------------------------------------------------------

M.item_anoints = function(p)
	ensureBuild()
	local item = requireItem(p)
	local anointable = item.canBeAnointed or (item.base and item.base.type == "Amulet")
	local current = strArray(build.itemsTab:getAnoint(item) or {})
	local nodes = array({})
	if anointable and p and p.withNodes then
		for _, node in pairs(build.spec.tree.nodes) do
			if node.recipe and #node.recipe >= 1 then
				nodes[#nodes + 1] = {
					id = node.id,
					name = node.dn or "?",
					stats = strArray(node.sd or {}),
					recipe = strArray(node.recipe),
					allocated = build.spec.allocNodes[node.id] ~= nil,
				}
			end
		end
		table.sort(nodes, function(a, b) return a.name < b.name end)
	end
	local slots = 1
	if item.canHaveTwoEnchants and #item.enchantModLines > 0 then slots = 2 end
	if item.canHaveThreeEnchants and #item.enchantModLines > 1 then slots = 3 end
	if item.canHaveFourEnchants and #item.enchantModLines > 2 then slots = 4 end
	return { anointable = anointable and true or false, current = current, slots = slots, nodes = nodes }
end

M.set_item_anoint = function(p)
	ensureBuild()
	local item = requireItem(p)
	local info = M.item_anoints(p)
	if not info.anointable then error("this item cannot be anointed", 0) end
	local node
	if p.nodeId ~= nil and p.nodeId ~= null then
		node = build.spec.tree.nodes[tonumber(p.nodeId)]
		if not node or not node.recipe then error("unknown anoint node " .. tostring(p.nodeId), 0) end
	end
	local slot = tonumber(p.slot) or 1
	if slot % 1 ~= 0 or slot < 1 or slot > info.slots then error("invalid anoint slot", 0) end
	if #item.enchantModLines >= slot then table.remove(item.enchantModLines, slot) end
	if node then table.insert(item.enchantModLines, slot, { enchant = true, line = "Allocates " .. node.dn }) end
	commitItemEdit(item)
	return M.item_anoints(p)
end

-- ---------------------------------------------------------------------------
-- Corruptions, mirroring ItemsTab:CorruptDisplayItem: corrupted implicits
-- from data.itemMods.Corruption, and roll-range corruption for uniques.
-- ---------------------------------------------------------------------------

-- PoE2 names the pool Corruption, PoE1 Corrupted.
local function corruptionMods()
	return data.itemMods.Corruption or data.itemMods.Corrupted or {}
end

M.item_corruptions = function(p)
	ensureBuild()
	local item = requireItem(p)
	local isGlimpse = item.base and item.base.type == "Helmet" and item.title == "Glimpse of Chaos"
	local function modList(modType)
		local out = {}
		for modId, mod in pairs(corruptionMods()) do
			if mod.type == modType and (modType == "SpecialCorrupted" or item:GetModSpawnWeight(mod) > 0) then
				out[#out + 1] = { id = modId, label = table.concat(mod, "/"), group = opt(mod.group) }
			end
		end
		table.sort(out, function(a, b) return a.label < b.label end)
		return array(out)
	end
	local ranges = array({})
	if item.rarity == "UNIQUE" or item.rarity == "RELIC" then
		for i, mod in ipairs(item.explicitModLines) do
			local scaled = itemLib.applyRange(mod.line, mod.range or main.defaultItemAffixQuality or 1, mod.valueScalar or 1, 2)
			if scaled ~= mod.line and item:CheckModLineVariant(mod) then
				ranges[#ranges + 1] = { index = i, line = mod.line, current = mod.corruptedRange or 1 }
			end
		end
	end
	return {
		corruptible = item.corruptible and true or false,
		corrupted = item.corrupted and true or false,
		enchantNum = isGlimpse and 8 or 2,
		mods = modList("Corrupted"),
		specialMods = isGlimpse and modList("SpecialCorrupted") or array({}),
		ranges = ranges,
	}
end

M.corrupt_item = function(p)
	ensureBuild()
	local item = requireItem(p)
	local info = M.item_corruptions(p)
	if not info.corruptible and not info.corrupted then error("this item cannot be corrupted", 0) end
	local allowed, groups = {}, {}
	for _, list in ipairs({ info.mods, info.specialMods }) do
		for _, mod in ipairs(list) do allowed[mod.id] = true end
	end
	if p.modIds and #p.modIds > info.enchantNum then error("too many corruption implicits", 0) end
	for _, id in ipairs(p.modIds or {}) do
		local mod = corruptionMods()[id]
		if not allowed[id] then error("invalid corruption modifier", 0) end
		if mod.group and groups[mod.group] then error("duplicate corruption group", 0) end
		if mod.group then groups[mod.group] = true end
	end
	local ranges = {}
	for _, r in ipairs(info.ranges) do ranges[r.index] = true end
	for _, r in ipairs(p.ranges or {}) do
		if not ranges[r.index] or type(r.value) ~= "number" or r.value < 0.78 or r.value > 1.22 then
			error("invalid corruption roll", 0)
		end
	end
	item.corrupted = true
	if p and p.modIds and #p.modIds > 0 then
		local newEnchant = {}
		for _, id in ipairs(p.modIds) do
			local mod = corruptionMods()[id]
			if not mod then error("unknown corruption mod " .. tostring(id), 0) end
			for i, modLine in ipairs(mod) do
				if mod.modTags[1] then
					newEnchant[#newEnchant + 1] = { line = "{tags:" .. table.concat(mod.modTags, ",") .. "}" .. modLine, enchant = true, order = mod.statOrder[i] }
				else
					newEnchant[#newEnchant + 1] = { line = modLine, enchant = true, order = mod.statOrder[i] }
				end
			end
		end
		-- keep anoints; corruption implicits replace any other enchants
		local keep = {}
		for _, m in ipairs(item.enchantModLines) do
			if m.line:match("Allocates .*") then keep[#keep + 1] = m end
		end
		item.enchantModLines = keep
		table.sort(newEnchant, function(a, b) return a.order < b.order end)
		for i, e in ipairs(newEnchant) do
			e.order = nil
			table.insert(item.enchantModLines, i, e)
		end
	end
	if p and p.ranges then
		for _, r in ipairs(p.ranges) do
			local ml = item.explicitModLines[tonumber(r.index) or -1]
			if ml then
				local v = tonumber(r.value) or 1
				ml.corruptedRange = v ~= 1 and v or nil
			end
		end
	end
	commitItemEdit(item)
	return { ok = true }
end

-- ---------------------------------------------------------------------------
-- Shared items (main.sharedItemList): PoB stores these in its own settings
-- file, which this app never writes — the list read from the user's real
-- PoB settings is available here, and additions are restored app-side.
-- ---------------------------------------------------------------------------

M.get_shared_items = function()
	local items = array({})
	for i, item in ipairs(main.sharedItemList) do
		items[#items + 1] = {
			index = i,
			name = item.name or "?",
			baseName = opt(item.baseName),
			rarity = opt(item.rarity),
			raw = item:BuildRaw(),
		}
	end
	return { items = items }
end

M.add_shared_item = function(p)
	ensureBuild()
	local raw
	if p and p.itemId then
		raw = requireItem(p):BuildRaw()
	elseif p and p.raw then
		raw = p.raw
	else
		error("params.itemId or params.raw is required", 0)
	end
	local item = new("Item"):Item(raw)
	if not item.base then error("unrecognised item text", 0) end
	table.insert(main.sharedItemList, item)
	return M.get_shared_items()
end

M.remove_shared_item = function(p)
	local index = tonumber(p and p.index)
	if not index or not main.sharedItemList[index] then error("unknown shared item index", 0) end
	table.remove(main.sharedItemList, index)
	return M.get_shared_items()
end

M.equip_shared_item = function(p)
	ensureBuild()
	local index = tonumber(p and p.index)
	local shared = main.sharedItemList[index or -1]
	if not shared then error("unknown shared item index", 0) end
	return M.equip_item_raw({ text = shared:BuildRaw(), slot = p and p.slotName })
end

-- ---------------------------------------------------------------------------
-- Trade search generation: PoB's TradeQueryGenerator driven headless. The
-- weight scan is a coroutine, pumped in chunks like the tree power builder.
-- First use builds Data/QueryMods.lua (downloading GGG's trade stats via the
-- host's HTTP shim); it lands in the user cache overlay and is reused.
-- ---------------------------------------------------------------------------

local tradeGen, tradeState

-- The trade site's listing filter, in the order TradeQueryGenerator indexes it.
local TRADE_STATUS = { "securable", "available", "onlineleague", "online", "any" }

-- A jewel socket has no base type to infer from, so the caller picks one.
-- The generator appends "Jewel" to this, so only these three read back as a
-- category it knows.
local TRADE_JEWEL_TYPES = { Base = true, Abyss = true, Any = true }

M.trade_status_options = function()
	local out = array({})
	local labels = {
		securable = "Instant buyout",
		available = "Buyout or fixed price",
		onlineleague = "Online in this league",
		online = "Online anywhere",
		any = "Any listing",
	}
	for i, id in ipairs(TRADE_STATUS) do
		out[i] = { id = id, label = labels[id] }
	end
	return { options = out }
end

M.trade_search_start = function(p)
	ensureBuild()
	local slot = build.itemsTab.slots[p and p.slotName or ""]
	if not slot then error("unknown slot " .. tostring(p and p.slotName), 0) end
	if not tradeGen then
		tradeGen = new("TradeQueryGenerator"):TradeQueryGenerator({ itemsTab = build.itemsTab })
	end
	tradeGen.itemsTab = build.itemsTab
	local weights = {}
	for _, w in ipairs((p and p.statWeights) or { { stat = "FullDPS", weightMult = 1 } }) do
		local found
		for _, s in ipairs(data.powerStatList) do
			if s.stat == w.stat then found = s break end
		end
		weights[#weights + 1] = {
			label = (found and found.label) or w.stat,
			stat = w.stat,
			transform = found and found.transform or nil,
			weightMult = tonumber(w.weightMult) or 1,
		}
	end
	local options = {
		statWeights = weights,
		includeCorrupted = p and p.includeCorrupted and true or false,
		includeRunes = p and p.includeRunes and true or false,
		includeMirrored = p and p.includeMirrored and true or false,
		maxLevel = tonumber(p and p.maxLevel),
		sockets = tonumber(p and p.sockets),
		jewelType = (p and p.jewelType) or "Base",
	}
	if not TRADE_JEWEL_TYPES[options.jewelType] then
		error("jewelType must be Base, Abyss or Any", 0)
	end
	local statusIndex = 4
	if p and p.status then
		statusIndex = nil
		for i, id in ipairs(TRADE_STATUS) do
			if id == p.status then statusIndex = i break end
		end
		if not statusIndex then error("unknown trade status " .. tostring(p.status), 0) end
	end
	tradeGen.tradeTypeIndex = statusIndex
	tradeState = { done = false }
	tradeGen.requesterContext = nil
	tradeGen.requesterCallback = function(_, queryJson, errMsg)
		tradeState.done = true
		tradeState.query = queryJson
		tradeState.err = errMsg and tostring(errMsg) or nil
	end
	tradeGen:StartQuery(slot, options)
	if not tradeGen.calcContext or not tradeGen.calcContext.co then
		-- StartQuery bailed (unsupported slot/item type)
		if not tradeState.done then error("this slot's item type is not supported for trade search generation", 0) end
	end
	return { started = true }
end

M.trade_search_step = function(p)
	if not tradeState then error("no trade search in progress", 0) end
	local steps = tonumber(p and p.steps) or 40
	for _ = 1, steps do
		if tradeState.done or not tradeGen.calcContext or not tradeGen.calcContext.co then break end
		tradeGen:OnFrame()
	end
	if not tradeState.done and (not tradeGen.calcContext or not tradeGen.calcContext.co) then
		tradeGen:FinishQuery()
	end
	return { done = tradeState.done and true or false }
end

M.trade_search_result = function()
	if not tradeState or not tradeState.done then error("no finished trade search", 0) end
	if tradeState.err then error(tradeState.err, 0) end
	return { query = tradeState.query }
end

M.trade_leagues = function()
	local api = IS_POE2 and "trade2" or "trade"
	local body, err = native.http_get("https://www.pathofexile.com/api/" .. api .. "/data/leagues", "Path of Building/" .. (launch and launch.versionNumber or "2"))
	if not body then error(err or "download failed", 0) end
	local decoded = dkjson.decode(body)
	-- GGG repeats every league once per realm (pc, xbox and sony on PoE1; poe2
	-- and its consoles on PoE2). The trade links this app builds carry no realm,
	-- so keep the first entry for each league and drop the rest.
	local leagues = array({})
	local seen = {}
	for _, l in ipairs((decoded and decoded.result) or {}) do
		if l.id and not seen[l.id] then
			seen[l.id] = true
			leagues[#leagues + 1] = { id = l.id, text = l.text or l.id, realm = opt(l.realm) }
		end
	end
	if #leagues == 0 then error("league list unavailable", 0) end
	return { leagues = leagues }
end

-- PoE1 only: the host fetches the character from pathofexile.com and PoB's ImportTab builds it.
M.import_character = function(p)
	if IS_POE2 then error("importing a character by account name works for Path of Exile 1 only", 0) end
	if not p or type(p.character) ~= "table" or type(p.passives) ~= "string" or type(p.items) ~= "string" then
		error("params.character, params.passives and params.items are required", 0)
	end
	local passives = dkjson.decode(p.passives)
	local items = dkjson.decode(p.items)
	if type(passives) ~= "table" or type(items) ~= "table" then
		error("the character data from pathofexile.com could not be read", 0)
	end
	main:SetMode("BUILD", false, (type(p.name) == "string" and p.name) or p.character.name or "Imported character")
	frame()
	build = main.modes["BUILD"]
	ensureBuild()
	local importTab = build.importTab
	passives.bandit_choice = passives.bandit_choice or build.configTab.input.bandit
	passives.pantheon_major = passives.pantheon_major or build.configTab.input.pantheonMajorGod
	passives.pantheon_minor = passives.pantheon_minor or build.configTab.input.pantheonMinorGod
	local treeData = copyTable(p.character)
	treeData.passives = passives
	treeData.jewels = passives.items
	importTab:ImportPassiveTreeAndJewels(treeData, true)
	local gearData = copyTable(p.character)
	gearData.equipment = items.items
	gearData.guardian = items.guardian
	importTab:ImportItemsAndSkills(gearData, true, true, false)
	refresh()
	return M.get_build()
end

-- ---------------------------------------------------------------------------
-- GGG Build Planner (*.build) import: JSON with passive stringIds, gem
-- metadata ids and gear hint text. Tree goes through PoB's own
-- ImportFromNodeList (weapon-set allocations included); gear hints land in
-- Notes. The class is inferred by walking the imported node set from each
-- class start.
-- ---------------------------------------------------------------------------

M.import_game_build = function(p)
	if not p or type(p.json) ~= "string" then error("params.json is required", 0) end
	-- strip a UTF-8 BOM; GGG writes plain UTF-8, other tools may not
	local jsonText = p.json:gsub("^\239\187\191", "")
	local dat, _, jsonErr = dkjson.decode(jsonText)
	if dat == nil then
		error("not a valid .build file: " .. tostring(jsonErr or "JSON parse failed"), 0)
	end
	if type(dat) ~= "table" or dat[1] ~= nil then
		error("not a valid .build file: the top level must be a single Build object", 0)
	end
	if dat.name ~= nil and type(dat.name) ~= "string" then
		error('not a valid .build file: "name" must be a string', 0)
	end
	-- The schema allows array entries to be plain id strings or objects with
	-- an `id`; validate both and collect precise problems instead of failing
	-- silently.
	local problems = {}
	local function checkArray(field)
		local v = dat[field]
		if v ~= nil and (type(v) ~= "table" or (next(v) ~= nil and v[1] == nil)) then
			problems[#problems + 1] = '"' .. field .. '" must be an array'
			dat[field] = nil
		end
	end
	checkArray("passives")
	checkArray("skills")
	checkArray("inventory_slots")
	local function entryId(v, where)
		if type(v) == "string" then return v, {} end
		if type(v) == "table" and type(v.id) == "string" then return v.id, v end
		problems[#problems + 1] = where .. ': expected an id string or an object with an "id" string'
		return nil
	end
	-- level_interval: [lo, hi] or a single level (taken as "from here on").
	-- Progression files list early-only entries; the import takes the final
	-- state, i.e. everything active at the highest level any entry reaches.
	local function interval(entry)
		local li = entry.level_interval
		if type(li) == "number" then return li, 100 end
		if type(li) == "table" and tonumber(li[1]) then return tonumber(li[1]), tonumber(li[2]) or 100 end
		return 0, 100
	end
	local targetLevel = 0
	for _, field in ipairs({ "passives", "skills" }) do
		for _, v in ipairs(dat[field] or {}) do
			if type(v) == "table" then
				local _, hi = interval(v)
				if hi > targetLevel then targetLevel = hi end
			end
		end
	end
	if targetLevel == 0 then targetLevel = 100 end
	local function activeAtTarget(entry)
		local lo, hi = interval(entry)
		return lo <= targetLevel and targetLevel <= hi
	end
	-- finish the unique-DB parse up front; gear import reads it, and pumping
	-- frames mid-import stalls the loader coroutine
	do
		local guard = 0
		while (main.uniqueDB.loading or main.rareDB.loading) and guard < 20000 do
			frame()
			guard = guard + 1
		end
	end
	M.new_build({ name = (p.name and p.name ~= "" and p.name) or dat.name or "Imported build" })
	local spec = build.spec
	local byString = {}
	for id, node in pairs(spec.tree.nodes) do
		if node.stringId then byString[node.stringId] = id end
	end
	-- A node listed without weapon_set is part of the base tree even if it is
	-- also listed for a set; only nodes that appear exclusively for one set
	-- become set-specific (PoB allocMode 1/2). Getting this wrong severs the
	-- base tree and PoB prunes everything downstream.
	local hashList, weaponSets, missing, seen = {}, {}, {}, {}
	local inBase, inSet = {}, {}
	for i, pass in ipairs(dat.passives or {}) do
		local pid, entry = entryId(pass, "passives[" .. i .. "]")
		if pid and not activeAtTarget(entry) then pid = nil end
		if pid then
			local nid = byString[pid]
			if nid then
				if not seen[nid] then
					seen[nid] = true
					hashList[#hashList + 1] = nid
				end
				local ws = tonumber(entry.weapon_set)
				if entry.weapon_set ~= nil and (not ws or ws < 0 or ws > 2) then
					problems[#problems + 1] = "passives[" .. i .. ']: "weapon_set" must be between 0 and 2'
					inBase[nid] = true
				elseif ws and ws > 0 then
					inSet[nid] = inSet[nid] or {}
					inSet[nid][ws] = true
				else
					inBase[nid] = true
				end
			else
				missing[#missing + 1] = pid
			end
		end
	end
	for nid, sets in pairs(inSet) do
		if not inBase[nid] and not (sets[1] and sets[2]) then
			weaponSets[nid] = sets[1] and 1 or 2
		end
	end
	-- class and ascendancy: explicit metadata wins; an imported ascendancy
	-- node pins both; otherwise BFS the imported set from each class start.
	-- Twin classes share a start node, so ties prefer PoE2-native classes
	-- over the legacy scaffolding classes still present in the tree data.
	-- How many imported nodes each class start can reach through the imported
	-- set. PoB deallocates anything not connected to the chosen start, so the
	-- class must match the tree even when the metadata says otherwise.
	local legacy = { Ranger = true, Duelist = true, Templar = true, Marauder = true, Shadow = true, Scion = true, Six = true }
	local reach, bestCid, bestReach, bestLegacy = {}, nil, -1, true
	local cids = {}
	for cid in pairs(spec.tree.classes) do
		if type(cid) == "number" then cids[#cids + 1] = cid end
	end
	table.sort(cids)
	for _, cid in ipairs(cids) do
		local class = spec.tree.classes[cid]
		if type(class) == "table" and class.startNodeId and spec.nodes[class.startNodeId] then
			local visited = { [class.startNodeId] = true }
			local queue, reached = { spec.nodes[class.startNodeId] }, 0
			while #queue > 0 do
				local node = table.remove(queue)
				for _, other in ipairs(node.linked or {}) do
					if not visited[other.id] and seen[other.id] then
						visited[other.id] = true
						reached = reached + 1
						queue[#queue + 1] = other
					end
				end
			end
			reach[cid] = reached
			local isLegacy = legacy[class.name] or false
			if reached > bestReach or (reached == bestReach and bestLegacy and not isLegacy) then
				bestReach, bestLegacy, bestCid = reached, isLegacy, cid
			end
		end
	end
	local function className(cid)
		local c = spec.tree.classes[cid]
		return (type(c) == "table" and c.name) or tostring(cid)
	end

	local classId, ascendClassId, classSource = nil, 0, nil
	local wantedAscend = dat.ascendancy or dat.ascendancy_class
	if type(wantedAscend) == "string" then
		-- display name ("Gemling Legionnaire") or the game's internal id ("Mercenary3")
		local m = (spec.tree.ascendNameMap and spec.tree.ascendNameMap[wantedAscend])
			or (spec.tree.internalAscendNameMap and spec.tree.internalAscendNameMap[wantedAscend])
		if m and m.classId and m.ascendClassId then
			classId, ascendClassId, classSource = m.classId, m.ascendClassId, '"ascendancy" ' .. wantedAscend
		else
			problems[#problems + 1] = '"ascendancy": unknown ascendancy ' .. wantedAscend .. " (inferred from the tree instead)"
		end
	end
	local ascFromNodes, ascNodeName
	for nid in pairs(seen) do
		local node = spec.nodes[nid]
		if node and node.ascendancyName and spec.tree.ascendNameMap and spec.tree.ascendNameMap[node.ascendancyName] then
			ascFromNodes, ascNodeName = spec.tree.ascendNameMap[node.ascendancyName], node.ascendancyName
			break
		end
	end
	if not classId and ascFromNodes then
		classId, ascendClassId, classSource = ascFromNodes.classId, ascFromNodes.ascendClassId, "its " .. ascNodeName .. " passives"
	end
	-- Ascendancy passives in the file settle the class. Guide sites export
	-- trees that need not path from the class start (the game's planner only
	-- highlights nodes), so reachability alone would misfile such a build
	-- under whichever start its nodes happen to touch.
	local pinned = ascFromNodes ~= nil and ascFromNodes.classId == classId
	if not pinned and classId and bestCid and #hashList > 0 and (reach[classId] or 0) * 2 < bestReach then
		problems[#problems + 1] = string.format(
			"%s says %s, but the passives connect to the %s start (%d of %d reachable vs %d) — imported as %s without an ascendancy; the file's class label is wrong or the tree is",
			classSource, className(classId), className(bestCid), reach[classId] or 0, #hashList, bestReach, className(bestCid))
		classId, ascendClassId = bestCid, 0
	end
	classId = classId or bestCid or build.classId or 0
	spec:ImportFromNodeList(nil, classId, ascendClassId, 0, hashList, weaponSets, {}, {}, latestTreeVersion)
	spec:AddUndoState()
	-- PoB prunes nodes it cannot connect to the class start; report rather
	-- than pretend they were allocated
	local allocatedCount, normalPoints, prunedNames = 0, 0, {}
	for _, nid in ipairs(hashList) do
		local node = spec.allocNodes[nid]
		if node then
			allocatedCount = allocatedCount + 1
			if not node.ascendancyName and node.type ~= "ClassStart" and (node.allocMode or 0) == 0 then
				normalPoints = normalPoints + 1
			end
		elseif #prunedNames < 5 then
			prunedNames[#prunedNames + 1] = (spec.nodes[nid] and spec.nodes[nid].dn) or tostring(nid)
		end
	end
	local pruned = #hashList - allocatedCount
	if pruned > 0 then
		problems[#problems + 1] = string.format(
			"%d of %d passives could not be connected to the tree from the %s start and were dropped by PoB (%s%s)",
			pruned, #hashList, className(classId), table.concat(prunedNames, ", "), pruned > #prunedNames and ", …" or "")
	end
	-- the format carries no character level; estimate one from the points
	-- spent (quest points come along the way, so points ≈ level early on)
	if normalPoints > 0 then
		build.characterLevel = math.min(100, math.max(1, normalPoints + 1))
		build.characterLevelAutoMode = false
	end
	-- skills: one socket group per entry. The game references BaseItemTypes
	-- ids; PoB keys some gems (notably supports) under internal ids with the
	-- game's id in `gameId`, so resolve through both.
	local gameIdIndex
	local function findGem(gid)
		gid = tostring(gid or "")
		if data.gems[gid] then return gid end
		if not gameIdIndex then
			gameIdIndex = {}
			for key, gem in pairs(data.gems) do
				if gem.gameId and not gameIdIndex[gem.gameId] then gameIdIndex[gem.gameId] = key end
			end
		end
		if gameIdIndex[gid] then return gameIdIndex[gid] end
		for _, alt in ipairs({ (gid:gsub("/Gems/", "/Gem/")), (gid:gsub("/Gem/", "/Gems/")) }) do
			if data.gems[alt] then return alt end
			if gameIdIndex[alt] then return gameIdIndex[alt] end
		end
	end
	local addedGroups, missingSkills = 0, {}
	-- planners often list a gem twice (socketed setup plus a bare mention);
	-- a bare repeat of an active already imported is dropped
	local seenActive = {}
	for i, sk in ipairs(dat.skills or {}) do
		local skId, skEntry = entryId(sk, "skills[" .. i .. "]")
		if skId and not activeAtTarget(skEntry) then skId = nil end
		local gid = skId and findGem(skId)
		local supports = type(skEntry and skEntry.support_skills) == "table" and skEntry.support_skills or {}
		if gid and seenActive[gid] and #supports == 0 then gid = nil end
		if gid then
			seenActive[gid] = true
			local group = { label = "", enabled = true, gemList = {} }
			local function addGem(id)
				local gemData = data.gems[id]
				local gem = {
					level = 1,
					quality = build.skillsTab.defaultGemQuality or 0,
					enabled = true,
					enableGlobal1 = true,
					enableGlobal2 = true,
					gemId = id,
					nameSpec = gemData.name,
					skillId = gemData.grantedEffectId,
				}
				gem.level = build.skillsTab:ProcessGemLevel(gemData)
				group.gemList[#group.gemList + 1] = gem
			end
			addGem(gid)
			if skEntry.support_skills ~= nil and type(skEntry.support_skills) ~= "table" then
				problems[#problems + 1] = "skills[" .. i .. ']: "support_skills" must be an array'
			end
			for j, sup in ipairs(type(skEntry.support_skills) == "table" and skEntry.support_skills or {}) do
				local supId = entryId(sup, "skills[" .. i .. "].support_skills[" .. j .. "]")
				local sgid = supId and findGem(supId)
				if sgid then addGem(sgid) elseif supId then missingSkills[#missingSkills + 1] = supId end
			end
			table.insert(build.skillsTab.socketGroupList, group)
			build.skillsTab:ProcessSocketGroup(group)
			addedGroups = addedGroups + 1
		elseif skId then
			missingSkills[#missingSkills + 1] = skId
		end
	end
	if addedGroups > 0 then
		build.mainSocketGroup = 1
		build.skillsTab:AddUndoState()
	end
	-- the format has no "main skill"; pick the socket group PoB rates highest
	-- for damage (auras, buffs and item-granted groups never win by accident)
	local function pickMainSkill()
		local candidates = {}
		for i, group in ipairs(build.skillsTab.socketGroupList) do
			if group.enabled and not group.source then candidates[#candidates + 1] = i end
		end
		if #candidates <= 1 then
			build.mainSocketGroup = candidates[1] or build.mainSocketGroup
			return
		end
		local best, bestDps = candidates[1], -1
		for _, i in ipairs(candidates) do
			build.mainSocketGroup = i
			build.buildFlag = true
			frame()
			local o = build.calcsTab.mainOutput
			local dps = o and (o.CombinedDPS or o.TotalDPS or 0) or 0
			if dps > bestDps then bestDps, best = dps, i end
		end
		build.mainSocketGroup = best
	end
	-- gear: the format carries text hints, but they usually name a base plus
	-- numbered mod lines (and unique_name for uniques) — enough to build real
	-- items through PoB's parser. Uniques come from PoB's own unique DB when
	-- the name matches. Only unresolvable hints fall back to Notes.
	local invMapIn = {
		Weapon1 = "Weapon 1",
		Offhand1 = "Weapon 2",
		Weapon2 = "Weapon 1 Swap",
		Offhand2 = "Weapon 2 Swap",
		Helm1 = "Helmet",
		BodyArmour1 = "Body Armour",
		Gloves1 = "Gloves",
		Boots1 = "Boots",
		Belt1 = "Belt",
		Amulet1 = "Amulet",
		Ring1 = "Ring 1",
		Ring2 = "Ring 2",
	}
	local gearAdded, gearHints = 0, {}
	local uniqueByTitle
	local function findUnique(title)
		if main.uniqueDB.loading then return nil end
		if not uniqueByTitle then
			-- the unique DB is keyed "Name, Base"
			uniqueByTitle = {}
			for key, dbItem in pairs(main.uniqueDB.list) do
				local t = key:match("^(.-),%s") or key
				if not uniqueByTitle[t] then uniqueByTitle[t] = dbItem end
			end
		end
		-- authors sometimes annotate the name, e.g. "Darkness Enthroned (gloves)"
		return uniqueByTitle[title] or main.uniqueDB.list[title] or uniqueByTitle[(title:gsub("%s*%b()%s*$", ""))]
	end
	for i, slot in ipairs(type(dat.inventory_slots) == "table" and dat.inventory_slots or {}) do
		if type(slot) ~= "table" then
			problems[#problems + 1] = "inventory_slots[" .. i .. "]: expected an object"
		else
			local iid = tostring(slot.inventory_id or "")
			local sx = tonumber(slot.slot_x) or 0
			local slotName = invMapIn[iid]
			if not slotName and iid == "Flask1" then slotName = "Flask " .. (sx + 1) end
			if not slotName and iid == "Charm1" then slotName = "Charm " .. (sx + 1) end
			local text = tostring(slot.additional_text or "")
			-- Guide sites wrap the hint text in their own markup (Maxroll:
			-- `<rgb(1,2,3)>{<b>{name}}`) and pad it with headings and `#`
			-- placeholder lines; only plain "base name, then one mod per line"
			-- is something PoB's item parser can take.
			local lines = {}
			for line in (text .. "\n"):gmatch("(.-)\n") do
				line = line:gsub("<[^<>]*>{", ""):gsub("}", ""):gsub("\r", "")
				line = line:match("^%s*(.-)%s*$")
				if line ~= "" and not line:find("#", 1, true) and not line:match("^%-+$") and not line:match(":$") then
					lines[#lines + 1] = line
				end
			end
			local baseName = lines[1]
			local mods = {}
			for j = 2, #lines do
				mods[#mods + 1] = (lines[j]:gsub("^%d+[%.%)]%s*", ""))
			end
			local function makeItem(raw)
				local ok, made = pcall(function() return new("Item"):Item(raw) end)
				return ok and made or nil
			end
			local item
			local uniqueName = type(slot.unique_name) == "string" and slot.unique_name ~= "" and slot.unique_name or nil
			if uniqueName then
				local dbItem = findUnique(uniqueName)
				if dbItem then
					item = makeItem(dbItem:BuildRaw())
				elseif baseName then
					item = makeItem("Rarity: UNIQUE\n" .. uniqueName .. "\n" .. baseName .. "\n" .. table.concat(mods, "\n"))
				end
			elseif baseName then
				item = makeItem("Rarity: RARE\nImported " .. baseName .. "\n" .. baseName .. "\n" .. table.concat(mods, "\n"))
				if not (item and item.base) and baseName:find("%(") then
					-- "Kamasan Tiara (Int Base)": the note after the base name is the author's
					local bare = baseName:gsub("%s*%b()%s*$", "")
					item = makeItem("Rarity: RARE\nImported " .. bare .. "\n" .. bare .. "\n" .. table.concat(mods, "\n"))
				end
			end
			if item and item.base then
				build.itemsTab:AddItem(item, true)
				if slotName and build.itemsTab.slots[slotName] and build.itemsTab:IsItemValidForSlot(item, slotName) then
					build.itemsTab.slots[slotName]:SetSelItemId(item.id)
				end
				gearAdded = gearAdded + 1
			elseif #lines > 0 or uniqueName then
				gearHints[#gearHints + 1] = { id = iid, unique = uniqueName, text = text }
			end
		end
	end
	if gearAdded > 0 then
		build.itemsTab:PopulateSlots()
		build.itemsTab:AddUndoState()
	end
	local notes = {}
	if dat.author then notes[#notes + 1] = "Author: " .. tostring(dat.author) end
	if dat.link then notes[#notes + 1] = "Link: " .. tostring(dat.link) end
	if dat.description then notes[#notes + 1] = tostring(dat.description) end
	if #gearHints > 0 then
		notes[#notes + 1] = ""
		notes[#notes + 1] = "== Gear hints that could not be turned into items =="
		for _, hint in ipairs(gearHints) do
			notes[#notes + 1] = ""
			notes[#notes + 1] = "[" .. hint.id .. "]"
			if hint.unique then notes[#notes + 1] = hint.unique end
			notes[#notes + 1] = hint.text
		end
	end
	if #notes > 0 then M.set_notes({ text = table.concat(notes, "\n") }) end
	if addedGroups > 0 then pickMainSkill() end
	refresh()
	return {
		info = M.get_build(),
		allocated = allocatedCount,
		requested = #hashList,
		missingPassives = strArray(missing),
		skillGroups = addedGroups,
		missingSkills = strArray(missingSkills),
		gearItems = gearAdded,
		gearHints = #gearHints,
		warnings = strArray(problems),
	}
end

-- Export the current build as a Build Planner *.build JSON. Only slot ids the
-- game's format is known to accept are emitted for gear hints.
-- Files the game writes carry the ascendancy as its internal id
-- ("Warrior1"); the importer accepts either, so write what the game does.
-- params: { author, link, description } (optional strings)
M.export_game_build = function(p)
	ensureBuild()
	local out = { name = build.buildName or "PoB Redux build" }
	local function text(v)
		if type(v) == "string" and v:match("%S") then return (v:gsub("^%s+", ""):gsub("%s+$", "")) end
		return nil
	end
	out.author = text(p and p.author)
	out.link = text(p and p.link)
	out.description = text(p and p.description)
	local asc = build.spec.curAscendClass
	if (build.spec.curAscendClassId or 0) > 0 and asc and asc.name ~= "None" then
		out.ascendancy = asc.internalId or asc.name
	end
	local passives = {}
	for _, node in pairs(build.spec.allocNodes) do
		if node.stringId and node.type ~= "ClassStart" and node.type ~= "AscendClassStart" then
			local entry = { id = node.stringId }
			if node.allocMode and node.allocMode > 0 then entry.weapon_set = node.allocMode end
			passives[#passives + 1] = entry
		end
	end
	table.sort(passives, function(a, b) return a.id < b.id end)
	out.passives = passives
	local skills = {}
	for _, group in ipairs(build.skillsTab.socketGroupList) do
		if group.enabled and not group.source then
			local actives, supports = {}, {}
			for _, gem in ipairs(group.gemList) do
				local gd = gem.gemData
				if gem.enabled and gd and (gd.gameId or gd.id) then
					local gid = gd.gameId or gd.id
					if gd.gemType == "Support" or (gd.grantedEffect and gd.grantedEffect.support) then
						supports[#supports + 1] = { id = gid }
					else
						actives[#actives + 1] = { id = gid }
					end
				end
			end
			for _, a in ipairs(actives) do
				if #supports > 0 then a.support_skills = supports end
				skills[#skills + 1] = a
			end
		end
	end
	out.skills = skills
	local invMap = {
		["Weapon 1"] = "Weapon1",
		["Helmet"] = "Helm1",
		["Body Armour"] = "BodyArmour1",
		["Gloves"] = "Gloves1",
		["Boots"] = "Boots1",
		["Belt"] = "Belt1",
		["Amulet"] = "Amulet1",
		["Ring 1"] = "Ring1",
		["Ring 2"] = "Ring2",
	}
	local inventory = {}
	for slotName, slot in pairs(build.itemsTab.slots) do
		local item = slot.selItemId and slot.selItemId ~= 0 and build.itemsTab.items[slot.selItemId]
		if item and not slot.nodeId then
			local invId, sx = invMap[slotName], 0
			local flaskNum = slotName:match("^Flask (%d)")
			if not invId and flaskNum then
				invId = "Flask1"
				sx = tonumber(flaskNum) - 1
			end
			if invId then
				local entry = { inventory_id = invId, slot_x = sx, slot_y = 0 }
				if item.rarity == "UNIQUE" or item.rarity == "RELIC" then
					entry.unique_name = item.title or item.name
					entry.additional_text = item.baseName
				else
					local lines = { item.baseName or item.name }
					for i, mod in ipairs(item.explicitModLines or {}) do
						lines[#lines + 1] = i .. ". " .. mod.line:gsub("^{.-}", "")
					end
					entry.additional_text = table.concat(lines, "\n")
				end
				inventory[#inventory + 1] = entry
			end
		end
	end
	table.sort(inventory, function(a, b)
		if a.inventory_id ~= b.inventory_id then return a.inventory_id < b.inventory_id end
		return (a.slot_x or 0) < (b.slot_x or 0)
	end)
	out.inventory_slots = inventory
	return { json = dkjson.encode(out), name = out.name, passives = #passives, skills = #skills, gear = #inventory }
end

-- Weapon set I/II, including PoB's main-skill hand-off to a group socketed in
-- the newly active set.
M.set_weapon_set = function(p)
	ensureBuild()
	local wantSecond = tonumber(p and p.set) == 2
	local itemsTab = build.itemsTab
	if itemsTab.activeItemSet.useSecondWeaponSet == wantSecond then
		return M.list_slots()
	end
	itemsTab.activeItemSet.useSecondWeaponSet = wantSecond
	itemsTab:AddUndoState()
	local from, to = wantSecond and 1 or 2, wantSecond and 2 or 1
	local mainSocketGroup = build.skillsTab.socketGroupList[build.mainSocketGroup]
	if mainSocketGroup and mainSocketGroup.slot and itemsTab.slots[mainSocketGroup.slot] and itemsTab.slots[mainSocketGroup.slot].weaponSet == from then
		for index, socketGroup in ipairs(build.skillsTab.socketGroupList) do
			if socketGroup.slot and itemsTab.slots[socketGroup.slot] and itemsTab.slots[socketGroup.slot].weaponSet == to then
				build.mainSocketGroup = index
				break
			end
		end
	end
	refresh()
	return M.list_slots()
end

-- ---------------------------------------------------------------------------
-- Configuration
-- ---------------------------------------------------------------------------

local configOptionsCache
M.list_config_options = function()
	if not configOptionsCache then
		local varList = LoadModule("Modules/ConfigOptions")
		local options = array({})
		local section = null
		for _, o in ipairs(varList) do
			if o.section then
				section = o.section
			elseif o.var then
				local entry = {
					var = o.var,
					label = opt(o.label),
					type = opt(o.type),
					section = section,
					tooltip = opt(o.tooltip),
					defaultState = opt(o.defaultState),
					ifSkill = o.ifSkill and (type(o.ifSkill) == "table" and strArray(o.ifSkill) or o.ifSkill) or null,
					ifFlag = o.ifFlag and (type(o.ifFlag) == "table" and strArray(o.ifFlag) or o.ifFlag) or null,
					ifCond = o.ifCond and (type(o.ifCond) == "table" and strArray(o.ifCond) or o.ifCond) or null,
					ifMod = o.ifMod and (type(o.ifMod) == "table" and strArray(o.ifMod) or o.ifMod) or null,
				}
				if o.list then
					local list = array({})
					for _, e in ipairs(o.list) do
						list[#list + 1] = { val = e.val == nil and null or (isScalar(e.val) and e.val or tostring(e.val)), label = e.label }
					end
					entry.list = list
				end
				options[#options + 1] = entry
			end
		end
		configOptionsCache = { options = options }
	end
	return configOptionsCache
end

M.get_config = function()
	ensureBuild()
	local configTab = build.configTab
	local set = configTab.configSets[configTab.activeConfigSetId]
	local config, placeholder = {}, {}
	for key, value in pairs(set.input) do
		if isScalar(value) then config[key] = value end
	end
	for key, value in pairs(set.placeholder or {}) do
		if isScalar(value) then placeholder[key] = value end
	end
	local sets = array({})
	for _, id in ipairs(configTab.configSetOrderList) do
		local s = configTab.configSets[id]
		if s then
			sets[#sets + 1] = { id = id, title = opt(s.title), active = id == configTab.activeConfigSetId }
		end
	end
	return { config = config, placeholder = placeholder, activeConfigSet = configTab.activeConfigSetId, sets = sets }
end

M.set_config = function(p)
	ensureBuild()
	if not p or not p.var then error("params.var is required", 0) end
	local configTab = build.configTab
	local input = configTab.configSets[configTab.activeConfigSetId].input
	if p.value == nil or p.value == null then
		input[p.var] = nil
	else
		input[p.var] = p.value
	end
	configTab:BuildModList()
	configTab.modFlag = true
	refresh()
	return M.get_config()
end

M.select_config_set = function(p)
	ensureBuild()
	local id = tonumber(p and p.id)
	if not id or not build.configTab.configSets[id] then error("unknown config set id " .. tostring(p and p.id), 0) end
	build.configTab:SetActiveConfigSet(id)
	followLoadout(build.configTab.configSets[id].title)
	refresh()
	return M.get_config()
end

M.create_config_set = function(p)
	ensureBuild()
	local set = build.configTab:NewConfigSet(nil, (p and p.title) or "New Set")
	build.configTab:SetActiveConfigSet(set.id)
	refresh()
	return M.get_config()
end

M.copy_config_set = function(p)
	ensureBuild()
	local src = tonumber(p and p.id) or build.configTab.activeConfigSetId
	if not build.configTab.configSets[src] then error("unknown config set id", 0) end
	local set = build.configTab:CopyConfigSet(src, p and p.title)
	build.configTab:SetActiveConfigSet(set.id)
	refresh()
	return M.get_config()
end

M.rename_config_set = function(p)
	ensureBuild()
	local id = tonumber(p and p.id)
	if not id or not build.configTab.configSets[id] then error("unknown config set id", 0) end
	if not p.title then error("params.title is required", 0) end
	build.configTab:RenameConfigSet(id, p.title)
	return M.get_config()
end

M.delete_config_set = function(p)
	ensureBuild()
	local id = tonumber(p and p.id)
	if not id or not build.configTab.configSets[id] then error("unknown config set id", 0) end
	if #build.configTab.configSetOrderList <= 1 then error("cannot delete the only config set", 0) end
	local orderIndex
	for i, sid in ipairs(build.configTab.configSetOrderList) do
		if sid == id then orderIndex = i break end
	end
	build.configTab:DeleteConfigSet(id, orderIndex)
	build.configTab:SetActiveConfigSet()
	refresh()
	return M.get_config()
end

-- ---------------------------------------------------------------------------
-- Loadouts: PoB's named (tree, item set, skill set, config set) groupings.
-- A loadout exists where set titles match exactly or share a {linkId} tag.
-- ---------------------------------------------------------------------------

M.get_loadouts = function()
	ensureBuild()
	build:SyncLoadouts(true)
	local names = array({})
	for _, entry in ipairs(build.controls.buildLoadouts.list) do
		if type(entry) == "string" and not entry:match("^%^7%^7") and entry ~= "No Loadouts" then
			names[#names + 1] = entry
		end
	end
	-- SyncLoadouts selects the dropdown entry whose sets are all active, so a
	-- freshly loaded build reports its loadout before anything is picked.
	local ctl = build.controls and build.controls.buildLoadouts
	local sel = ctl and ctl.list and ctl.list[ctl.selIndex or 0]
	local name = type(sel) == "string" and not sel:match("^%^7%^7") and sel ~= "No Loadouts" and sel or nil
	return { loadouts = names, active = name or names[build.activeLoadout or 0] or null }
end

M.select_loadout = function(p)
	ensureBuild()
	if not p or not p.name then error("params.name is required", 0) end
	local loadout = build:GetLoadoutByName(p.name)
	if not loadout then error("unknown loadout " .. tostring(p.name), 0) end
	build:SetActiveLoadout(loadout)
	refresh()
	return M.get_loadouts()
end

M.new_loadout = function(p)
	ensureBuild()
	if not p or not p.name then error("params.name is required", 0) end
	build:NewLoadout(p.name)
	refresh()
	return M.get_loadouts()
end

M.copy_loadout = function(p)
	ensureBuild()
	if not p or not p.source or not p.name then error("params.source and params.name are required", 0) end
	build:CopyLoadout(p.source, p.name)
	refresh()
	return M.get_loadouts()
end

M.rename_loadout = function(p)
	ensureBuild()
	if not p or not p.name or not p.newName then error("params.name and params.newName are required", 0) end
	build:RenameLoadout(p.name, p.newName)
	build:SyncLoadouts(true)
	refresh()
	return M.get_loadouts()
end

M.delete_loadout = function(p)
	ensureBuild()
	if not p or not p.name then error("params.name is required", 0) end
	local state = M.get_loadouts()
	local nextName
	for _, n in ipairs(state.loadouts) do
		if n ~= p.name then nextName = n break end
	end
	if not nextName then error("cannot delete the only loadout", 0) end
	build:DeleteLoadout(p.name, nextName)
	refresh()
	return M.get_loadouts()
end

-- ---------------------------------------------------------------------------
-- Party tab: imported support-build buffs (auras, curses, warcries, links,
-- party member stats, enemy conditions/mods), driven exactly like PoB's
-- PartyTab buttons drive it.
-- ---------------------------------------------------------------------------

local partyKinds = {
	partyMemberStats = { ctl = "editPartyMemberStats" },
	auras = { ctl = "editAuras", simple = "simpleAuras" },
	warcries = { ctl = "editWarcries", simple = "simpleWarcries" },
	links = { ctl = "editLinks", simple = "simpleLinks" },
	enemyConditions = { ctl = "enemyCond", simple = "simpleEnemyCond" },
	enemyMods = { ctl = "enemyMods", simple = "simpleEnemyMods" },
	curses = { ctl = "editCurses", simple = "simpleCurses" },
}

local function partyWipeActor(pt)
	wipeTable(pt.actor)
	wipeTable(pt.enemyModList)
	pt.actor = { Aura = {}, Curse = {}, Warcry = {}, Link = {}, modDB = new("ModDB"):ModDB(), output = {} }
	pt.actor.modDB.actor = pt.actor
	pt.enemyModList = new("ModList"):ModList()
end

-- PoB's "Rebuild All" button: reparse every buffer into the party actor.
local function partyRebuild(pt)
	partyWipeActor(pt)
	pt:ParseBuffs(pt.actor["modDB"], pt.controls.editPartyMemberStats.buf, "PartyMemberStats", pt.actor["output"])
	pt:ParseBuffs(pt.actor["Aura"], pt.controls.editAuras.buf, "Aura", pt.controls.simpleAuras)
	pt:ParseBuffs(pt.actor["Curse"], pt.controls.editCurses.buf, "Curse", pt.controls.simpleCurses)
	pt:ParseBuffs(pt.actor["Warcry"], pt.controls.editWarcries.buf, "Warcry", pt.controls.simpleWarcries)
	pt:ParseBuffs(pt.actor["Link"], pt.controls.editLinks.buf, "Link", pt.controls.simpleLinks)
	pt:ParseBuffs(pt.enemyModList, pt.controls.enemyCond.buf, "EnemyConditions")
	pt:ParseBuffs(pt.enemyModList, pt.controls.enemyMods.buf, "EnemyMods", pt.controls.simpleEnemyMods)
	build.buildFlag = true
end

-- The spectre and beast libraries: which monsters this build owns. PoB keeps
-- them on the build, and they only do anything once a Raise Spectre or
-- Companion gem in the build is set to one of them.
local function libraryField(kind)
	return kind == "beast" and "beastList" or "spectreList"
end

local function minionEntry(id)
	local m = build.data.minions[id]
	if not m then return nil end
	local flags = m.extraFlags or {}
	return {
		id = id,
		name = m.name or id,
		category = opt(m.monsterCategory),
		recommended = (flags.recommendedSpectre or flags.recommendedBeast) and true or false,
	}
end

M.minion_library = function(p)
	ensureBuild()
	local kind = (p and p.kind == "beast") and "beast" or "spectre"
	local field = libraryField(kind)
	local owned = array({})
	for _, id in ipairs(build[field] or {}) do
		local e = minionEntry(id)
		if e then owned[#owned + 1] = e end
	end
	local available = array({})
	local categories, seen = array({}), {}
	for id in pairs(build.data.spectres or {}) do
		local e = minionEntry(id)
		if e then
			-- A beast library only offers what PoB counts as a beast.
			if kind ~= "beast" or e.category == "Beast" then
				available[#available + 1] = e
				if e.category ~= null and not seen[e.category] then
					seen[e.category] = true
					categories[#categories + 1] = e.category
				end
			end
		end
	end
	table.sort(available, function(a, b)
		if a.name == b.name then return a.id < b.id end
		return a.name < b.name
	end)
	table.sort(categories)
	return {
		kind = kind,
		owned = owned,
		available = available,
		categories = categories,
		-- PoE1 has no separate beast library.
		hasBeasts = build.beastList ~= nil,
	}
end

M.set_minion_library = function(p)
	ensureBuild()
	if not p or type(p.ids) ~= "table" then error("params.ids is required", 0) end
	local kind = (p.kind == "beast") and "beast" or "spectre"
	local field = libraryField(kind)
	if kind == "beast" and build.beastList == nil then error("this game has no beast library", 0) end
	local next_ = {}
	local seen = {}
	for _, id in ipairs(p.ids) do
		id = tostring(id)
		if build.data.minions[id] and not seen[id] then
			seen[id] = true
			next_[#next_ + 1] = id
		end
	end
	build[field] = next_
	build.modFlag = true
	refresh()
	return M.minion_library({ kind = kind })
end

-- ---------------------------------------------------------------------------
-- Tattoos (PoE1): TreeTab:ModifyNodePopup. A tattoo replaces one tree node's
-- modifier in place; the spec keeps it in hashOverrides so it survives a save.
-- ---------------------------------------------------------------------------

local TATTOO_LIMIT = 50

local function tattooTree()
	local tree = build.spec and build.spec.tree
	return tree and tree.tattoo and tree.tattoo.nodes or nil
end

local function tattooCount()
	local n = 0
	for _, node in pairs(build.spec.hashOverrides or {}) do
		if node and node.isTattoo then n = n + 1 end
	end
	return n
end

M.node_tattoos = function(p)
	ensureBuild()
	local pool = tattooTree()
	local nodeId = tonumber(p and p.node)
	if not pool or not nodeId then
		return { available = false, options = array({}), applied = null, count = tattooCount(), limit = TATTOO_LIMIT }
	end
	local node = build.spec.nodes[nodeId]
	local treeNode = build.spec.tree.nodes[nodeId]
	if not node or not treeNode then error("unknown node id", 0) end
	local showLegacy = p.legacy == true
	local nodeName = treeNode.dn or ""
	local nodeValue = (treeNode.sd and treeNode.sd[1]) or ""
	local linked = node.linkedId and #node.linkedId or 0
	local options = array({})
	for id, t in pairs(pool) do
		local target = t.targetType or ""
		local matches = nodeName:match((target:gsub("^Small ", "")))
			or (t.targetValue ~= "" and t.targetValue ~= nil and nodeValue:match(t.targetValue))
			or (target == "Small Attribute" and (nodeName == "Intelligence" or nodeName == "Strength" or nodeName == "Dexterity"))
			or (target == "Keystone" and treeNode.type == target)
		local legacyOk = (t.legacy == nil or t.legacy == false) or t.legacy == showLegacy
		if matches and (t.MinimumConnected or 0) <= linked and legacyOk then
			options[#options + 1] = { id = id, name = t.dn or id, stats = strArray(t.sd or {}), legacy = t.legacy == true }
		end
	end
	table.sort(options, function(a, b) return a.name < b.name end)
	local current = build.spec.hashOverrides and build.spec.hashOverrides[nodeId]
	return {
		available = #options > 0,
		options = options,
		applied = (current and current.isTattoo) and { id = opt(current.id2 or current.skill), name = opt(current.dn), stats = strArray(current.sd or {}) } or null,
		nodeName = nodeName,
		count = tattooCount(),
		limit = TATTOO_LIMIT,
	}
end

M.set_node_tattoo = function(p)
	ensureBuild()
	local pool = tattooTree()
	if not pool then error("this game has no tattoos", 0) end
	local nodeId = tonumber(p and p.node)
	local node = nodeId and build.spec.nodes[nodeId]
	if not node then error("unknown node id", 0) end
	if p.remove then
		build.spec.tree.nodes[nodeId].isTattoo = false
		build.spec.hashOverrides[nodeId] = nil
		build.spec:ReplaceNode(node, build.spec.tree.nodes[nodeId])
		node.allMasteryOptions = false
	else
		local t = p.tattoo and pool[p.tattoo]
		if not t then error("unknown tattoo id", 0) end
		if tattooCount() >= TATTOO_LIMIT and not (build.spec.hashOverrides[nodeId] and build.spec.hashOverrides[nodeId].isTattoo) then
			error("a character may carry " .. TATTOO_LIMIT .. " tattoos", 0)
		end
		t.id = nodeId
		build.spec.hashOverrides[nodeId] = t
		build.spec:ReplaceNode(node, t)
		if node.type == "Mastery" then node.allMasteryOptions = false end
	end
	build.spec:BuildAllDependsAndPaths()
	build.spec:AddUndoState()
	build.modFlag = true
	refresh()
	return M.node_tattoos({ node = nodeId, legacy = p.legacy })
end

-- ---------------------------------------------------------------------------
-- Timeless jewel search (PoE1): TreeTab:FindTimelessJewel. Every seed of a
-- legion jewel rewrites the passives around one socket, and data.readLUT says
-- into what. The search walks the jewel's seed range, weighs each seed by the
-- nodes the caller asked for and ranks them. It runs in steps because a range
-- is thousands of seeds wide.
--
-- The abyss jewels (types 7 and up) are left out: they transform a node from
-- components stored on the jewel rather than from a seed, so a seed search
-- does not apply to them.
-- ---------------------------------------------------------------------------

local TIMELESS_TYPES = {
	{ id = 1, label = "Glorious Vanity", name = "vaal" },
	{ id = 2, label = "Lethal Pride", name = "karui" },
	{ id = 3, label = "Brutal Restraint", name = "maraketh" },
	{ id = 4, label = "Militant Faith", name = "templar" },
	{ id = 5, label = "Elegant Hubris", name = "eternal" },
	{ id = 6, label = "Heroic Tragedy", name = "kalguur" },
}

local TIMELESS_CONQUERORS = {
	[1] = { "Any", "Doryani (Corrupted Soul)", "Xibaqua (Divine Flesh)", "Ahuana (Immortal Ambition)" },
	[2] = { "Any", "Kaom (Strength of Blood)", "Rakiata (Tempered by War)", "Akoya (Chainbreaker)" },
	[3] = { "Any", "Asenath (Dance with Death)", "Nasima (Second Sight)", "Balbala (The Traitor)" },
	[4] = { "Any", "Avarius (Power of Purpose)", "Dominus (Inner Conviction)", "Maxarius (Transcendence)" },
	[5] = { "Any", "Cadiro (Supreme Decadence)", "Victario (Supreme Grandstanding)", "Caspiro (Supreme Ostentation)" },
	[6] = { "Any", "Vorana (Black Scythe Training)", "Uhtred (Celestial Mathematics)", "Medved (The Unbreaking Circle)" },
}

local TIMELESS_DEVOTION = {
	"Any", "Totem Damage", "Brand Damage", "Channelling Damage", "Area Damage",
	"Elemental Damage", "Elemental Resistances", "Effect of non-Damaging Ailments",
	"Elemental Ailment Duration", "Duration of Curses", "Minion Attack and Cast Speed",
	"Minions Accuracy Rating", "Mana Regen", "Mana Cost (legacy)", "Non-Curse Aura Effect",
	"Defences from Shield", "Mana Cost Efficiency",
}

-- Devotion variants are listed in the order the trade ids expect, which is not
-- the order the dropdown shows them in.
local TIMELESS_DEVOTION_TRADE = { 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 16, 15 }

local TIMELESS_IGNORED = {
	["Might of the Vaal"] = true, ["Legacy of the Vaal"] = true, ["Strength"] = true,
	["Add Strength"] = true, ["Dex"] = true, ["Add Dexterity"] = true, ["Devotion"] = true,
	["Price of Glory"] = true, ["Ward"] = true,
}

local TIMELESS_TOTALS = { [2] = "Strength", [3] = "Dexterity", [4] = "Devotion" }

-- Legion ids that roll into the "Total <stat>" pseudo-entry.
local TIMELESS_TOTAL_MEMBERS = {
	karui_notable_add_strength = true, karui_attribute_strength = true, karui_small_strength = true,
	maraketh_notable_add_dexterity = true, maraketh_attribute_dex = true, maraketh_small_dex = true,
	templar_notable_devotion = true, templar_devotion_node = true, templar_small_devotion = true,
}

local TIMELESS_ATTRIBUTES = { Strength = true, Dexterity = true, Intelligence = true }

local function timelessTree()
	if IS_POE2 then error("the timeless jewel search is Path of Exile 1 only", 0) end
	local tree = build.spec and build.spec.tree
	if not tree or not tree.legion then error("this tree has no legion data", 0) end
	return tree
end

local function timelessType(id)
	for _, t in ipairs(TIMELESS_TYPES) do
		if t.id == id then return t end
	end
	error("jewel type " .. tostring(id) .. " cannot be searched by seed", 0)
end

--- The jewel sockets on the tree, labelled by their nearest keystone the way
--- PoB labels them.
local function timelessSockets(tree)
	local out = array({})
	for socketId, socketData in pairs(build.spec.nodes) do
		if socketData.isJewelSocket and socketData.name ~= "Charm Socket" then
			local keystone = "Unknown"
			if socketId == 26725 then
				keystone = "Marauder"
			elseif socketId == 54127 then
				keystone = "Duelist"
			elseif socketId == 7960 then
				keystone = "Templar/Witch"
			else
				local best = math.huge
				for _, near in pairs(tree.nodes[socketId] and tree.nodes[socketId].nodesInRadius[3] or {}) do
					if near.isKeystone then
						local d = (near.x - socketData.x) ^ 2 + (near.y - socketData.y) ^ 2
						if d < best then
							keystone = near.name
							best = d
						end
					end
				end
			end
			out[#out + 1] = {
				id = socketId,
				keystone = keystone,
				label = keystone .. ": " .. socketId,
				allocated = build.spec.allocNodes[socketId] ~= nil,
			}
		end
	end
	table.sort(out, function(a, b) return a.label < b.label end)
	return out
end

--- The nodes a seed can produce for one jewel type, which is the list the
--- caller picks its wanted nodes from.
local function timelessNodes(tree, jewelType)
	local kind = timelessType(jewelType)
	local out = array({})
	local total = TIMELESS_TOTALS[jewelType]
	if total then
		out[#out + 1] = {
			id = "total_" .. total:lower(),
			name = "Total " .. total,
			stats = strArray({ "Every addition to " .. total .. " around the socket, counted together" }),
			notable = false,
			total = true,
		}
	end
	for _, node in pairs(tree.legion.nodes) do
		if node.id:match("^" .. kind.name .. "_.+")
			and not node.id:match("^abyss_special_ascendancy_notable_")
			and not TIMELESS_IGNORED[node.dn] and not node.ks then
			out[#out + 1] = {
				id = node.id,
				name = node.dn,
				stats = strArray(node.sd),
				notable = node["not"] and true or false,
				total = false,
			}
		end
	end
	if kind.name ~= "vaal" then
		for _, addition in pairs(tree.legion.additions) do
			if addition.id:match("^" .. kind.name .. "_.+") and not TIMELESS_IGNORED[addition.dn] then
				out[#out + 1] = {
					id = addition.id,
					name = addition.dn,
					stats = strArray(addition.sd),
					notable = false,
					total = false,
				}
			end
		end
	end
	table.sort(out, function(a, b)
		if a.total ~= b.total then return a.total end
		if a.notable ~= b.notable then return a.notable end
		return a.name < b.name
	end)
	return out
end

--- The passives one socket's radius covers, which is what a seed can rewrite.
local function timelessRadius(tree, socketId)
	local out = array({})
	local socket = socketId and tree.nodes[socketId]
	if not socket or not socket.isJewelSocket then return out end
	local roots = {}
	for _, class in pairs(tree.classes) do roots[class.startNodeId] = true end
	for nodeId in pairs(socket.nodesInRadius[3] or {}) do
		local node = tree.nodes[nodeId]
		if node and not roots[nodeId] and not node.isJewelSocket then
			out[#out + 1] = {
				id = nodeId,
				name = node.dn,
				notable = node.isNotable and true or false,
				keystone = node.isKeystone and true or false,
				allocated = build.spec.allocNodes[nodeId] ~= nil,
			}
		end
	end
	table.sort(out, function(a, b)
		if a.keystone ~= b.keystone then return a.keystone end
		if a.notable ~= b.notable then return a.notable end
		return a.name < b.name
	end)
	return out
end

M.timeless_info = function(p)
	ensureBuild()
	local tree = timelessTree()
	local jewelType = tonumber(p and p.jewelType) or 1
	local jewels = array({})
	for _, t in ipairs(TIMELESS_TYPES) do
		local conquerors = array({})
		for i, label in ipairs(TIMELESS_CONQUERORS[t.id]) do
			conquerors[i] = { id = i, label = label }
		end
		jewels[#jewels + 1] = {
			id = t.id,
			label = t.label,
			name = t.name,
			-- Elegant Hubris stores its seed divided by twenty; the number a
			-- player reads off the jewel, and the one trade wants, is the
			-- multiple, so the range is reported already multiplied.
			seedMin = data.timelessJewelSeedMin[t.id] * (t.id == 5 and 20 or 1),
			seedMax = data.timelessJewelSeedMax[t.id] * (t.id == 5 and 20 or 1),
			step = t.id == 5 and 20 or 1,
			conquerors = conquerors,
			total = opt(TIMELESS_TOTALS[t.id]),
		}
	end
	local devotion = array({})
	for i, label in ipairs(TIMELESS_DEVOTION) do
		devotion[i] = { id = i, label = label }
	end
	return {
		available = true,
		jewelType = jewelType,
		jewels = jewels,
		sockets = timelessSockets(tree),
		nodes = timelessNodes(tree, jewelType),
		radius = timelessRadius(tree, tonumber(p and p.socket)),
		devotion = devotion,
	}
end

local timeless = nil

M.timeless_search_start = function(p)
	ensureBuild()
	p = p or {}
	local tree = timelessTree()
	local jewelType = tonumber(p.jewelType) or 1
	local kind = timelessType(jewelType)
	local socketId = tonumber(p.socket)
	if not socketId then error("params.socket is required", 0) end
	local socket = tree.nodes[socketId]
	if not socket or not socket.isJewelSocket then error("node " .. socketId .. " is not a jewel socket", 0) end

	local legionNodes, legionAdditions = tree.legion.nodes, tree.legion.additions
	local totalId = TIMELESS_TOTALS[jewelType] and ("total_" .. TIMELESS_TOTALS[jewelType]:lower()) or nil

	-- The wanted nodes, keyed the way a LUT result resolves: a legion node id,
	-- or "totalStat" for the pooled attribute or devotion entry.
	local desired, minimums, order = {}, {}, 0
	for _, want in ipairs(p.desired or {}) do
		local id = tostring(want.id or "")
		if totalId and id == totalId then id = "totalStat" end
		local name = id
		if id ~= "totalStat" then
			for _, node in pairs(legionNodes) do
				if node.id == id then name = node.dn break end
			end
			if name == id then
				for _, addition in pairs(legionAdditions) do
					if addition.id == id then name = addition.dn break end
				end
			end
		else
			name = "Total " .. TIMELESS_TOTALS[jewelType]
		end
		if not desired[id] then
			order = order + 1
			desired[id] = {
				weight = tonumber(want.weight) or 1,
				weight2 = tonumber(want.weight2) or 0,
				name = name,
				order = order,
			}
			local min = tonumber(want.minWeight)
			if min and min > 0 then minimums[#minimums + 1] = { id = id, weight = min } end
		end
	end
	if not next(desired) then error("params.desired needs at least one node", 0) end

	local protect = {}
	for _, name in ipairs(p.protect or {}) do protect[tostring(name)] = true end

	local roots = {}
	for _, class in pairs(tree.classes) do roots[class.startNodeId] = true end

	local filter = p.socketFilter and true or false
	local reach = tonumber(p.socketFilterDistance) or 0
	local grantedPassives = build.calcsTab.mainEnv and build.calcsTab.mainEnv.grantedPassives or {}

	local targets, smalls, attributeSmalls = {}, 0, 0
	for nodeId in pairs(socket.nodesInRadius[3] or {}) do
		local node = tree.nodes[nodeId]
		local wanted = node and not roots[nodeId] and not node.isJewelSocket and not node.isKeystone
		if wanted and filter then
			local alloc = grantedPassives[nodeId] ~= nil or build.spec.allocNodes[nodeId] ~= nil
			local dist = build.spec.nodes[nodeId] and build.spec.nodes[nodeId].pathDist or 1000
			wanted = alloc or (reach > 0 and dist <= reach)
		end
		if wanted then
			if node.isNotable or jewelType == 1 then
				targets[#targets + 1] = nodeId
			elseif desired.totalStat then
				if TIMELESS_ATTRIBUTES[node.dn] then
					attributeSmalls = attributeSmalls + 1
				else
					smalls = smalls + 1
				end
			end
		end
	end
	if #targets == 0 then error("no passives in that socket's radius match the filter", 0) end
	table.sort(targets)

	local step = jewelType == 5 and 20 or 1
	timeless = {
		tree = tree,
		jewelType = jewelType,
		kind = kind,
		socket = socketId,
		targets = targets,
		desired = desired,
		minimums = minimums,
		protect = protect,
		smalls = smalls,
		attributeSmalls = attributeSmalls,
		step = step,
		seed = data.timelessJewelSeedMin[jewelType] * step,
		seedMax = data.timelessJewelSeedMax[jewelType] * step,
		total = data.timelessJewelSeedMax[jewelType] - data.timelessJewelSeedMin[jewelType] + 1,
		totalMinWeight = tonumber(p.totalMinWeight) or 0,
		checked = 0,
		results = {},
	}
	return { done = false, progress = 0, checked = 0, found = 0, total = timeless.total }
end

--- Score one seed. Returns the per-node weights and the seed total, or nil if
--- the seed is invalid for this search.
local function timelessScore(seed)
	local t = timeless
	local legionNodes, legionAdditions = t.tree.legion.nodes, t.tree.legion.additions
	local desired, protect = t.desired, t.protect
	local hits, weight = {}, 0

	local function credit(id, amount, targetName)
		local hit = hits[id]
		if not hit then
			hit = { weight = 0, targets = {} }
			hits[id] = hit
		end
		hit.weight = hit.weight + amount
		hit.targets[#hit.targets + 1] = targetName
		weight = weight + amount
	end

	for _, targetId in ipairs(t.targets) do
		local lut = data.readLUT(seed, targetId, t.jewelType)
		if next(lut) then
			local targetNode = t.tree.nodes[targetId]
			local node, id
			local protected = t.jewelType == 4 and protect[targetNode.dn]
			if protected then
				-- Militant Faith cannot keep a protected keystone that the seed
				-- replaces, so such a seed is no use.
				if lut[1] >= data.timelessJewelAdditions then return nil end
				if not desired.totalStat then
					desired.totalStat = { weight = 0.1, weight2 = 0, name = "Devotion", order = 99 }
				end
				id = "totalStat"
			end
			if lut[1] >= data.timelessJewelAdditions and not protected then
				node = legionNodes[lut[1] + 1 - data.timelessJewelAdditions]
				id = node and node.id or nil
			elseif not protected then
				node = legionAdditions[lut[1] + 1]
				id = node and node.id or nil
			end
			if desired.totalStat and TIMELESS_TOTAL_MEMBERS[id] then id = "totalStat" end

			if t.jewelType == 1 then
				local size = #lut
				if size == 2 or size == 3 then
					local want = desired[id]
					if want and node then
						local first = node.stats[node.sortedStats[1]]
						local amount = want.weight * (lut[first.index + 1] or 0)
						local second = node.stats[node.sortedStats[2]]
						if second then amount = amount + want.weight2 * (lut[second.index + 1] or 0) end
						credit(id, amount, targetNode.name)
					end
				elseif size == 6 or size == 8 then
					for i = 1, size / 2 do
						local addition = legionAdditions[lut[i] + 1]
						local addId = addition and addition.id or nil
						local want = addId and desired[addId]
						if want then
							credit(addId, want.weight * (lut[i + size / 2] or 0), targetNode.name)
						end
					end
				end
			elseif id and desired[id] then
				credit(id, desired[id].weight, targetNode.name)
			end
		end
	end

	if desired.totalStat then
		local hit = hits.totalStat
		if not hit then
			hit = { weight = 0, targets = {} }
			hits.totalStat = hit
		end
		local base = desired.totalStat.weight
		local extra
		if t.jewelType == 4 then
			extra = base * (5 * t.smalls + 10 * t.attributeSmalls) + hit.weight * 4
		else
			extra = base * (4 * t.smalls + 2 * t.attributeSmalls) + hit.weight * 19
		end
		hit.weight = hit.weight + extra
		weight = weight + extra
	end

	for _, min in ipairs(t.minimums) do
		if (hits[min.id] and hits[min.id].weight or 0) < min.weight then return nil end
	end
	if weight <= 0 or weight < t.totalMinWeight then return nil end
	return hits, weight
end

M.timeless_search_step = function(p)
	ensureBuild()
	if not timeless then error("no search is running; call timeless_search_start first", 0) end
	local t = timeless
	local budget = tonumber(p and p.budgetMs) or 150
	local t0 = GetTime()
	while t.seed <= t.seedMax and GetTime() - t0 < budget do
		local hits, weight = timelessScore(t.seed)
		if hits then t.results[#t.results + 1] = { seed = t.seed, weight = weight, hits = hits } end
		t.seed = t.seed + t.step
		t.checked = t.checked + 1
	end
	return {
		done = t.seed > t.seedMax,
		progress = math.min(t.checked / t.total, 1),
		checked = t.checked,
		found = #t.results,
		total = t.total,
	}
end

M.timeless_search_result = function(p)
	ensureBuild()
	if not timeless then error("no search has been run", 0) end
	local t = timeless
	local limit = math.max(math.min(tonumber(p and p.limit) or 100, 500), 1)
	table.sort(t.results, function(a, b)
		if a.weight ~= b.weight then return a.weight > b.weight end
		return a.seed < b.seed
	end)
	local wanted = array({})
	for id, want in pairs(t.desired) do
		wanted[#wanted + 1] = { id = id, name = want.name, order = want.order }
	end
	table.sort(wanted, function(a, b) return a.order < b.order end)
	local out = array({})
	for i = 1, math.min(#t.results, limit) do
		local r = t.results[i]
		local nodes = array({})
		for _, want in ipairs(wanted) do
			local hit = r.hits[want.id]
			if hit then
				nodes[#nodes + 1] = {
					id = want.id,
					name = want.name,
					weight = hit.weight,
					targets = strArray(hit.targets),
				}
			end
		end
		out[#out + 1] = { seed = r.seed, weight = r.weight, nodes = nodes }
	end
	return {
		results = out,
		found = #t.results,
		checked = t.checked,
		total = t.total,
		jewelType = t.jewelType,
		jewelName = t.kind.label,
		socket = t.socket,
		desired = wanted,
	}
end

--- The pathofexile.com trade search for a set of seeds, built the way
--- TreeTab's "Open Trade URL" builds it.
M.timeless_trade_url = function(p)
	ensureBuild()
	timelessTree()
	p = p or {}
	local jewelType = tonumber(p.jewelType) or (timeless and timeless.jewelType) or 1
	timelessType(jewelType)
	local tradeIds = data.timelessJewelTradeIDs[jewelType]
	if not tradeIds then error("no trade ids for that jewel type", 0) end
	local conqueror = tonumber(p.conqueror) or 1
	local keystones = {}
	if conqueror > 1 then
		keystones[1] = tradeIds.keystone[conqueror - 1]
	else
		keystones = { tradeIds.keystone[1], tradeIds.keystone[2], tradeIds.keystone[3] }
	end
	local filters = {}
	for _, seed in ipairs(p.seeds or {}) do
		local n = tonumber(seed)
		if n then
			for _, id in ipairs(keystones) do
				filters[#filters + 1] = { id = id, value = { min = n, max = n } }
			end
		end
	end
	if #filters == 0 then error("params.seeds needs at least one seed", 0) end
	local search = {
		query = {
			status = { option = tostring(p.status or "online") },
			stats = { { filters = filters, type = "count", value = { min = 1 } } },
		},
		sort = { price = "asc" },
	}
	if tradeIds.devotion then
		local devotion = {}
		for _, variant in ipairs(p.devotion or {}) do
			local idx = tonumber(variant)
			if idx and idx > 1 then
				local tradeIdx = TIMELESS_DEVOTION_TRADE[idx] and TIMELESS_DEVOTION_TRADE[idx] - 1 or nil
				if tradeIdx and tradeIds.devotion[tradeIdx] then
					devotion[#devotion + 1] = { id = tradeIds.devotion[tradeIdx] }
				end
			end
		end
		if #devotion > 0 then
			search.query.stats[#search.query.stats + 1] = { filters = devotion, type = "and" }
		end
	end
	local realm = tostring(p.realm or "pc"):lower()
	local league = tostring(p.league or "Standard")
	local url = "https://www.pathofexile.com/trade/search/"
		.. (realm == "pc" and "" or (realm .. "/"))
		.. league:gsub("[^a-zA-Z0-9]", function(c) return string.format("%%%02X", c:byte()) end)
		.. "/?q=" .. dkjson.encode(search):gsub("[^a-zA-Z0-9]", function(c) return string.format("%%%02X", c:byte()) end)
	return { url = url, seeds = #(p.seeds or {}), jewelType = jewelType }
end

-- ---------------------------------------------------------------------------
-- Compare: a second build held beside the open one. PoB's own CompareEntry is
-- a build without the UI chrome, living in the same Lua state, so both sides'
-- numbers come from the same calculator and nothing has to be reimplemented.
--
-- A CompareEntry rebuild wipes the shared calc cache, so the open build pays
-- for its next recalculation after one is added or its loadout changes.
-- ---------------------------------------------------------------------------

local compares = {}
local compareActive = 0

local function compareEntry()
	local c = compares[compareActive]
	if not c then error("no comparison build is loaded; call compare_add first", 0) end
	return c.entry
end

local configLabels
local function configLabel(var)
	if not configLabels then
		configLabels = {}
		for _, v in ipairs(require("Modules.ConfigOptions")) do
			if v.var then configLabels[v.var] = (v.label or v.var):gsub(":%s*$", "") end
		end
	end
	return configLabels[var] or var
end

local function fmtStat(value, fmt)
	if type(value) ~= "number" then return value == nil and null or tostring(value) end
	if not fmt or fmt == "" then return tostring(value) end
	local ok, s = pcall(string.format, "%" .. fmt, value)
	return ok and s or tostring(value)
end

local function statValue(output, entry)
	if not output then return nil end
	local v = output[entry.stat]
	if entry.childStat then
		if type(v) ~= "table" then return nil end
		v = v[entry.childStat]
	end
	if type(v) ~= "number" then return nil end
	return v
end

--- Both sides of PoB's sidebar stat list, as rows the UI can diff.
local function compareStatRows(mine, theirs, onlyDiff)
	local rows = array({})
	for _, entry in ipairs(build.displayStats or {}) do
		if entry.stat then
			local a, b = statValue(mine, entry), statValue(theirs, entry)
			if a ~= nil or b ~= nil then
				local same = a == b
				if not (onlyDiff and same) then
					local delta = (a ~= nil and b ~= nil) and (b - a) or nil
					local better = null
					if delta and delta ~= 0 then
						better = entry.lowerIsBetter and delta < 0 or (not entry.lowerIsBetter and delta > 0)
					end
					rows[#rows + 1] = {
						stat = entry.stat .. (entry.childStat and ("." .. entry.childStat) or ""),
						label = entry.label or entry.stat,
						mine = a == nil and null or a,
						theirs = b == nil and null or b,
						mineText = a == nil and null or fmtStat(a, entry.fmt),
						theirsText = b == nil and null or fmtStat(b, entry.fmt),
						delta = delta == nil and null or delta,
						deltaText = delta == nil and null or fmtStat(delta, entry.fmt),
						percent = (delta and a and a ~= 0) and (delta / math.abs(a) * 100) or null,
						better = better,
						same = same,
					}
				end
			end
		end
	end
	return rows
end

local function compareMeta(c)
	local e = c.entry
	local spec = e.treeTab and e.treeTab.specList and e.treeTab.specList[e.treeTab.activeSpec]
	return {
		label = c.label,
		className = spec and spec.curClassName or null,
		ascendClassName = (spec and spec.curAscendClassName ~= "None") and spec.curAscendClassName or null,
		level = e.characterLevel,
	}
end

M.compare_list = function()
	ensureBuild()
	local entries = array({})
	for i, c in ipairs(compares) do
		local m = compareMeta(c)
		m.index = i
		m.active = i == compareActive
		entries[#entries + 1] = m
	end
	return { entries = entries, active = compareActive }
end

M.compare_add = function(p)
	ensureBuild()
	p = p or {}
	local xml = p.xml
	if not xml and p.code then xml = decodeCode(p.code) end
	if not xml and p.path then
		local f = io.open(p.path, "r")
		if not f then error("could not read " .. tostring(p.path), 0) end
		xml = f:read("*a")
		f:close()
	end
	if type(xml) ~= "string" or xml == "" then error("params.xml, params.code or params.path is required", 0) end
	local root = IS_POE2 and "PathOfBuilding2" or "PathOfBuilding"
	if not xml:find("<" .. root .. "[%s>]") then
		error("that build is not a " .. (IS_POE2 and "Path of Exile 2" or "Path of Exile 1") .. " build", 0)
	end
	local label = tostring(p.label or "Comparison")
	local ok, entry = pcall(function() return new("CompareEntry"):CompareEntry(xml, label) end)
	if not ok or not entry or not entry.calcsTab then
		error("could not load that build for comparison" .. (ok and "" or (": " .. tostring(entry))), 0)
	end
	compares[#compares + 1] = { entry = entry, label = label, xml = xml }
	compareActive = #compares
	return M.compare_list()
end

M.compare_select = function(p)
	ensureBuild()
	local i = tonumber(p and p.index) or 0
	if not compares[i] then error("no comparison build at " .. tostring(i), 0) end
	compareActive = i
	return M.compare_list()
end

M.compare_remove = function(p)
	ensureBuild()
	local i = tonumber(p and p.index) or compareActive
	if not compares[i] then error("no comparison build at " .. tostring(i), 0) end
	table.remove(compares, i)
	compareActive = math.min(compareActive, #compares)
	return M.compare_list()
end

M.compare_clear = function()
	ensureBuild()
	compares = {}
	compareActive = 0
	return M.compare_list()
end

M.compare_summary = function(p)
	ensureBuild()
	local entry = compareEntry()
	return {
		rows = compareStatRows(build.calcsTab.mainOutput, entry:GetOutput(), p and p.onlyDifferences and true or false),
		mine = { label = build.buildName or "This build", level = build.characterLevel },
		theirs = compareMeta(compares[compareActive]),
	}
end

--- The loadout each side is showing: trees, item sets, skill sets and the
--- main socket group.
local function loadoutSide(b)
	local specs = array({})
	for i, spec in ipairs(b.treeTab.specList or {}) do
		local count = 0
		for _, node in pairs(spec.nodes) do
			if node.alloc then count = count + 1 end
		end
		specs[#specs + 1] = { index = i, title = spec.title or "Default", nodes = count, active = i == b.treeTab.activeSpec }
	end
	local itemSets = array({})
	for _, id in ipairs(b.itemsTab.itemSetOrderList or {}) do
		local set = b.itemsTab.itemSets[id]
		if set then itemSets[#itemSets + 1] = { id = id, title = set.title or "Default", active = id == b.itemsTab.activeItemSetId } end
	end
	local skillSets = array({})
	for _, id in ipairs(b.skillsTab.skillSetOrderList or {}) do
		local set = b.skillsTab.skillSets[id]
		if set then skillSets[#skillSets + 1] = { id = id, title = set.title or "Default", active = id == b.skillsTab.activeSkillSetId } end
	end
	local groups = array({})
	for i, group in ipairs(b.skillsTab.socketGroupList or {}) do
		groups[#groups + 1] = { index = i, label = group.displayLabel or group.label or ("Group " .. i), active = i == b.mainSocketGroup }
	end
	return { specs = specs, itemSets = itemSets, skillSets = skillSets, socketGroups = groups }
end

M.compare_loadouts = function()
	ensureBuild()
	return { mine = loadoutSide(build), theirs = loadoutSide(compareEntry()) }
end

M.compare_set_loadout = function(p)
	ensureBuild()
	local entry = compareEntry()
	p = p or {}
	if p.spec then entry:SetActiveSpec(tonumber(p.spec)) end
	if p.itemSet then entry:SetActiveItemSet(tonumber(p.itemSet)) end
	if p.skillSet then entry:SetActiveSkillSet(tonumber(p.skillSet)) end
	if p.socketGroup then
		entry:SetMainSocketGroup(tonumber(p.socketGroup))
		entry:SyncCalcsSkillSelection()
		entry:Rebuild()
	end
	return M.compare_loadouts()
end

--- One row per equipment slot, with what each side has in it.
M.compare_items = function(p)
	ensureBuild()
	local entry = compareEntry()
	local onlyDiff = p and p.onlyDifferences and true or false
	local rows = array({})
	for _, slot in ipairs(build.itemsTab.orderedSlots) do
		local shown = true
		if type(slot.shown) == "function" then
			local ok, s = pcall(slot.shown)
			shown = ok and s and true or false
		end
		if shown and not slot.inactive then
			local mineItem = slot.selItemId and slot.selItemId ~= 0 and build.itemsTab.items[slot.selItemId] or nil
			local theirSlot = entry.itemsTab.slots[slot.slotName]
			local theirItem = theirSlot and theirSlot.selItemId and theirSlot.selItemId ~= 0
				and entry.itemsTab.items[theirSlot.selItemId] or nil
			local same = (mineItem and mineItem.name or "") == (theirItem and theirItem.name or "")
			if not (onlyDiff and same) then
				rows[#rows + 1] = {
					slot = slot.slotName,
					label = opt(slot.label),
					mine = mineItem and { name = mineItem.name, rarity = opt(mineItem.rarity) } or null,
					theirs = theirItem and { name = theirItem.name, rarity = opt(theirItem.rarity) } or null,
					same = same,
				}
			end
		end
	end
	return { rows = rows }
end

--- The raw item text of one slot on either side, for a tooltip or a copy.
M.compare_item_text = function(p)
	ensureBuild()
	local entry = compareEntry()
	local slotName = tostring(p and p.slot or "")
	local side = (p and p.side) == "mine" and "mine" or "theirs"
	local b = side == "mine" and build or entry
	local slot = b.itemsTab.slots[slotName]
	if not slot then error("unknown slot " .. slotName, 0) end
	local item = slot.selItemId and slot.selItemId ~= 0 and b.itemsTab.items[slot.selItemId] or nil
	if not item then return { text = null, name = null } end
	return { text = item.raw or item:BuildRaw(), name = item.name }
end

--- Put the comparison build's item for one slot into the open build.
M.compare_copy_item = function(p)
	ensureBuild()
	local entry = compareEntry()
	local slotName = tostring(p and p.slot or "")
	local theirSlot = entry.itemsTab.slots[slotName]
	if not theirSlot then error("unknown slot " .. slotName, 0) end
	local theirItem = theirSlot.selItemId and theirSlot.selItemId ~= 0 and entry.itemsTab.items[theirSlot.selItemId] or nil
	if not theirItem then error("the comparison build has nothing in " .. slotName, 0) end
	local raw = theirItem.raw or theirItem:BuildRaw()
	local item = new("Item"):Item(raw)
	if not item.base then error("could not read that item", 0) end
	build.itemsTab:AddItem(item, true)
	if not build.itemsTab:IsItemValidForSlot(item, slotName) then
		error(item.name .. " does not fit " .. slotName, 0)
	end
	build.itemsTab.slots[slotName]:SetSelItemId(item.id)
	build.itemsTab:AddUndoState()
	refresh()
	return { ok = true, slot = slotName, itemName = item.name }
end

-- The trade URL comes from CompareBuySimilar's own buildURL, read as an upvalue of its popup.
local buySimilar, buySimilarUrl, buySimilarListed, buySimilarHelpers

local function buySimilarModule()
	if buySimilar then return buySimilar end
	if IS_POE2 then
		buySimilar = LoadModule("Classes/CompareBuySimilar")
		buySimilarHelpers = LoadModule("Classes/TradeHelpers")
	else
		buySimilar = require("Classes.CompareBuySimilar")
		buySimilarHelpers = require("Classes.TradeHelpers")
	end
	local i = 1
	while true do
		local name, value = debug.getupvalue(buySimilar.openPopup, i)
		if not name then break end
		if name == "buildURL" then buySimilarUrl = value end
		if name == "LISTED_STATUS_LABELS" then buySimilarListed = value end
		i = i + 1
	end
	if not buySimilarUrl then error("this Path of Building version has no Buy Similar search", 0) end
	return buySimilar
end

-- An unequipped item borrows the trade category of the slot its type goes in.
local BUY_SIMILAR_TYPE_SLOT = {
	["Body Armour"] = "Body Armour", Helmet = "Helmet", Gloves = "Gloves", Boots = "Boots",
	Amulet = "Amulet", Ring = "Ring 1", Belt = "Belt", Jewel = "Jewel", Flask = "Flask 1",
}

local function buySimilarTarget(p)
	if p.side == "mine" or p.side == "theirs" then
		local b = p.side == "mine" and build or compareEntry()
		local slotName = tostring(p.slot or "")
		local slot = b.itemsTab.slots[slotName]
		if not slot then error("unknown slot " .. slotName, 0) end
		local item = slot.selItemId and slot.selItemId ~= 0 and b.itemsTab.items[slot.selItemId] or nil
		if not item then error("nothing is equipped in " .. slotName, 0) end
		return item, slotName
	end
	local item = build.itemsTab.items[tonumber(p.itemId) or -1]
	if not item then error("unknown item " .. tostring(p.itemId), 0) end
	for name, slot in pairs(build.itemsTab.slots) do
		if slot.selItemId == item.id then return item, name end
	end
	local itemType = item.type or (item.base and item.base.type) or ""
	return item, BUY_SIMILAR_TYPE_SLOT[itemType] or (itemType:find("Jewel") and "Jewel") or "Weapon 1"
end

local function buySimilarRows(item)
	local isUnique = item.rarity == "UNIQUE" or item.rarity == "RELIC"
	local sources = {
		{ list = item.enchantModLines, type = "enchant" },
		{ list = item.implicitModLines, type = "implicit" },
		{ list = item.explicitModLines, type = "explicit" },
	}
	if not IS_POE2 then
		sources[#sources + 1] = { list = item.scourgeModLines, type = "scourge" }
	end
	local mods = buySimilarModule().addModEntries(item, sources)
	local defences = {}
	if not isUnique and item.armourData and item.base and item.base.armour then
		for _, def in ipairs({
			{ key = "Armour", label = "Armour", tradeKey = "ar" },
			{ key = "Evasion", label = "Evasion", tradeKey = "ev" },
			{ key = "EnergyShield", label = "Energy Shield", tradeKey = "es" },
			{ key = "Ward", label = IS_POE2 and "Runic Ward" or "Ward", tradeKey = "ward" },
		}) do
			local val = item.armourData[def.key]
			if val and val > 0 then
				defences[#defences + 1] = { label = def.label, value = val, tradeKey = def.tradeKey }
			end
		end
	end
	return isUnique, mods, defences
end

--- The rows PoB's Buy Similar popup offers for an item.
--- params: { itemId } for this build's item, or { slot, side = "mine"|"theirs" } on the Compare tab
M.buy_similar_info = function(p)
	ensureBuild()
	local item, slotName = buySimilarTarget(p or {})
	local isUnique, mods, defences = buySimilarRows(item)
	local outMods = array({})
	for i, m in ipairs(mods) do
		local lines = array({})
		for j, l in ipairs(m.formattedLines) do lines[j] = l end
		outMods[i] = {
			lines = lines,
			type = m.type,
			searchable = #m.tradeIds > 0,
			value = opt(m.value),
			ranged = not (m.isOption or m.needsExactValue) and m.value ~= nil,
		}
	end
	local outDefences = array({})
	for i, d in ipairs(defences) do
		outDefences[i] = { label = d.label, value = math.floor(d.value) }
	end
	local listed = array({})
	for i, l in ipairs(buySimilarListed or { "Instant Buyout", "Instant Buyout & In Person", "In Person (Online)", "Any" }) do
		listed[i] = l
	end
	return {
		name = item.name,
		unique = isUnique,
		category = buySimilarHelpers.getTradeCategoryLabel(slotName, item),
		baseName = opt(item.baseName),
		realms = IS_POE2 and array({ "PoE2" }) or array({ "PC", "PS4", "Xbox" }),
		listed = listed,
		defences = outDefences,
		mods = outMods,
	}
end

--- The trade site URL for the choices made against buy_similar_info's rows.
--- params: the item as for buy_similar_info, plus realm, league, listed (1-based),
--- baseType, ilvlMin, ilvlMax, defences = [{checked,min,max}], mods = [{checked,min,max}]
M.buy_similar_url = function(p)
	ensureBuild()
	p = p or {}
	local item, slotName = buySimilarTarget(p)
	local isUnique, mods, defences = buySimilarRows(item)
	local function given(v)
		return v ~= nil and v ~= null and tostring(v) or ""
	end
	local function choice(v)
		return { GetSelValue = function() return v end }
	end
	local controls = {
		realmDrop = choice(given(p.realm) ~= "" and p.realm or (IS_POE2 and "PoE2" or "PC")),
		leagueDrop = choice(given(p.league) ~= "" and p.league or "Standard"),
		listedDrop = { selIndex = tonumber(p.listed) or 1 },
		baseTypeCheck = { state = p.baseType == true },
		ilvlMin = { buf = given(p.ilvlMin) },
		ilvlMax = { buf = given(p.ilvlMax) },
	}
	for i = 1, #defences do
		local d = type(p.defences) == "table" and p.defences[i] or {}
		controls["def" .. i .. "Check"] = { state = d.checked == true }
		controls["def" .. i .. "Min"] = { buf = given(d.min) }
		controls["def" .. i .. "Max"] = { buf = given(d.max) }
	end
	for i = 1, #mods do
		local m = type(p.mods) == "table" and p.mods[i] or {}
		controls["mod" .. i .. "Check"] = { state = m.checked == true }
		controls["mod" .. i .. "Min"] = { buf = given(m.min) }
		controls["mod" .. i .. "Max"] = { buf = given(m.max) }
	end
	return { url = buySimilarUrl(item, slotName, controls, mods, defences, isUnique) }
end

--- Socket groups on both sides, matched by the order they appear in.
M.compare_skills = function(p)
	ensureBuild()
	local entry = compareEntry()
	local onlyDiff = p and p.onlyDifferences and true or false
	local function groupRow(group)
		if not group then return null end
		local gems = array({})
		for _, gem in ipairs(group.gemList or {}) do
			gems[#gems + 1] = {
				name = gem.nameSpec or (gem.gemData and gem.gemData.name) or "?",
				level = gem.level,
				quality = gem.quality,
				enabled = gem.enabled ~= false,
			}
		end
		return { label = group.displayLabel or group.label or "", slot = opt(group.slot), enabled = group.enabled ~= false, gems = gems }
	end
	local function key(group)
		if not group then return "" end
		local parts = {}
		for _, gem in ipairs(group.gemList or {}) do
			parts[#parts + 1] = (gem.nameSpec or "?") .. "/" .. tostring(gem.level) .. "/" .. tostring(gem.quality)
		end
		return table.concat(parts, ",")
	end
	local mine, theirs = build.skillsTab.socketGroupList or {}, entry.skillsTab.socketGroupList or {}
	local rows = array({})
	for i = 1, math.max(#mine, #theirs) do
		local same = key(mine[i]) == key(theirs[i])
		if not (onlyDiff and same) then
			rows[#rows + 1] = { index = i, mine = groupRow(mine[i]), theirs = groupRow(theirs[i]), same = same }
		end
	end
	return { rows = rows }
end

--- Config options the two builds set differently.
M.compare_config = function(p)
	ensureBuild()
	local entry = compareEntry()
	local onlyDiff = p and p.onlyDifferences
	if onlyDiff == nil then onlyDiff = true end
	local mine = build.configTab.configSets[build.configTab.activeConfigSetId].input
	local theirs = entry.configTab.input
	local keys, seen = {}, {}
	for k, v in pairs(mine) do if isScalar(v) and not seen[k] then seen[k] = true keys[#keys + 1] = k end end
	for k, v in pairs(theirs) do if isScalar(v) and not seen[k] then seen[k] = true keys[#keys + 1] = k end end
	table.sort(keys)
	local rows = array({})
	for _, k in ipairs(keys) do
		local a, b = mine[k], theirs[k]
		local same = a == b
		if not (onlyDiff and same) then
			rows[#rows + 1] = {
				var = k,
				label = configLabel(k),
				mine = a == nil and null or a,
				theirs = b == nil and null or b,
				same = same,
			}
		end
	end
	return { rows = rows }
end

--- What the two trees allocate, and the keystones each side has that the
--- other does not.
M.compare_tree = function()
	ensureBuild()
	local entry = compareEntry()
	local mySpec = build.spec
	local theirSpec = entry.treeTab.specList[entry.treeTab.activeSpec]
	if not theirSpec then error("the comparison build has no tree", 0) end
	local function allocated(spec)
		local ids, keystones, count = {}, {}, 0
		for id, node in pairs(spec.nodes) do
			if node.alloc then
				count = count + 1
				ids[id] = true
				if node.type == "Keystone" and node.dn then keystones[node.dn] = true end
			end
		end
		return ids, keystones, count
	end
	local myIds, myKeys, myCount = allocated(mySpec)
	local theirIds, theirKeys, theirCount = allocated(theirSpec)
	local gained, lost = array({}), array({})
	for id in pairs(theirIds) do if not myIds[id] then gained[#gained + 1] = id end end
	for id in pairs(myIds) do if not theirIds[id] then lost[#lost + 1] = id end end
	table.sort(gained)
	table.sort(lost)
	local keyGained, keyLost = strArray({}), strArray({})
	for k in pairs(theirKeys) do if not myKeys[k] then keyGained[#keyGained + 1] = k end end
	for k in pairs(myKeys) do if not theirKeys[k] then keyLost[#keyLost + 1] = k end end
	table.sort(keyGained)
	table.sort(keyLost)
	return {
		mine = { nodes = myCount, title = mySpec.title or "Default", className = mySpec.curClassName },
		theirs = { nodes = theirCount, title = theirSpec.title or "Default", className = theirSpec.curClassName },
		gained = gained,
		lost = lost,
		keystonesGained = keyGained,
		keystonesLost = keyLost,
	}
end

--- Copy the comparison build's tree into the open build as a new spec, which
--- the tree tab's compare overlay can then draw against.
M.compare_copy_tree = function()
	ensureBuild()
	local entry = compareEntry()
	local theirSpec = entry.treeTab.specList[entry.treeTab.activeSpec]
	if not theirSpec then error("the comparison build has no tree", 0) end
	local url = theirSpec:EncodeURL("https://www.pathofexile.com/passive-skill-tree/")
	local spec = new("PassiveSpec"):PassiveSpec(build, theirSpec.treeVersion or build.spec.treeVersion)
	local err = spec:DecodeURL(url)
	if err then error("could not copy that tree: " .. tostring(err), 0) end
	spec.title = (compares[compareActive].label or "Comparison") .. " tree"
	spec:BuildAllDependsAndPaths()
	table.insert(build.treeTab.specList, spec)
	build.treeTab:SetActiveSpec(#build.treeTab.specList)
	build.modFlag = true
	refresh()
	return { ok = true, index = #build.treeTab.specList, title = spec.title }
end

M.get_party = function()
	ensureBuild()
	local pt = build.partyTab
	local out = {}
	for key, def in pairs(partyKinds) do
		out[key] = {
			text = pt.controls[def.ctl].buf or "",
			summary = def.simple and (pt.controls[def.simple].label or "") or "",
		}
	end
	out.enableExportBuffs = pt.enableExportBuffs and true or false
	return out
end

M.set_party_text = function(p)
	ensureBuild()
	local def = partyKinds[p and p.kind or ""]
	if not def then error("unknown party kind " .. tostring(p and p.kind), 0) end
	build.partyTab.controls[def.ctl]:SetText(p.text or "")
	partyRebuild(build.partyTab)
	refresh()
	return M.get_party()
end

-- Import a support build's exported buffs (its share code) into the party tab.
-- Mirrors finishImport with destination "All".
M.party_import = function(p)
	ensureBuild()
	local xmlText = p and p.xml
	if not xmlText then
		local code = p and p.code
		if not code then error("params.code or params.xml is required", 0) end
		xmlText = Inflate(common.base64.decode(code:gsub("-", "+"):gsub("_", "/")))
		if not xmlText then error("could not decode the build code", 0) end
	end
	local dbXML, errMsg = common.xml.ParseXML(xmlText)
	if not dbXML then error("could not parse build XML: " .. tostring(errMsg), 0) end
	local rootElem = IS_POE2 and "PathOfBuilding2" or "PathOfBuilding"
	if dbXML[1].elem ~= rootElem then error("'" .. rootElem .. "' root element missing", 0) end
	local pt = build.partyTab
	local append = p and p.append and true or false
	local only = p and p.only
	if only == "" or only == "all" then only = nil end
	if not append then
		for _, def in pairs(partyKinds) do
			pt.controls[def.ctl]:SetText("")
		end
	end
	local names = {
		["PartyMemberStats"] = "editPartyMemberStats",
		["Aura"] = "editAuras",
		["Curse"] = "editCurses",
		["Warcry Skills"] = "editWarcries",
		["Link Skills"] = "editLinks",
		["EnemyConditions"] = "enemyCond",
		["EnemyMods"] = "enemyMods",
	}
	local found = false
	for _, tabNode in ipairs(dbXML[1]) do
		if type(tabNode) == "table" and tabNode.elem == "Party" then
			for _, node in ipairs(tabNode) do
				if node.elem == "ExportedBuffs" and node.attrib.name and names[node.attrib.name] and (not only or names[node.attrib.name] == only) then
					found = true
					local ctl = pt.controls[names[node.attrib.name]]
					local text = node[1] or ""
					if append and #ctl.buf > 0 then
						text = ctl.buf .. "\n" .. text
					end
					ctl:SetText(text)
				end
			end
			break
		end
	end
	if not found then
		error("that build has no exported support buffs (export it with \"Export support\" enabled)", 0)
	end
	partyRebuild(pt)
	refresh()
	return M.get_party()
end

M.party_clear = function()
	ensureBuild()
	local pt = build.partyTab
	for _, def in pairs(partyKinds) do
		pt.controls[def.ctl]:SetText("")
	end
	for _, def in pairs(partyKinds) do
		if def.simple then pt.controls[def.simple].label = "" end
	end
	partyWipeActor(pt)
	build.buildFlag = true
	refresh()
	return M.get_party()
end

-- Turns the party's effects off without losing the pasted data; party_rebuild
-- puts them back. PoB calls this "Disable Party Effects".
M.party_disable = function()
	ensureBuild()
	partyWipeActor(build.partyTab)
	build.buildFlag = true
	refresh()
	return M.get_party()
end

M.party_rebuild = function()
	ensureBuild()
	partyRebuild(build.partyTab)
	refresh()
	return M.get_party()
end

M.party_set_export = function(p)
	ensureBuild()
	build.partyTab.enableExportBuffs = p and p.enabled and true or false
	build.buildFlag = true
	refresh()
	return M.get_party()
end

-- ---------------------------------------------------------------------------
-- App options that live on PoB's `main` (never saved via SaveSettings; the
-- frontend persists them and re-applies on boot).
-- ---------------------------------------------------------------------------

local appOptionKeys = {
	showThousandsSeparators = "boolean",
	thousandsSeparator = "string",
	decimalSeparator = "string",
	defaultGemQuality = "number",
	defaultCharLevel = "number",
	defaultItemAffixQuality = "number",
}

M.get_app_options = function()
	local out = {}
	for key in pairs(appOptionKeys) do
		local v = main[key]
		out[key] = v == nil and null or v
	end
	return { options = out }
end

M.set_app_options = function(p)
	if not p then error("params are required", 0) end
	for key, wanted in pairs(appOptionKeys) do
		local v = p[key]
		if v ~= nil and v ~= null then
			if type(v) ~= wanted then error(key .. " must be a " .. wanted, 0) end
			main[key] = v
		end
	end
	if build then refresh() end
	return M.get_app_options()
end

-- ---------------------------------------------------------------------------
-- Custom modifiers (per config set)
-- ---------------------------------------------------------------------------

local function customModsList()
	local configTab = build.configTab
	local set = configTab.configSets[configTab.activeConfigSetId]
	if not set.customModsList then set.customModsList = {} end
	if #set.customModsList == 0 then
		table.insert(set.customModsList, { title = "Default", enabled = true, text = (set.input and set.input.customMods) or "" })
	end
	return set.customModsList
end

local function customModsChanged()
	local configTab = build.configTab
	pcall(function() configTab:UpdateCustomModsControls() end)
	configTab:AddUndoState()
	configTab:BuildModList()
	refresh()
end

M.get_custom_mods = function()
	ensureBuild()
	local blocks = array({})
	for i, block in ipairs(customModsList()) do
		local lines = array({})
		for line in ((block.text or "") .. "\n"):gmatch("(.-)\n") do
			local trimmed = line:match("^%s*(.-)%s*$")
			local status = "empty"
			if #trimmed > 0 then
				local modList, extra = modLib.parseMod(trimmed)
				status = modList and (extra and "partial" or "ok") or "none"
			end
			lines[#lines + 1] = { text = line, status = status }
		end
		-- drop the trailing empty line the split produces
		if #lines > 0 and lines[#lines].text == "" then lines[#lines] = nil end
		blocks[#blocks + 1] = {
			index = i,
			title = opt(block.title),
			enabled = block.enabled and true or false,
			text = block.text or "",
			lines = lines,
		}
	end
	return { blocks = blocks }
end

M.set_custom_mod_block = function(p)
	ensureBuild()
	local blocks = customModsList()
	local block = blocks[tonumber(p and p.index) or -1]
	if not block then error("unknown custom mod block index", 0) end
	if p.title ~= nil and p.title ~= null then block.title = p.title end
	if p.enabled ~= nil and p.enabled ~= null then block.enabled = p.enabled and true or false end
	if p.text ~= nil and p.text ~= null then block.text = p.text end
	customModsChanged()
	return M.get_custom_mods()
end

M.add_custom_mod_block = function(p)
	ensureBuild()
	local blocks = customModsList()
	table.insert(blocks, { title = (p and p.title) or ("Group " .. (#blocks + 1)), enabled = true, text = "" })
	customModsChanged()
	return M.get_custom_mods()
end

M.delete_custom_mod_block = function(p)
	ensureBuild()
	local blocks = customModsList()
	local index = tonumber(p and p.index)
	if not index or not blocks[index] then error("unknown custom mod block index", 0) end
	table.remove(blocks, index)
	customModsChanged()
	return M.get_custom_mods()
end

-- Supported example mod lines for the custom-mod browser: the same collection
-- PoB's Mod Browser popup builds (Modules/ConfigModBrowser.lua) — tree node
-- stats plus item/crafted/veiled mod pools — filtered to parseable lines.
local modBrowserCache
M.custom_mod_browser = function()
	ensureBuild()
	local treeVersion = build.spec and build.spec.treeVersion or "?"
	if modBrowserCache and modBrowserCache.version == treeVersion then
		return modBrowserCache.result
	end
	local modSet = {}
	local function templateOf(text)
		return text
			:gsub("([%+-]?)%((%-?%d+%.?%d*)%-(%-?%d+%.?%d*)%)", "%1#")
			:gsub("%d+%.?%d*", "#")
			:lower()
	end
	local function addModLine(line, source, supported)
		local template = templateOf(line)
		local entry = modSet[template]
		if not entry then
			entry = { text = line, sources = {}, template = template }
			modSet[template] = entry
		end
		entry.sources[source] = true
		if supported ~= nil then
			if supported and not entry.supported then entry.text = line end
			entry.supported = entry.supported or supported
		end
	end
	local seenGroups = {}
	local function addItemMod(mod, source)
		if seenGroups[mod.group] then return end
		seenGroups[mod.group] = true
		if mod.tradeHashes then
			for _, statDescription in pairs(mod.tradeHashes) do
				local line = table.concat(statDescription, " ")
				local rangedLine = itemLib.applyRange(line, 0.5)
				local modList, extra = modLib.parseMod(rangedLine)
				addModLine(rangedLine, source, (not not modList) and not extra)
			end
		else
			for _, line in ipairs(mod) do
				local rangedLine = itemLib.applyRange(line, 0.5)
				local modList, extra = modLib.parseMod(rangedLine)
				addModLine(rangedLine, source, (not not modList) and not extra)
			end
		end
	end
	local tree = build.spec.tree
	for _, node in pairs(tree.nodes) do
		if node.type == "Mastery" and node.masteryEffects then
			for _, masteryEffect in ipairs(node.masteryEffects) do
				for _, statLine in ipairs(masteryEffect.stats) do
					local modList, extra = modLib.parseMod(statLine)
					addModLine(statLine, string.format("%s Node", node.type), (not not modList) and not extra)
				end
			end
		elseif node.stats and node.mods then
			local i = 1
			while node.stats[i] do
				local combinedLine = node.stats[i]
				while node.mods[i + 1] do
					if node.mods[i + 1].combined then
						combinedLine = combinedLine .. " " .. node.stats[i + 1]
						i = i + 1
					else
						break
					end
				end
				local supported = not not node.mods[i].list and not node.mods[i].extra
				local ascendancyPrefix = node.isBloodline and "Bloodline " or node.ascendancyName and "Ascendancy " or ""
				addModLine(combinedLine, string.format("%s%s Node", ascendancyPrefix, node.type), supported)
				i = i + 1
			end
		end
	end
	local bData = build.data
	if bData.masterMods then
		for _, mod in ipairs(bData.masterMods) do addItemMod(mod, "Crafted mod") end
	end
	local ignoredCats = { Item = true, Runes = true }
	if bData.itemMods then
		for catName, catMods in pairs(bData.itemMods) do
			if not ignoredCats[catName] and type(catMods) == "table" then
				for _, mod in pairs(catMods) do addItemMod(mod, "Item mod") end
			end
		end
	end
	if bData.veiledMods then
		for _, mod in pairs(bData.veiledMods) do addItemMod(mod, "Veiled mod") end
	end
	if bData.beastCraft then
		for _, mod in pairs(bData.beastCraft) do addItemMod(mod, "Item mod") end
	end
	local supportedList = {}
	for template, mod in pairs(modSet) do
		if mod.supported then
			mod.sortKey = template:gsub("#", " "):gsub("[^%a]+", " "):match("^%s*(.-)%s*$")
			table.insert(supportedList, mod)
		end
	end
	table.sort(supportedList, function(a, b)
		if a.sortKey ~= b.sortKey then return a.sortKey < b.sortKey end
		if a.template ~= b.template then return a.template < b.template end
		return a.text < b.text
	end)
	local mods = array({})
	for _, mod in ipairs(supportedList) do
		local sources = array({})
		for source in pairs(mod.sources) do sources[#sources + 1] = source end
		table.sort(sources)
		mods[#mods + 1] = { text = mod.text, sources = sources }
	end
	local result = { mods = mods }
	modBrowserCache = { version = treeVersion, result = result }
	return result
end

-- ---------------------------------------------------------------------------
-- Notes
-- ---------------------------------------------------------------------------

M.get_notes = function()
	ensureBuild()
	local ctl = build.notesTab and build.notesTab.controls and build.notesTab.controls.edit
	return { text = ctl and ctl.buf or "" }
end

M.set_notes = function(p)
	ensureBuild()
	local ctl = build.notesTab and build.notesTab.controls and build.notesTab.controls.edit
	if not ctl then error("notes tab unavailable", 0) end
	ctl:SetText(p and p.text or "")
	build.notesTab.modFlag = true
	build.modFlag = true
	return { ok = true }
end

-- ---------------------------------------------------------------------------
-- Gear optimiser. For each slot, a greedy search over the affix families that
-- roll on the slot's base, each candidate scored by PoB's own calculation
-- through the slot-replacement override (the same path as the item compare
-- tooltip), so nothing is equipped until the caller applies a proposal. A
-- decided slot is equipped for the rest of the run so later slots see it, and
-- the original gear is restored at the end.
-- ---------------------------------------------------------------------------

local OPT_SLOTS = { "Weapon 1", "Weapon 2", "Helmet", "Body Armour", "Gloves", "Boots", "Belt", "Amulet", "Ring 1", "Ring 2" }
local OPT_PRESETS = {
	balanced = { dps = 1.0, life = 1.0, ehp = 1.0 },
	defence = { dps = 0.5, life = 1.5, ehp = 1.5 },
	damage = { dps = 2.0, life = 0.6, ehp = 0.6 },
}
local OPT_HEADLINE = { "Life", "EnergyShield", "TotalEHP", "Armour", "CombinedDPS", "MinionDPS", "Mana", "FireResist", "ColdResist", "LightningResist", "ChaosResist", "Str", "Dex", "Int", "ReqStr", "ReqDex", "ReqInt", "MovementSpeedMod" }

local gearOpt = nil

local function optMinionDps(o)
	return o.Minion and (o.Minion.CombinedDPS or o.Minion.TotalDPS) or 0
end

local function optHeadline(o)
	local out = {}
	for _, k in ipairs(OPT_HEADLINE) do out[k] = o[k] or 0 end
	out.MinionDPS = optMinionDps(o)
	return out
end

-- Read from PoB's modifiers, so a keystone that an item grants counts too.
local keystone = {}

function keystone.profile()
	local o = build.calcsTab and build.calcsTab.mainOutput or {}
	local env = build.calcsTab and build.calcsTab.mainEnv
	local function sum(name)
		local ok, v = pcall(function() return env.modDB:Sum("BASE", nil, name) end)
		return ok and tonumber(v) or 0
	end
	local esToMana = math.min(sum("EnergyShieldConvertToMana"), 100)
	local manaFirst = math.min(tonumber(o.sharedMindOverMatter) or 0, 100)
	return {
		chaosImmune = o.ChaosInoculation == true,
		noMana = (o.Mana or 0) <= 0,
		esToMana = esToMana,
		manaFirst = manaFirst,
		evasionToArmour = math.min(sum("EvasionConvertToArmour"), 100),
		manaPool = esToMana > 0 or manaFirst > 0,
	}
end

function keystone.names()
	local names, seen = array({}), {}
	local function add(name)
		if type(name) == "string" and not seen[name] then
			seen[name] = true
			names[#names + 1] = name
		end
	end
	for _, node in pairs(build.spec and build.spec.allocNodes or {}) do
		if node.type == "Keystone" then add(node.dn) end
	end
	local env = build.calcsTab and build.calcsTab.mainEnv
	local ok, granted = pcall(function() return env.modDB:List(nil, "Keystone") end)
	if ok and type(granted) == "table" then
		for _, name in ipairs(granted) do add(name) end
	end
	table.sort(names)
	return names
end

function keystone.rules(k)
	local rules = array({})
	if k.chaosImmune then
		rules[#rules + 1] = "Maximum life is 1 (Chaos Inoculation): life lines and life recovery do nothing, and chaos resistance does not matter because the build is immune to chaos damage. Energy shield is the pool to raise."
	end
	if k.esToMana > 0 then
		rules[#rules + 1] = string.format("%d%% of energy shield becomes mana (Eldritch Battery): energy shield lines raise mana.", k.esToMana)
	end
	if k.manaFirst > 0 then
		rules[#rules + 1] = string.format("%d%% of damage is taken from mana before life (Mind Over Matter): mana is a defensive pool, so mana lines count as defence.", k.manaFirst)
	end
	if k.noMana then
		rules[#rules + 1] = "The build has no mana (Blood Magic): skills cost life, so mana lines do nothing."
	end
	if k.evasionToArmour > 0 then
		rules[#rules + 1] = "Evasion becomes armour (Iron Reflexes): evasion lines raise armour."
	end
	return rules
end

-- Log-ratio gains against the starting build, minus the cost of breaking a
-- constraint. Log ratios keep DPS in the hundreds of thousands and life in
-- the thousands on one scale; the penalties are sized so ten missing points
-- of resistance weigh about the same as a 20% loss of DPS.
local function optScore(o, base, w, cfg)
	-- +1 keeps a build that starts at zero (no weapon, no DPS) scoring its
	-- first real number as the large gain it is.
	local function lr(a, b)
		return math.log((math.max(0, a or 0) + 1) / (math.max(0, b or 0) + 1))
	end
	local function dps(x)
		return math.max(x.CombinedDPS or 0, x.MinionDPS or optMinionDps(x))
	end
	local keys = cfg.keys or {}
	local function pool(x)
		return (x.Life or 0) + (x.EnergyShield or 0) + (keys.manaPool and (x.Mana or 0) or 0)
	end
	local s = w.dps * lr(dps(o), dps(base)) + w.life * lr(pool(o), pool(base)) + w.ehp * lr(o.TotalEHP, base.TotalEHP)
	-- Unless damage outweighs defence, no amount of DPS buys more than a 5% EHP loss.
	if w.ehp >= w.dps and (base.TotalEHP or 0) > 0 then
		local kept = (o.TotalEHP or 0) / base.TotalEHP
		if kept < 0.95 then s = s - (0.95 - kept) * 50 end
	end
	for _, r in ipairs({ "FireResist", "ColdResist", "LightningResist" }) do
		local v = o[r] or 0
		if v < cfg.resist then s = s - (cfg.resist - v) * 0.02 end
	end
	local chaos = o.ChaosResist or 0
	if chaos < cfg.chaos and not keys.chaosImmune then s = s - (cfg.chaos - chaos) * 0.01 end
	for _, a in ipairs({ "Str", "Dex", "Int" }) do
		local have, need = o[a] or 0, o["Req" .. a] or 0
		if have < need then s = s - (need - have) * 0.05 end
	end
	local ms = o.MovementSpeedMod or 1
	if ms < cfg.moveSpeed then s = s - (cfg.moveSpeed - ms) * 2 end
	return s
end

local function optProgress(done, total, slot, note)
	if gearOpt then
		local prev = gearOpt.progress or {}
		gearOpt.progress = { done = done or prev.done or 0, total = total or prev.total or 0, slot = opt(slot), note = note or "" }
	end
end

-- Choosing a base for an empty slot, from the build alone: the defence type
-- from the character's attributes, the weapon type from the main skill, a
-- shield when a skill needs one or Giant's Blood allows one, and within a
-- family the best base the character's level can wear.

local JEWELLERY_PREFERENCE = IS_POE2 and {
	Amulet = { "Stellar Amulet", "Solar Amulet", "Lunar Amulet", "Bloodstone Amulet", "Amber Amulet" },
	["Ring 1"] = { "Ruby Ring", "Iron Ring" },
	["Ring 2"] = { "Sapphire Ring", "Topaz Ring", "Iron Ring" },
	Belt = { "Heavy Belt", "Plate Belt", "Wide Belt", "Linen Belt" },
} or {
	Amulet = { "Onyx Amulet", "Citrine Amulet", "Amber Amulet", "Jade Amulet", "Lapis Amulet" },
	["Ring 1"] = { "Diamond Ring", "Ruby Ring", "Iron Ring" },
	["Ring 2"] = { "Sapphire Ring", "Topaz Ring", "Iron Ring" },
	Belt = { "Stygian Vise", "Leather Belt", "Heavy Belt" },
}
-- The weapon family to try when the main skill does not say.
local DEFAULT_WEAPON_TYPES = IS_POE2 and { "Two Hand Mace", "One Hand Mace" } or { "Two Handed Mace", "One Handed Mace" }

local function defenceProfile(o)
	local str, dex, int = o.Str or 0, o.Dex or 0, o.Int or 0
	local ranked = { { "Armour", str }, { "Evasion", dex }, { "Energy Shield", int } }
	table.sort(ranked, function(a, b) return a[2] > b[2] end)
	-- A second attribute close behind the first means the tree straddles two
	-- kinds of defence and a hybrid base fits better than a pure one.
	if ranked[2][2] >= ranked[1][2] * 0.75 and ranked[2][2] > 20 then
		local pair = { ranked[1][1], ranked[2][1] }
		local order = { Armour = 1, Evasion = 2, ["Energy Shield"] = 3 }
		table.sort(pair, function(a, b) return order[a] < order[b] end)
		return pair[1] .. "/" .. pair[2]
	end
	return ranked[1][1]
end

-- Runeforged variants are a mapping-tier version of each base; a campaign
-- character does not find them, so the picker ignores them before 70.
local function campaignBase(e, level)
	return level >= 70 or not e.name:find("^Runeforged ")
end

local function bestArmourBase(listName, level)
	local best, bestVal
	for _, e in ipairs(data.itemBaseLists[listName] or {}) do
		local b = e.base
		local req = (campaignBase(e, level) and b.req and b.req.level) or 999
		local a = b.armour or {}
		local val = (a.Armour or 0) + (a.Evasion or 0) + (a.EnergyShield or 0)
		if req <= level and val > 0 and (not best or val > bestVal) then best, bestVal = e, val end
	end
	return best
end

local function bestWeaponBase(typeName, level)
	local best, bestVal
	for _, e in ipairs(data.itemBaseLists[typeName] or {}) do
		local b = e.base
		local req = (campaignBase(e, level) and b.req and b.req.level) or 999
		local w = b.weapon
		if w and req <= level then
			local val = ((w.PhysicalMin or 0) + (w.PhysicalMax or 0)) / 2 * (w.AttackRateBase or 1)
			if not best or val > bestVal then best, bestVal = e, val end
		end
	end
	return best
end

local function mainSkillWeaponTypes()
	local group = build.skillsTab.socketGroupList[build.mainSocketGroup or 1]
	if not group then return nil end
	for _, gem in ipairs(group.gemList) do
		local gd = gem.gemData
		if gd and gd.grantedEffect and not gd.grantedEffect.support then
			if gd.weaponRequirements and gd.weaponRequirements ~= "" then
				local types = {}
				for t in gd.weaponRequirements:gmatch("[^,]+") do types[#types + 1] = t:gsub("^%s+", ""):gsub("%s+$", "") end
				return types
			end
			-- PoE1 skills carry a weaponTypes set instead.
			if type(gd.grantedEffect.weaponTypes) == "table" then
				local types = {}
				for t, on in pairs(gd.grantedEffect.weaponTypes) do
					if on and data.weaponTypeInfo[t] then types[#types + 1] = t end
				end
				table.sort(types)
				if #types > 0 then return types end
			end
			return nil
		end
	end
	return nil
end

local function buildWantsShield()
	for _, group in ipairs(build.skillsTab.socketGroupList) do
		for _, gem in ipairs(group.gemList) do
			local gd = gem.gemData
			if gd and gd.weaponRequirements and gd.weaponRequirements:lower():find("shield", 1, true) then return true end
			if gd and gd.name and gd.name:lower():find("shield", 1, true) then return true end
		end
	end
	return false
end

local function hasGiantsBlood()
	local env = build.calcsTab and build.calcsTab.mainEnv
	local ok, flag = pcall(function() return env.modDB:Flag(nil, "GiantsBlood") end)
	return ok and flag == true
end

-- Returns entry, itemType, reason; or nil, reason when the slot should stay empty.
local function pickBaseForSlot(slotName, level, o)
	local profile = defenceProfile(o)
	local family = ({ Helmet = "Helmet", ["Body Armour"] = "Body Armour", Gloves = "Gloves", Boots = "Boots" })[slotName]
	if family then
		local listName = family .. ": " .. profile
		local e = bestArmourBase(listName, level) or bestArmourBase(family .. ": Armour", level)
		if not e then return nil, "no " .. listName .. " base at level " .. level end
		return e, listName, string.format("best %s at level %d", listName, level)
	end
	if slotName == "Weapon 1" then
		local allowed = mainSkillWeaponTypes()
		local twoHanded = {}
		local oneHanded = {}
		for _, t in ipairs(allowed or DEFAULT_WEAPON_TYPES) do
			local info = data.weaponTypeInfo[t]
			if info then
				if info.oneHand then oneHanded[#oneHanded + 1] = t else twoHanded[#twoHanded + 1] = t end
			end
		end
		local wantShield = buildWantsShield()
		local order = {}
		-- A shield user without Giant's Blood needs a free hand.
		if wantShield and not hasGiantsBlood() then
			for _, t in ipairs(oneHanded) do order[#order + 1] = t end
			for _, t in ipairs(twoHanded) do order[#order + 1] = t end
		else
			for _, t in ipairs(twoHanded) do order[#order + 1] = t end
			for _, t in ipairs(oneHanded) do order[#order + 1] = t end
		end
		for _, t in ipairs(order) do
			local e = bestWeaponBase(t, level)
			if e then return e, t, string.format("%s for %s at level %d", t, allowed and "the main skill" or "the class", level) end
		end
		return nil, "no weapon base for the main skill at level " .. level
	end
	if slotName == "Weapon 2" then
		local w1 = build.itemsTab.slots["Weapon 1"]
		local w1Item = w1 and w1.selItemId and w1.selItemId ~= 0 and build.itemsTab.items[w1.selItemId] or nil
		local w1TwoHanded = w1Item and w1Item.base and w1Item.base.type and not (data.weaponTypeInfo[w1Item.base.type] or {}).oneHand
		if w1TwoHanded and not hasGiantsBlood() then return nil, "two-handed weapon in Weapon 1" end
		if not (buildWantsShield() or hasGiantsBlood() or (w1Item and not w1TwoHanded)) then return nil, "no skill needs a shield" end
		local listName = "Shield: " .. profile
		local e = bestArmourBase(listName, level) or bestArmourBase("Shield: Armour", level)
		if not e then return nil, "no shield base at level " .. level end
		return e, listName, string.format("shield, %s at level %d", hasGiantsBlood() and "Giant's Blood" or "one free hand", level)
	end
	local prefs = JEWELLERY_PREFERENCE[slotName]
	if prefs then
		local family = slotName:match("^Ring") and "Ring" or slotName
		for _, name in ipairs(prefs) do
			for _, e in ipairs(data.itemBaseLists[family] or {}) do
				if e.name == name and (e.base.req and e.base.req.level or 0) <= level then
					return e, family, string.format("%s at level %d", name, level)
				end
			end
		end
		return nil, "no " .. family .. " base at level " .. level
	end
	return nil, "slot is not optimised"
end

local function optimiseSlot(slotName, cfg, w, base, itemLevel, range, title)
	local slot = build.itemsTab.slots[slotName]
	if not slot or slot.inactive then return nil, "no such slot" end
	local current = slot.selItemId and slot.selItemId ~= 0 and build.itemsTab.items[slot.selItemId] or nil
	local entry, itemType, baseReason
	local wanted = cfg.bases and cfg.bases[slotName]
	if wanted then
		local wantType = type(wanted) == "table" and wanted.type or (current and current.base.type) or slotName
		local wantName = type(wanted) == "table" and wanted.name or wanted
		entry, itemType = findBase(wantType, wantName)
	elseif current then
		if current.rarity == "UNIQUE" or current.rarity == "RELIC" then return nil, "unique kept" end
		local ok, e, t = pcall(findBase, current.base.type, current.baseName)
		if not ok then return nil, "base not found: " .. tostring(e) end
		entry, itemType = e, t
	else
		local e, t, why = pickBaseForSlot(slotName, build.characterLevel or 1, build.calcsTab.mainOutput or {})
		if not e then return nil, t end
		entry, itemType, baseReason = e, t, why
	end
	local item = makeCraftedItem(entry, "RARE", title, range)
	if not item.affixes then return nil, "base takes no affixes" end
	item.itemLevel = itemLevel
	-- Runes carry over when the base is unchanged: the same sockets, the same
	-- rules for what fits.
	if current and current.runes and item.itemSocketCount and item.itemSocketCount > 0 and current.baseName == entry.name then
		for i = 1, item.itemSocketCount do item.runes[i] = current.runes[i] or "None" end
		item:UpdateRunes()
	end
	local pools = {
		prefixes = affixPool(item, itemLevel, "Prefix", nil),
		suffixes = affixPool(item, itemLevel, "Suffix", nil),
	}
	-- A pure armour build gets nothing from an evasion or energy shield line
	-- in play, whatever the effective-HP number says of it.
	local keys = cfg.keys or {}
	local profile = defenceProfile(base)
	if (keys.evasionToArmour or 0) > 0 and (profile == "Armour" or profile == "Evasion") then profile = "Armour/Evasion" end
	local unwanted = {}
	if not profile:find("/", 1, true) then
		if profile ~= "Evasion" then unwanted[#unwanted + 1] = "evasion" end
		if profile ~= "Energy Shield" then unwanted[#unwanted + 1] = "energyshield" end
		if profile ~= "Armour" then unwanted[#unwanted + 1] = "armour" end
	end
	-- No mana pool (Blood Magic) makes every mana line dead weight.
	if (base.Mana or 0) <= 0 then unwanted[#unwanted + 1] = "mana" end
	-- Chaos Inoculation kills pure life lines; hybrids, minion and MoM lines still count.
	local function lifeOnly(g)
		if not keys.chaosImmune or not g:find("life", 1, true) then return false end
		for _, other in ipairs({ "minion", "allies", "nolife", "beforelife", "energyshield", "armour", "evasion", "mana", "spirit" }) do
			if g:find(other, 1, true) then return false end
		end
		return true
	end
	for _, t in ipairs({ "prefixes", "suffixes" }) do
		local kept = array({})
		for _, fam in ipairs(pools[t]) do
			local g = (fam.group or ""):lower():gsub("physicaldamagereductionrating", "armour")
			local drop = lifeOnly(g)
			for _, u in ipairs(unwanted) do
				if g:find(u, 1, true) and not g:find("applies", 1, true) then drop = true end
			end
			if not drop then kept[#kept + 1] = fam end
		end
		pools[t] = kept
	end
	local limits = {
		prefixes = item.prefixes.limit or (item.affixLimit / 2),
		suffixes = item.suffixes.limit or (item.affixLimit / 2),
	}
	local calcFunc = build.calcsTab:GetMiscCalculator()
	local function evaluate()
		item:Craft()
		item:BuildAndParseRaw()
		return withoutFullDPS(calcFunc, { repSlotName = slotName, repItem = item })
	end
	-- Craft() reads every slot up to the limit, so empty ones hold "None".
	for _, t in ipairs({ "prefixes", "suffixes" }) do
		for i = 1, limits[t] do item[t][i] = { modId = "None" } end
	end
	local used, chosen = {}, { prefixes = {}, suffixes = {} }
	-- A chosen mod's tags zero the spawn weight of what the game forbids beside it.
	local tags = {}
	local function take(fam)
		used[fam.group] = true
		for _, tag in ipairs(item.affixes[fam.modId].tags or {}) do tags[tag] = true end
	end
	-- Boots carry movement speed on 57 of 63 published builds; it is the one
	-- line the score cannot see the value of, so it is taken first.
	if slotName == "Boots" then
		for _, fam in ipairs(pools.prefixes) do
			if fam.group == "MovementVelocity" then
				chosen.prefixes[1] = fam
				take(fam)
				item.prefixes[1] = { modId = fam.modId, range = range }
				break
			end
		end
	end
	local bestScore = optScore(evaluate(), base, w, cfg)
	local bestOutput = nil
	local steps = limits.prefixes + limits.suffixes
	local evals = 0
	for step = 1, steps do
		local best, bestType, bestOut = nil, nil, nil
		for _, t in ipairs({ "prefixes", "suffixes" }) do
			local n = #chosen[t]
			if n < limits[t] then
				for _, fam in ipairs(pools[t]) do
					if not used[fam.group] and item:GetModSpawnWeight(item.affixes[fam.modId], tags) > 0 then
						item[t][n + 1] = { modId = fam.modId, range = range }
						local out = evaluate()
						evals = evals + 1
						local sc = optScore(out, base, w, cfg)
						if sc > bestScore + 1e-9 then best, bestType, bestScore, bestOut = fam, t, sc, out end
						item[t][n + 1] = { modId = "None" }
						if evals % 8 == 0 then
							optProgress(nil, nil, slotName, string.format("%s: affix %d of %d, %d candidates scored", slotName, step, steps, evals))
							coroutine.yield()
						end
					end
				end
			end
		end
		if not best then break end
		chosen[bestType][#chosen[bestType] + 1] = best
		used[best.group] = true
		item[bestType][#chosen[bestType]] = { modId = best.modId, range = range }
		bestOutput = bestOut
	end
	if #chosen.prefixes + #chosen.suffixes == 0 then return nil, "no affix improved the build" end
	-- The search starts from an empty base, so it must also beat the current item.
	if current and bestScore <= optScore(withoutFullDPS(calcFunc, {}), base, w, cfg) + 1e-9 then
		return nil, "the current item scores higher"
	end
	item:Craft()
	item:BuildAndParseRaw()
	local lines = array({})
	for _, m in ipairs(item.explicitModLines) do lines[#lines + 1] = m.line end
	local affixes = array({})
	-- What to look for in game: the mod lines without their numbers, since
	-- a player shops by line, and the tier they find is what it is.
	local lookFor = array({})
	local seenLine = {}
	for _, t in ipairs({ "prefixes", "suffixes" }) do
		for _, fam in ipairs(chosen[t]) do
			affixes[#affixes + 1] = { slot = t == "prefixes" and "Prefix" or "Suffix", group = fam.group, modId = fam.modId, text = fam.label }
			for line in fam.label:gmatch("[^/]+") do
				local plain = line
					:gsub("%(?[%d%.]+%-?[%d%.]*%)?%%?%s+to%s+%(?[%d%.]+%-?[%d%.]*%)?%%?", "")
					:gsub("[%+%-]?%(?[%d%.]+%-?[%d%.]*%)?%%?", "")
					:gsub("^%s*to%s+", ""):gsub("%s+", " "):gsub("^%s+", ""):gsub("%s+$", "")
				if plain ~= "" and not seenLine[plain:lower()] then
					seenLine[plain:lower()] = true
					lookFor[#lookFor + 1] = plain
				end
			end
		end
	end
	return {
		slot = slotName,
		base = entry.name,
		type = itemType,
		title = title,
		replaces = current and current.name or null,
		baseReason = opt(baseReason),
		implicit = opt(entry.base.implicit),
		runes = strArray(item.runes or {}),
		affixes = affixes,
		lookFor = lookFor,
		mods = lines,
		requirements = { level = opt(item.requirements.level), str = opt(item.requirements.str), dex = opt(item.requirements.dex), int = opt(item.requirements.int) },
		raw = item.raw,
		evaluations = evals,
		output = bestOutput and optHeadline(bestOutput) or null,
		item = item,
	}
end

local function optDelta(before, after)
	local d = {}
	for _, k in ipairs(OPT_HEADLINE) do
		local x, y = before[k] or 0, after[k] or 0
		if math.abs(y - x) > 1e-6 then d[k] = math.floor((y - x) * 100 + 0.5) / 100 end
	end
	return d
end

-- Runs inside a coroutine; yields for progress.
local function runGearOpt(p)
	local w = OPT_PRESETS[p.preset or "balanced"] or OPT_PRESETS.balanced
	if type(p.weights) == "table" then
		w = { dps = tonumber(p.weights.dps) or w.dps, life = tonumber(p.weights.life) or w.life, ehp = tonumber(p.weights.ehp) or w.ehp }
	end
	local cfg = {
		resist = tonumber(p.resist) or 75,
		chaos = tonumber(p.chaos) or 0,
		moveSpeed = tonumber(p.moveSpeed) or 1.0,
		bases = type(p.bases) == "table" and p.bases or nil,
		keys = keystone.profile(),
	}
	-- Mods need an item level the character could have found; past 82 nothing
	-- new rolls.
	local itemLevel = tonumber(p.itemLevel) or math.min(82, build.characterLevel or 82)
	local range = tonumber(p.range)
	if range == nil then range = 1 end
	range = math.max(0, math.min(1, range))
	local slots, unknownSlots = {}, {}
	if type(p.slots) == "table" and #p.slots > 0 then
		for _, s in ipairs(p.slots) do
			local ok, name = pcall(resolveSlotName, s)
			if ok and name then slots[#slots + 1] = name else unknownSlots[#unknownSlots + 1] = tostring(s) end
		end
	else
		-- Every slot the optimiser knows, filled or empty; Weapon 2 only when
		-- the build has a use for a shield.
		for _, s in ipairs(OPT_SLOTS) do
			local slot = build.itemsTab.slots[s]
			if slot and not slot.inactive then
				local filled = slot.selItemId and slot.selItemId ~= 0
				if filled or s ~= "Weapon 2" or buildWantsShield() or hasGiantsBlood() then slots[#slots + 1] = s end
			end
		end
	end
	local before = optHeadline(build.calcsTab.mainOutput or {})
	local original = {}
	for _, s in ipairs(slots) do
		local slot = build.itemsTab.slots[s]
		original[s] = slot and slot.selItemId or 0
	end
	local added = {}
	local proposals, skipped = array({}), array({})
	for _, name in ipairs(unknownSlots) do skipped[#skipped + 1] = { slot = name, reason = "not a gear slot" } end
	local ok, err = pcall(function()
		for i, slotName in ipairs(slots) do
			optProgress(i - 1, #slots, slotName, slotName)
			coroutine.yield()
			local stepBefore = optHeadline(build.calcsTab.mainOutput or {})
			local title = (p.titlePrefix or "Optimised") .. " " .. slotName
			local prop, why = optimiseSlot(slotName, cfg, w, before, itemLevel, range, title)
			if prop then
				-- Equip it for the rest of the run so the next slot is scored
				-- against the build as it will be.
				build.itemsTab:AddItem(prop.item, true)
				build.itemsTab.slots[slotName]:SetSelItemId(prop.item.id)
				build.itemsTab:PopulateSlots()
				added[#added + 1] = prop.item
				refresh()
				local after = optHeadline(build.calcsTab.mainOutput or {})
				prop.delta = optDelta(stepBefore, after)
				prop.output = after
				prop.item = nil
				proposals[#proposals + 1] = prop
			else
				skipped[#skipped + 1] = { slot = slotName, reason = why }
			end
		end
	end)
	local after = optHeadline(build.calcsTab.mainOutput or {})
	-- Put the original gear back; the proposals live on as raw text.
	for s, id in pairs(original) do
		local slot = build.itemsTab.slots[s]
		if slot then slot:SetSelItemId(id) end
	end
	for _, item in ipairs(added) do build.itemsTab:DeleteItem(item, true) end
	build.itemsTab:PopulateSlots()
	refresh()
	if not ok then error("gear optimiser failed: " .. tostring(err), 0) end
	optProgress(#slots, #slots, nil, "done")
	local d = optDelta(before, after)
	local parts = {}
	for _, k in ipairs({ "Life", "TotalEHP", "CombinedDPS", "Armour" }) do
		if d[k] then parts[#parts + 1] = string.format("%s %+d", k, math.floor(d[k] + 0.5)) end
	end
	local skippedText = {}
	for _, sk in ipairs(skipped) do skippedText[#skippedText + 1] = sk.slot .. " (" .. sk.reason .. ")" end
	local summary
	if #proposals == 0 then
		summary = "No items proposed. Skipped: " .. table.concat(skippedText, ", ")
	else
		summary = string.format("%d item%s proposed, not equipped yet: %s.", #proposals, #proposals == 1 and "" or "s", #parts > 0 and table.concat(parts, ", ") or "no change")
		if #skippedText > 0 then summary = summary .. " Skipped: " .. table.concat(skippedText, ", ") end
	end
	return {
		summary = summary,
		preset = p.preset or (p.weights and "custom" or "balanced"),
		weights = w,
		itemLevel = itemLevel,
		range = range,
		slots = strArray(slots),
		before = before,
		after = after,
		delta = optDelta(before, after),
		proposals = proposals,
		skipped = skipped,
	}
end

-- Highest gem level the character can have found: the tier ladder maps a
-- character level to the top uncut gem tier, and PoE2 gem levels match tiers.
local function maxGemLevelFor(level)
	local best = 1
	for tier, req in ipairs(GEM_TIER_LEVEL) do
		if req <= level then best = tier end
	end
	return best
end

-- Cap every socketed gem at the level the character could have. Imported
-- planner files carry max-level gems whatever the stage, which inflates
-- attribute requirements and damage at low level.
M.set_gem_levels = function(p)
	ensureBuild()
	p = p or {}
	local level = tonumber(p.level) or build.characterLevel or 1
	local cap = maxGemLevelFor(level)
	local changed = 0
	for _, group in ipairs(build.skillsTab.socketGroupList) do
		for _, gem in ipairs(group.gemList) do
			local gd = gem.gemData
			if gd and gd.grantedEffect and not gd.grantedEffect.support then
				local max = gd.naturalMaxLevel or 20
				local want = math.min(cap, max)
				if gem.level ~= want then
					gem.level = want
					changed = changed + 1
				end
			end
		end
		build.skillsTab:ProcessSocketGroup(group)
	end
	build.skillsTab:AddUndoState()
	refresh()
	return { level = level, gemLevelCap = cap, changed = changed }
end

M.gear_opt_start = function(p)
	ensureBuild()
	p = p or {}
	gearOpt = { progress = { done = 0, total = 0, slot = null, note = "starting" }, result = nil, started = GetTime() }
	gearOpt.co = coroutine.create(function() return runGearOpt(p) end)
	return { done = false, progress = gearOpt.progress }
end

M.gear_opt_step = function(p)
	ensureBuild()
	if not gearOpt or not gearOpt.co then error("no gear optimisation is running", 0) end
	local budget = tonumber(p and p.budgetMs) or 150
	local t0 = GetTime()
	while gearOpt.co and coroutine.status(gearOpt.co) ~= "dead" and GetTime() - t0 < budget do
		local ok, res = coroutine.resume(gearOpt.co)
		if not ok then
			gearOpt.co = nil
			error(tostring(res), 0)
		end
		if coroutine.status(gearOpt.co) == "dead" then
			gearOpt.result = res
			gearOpt.result.ms = GetTime() - gearOpt.started
			gearOpt.co = nil
		end
	end
	return { done = gearOpt.co == nil, progress = gearOpt.progress }
end

M.gear_opt_result = function()
	if not gearOpt or not gearOpt.result then error("no gear optimisation result", 0) end
	return gearOpt.result
end

-- Blocking form for the CLI and the assistant.
M.optimise_gear = function(p)
	local r = M.gear_opt_start(p)
	while not r.done do
		r = M.gear_opt_step({ budgetMs = 1000 })
	end
	return M.gear_opt_result()
end

-- ---------------------------------------------------------------------------
-- Unique jewel suggestions. Every unique jewel PoB knows is scored in each
-- allocated socket through the slot-replacement override (the item compare
-- tooltip's path), so nothing is equipped and the build is untouched. A jewel
-- with variants is searched over the variants that can matter here: a
-- skill-level jewel over the skills the build runs, a notable jewel over
-- notables not yet allocated; a jewel with several picks is filled greedily,
-- best single pick first, partners from the top singles.
--
-- Three steps so the single-variant pass can run across the worker pool:
-- jewel_plan lists the (jewel, socket, variants) jobs, score_jewel_variants
-- runs one job on any engine holding the build, jewel_finish ranks and fills
-- partner picks on the main engine. suggest_unique_jewels runs all three in
-- one engine.
-- ---------------------------------------------------------------------------

local JEWEL_PARTNER_POOL = 24
local jewelRun = nil

local function variantLines(item, idx)
	local out = {}
	for _, ml in ipairs(item.explicitModLines or {}) do
		if ml.variantList and ml.variantList[idx] then out[#out + 1] = ml.line end
	end
	return out
end

local function pickNames(item, picks)
	local names = array({})
	for _, idx in ipairs(picks) do names[#names + 1] = item.variantList[idx] end
	return names
end

local function dbJewelItem(dbItem, range)
	local item = new("Item"):Item(dbItem:BuildRaw())
	-- Every line, not rangeLineList: that holds only the lines active under
	-- the default variant, and the scorer switches variants afterwards.
	if range then
		for _, ml in ipairs(item.explicitModLines or {}) do ml.range = range end
	end
	item:BuildAndParseRaw()
	return item
end

local function activeJewelSockets()
	pcall(build.itemsTab.UpdateSockets, build.itemsTab)
	local out = {}
	for _, slot in ipairs(build.itemsTab.orderedSlots) do
		if slot.nodeId and not slot.inactive then
			local node = build.spec.nodes[slot.nodeId] or (build.spec.tree.nodes and build.spec.tree.nodes[slot.nodeId])
			out[#out + 1] = { slot = slot, node = node, label = slot.label or slot.slotName }
		end
	end
	return out
end

local function socketRow(sk)
	local item = sk.slot.selItemId and sk.slot.selItemId ~= 0 and build.itemsTab.items[sk.slot.selItemId] or nil
	return {
		socket = sk.label,
		slot = sk.slot.slotName,
		nodeId = sk.slot.nodeId,
		item = item and item.name or null,
		itemRarity = item and opt(item.rarity) or null,
		sinister = sk.node and sk.node.sinister == true or false,
	}
end

-- Sockets as callers name them: "Socket #2", "#2", 2, "Jewel 26725" or the node id.
local function resolveJewelSockets(sockets, wanted)
	if type(wanted) ~= "table" or #wanted == 0 then return sockets end
	local out, unknown = {}, {}
	for _, w in ipairs(wanted) do
		local s = tostring(w):lower():gsub("^%s+", ""):gsub("%s+$", "")
		local found
		for i, sk in ipairs(sockets) do
			local id = tostring(sk.slot.nodeId)
			if s == sk.slot.slotName:lower() or s == (sk.label or ""):lower() or s == id or s == tostring(i)
				or s == "#" .. i or s == "socket " .. i or s == "socket #" .. i or s == "jewel " .. id then
				found = sk
				break
			end
		end
		if found then out[#out + 1] = found else unknown[#unknown + 1] = tostring(w) end
	end
	if #unknown > 0 then
		local names = {}
		for _, sk in ipairs(sockets) do names[#names + 1] = sk.label .. " (" .. sk.slot.slotName .. ")" end
		error("unknown socket " .. table.concat(unknown, ", ") .. "; allocated sockets: " .. table.concat(names, ", "), 0)
	end
	return out
end

local function keystoneSet()
	local set = {}
	for _, name in ipairs(data.keystones or {}) do set[name] = true end
	return set
end

-- What each variant name counts as, and how many of a kind one item takes.
-- A pick marked required is filled even when every option costs something
-- (a Flesh Crucible always carries its price).
local JEWEL_PICK_RULES = {
	["Flesh Crucible"] = function()
		local ks = keystoneSet()
		return {
			category = function(name) return ks[name] and "keystone" or "price" end,
			caps = { keystone = 1, price = 1 },
			required = { price = true },
		}
	end,
	["Heart of the Well"] = function()
		return {
			category = function(name) return name:match("^Prefix ") and "prefix" or name:match("^Suffix ") and "suffix" or "other" end,
			caps = { prefix = 2, suffix = 2 },
		}
	end,
}

local function jewelKind(item)
	local jd = item.jewelData or {}
	if item.baseName == "Timeless Jewel" or jd.conqueredBy then return "timeless" end
	-- PoB gives every jewel an empty fromNothingKeystones table.
	if (jd.fromNothingKeystones and next(jd.fromNothingKeystones)) or jd.alternateClassStart or jd.intuitiveLeapLike then return "tree" end
	for _, ml in ipairs(item.explicitModLines or {}) do
		local l = ml.line:lower()
		if l:find("can be allocated without being connected", 1, true) or l:find("sinister jewel socket", 1, true)
			or l:find("can allocate passive skills from", 1, true) then
			return "tree"
		end
	end
	return "stat"
end

local function buildSkillNames()
	local names = {}
	for _, group in ipairs(build.skillsTab.socketGroupList) do
		for _, gem in ipairs(group.gemList) do
			local gd = gem.gemData
			if gd and gd.grantedEffect and not gd.grantedEffect.support then
				names[gd.name:lower()] = true
				if gd.grantedEffect.name then names[gd.grantedEffect.name:lower()] = true end
			end
		end
	end
	return names
end

local function allocatedNodeNames()
	local names = {}
	for _, node in pairs(build.spec.allocNodes or {}) do
		if node.name then names[node.name:lower()] = true end
		if node.dn then names[node.dn:lower()] = true end
	end
	return names
end

-- Stat text of every notable by name, for judging what "Allocates X" gives.
local function notableStatText()
	local text = {}
	for _, node in pairs(build.spec.tree.nodes or {}) do
		if node.name and node.sd then text[node.name:lower()] = table.concat(node.sd, " "):lower() end
	end
	return text
end

-- Variants worth scoring on this build. A skill-level line for a skill the
-- build does not run, or a notable it already has, cannot change a number;
-- an old version of the item is not what drops. With no mana pool (Blood
-- Magic), no energy shield or life fixed at 1 (Chaos Inoculation), every line
-- naming that pool is dead, including "while not on Low Mana", which PoB does
-- not derive from the missing pool.
local function relevantVariants(item, ctx)
	local function namesDeadPool(text, pool)
		if pool == "life" then
			for _, alive in ipairs({ "full life", "minion", "allies", " ally", "before life", "energy shield", "mana" }) do
				if text:find(alive, 1, true) then return false end
			end
		end
		return text:find(pool, 1, true) ~= nil
	end
	local hasCurrent = false
	for _, name in ipairs(item.variantList) do
		if name == "Current" then hasCurrent = true end
	end
	local out = {}
	for idx, name in ipairs(item.variantList) do
		local keep = not (hasCurrent and name:match("^Pre "))
		for _, line in ipairs(variantLines(item, idx)) do
			local skill = line:match("to Level of all (.+) Skills$")
			if skill then keep = keep and ctx.skills[skill:lower()] == true end
			local notable = line:match("^Allocates (.+)$")
			if notable and ctx.allocated[notable:lower()] then keep = false end
			if #ctx.deadPools > 0 then
				local text = line:lower()
				if notable then text = ctx.notables[notable:lower()] or text end
				for _, pool in ipairs(ctx.deadPools) do
					if namesDeadPool(text, pool) then
						keep = false
						ctx.deadSkipped = (ctx.deadSkipped or 0) + 1
						break
					end
				end
			end
		end
		if keep then out[#out + 1] = idx end
	end
	return out
end

local function hasCorruptedMagicJewel(sockets)
	for _, sk in ipairs(sockets) do
		local item = sk.slot.selItemId and sk.slot.selItemId ~= 0 and build.itemsTab.items[sk.slot.selItemId] or nil
		if item and item.rarity == "MAGIC" and item.corrupted then return true end
	end
	return false
end

local function jewelDelta(before, after)
	local d = optDelta(before, after)
	local out = {}
	for k, v in pairs(d) do
		if not k:find("^Req") then out[k] = v end
	end
	return out
end

local function round3(x) return math.floor(x * 1000 + 0.5) / 1000 end

local function jewelHeadline(calcFunc, slotName, item)
	return optHeadline(withoutFullDPS(calcFunc, { repSlotName = slotName, repItem = item }))
end

-- Step 1: what to score. Keeps the run's state for jewel_finish.
M.jewel_plan = function(p)
	ensureBuild()
	ensureItemDb()
	p = p or {}
	local range = tonumber(p.range)
	if range == nil then range = main.defaultItemAffixQuality or 0.5 end
	range = math.max(0, math.min(1, range))
	local run = {
		started = GetTime(),
		preset = p.preset or "balanced",
		w = OPT_PRESETS[p.preset or "balanced"] or OPT_PRESETS.balanced,
		cfg = { resist = tonumber(p.resist) or 75, chaos = tonumber(p.chaos) or 0, moveSpeed = 1.0, keys = keystone.profile() },
		range = range,
		limit = tonumber(p.limit) or 20,
		notScored = array({}),
		errors = array({}),
		jobs = {},
		items = {},
	}
	run.allSockets = activeJewelSockets()
	run.socketRows = array({})
	for _, sk in ipairs(run.allSockets) do run.socketRows[#run.socketRows + 1] = socketRow(sk) end
	run.sockets = {}
	for _, sk in ipairs(resolveJewelSockets(run.allSockets, p.sockets)) do
		-- A sinister socket takes no unique.
		if not (sk.node and sk.node.sinister) then run.sockets[#run.sockets + 1] = sk end
	end
	jewelRun = run
	if #run.sockets == 0 then
		return { jobs = array({}), sockets = run.socketRows }
	end

	local wanted = nil
	if type(p.names) == "table" and #p.names > 0 then
		wanted = {}
		for _, n in ipairs(p.names) do wanted[#wanted + 1] = tostring(n):lower() end
	end
	local candidates = {}
	for name, dbItem in pairs(main.uniqueDB.list) do
		if dbItem.type == "Jewel" then
			local keep = wanted == nil
			if wanted then
				for _, q in ipairs(wanted) do
					if name:lower():find(q, 1, true) then keep = true end
				end
			end
			if keep then candidates[#candidates + 1] = { name = name, dbItem = dbItem } end
		end
	end
	table.sort(candidates, function(a, b) return a.name < b.name end)
	if #candidates == 0 then error("no unique jewel matches " .. table.concat(wanted, ", "), 0) end

	local mainOut = build.calcsTab.mainOutput or {}
	local ctx = { skills = buildSkillNames(), allocated = allocatedNodeNames(), deadPools = {} }
	local keys = run.cfg.keys
	if (mainOut.Mana or 0) <= 0 then ctx.deadPools[#ctx.deadPools + 1] = "mana" end
	if (mainOut.EnergyShield or 0) <= 0 and keys.esToMana == 0 then ctx.deadPools[#ctx.deadPools + 1] = "energy shield" end
	if keys.chaosImmune then ctx.deadPools[#ctx.deadPools + 1] = "life" end
	if #ctx.deadPools > 0 then ctx.notables = notableStatText() end
	run.ctx = ctx
	local jobs = array({})
	for _, cand in ipairs(candidates) do
		local ok, err = pcall(function()
			local item = dbJewelItem(cand.dbItem, range)
			local kind = jewelKind(item)
			local mods = activeModLines(item)
			if kind == "timeless" then
				run.notScored[#run.notScored + 1] = { name = cand.name, reason = "a Timeless Jewel changes the passives in its radius by seed; PoB needs the exact seed and socket, and a seed search is not available here", mods = mods }
				return
			end
			if kind == "tree" then
				run.notScored[#run.notScored + 1] = { name = cand.name, reason = "changes what the tree can allocate rather than a stat; plan the tree around it, PoB's numbers do not capture it", mods = mods }
				return
			end
			if item.jewelData and item.jewelData.corruptedMagicJewelIncEffect and not hasCorruptedMagicJewel(run.allSockets) then
				run.notScored[#run.notScored + 1] = { name = cand.name, reason = "multiplies corrupted magic jewels and none is socketed", mods = mods }
				return
			end
			local variantIdx = item.variantList and relevantVariants(item, ctx) or {}
			if item.variantList and #variantIdx == 0 then
				run.notScored[#run.notScored + 1] = { name = cand.name, reason = "none of its " .. #item.variantList .. " variants applies to this build (its skills or unallocated notables)", mods = mods }
				return
			end
			local radius = item.jewelRadiusIndex ~= nil or (item.jewelData and item.jewelData.radiusIndex ~= nil)
			local ruleMaker = JEWEL_PICK_RULES[item.title or ""]
			run.items[cand.name] = { item = item, radius = radius, rules = ruleMaker and ruleMaker() or nil, variantIdx = variantIdx }
			local slots = radius and run.sockets or { run.sockets[1] }
			for _, sk in ipairs(slots) do
				local job = { name = cand.name, slot = sk.slot.slotName, range = range, idx = null }
				if #variantIdx > 0 then
					job.idx = array({})
					for i, v in ipairs(variantIdx) do job.idx[i] = v end
				end
				jobs[#jobs + 1] = job
				run.jobs[#run.jobs + 1] = job
			end
		end)
		if not ok then run.errors[#run.errors + 1] = { name = cand.name, error = tostring(err) } end
	end
	return { jobs = jobs, sockets = run.socketRows }
end

-- Step 2: score one job's variants, singly. Any engine holding the build.
M.score_jewel_variants = function(chunk)
	ensureBuild()
	ensureItemDb()
	if not chunk or not chunk.name or not chunk.slot then error("params.name and params.slot are required", 0) end
	local dbItem = main.uniqueDB.list[chunk.name]
	if not dbItem then error("unknown unique " .. tostring(chunk.name), 0) end
	local item = dbJewelItem(dbItem, tonumber(chunk.range))
	local calcFunc = build.calcsTab:GetMiscCalculator()
	local results = array({})
	local idx = chunk.idx
	if type(idx) ~= "table" or #idx == 0 then
		results[1] = { idx = null, out = jewelHeadline(calcFunc, chunk.slot, item) }
	else
		for _, i in ipairs(idx) do
			setPicks(item, { tonumber(i) })
			item:BuildAndParseRaw()
			results[#results + 1] = { idx = tonumber(i), out = jewelHeadline(calcFunc, chunk.slot, item) }
		end
	end
	return { name = chunk.name, slot = chunk.slot, results = results }
end

-- Step 3: rank, fill partner picks, try stacks. Main engine only.
M.jewel_finish = function(p)
	ensureBuild()
	local run = jewelRun
	if not run then error("call jewel_plan first", 0) end
	jewelRun = nil
	if #run.sockets == 0 then
		return {
			summary = #run.allSockets == 0 and "No jewel socket is allocated on the tree, so no jewel can be socketed; allocate a socket node first."
				or "Every allocated socket is a Sinister socket, which takes no unique jewel.",
			sockets = run.socketRows, suggestions = array({}), notScored = array({}), errors = array({}), evaluations = 0,
		}
	end
	local calcFunc = build.calcsTab:GetMiscCalculator()
	local base = optHeadline(withoutFullDPS(calcFunc, {}))
	local w, cfg = run.w, run.cfg
	local baseScore = optScore(base, base, w, cfg)
	local function scoreOf(out) return optScore(out, base, w, cfg) - baseScore end
	local evals = 0
	local function score(slotName, item)
		evals = evals + 1
		local out = jewelHeadline(calcFunc, slotName, item)
		return scoreOf(out), out
	end

	-- Singles per job, merged from however many chunks scored them.
	local singlesByJob = {}
	for _, r in ipairs(p and p.results or {}) do
		local key = r.name .. "\n" .. r.slot
		local list = singlesByJob[key] or {}
		singlesByJob[key] = list
		for _, s in ipairs(r.results or {}) do
			evals = evals + 1
			list[#list + 1] = { idx = s.idx ~= null and tonumber(s.idx) or nil, out = s.out, score = scoreOf(s.out) }
		end
	end

	local socketBySlot = {}
	for _, sk in ipairs(run.sockets) do socketBySlot[sk.slot.slotName] = sk end

	local function row(item, sk, picks, sc, out, extra)
		local current = sk and sk.slot.selItemId and sk.slot.selItemId ~= 0 and build.itemsTab.items[sk.slot.selItemId] or nil
		local r = {
			name = item.name,
			item = item.title or item.name,
			base = opt(item.baseName),
			socket = sk and sk.label or "any",
			slot = sk and sk.slot.slotName or null,
			replaces = current and current.name or null,
			variants = picks and pickNames(item, picks) or array({}),
			mods = activeModLines(item),
			delta = jewelDelta(base, out),
			score = round3(sc),
			raw = item.raw,
		}
		for k, v in pairs(extra or {}) do r[k] = v end
		return r
	end

	-- The best row for one jewel in one socket from its scored singles;
	-- further picks are filled greedily from the top singles.
	local function bestInSocket(entry, sk, singles)
		local item, rules = entry.item, entry.rules
		local slotName = sk.slot.slotName
		if #singles == 0 then return nil end
		local picks = variantPickCount(item)
		if picks == 0 then
			return row(item, sk, nil, singles[1].score, singles[1].out)
		end
		table.sort(singles, function(a, b)
			if a.score ~= b.score then return a.score > b.score end
			return a.idx < b.idx
		end)
		local chosen, counts = {}, {}
		local function category(idx) return rules and rules.category(item.variantList[idx]) or "any" end
		local function allowed(idx)
			local c = category(idx)
			local cap = rules and rules.caps and rules.caps[c]
			return not cap or (counts[c] or 0) < cap
		end
		local function take(idx)
			chosen[#chosen + 1] = idx
			local c = category(idx)
			counts[c] = (counts[c] or 0) + 1
		end
		local first
		for _, s in ipairs(singles) do
			if allowed(s.idx) and not (rules and rules.required and rules.required[category(s.idx)]) then
				first = s
				break
			end
		end
		if not first then return nil end
		take(first.idx)
		local bestSc, bestOut = first.score, first.out
		local alternatives = array({})
		for _, s in ipairs(singles) do
			if #alternatives >= 3 then break end
			if s.idx ~= first.idx and category(s.idx) == category(first.idx) then
				alternatives[#alternatives + 1] = { variants = pickNames(item, { s.idx }), score = round3(s.score), delta = jewelDelta(base, s.out) }
			end
		end
		if picks > 1 then
			local pool = {}
			for i = 1, math.min(JEWEL_PARTNER_POOL, #singles) do pool[#pool + 1] = singles[i] end
			-- A required category is filled from every option, not only the top singles.
			if rules and rules.required then
				local seen = {}
				for _, s in ipairs(pool) do seen[s.idx] = true end
				for _, s in ipairs(singles) do
					if not seen[s.idx] and rules.required[category(s.idx)] then pool[#pool + 1] = s end
				end
			end
			for _ = 2, picks do
				local requiredLeft = nil
				if rules and rules.required then
					for c in pairs(rules.required) do
						if (counts[c] or 0) < (rules.caps and rules.caps[c] or 1) then requiredLeft = c end
					end
				end
				local best, bestEntrySc, bestEntryOut = nil, nil, nil
				for _, s in ipairs(pool) do
					local used = false
					for _, idx in ipairs(chosen) do
						if idx == s.idx then used = true end
					end
					if not used and allowed(s.idx) and (not requiredLeft or category(s.idx) == requiredLeft) then
						local trial = {}
						for _, idx in ipairs(chosen) do trial[#trial + 1] = idx end
						trial[#trial + 1] = s.idx
						setPicks(item, trial)
						item:BuildAndParseRaw()
						local sc, out = score(slotName, item)
						local better = sc > bestSc + 1e-9
						if (better or requiredLeft) and (not bestEntrySc or sc > bestEntrySc) then
							best, bestEntrySc, bestEntryOut = s, sc, out
						end
					end
				end
				if not best then break end
				take(best.idx)
				bestSc, bestOut = bestEntrySc, bestEntryOut
			end
		end
		setPicks(item, chosen)
		item:BuildAndParseRaw()
		return row(item, sk, chosen, bestSc, bestOut, { alternatives = alternatives })
	end

	-- k copies of a stackable jewel: real copies in the other sockets, the
	-- last one through the override.
	local function stacked(item, k, fn)
		local added, saved = {}, {}
		for i = 2, k do
			local copy = new("Item"):Item(item.raw)
			build.itemsTab:AddItem(copy, true)
			saved[i] = run.sockets[i].slot.selItemId or 0
			run.sockets[i].slot:SetSelItemId(copy.id)
			added[#added + 1] = copy
		end
		local ok, a = pcall(fn)
		for i = 2, k do run.sockets[i].slot:SetSelItemId(saved[i]) end
		for _, c in ipairs(added) do build.itemsTab:DeleteItem(c, true) end
		if not ok then error(a, 0) end
		return a
	end

	local suggestions = array({})
	local names = {}
	for name in pairs(run.items) do names[#names + 1] = name end
	table.sort(names)
	for _, name in ipairs(names) do
		local entry = run.items[name]
		local ok, err = pcall(function()
			local best = nil
			for _, job in ipairs(run.jobs) do
				if job.name == name then
					local sk = socketBySlot[job.slot]
					local singles = singlesByJob[name .. "\n" .. job.slot]
					if sk and singles then
						local r = bestInSocket(entry, sk, singles)
						if r then
							if not entry.radius then
								r.socket = "any"
								r.slot = null
								r.replaces = null
							end
							if not best or r.score > best.score then best = r end
							if entry.radius then suggestions[#suggestions + 1] = r end
						end
					end
				end
			end
			if best and not entry.radius then suggestions[#suggestions + 1] = best end
			-- Copies of a stackable jewel, one per free socket.
			local item = entry.item
			local limitN = tonumber(item.limit) or 1
			if best and not entry.radius and limitN > 1 and #run.sockets > 1 then
				for k = 2, math.min(limitN, #run.sockets) do
					local r = stacked(item, k, function()
						local sc, out = score(run.sockets[1].slot.slotName, item)
						return row(item, nil, nil, sc, out, { copies = k, socket = k .. " sockets", note = k .. " copies, one per socket" })
					end)
					suggestions[#suggestions + 1] = r
				end
			end
		end)
		if not ok then run.errors[#run.errors + 1] = { name = name, error = tostring(err) } end
	end
	refresh()

	table.sort(suggestions, function(a, b) return a.score > b.score end)
	local kept = array({})
	for i = 1, math.min(run.limit, #suggestions) do kept[#kept + 1] = suggestions[i] end
	local gains = 0
	for _, s in ipairs(suggestions) do
		if s.score > 1e-6 then gains = gains + 1 end
	end
	local parts = {}
	for i = 1, math.min(3, #suggestions) do
		local s = suggestions[i]
		if s.score > 1e-6 then
			local bits = {}
			for _, k in ipairs({ "Life", "TotalEHP", "CombinedDPS" }) do
				if s.delta[k] then bits[#bits + 1] = string.format("%s %+d", k, math.floor(s.delta[k] + 0.5)) end
			end
			local what = s.item .. (#s.variants > 0 and " (" .. table.concat(s.variants, ", ") .. ")" or "") .. (s.copies and " x" .. s.copies or "")
			if s.slot ~= null then what = what .. " in " .. s.socket end
			parts[#parts + 1] = what .. (#bits > 0 and ": " .. table.concat(bits, ", ") or "")
		end
	end
	local summary
	if gains == 0 then
		summary = string.format("No unique jewel improves this build by PoB's numbers (%d options scored, %d not scored).", #suggestions, #run.notScored)
	else
		summary = string.format("%d of %d jewel options improve the build. Best: %s. Nothing is equipped.", gains, #suggestions, table.concat(parts, "; "))
	end
	local notes = array({})
	if run.ctx and #run.ctx.deadPools > 0 then
		notes[#notes + 1] = string.format("Lines about %s do nothing on this build (no such pool, or life fixed at 1 by Chaos Inoculation), so %d variants naming them were skipped, including \"while not on Low ...\" conditions.", table.concat(run.ctx.deadPools, " or "), run.ctx.deadSkipped or 0)
	end
	return {
		summary = summary,
		notes = notes,
		preset = run.preset,
		range = run.range,
		sockets = run.socketRows,
		baseline = { Life = base.Life, TotalEHP = base.TotalEHP, CombinedDPS = base.CombinedDPS, Armour = base.Armour },
		suggestions = kept,
		notScored = run.notScored,
		errors = run.errors,
		evaluations = evals,
		ms = GetTime() - run.started,
	}
end

-- One-engine form for the CLI and a pool-less host.
M.suggest_unique_jewels = function(p)
	local plan = M.jewel_plan(p)
	local results = array({})
	for _, job in ipairs(plan.jobs) do results[#results + 1] = M.score_jewel_variants(job) end
	return M.jewel_finish({ results = results })
end

-- Exposed for `pobctl eval` scripting: __bridge.tree_click({ id = 123 })
_G.__bridge = M

-- Whole-build snapshots, kept in memory for the session. Tree edits have undo;
-- gear, gems and config do not, so this is the way back from a change that
-- made things worse.
local checkpoints, checkpointOrder = {}, {}

M.checkpoint = function(p)
	ensureBuild()
	local label = p and p.label and tostring(p.label) or ("checkpoint " .. (#checkpointOrder + 1))
	if not checkpoints[label] then checkpointOrder[#checkpointOrder + 1] = label end
	local o = build.calcsTab.mainOutput or {}
	checkpoints[label] = {
		xml = build:SaveDB("checkpoint"),
		name = build.buildName,
		file = build.dbFileName,
		life = o.Life or 0,
		dps = o.CombinedDPS or o.TotalDPS or 0,
		ehp = o.TotalEHP or 0,
	}
	return { label = label, checkpoints = strArray(checkpointOrder) }
end

M.rollback = function(p)
	ensureBuild()
	local label = p and p.label and tostring(p.label) or checkpointOrder[#checkpointOrder]
	local cp = label and checkpoints[label]
	if not cp then error("no checkpoint " .. tostring(label) .. "; existing: " .. table.concat(checkpointOrder, ", "), 0) end
	M.load_build_xml({ xml = cp.xml, name = cp.name })
	build.dbFileName = cp.file
	return { restored = label, life = cp.life, dps = cp.dps, ehp = cp.ehp }
end

M.list_checkpoints = function()
	local out = array({})
	for _, label in ipairs(checkpointOrder) do
		local cp = checkpoints[label]
		out[#out + 1] = { label = label, life = cp.life, dps = cp.dps, ehp = cp.ehp }
	end
	return { checkpoints = out }
end

M.build_summary = function()
	ensureBuild()
	local o = build.calcsTab.mainOutput or {}
	local spec = build.spec
	local used, ascUsed, _, socketCount, ws1, ws2 = countAllocNodes(spec)
	local level = build.characterLevel or 1
	local questLow, questHigh = questPointsForLevel(level)

	local active, persistent, trigger, meta = 0, 0, 0, 0
	local mainSupports, mainName = 0, null
	local skills = array({})
	local granted = 0
	for gi, group in ipairs(build.skillsTab.socketGroupList) do
		local supports, skillName, press, gemNote = 0, nil, nil, nil
		for _, gem in ipairs(group.gemList) do
			local gd = gem.gemData
			local isSupport = (gd and gd.grantedEffect and gd.grantedEffect.support) and true or false
			if isSupport then
				supports = supports + 1
			elseif not skillName then
				skillName = gd and gd.name or gem.nameSpec
				press = gemPressClass(gd)
				gemNote = grantedGemNote(gd)
			end
		end
		local isMain = gi == build.mainSocketGroup
		-- Item-granted groups (group.source) are not something the player socketed
		-- or presses.
		if group.source then
			skillName = (skillName and skillName ~= "") and skillName or group.displayLabel or "granted"
			press = "granted"
		end
		if skillName and skillName ~= "" then
			-- A default weapon attack or Raise Shield is there because of the
			-- weapon; it is a button only if the build plays it, so it is
			-- counted apart from the skills the player chose.
			if group.enabled ~= false and press ~= "granted" and gemNote then
				granted = granted + 1
			elseif group.enabled ~= false and press ~= "granted" then
				if press == "active" then active = active + 1
				elseif press == "persistent" then persistent = persistent + 1
				elseif press == "trigger" then trigger = trigger + 1
				else meta = meta + 1 end
			end
			if isMain then mainSupports, mainName = supports, skillName end
			skills[#skills + 1] = {
				group = gi,
				skill = skillName,
				press = press,
				supports = supports,
				enabled = group.enabled ~= false,
				main = isMain,
				grantedBy = grantedBy(group),
				granted = opt(gemNote),
			}
		end
	end
	-- Mark an item's copy of a skill that is also socketed with supports.
	local byName = {}
	for _, k in ipairs(skills) do
		if k.grantedBy == null then byName[k.skill] = k.group end
	end
	for _, k in ipairs(skills) do
		if k.grantedBy ~= null and byName[k.skill] then k.duplicateOf = byName[k.skill] end
	end

	-- Charm slots come from the belt. PoB's EmptyCharms counts charms not
	-- toggled active rather than empty slots, so count the slots directly.
	local charmLimit = o.CharmLimit or 0
	local emptyCharms, charmsEquipped = 0, 0
	for i = 1, 3 do
		local slot = build.itemsTab.slots["Charm " .. i]
		local filled = slot and slot.selItemId and slot.selItemId ~= 0
		if filled then charmsEquipped = charmsEquipped + 1 end
		if i <= charmLimit and not filled then emptyCharms = emptyCharms + 1 end
	end
	local resists = {}
	for _, r in ipairs({ "FireResist", "ColdResist", "LightningResist", "ChaosResist" }) do
		resists[r] = o[r] or 0
	end
	return {
		characterLevel = level,
		className = spec.curClassName,
		ascendancyName = opt(spec.curAscendClassName),
		mainSkill = mainName,
		mainSkillGroup = build.mainSocketGroup,
		mainSkillSupports = mainSupports,
		-- Only `active` skills cost the player a keypress. See the buttons file in
		-- library/ for why this matters more than total gem count.
		activeSkills = active,
		persistentSkills = persistent,
		triggerSkills = trigger,
		metaSkills = meta,
		-- Skills the weapon or an item hands out (default attacks, Raise
		-- Shield): present because of the item, not chosen, not counted above.
		grantedSkills = granted,
		skills = skills,
		-- A point buys a node in either weapon set, so PoB charges only the larger set.
		pointsUsed = used,
		passivePointsSpent = used - math.min(ws1, ws2),
		extraPoints = o.ExtraPoints or 0,
		pointsAvailableMin = math.max(0, level - 1) + questLow + (o.ExtraPoints or 0),
		pointsAvailableMax = math.max(0, level - 1) + questHigh + (o.ExtraPoints or 0),
		ascendancyPointsUsed = ascUsed,
		jewelSocketsUsed = socketCount,
		weaponSetPointsUsed = ws1 + ws2,
		life = o.Life or 0,
		energyShield = o.EnergyShield or 0,
		mana = o.Mana or 0,
		spirit = o.Spirit or 0,
		spiritReserved = o.SpiritReserved or 0,
		spiritUnreserved = o.SpiritUnreserved or 0,
		charmLimit = charmLimit,
		emptyCharms = emptyCharms,
		charmsEquipped = charmsEquipped,
		-- PoB only applies a charm the user has toggled active, as with flasks.
		charmsActive = math.max(0, charmLimit - (o.EmptyCharms or charmLimit)),
		fireResist = resists.FireResist,
		coldResist = resists.ColdResist,
		lightningResist = resists.LightningResist,
		chaosResist = resists.ChaosResist,
		str = o.Str or 0,
		dex = o.Dex or 0,
		int = o.Int or 0,
		-- need is the highest single source (item, skill gem, or the shared
		-- support-gem source), as PoB computes it. Never add sources together.
		requirements = requirementSummary(),
		movementSpeedMod = o.MovementSpeedMod or 0,
		totalDPS = o.TotalDPS or o.CombinedDPS or 0,
		keystones = keystone.names(),
		keystoneRules = keystone.rules(keystone.profile()),
	}
end

M.sanity_check = function()
	ensureBuild()
	local o = build.calcsTab.mainOutput or {}
	local s = M.build_summary()
	local keys = keystone.profile()
	local findings = array({})
	local function add(severity, area, message, fix)
		findings[#findings + 1] = { severity = severity, area = area, message = message, fix = opt(fix) }
	end

	local uncapped, worst = {}, 75
	for _, r in ipairs({ { "fire", s.fireResist }, { "cold", s.coldResist }, { "lightning", s.lightningResist } }) do
		if r[2] < 75 then
			uncapped[#uncapped + 1] = string.format("%s %.0f%%", r[1], r[2])
			worst = math.min(worst, r[2])
		end
	end
	if #uncapped > 0 then
		add(worst < 50 and "high" or "medium", "resistances",
			string.format("below the 75%% cap: %s", table.concat(uncapped, ", ")),
			"Characters start at -50%. Quest rewards first, then suffixes on belt, boots, rings and body armour. An elemental rune in an armour piece is +14%.")
	end
	if s.chaosResist < 0 and not keys.chaosImmune then
		add("low", "resistances", string.format("chaos resistance is %.0f%%", s.chaosResist),
			"Chaos damage removes twice as much energy shield, and poison bypasses it entirely.")
	end

	-- Live characters hold one point more than PoB's quest data allows.
	if s.passivePointsSpent > s.pointsAvailableMax + 1 then
		add("high", "passive points", string.format("%d passive points spent but at level %d the maximum is %d", s.passivePointsSpent, s.characterLevel, s.pointsAvailableMax),
			"Either the level is unset or the tree is over budget. Call set_level if the level is wrong.")
	elseif s.passivePointsSpent < s.pointsAvailableMin then
		add("low", "passive points", string.format("%d of %d available passive points spent", s.passivePointsSpent, s.pointsAvailableMin),
			"Unspent points.")
	end
	local ascended = s.ascendancyName ~= null and s.ascendancyName ~= "None"
	if not ascended and s.characterLevel >= 20 then
		add("medium", "ascendancy", "no ascendancy chosen",
			"Ascendancies carry 8 points of build-defining bonuses and are usually the reason to pick a class.")
	elseif ascended and s.ascendancyPointsUsed < 8 then
		add("medium", "ascendancy", string.format("%d of 8 ascendancy points allocated", s.ascendancyPointsUsed))
	end

	if s.mainSkillSupports < 4 and s.mainSkill ~= null then
		add("high", "supports", string.format("main skill %s has %d supports", tostring(s.mainSkill), s.mainSkillSupports),
			"Published builds run 4-5 supports on the damage skill. Call list_valid_supports for the legal options.")
	end
	if s.activeSkills > 6 then
		add("low", "buttons", string.format("%d skills need a keypress", s.activeSkills),
			IS_POE2 and "Most builds settle at 4-5. Extra power is usually better spent on a persistent buff, trigger or meta gem."
				or "Most builds settle at 4-5. Extra power is usually better spent on an aura, a trigger setup or a guard skill.")
	end

	if not IS_POE2 then
		-- Flasks are a PoE1 build's cheapest defence and utility.
		local emptyFlasks = 0
		for i = 1, 5 do
			local slot = build.itemsTab.slots["Flask " .. i]
			if slot and (not slot.selItemId or slot.selItemId == 0) then emptyFlasks = emptyFlasks + 1 end
		end
		if emptyFlasks > 0 then
			add(emptyFlasks >= 3 and "medium" or "low", "flasks", string.format("%d of 5 flask slots empty", emptyFlasks),
				"A life flask, a resistance or armour utility flask and a quicksilver flask cover most gaps. Flasks with 'used when' enchants need no keypress.")
		end
		local input = build.configTab and build.configTab.input or {}
		local major, minor = input.pantheonMajorGod, input.pantheonMinorGod
		if s.characterLevel >= 60 and (major == nil or major == "None") and (minor == nil or minor == "None") then
			add("low", "pantheon", "no pantheon gods chosen",
				"Pantheon powers are free defences unlocked in the campaign; set them on the Config tab so the calculation includes them.")
		end
	end

	-- 30 spirit is the cheapest herald, so below that there is nothing to spend on.
	if s.spiritUnreserved >= 30 then
		add(s.spiritReserved == 0 and "medium" or "low", "spirit",
			string.format("%d of %d spirit unreserved", s.spiritUnreserved, s.spirit),
			"Spirit is only useful when spent. A herald reserves 30; meta gems reserve more.")
	end

	if s.charmLimit > 0 and s.emptyCharms > 0 then
		add("medium", "charms", string.format("%d of %d charm slots empty", s.emptyCharms, s.charmLimit),
			"Charms trigger automatically and are the cheapest answer to a specific ailment. Unique charms add a large rider on top.")
	end
	if s.charmsEquipped > s.charmLimit then
		add("low", "charms", string.format("%d charms equipped but the belt gives %d charm slot(s)", s.charmsEquipped, s.charmLimit),
			"The extra charms do nothing. Charm slots are a belt property; a Heavy Belt base can carry up to 3.")
	end

	if s.characterLevel >= 30 and s.life < 500 and s.energyShield < 500 and not (keys.manaFirst > 0 and s.mana >= 500) then
		add("high", "survivability", string.format("life %.0f and energy shield %.0f at level %d", s.life, s.energyShield, s.characterLevel),
			"Both pools are very low for this level.")
	end
	-- movementSpeedMod is a multiplier, so 1.0 is no bonus.
	if s.characterLevel >= 15 and s.movementSpeedMod < 1.15 then
		add("medium", "movement", string.format("movement speed is %+.0f%%", (s.movementSpeedMod - 1) * 100),
			"57 of 63 published builds carry movement speed on boots, usually 20-35%. It shortens the campaign and is a primary avoidance layer.")
	end

	if IS_POE2 and s.characterLevel >= 60 and s.weaponSetPointsUsed == 0 then
		add("low", "weapon sets", "no weapon set passive points allocated",
			"24 points per set that only count while that set is active, so a build that never swaps still gains them on set I. A poe.ninja export can leave them out; if the character has them in game, this finding is an import gap.")
	end

	for _, attr in ipairs({ "str", "dex", "int" }) do
		local r = s.requirements[attr]
		if r and not r.met then
			add("high", "requirements", string.format("%s %d needed, %d available (from %s)", attr, r.need, r.have, r.from ~= null and r.from or "?"),
				"The requirement is the highest single source, not a sum. Meet it with attribute travel nodes, an attribute affix, or a lower gem level; or swap the binding item or gem.")
		end
	end

	-- Affixes that spend a slot on nothing the build can use.
	local deadLines = {}
	for slotName, slot in pairs(build.itemsTab.slots) do
		local item = slot.selItemId and slot.selItemId ~= 0 and build.itemsTab.items[slot.selItemId]
		if item and not slot.nodeId then
			for _, mod in ipairs(item.explicitModLines or {}) do
				local line = (mod.line or ""):lower()
				if line:find("reduced attribute requirements", 1, true) then
					deadLines[#deadLines + 1] = string.format("%s (%s): %s", item.name or "?", slotName, (mod.line:gsub("^{.-}", "")))
				end
			end
		end
	end
	if #deadLines > 0 then
		add("low", "gear", string.format("%d affix(es) spent on reduced attribute requirements: %s", #deadLines, table.concat(deadLines, "; ")),
			"Usually a dead affix: the same suffix slot could carry resistance, life or an attribute, which meets the requirement and adds something. Keep it only when the item is otherwise the best available and nothing else would meet the requirement.")
	end

	-- Attribute shortfalls surface as per-gem errors from PoB itself.
	local gemErrors = {}
	for _, group in ipairs(build.skillsTab.socketGroupList) do
		for _, gem in ipairs(group.gemList) do
			if gem.errMsg then gemErrors[#gemErrors + 1] = (gem.nameSpec or "gem") .. ": " .. tostring(gem.errMsg) end
		end
	end
	if #gemErrors > 0 then
		add("high", "gems", string.format("%d gem problem(s): %s", #gemErrors, table.concat(gemErrors, "; ")),
			"Usually an unmet attribute requirement or a missing weapon type.")
	end

	local order = { high = 1, medium = 2, low = 3 }
	table.sort(findings, function(a, b)
		if order[a.severity] ~= order[b.severity] then return order[a.severity] < order[b.severity] end
		return a.area < b.area
	end)
	local counts = { high = 0, medium = 0, low = 0 }
	for _, f in ipairs(findings) do counts[f.severity] = counts[f.severity] + 1 end
	return { findings = findings, high = counts.high, medium = counts.medium, low = counts.low }
end

return M
