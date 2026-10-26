-- Where the character's XP comes from, level by level: the quests turned in, the mobs of the
-- quests, the mobs killed off the guide's quests (on the way, in the way, respawns...) and the
-- rest (discoveries, escorts...). Guide.lua sorts each XP gain (see Settle); this keeps the sums,
-- per character (FactoruideDB.characters[...].xp[level]), so that they stay for later sessions and
-- for the app. The guide plans some kills on the way (the steps' `fx`): the kills off the guide
-- are compared with them, and the gap with the plan they make feeds the re-plan (Replan.lua).
-- "XP sources (in the chat)", a key binding or `/fg xp`, prints them.

local FG = Factoruide
local T = FG.T

local XpLedger = {}
FG.XpLedger = XpLedger

local L = {
    title = T("XP by source, level %d:"),
    line = T("quests %s · quest mobs %s · kills off the guide %s (planned %s) · other %s"),
    none = T("no XP recorded yet at this level"),
    total = T("kills off the guide since level %d: %s XP (planned %s)"),
}

-- The kinds of XP kept: quest turn-ins, kills counting for a quest, other kills, the rest.
local KINDS = { quest = true, questMob = true, kill = true, other = true }

local log -- [level] = { quest, questMob, kill, kills, other }

local function Thousands(n)
    n = math.floor((n or 0) + 0.5)
    if BreakUpLargeNumbers then return BreakUpLargeNumbers(n) end
    return tostring(n)
end

--- XP `xp` of kind `kind` earned at `level` (the character's level by default).
function XpLedger:Add(kind, xp, level)
    if not log or not KINDS[kind] or type(xp) ~= "number" or xp <= 0 then return end
    level = level or UnitLevel("player")
    local row = log[level]
    if not row then
        row = {}
        log[level] = row
    end
    row[kind] = (row[kind] or 0) + xp
    if kind == "kill" then row.kills = (row.kills or 0) + 1 end
end

--- The sums of a level (an empty table when nothing was recorded at it).
function XpLedger:Level(level) return log and log[level] or {} end

--- The XP the guide in use plans from the mobs killed on the way at `level` (its steps' `fx`).
function XpLedger:Planned(level)
    local route = FG.Guide and FG.Guide:Route()
    local xp = 0
    for _, s in ipairs(route and route.steps or {}) do
        if s.lvl == level and s.fx then xp = xp + s.fx end
    end
    return xp
end

--- XP of the kills off the guide's quests from level `from` to the character's, and what the guide
--- planned for them.
function XpLedger:Extra(from)
    local got, planned = 0, 0
    for level = from or UnitLevel("player"), UnitLevel("player") do
        got = got + (self:Level(level).kill or 0)
        planned = planned + self:Planned(level)
    end
    return got, planned
end

--- The sums of the current level in the chat, and the kills off the guide since the guide's start.
function XpLedger:Print()
    local level = UnitLevel("player")
    local row = self:Level(level)
    FG:Print(L.title, level)
    if not next(row) then
        FG:Print("%s", L.none)
    else
        FG:Print(
            L.line,
            Thousands(row.quest),
            Thousands(row.questMob),
            Thousands(row.kill),
            Thousands(self:Planned(level)),
            Thousands(row.other)
        )
    end
    local route = FG.Guide and FG.Guide:Route()
    local from = route and route.fromLevel or 1
    local got, planned = self:Extra(from)
    FG:Print(L.total, from, Thousands(got), Thousands(planned))
end

FG:On("PLAYER_LOGIN", function()
    local char = FactoruideDB.characters[FG:CharacterKey()]
    char.xp = char.xp or {}
    log = char.xp
end)

BINDING_NAME_FACTORUIDE_XP = T("XP sources (in the chat)")
