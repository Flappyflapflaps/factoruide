-- Self-check (`/fg check`): what the addon sees of this client, a line each, ok or to look at.
-- Written for the first night of a new client (WoW Forever's launch): the client's version and
-- the edition read from it, the guide followed and where it comes from, positions on the map (the
-- compass needs them), the spells and items the XP buff reminders know by number, the auras the
-- character has now with their numbers (to correct those), the quest log, the add-ons it works
-- with, and the errors counted. The report is kept in FactoruideDB.check (SavedVariables), so that
-- what one client shows can be read after the game.

local FG = Factoruide
local T = FG.T

local SelfCheck = {}
FG.SelfCheck = SelfCheck

-- What the XP buff reminders and the rested reminder look for (XpBuffs.lua, Rested.lua).
SelfCheck.SPELLS = {
    { 19705, "Well Fed" },
    { 429959, "Cozy Sleeping Bag buff" },
    { 1229451, "Boosted Rest (Camp Tent)" },
}
SelfCheck.ITEMS = {
    { 211527, "Cozy Sleeping Bag" },
}
-- The add-ons the guide hands over to or reads, when installed.
SelfCheck.ADDONS = {
    { "Factoruide_Data", "guides made in the app" },
    { "Factoruide_Ready", "ready-made guides" },
    { "ShortestPathForever", "live paths for the compass" },
    { "RXPGuides", "RestedXP guides, for the app's import" },
}

local OK, LOOK = "ok", "look"
local COLOR = { [OK] = "|cff40ff40", [LOOK] = "|cffffcc00" }

local function Loaded(name)
    if C_AddOns and C_AddOns.IsAddOnLoaded then return FG.Readable(C_AddOns.IsAddOnLoaded(name)) and true or false end
    return IsAddOnLoaded and IsAddOnLoaded(name) and true or false
end

--- The lines of the check: { status ("ok" or "look"), text }.
function SelfCheck:Lines()
    local out = {}
    local function Add(status, fmt, ...) out[#out + 1] = { status, string.format(T(fmt), ...) } end

    -- The client and the edition read from it.
    local version, build, _, toc = "?", "?", nil, nil
    if GetBuildInfo then
        version, build, _, toc = GetBuildInfo()
    end
    Add(
        OK,
        "Client %s (build %s, interface %s): edition %s",
        tostring(version),
        tostring(build),
        tostring(toc or "?"),
        FG.Edition
    )

    -- The guide followed.
    local Guide = FG.Guide
    local route, state = Guide and Guide:Route(), Guide and Guide:State()
    local name = Guide and Guide:RouteName()
    if route and name then
        local info = FG.Import and FG.Import.info and FG.Import.info[name] or {}
        Add(
            OK,
            "Guide: %s (from %s), step %d of %d",
            name,
            tostring(info.source or "?"),
            state and state.step or 0,
            #(route.steps or {})
        )
        local level = UnitLevel("player")
        if route.fromLevel and route.toLevel and (level < route.fromLevel or level > route.toLevel) then
            Add(LOOK, "Level %d is outside the guide's levels (%d-%d)", level, route.fromLevel, route.toLevel)
        end
    else
        Add(LOOK, "No guide followed: install Factoruide_Ready, or make one in the app and /reload")
    end
    local tooNew = FG.Import and FG.Import.tooNew or {}
    if #tooNew > 0 then Add(LOOK, "%d guides need a newer Factoruide Guide: update it", #tooNew) end

    -- Positions on the map.
    local map = C_Map and C_Map.GetBestMapForUnit and FG.Readable(C_Map.GetBestMapForUnit("player"))
    local pos = map and C_Map.GetPlayerMapPosition and C_Map.GetPlayerMapPosition(map, "player")
    local x, y
    if pos and pos.GetXY then
        x, y = pos:GetXY()
    end
    x, y = FG.Readable(x), FG.Readable(y)
    if x and y then
        Add(OK, "Position: map %d at %.1f, %.1f", map, x * 100, y * 100)
    elseif IsInInstance and FG.Readable(IsInInstance()) then
        Add(OK, "Position: in an instance (none there, as expected)")
    else
        Add(LOOK, "No position on the map here: the compass and the arrow cannot point")
    end

    -- The numbers of the XP buffs' spells and items.
    for _, s in ipairs(self.SPELLS) do
        local found = FG.Readable(FG.SpellName(s[1]))
        if found then
            Add(OK, "Spell %d: %s", s[1], found)
        else
            Add(LOOK, "Spell %d (%s) unknown to this client: its reminder cannot see it", s[1], T(s[2]))
        end
    end
    for _, item in ipairs(self.ITEMS) do
        local found = FG.Readable((FG.ItemInfo(item[1])))
        if found then
            Add(OK, "Item %d: %s", item[1], found)
        else
            Add(LOOK, "Item %d (%s) unknown here yet: run /fg check again in a minute", item[1], T(item[2]))
        end
    end

    -- The auras the character has now, with their numbers.
    if FG.XpBuffs then
        local auras = {}
        for _, a in ipairs(FG.XpBuffs:Auras()) do
            if a.name then auras[#auras + 1] = string.format("%s %s", a.name, tostring(a.spell or "?")) end
        end
        if #auras > 0 then
            Add(OK, "Auras now: %s", table.concat(auras, ", "))
        else
            Add(OK, "Auras now: none (eat cooked food and run it again to check Well Fed)")
        end
        local missing = {}
        for _, m in ipairs(FG.XpBuffs:Missing()) do
            missing[#missing + 1] = m[1]
        end
        Add(OK, "XP buff reminders due now: %s", #missing > 0 and table.concat(missing, ", ") or T("none"))
    end

    -- The quest log.
    local QuestLog = FG.QuestLog
    local entries = QuestLog and QuestLog.GetNumQuestLogEntries and FG.Readable(QuestLog.GetNumQuestLogEntries())
    if type(entries) == "number" then
        Add(OK, "Quest log readable: %d lines", entries)
    else
        Add(LOOK, "Quest log not readable: the guide cannot follow quests")
    end

    -- The add-ons around.
    for _, a in ipairs(self.ADDONS) do
        Add(OK, "%s (%s): %s", a[1], T(a[2]), Loaded(a[1]) and T("loaded") or T("not loaded"))
    end

    -- The errors counted.
    local errors, n = FactoruideDB and FactoruideDB.errors or {}, 0
    local worst, most = nil, 0
    for msg, count in pairs(errors) do
        n = n + 1
        if type(count) == "number" and count > most then
            worst, most = msg, count
        end
    end
    if n == 0 then
        Add(OK, "No error counted")
    else
        Add(LOOK, "%d errors counted, the most often (%dx): %s", n, most, tostring(worst))
    end
    return out
end

--- The check in the chat, kept in FactoruideDB.check.
function SelfCheck:Run()
    local lines = self:Lines()
    local look, kept = 0, {}
    FG:Print("%s", T("self-check:"))
    for _, line in ipairs(lines) do
        if line[1] == LOOK then look = look + 1 end
        FG:Print("%s%s|r %s", COLOR[line[1]], line[1] == OK and T("ok") or T("look"), line[2])
        kept[#kept + 1] = line[1] .. " " .. line[2]
    end
    FG:Print(look == 0 and T("self-check done: all ok") or T("self-check done: %d lines to look at"), look)
    if type(FactoruideDB) == "table" then
        FactoruideDB.check = { at = time and time() or 0, character = FG:CharacterKey(), lines = kept }
    end
    return lines
end

-- `/fg check`; the rest of `/fg` goes on to the other commands.
do
    local open = SlashCmdList.FACTORUIDE
    SlashCmdList.FACTORUIDE = function(msg)
        if strtrim(msg or ""):lower() == "check" then return SelfCheck:Run() end
        if open then return open(msg) end
    end
end
