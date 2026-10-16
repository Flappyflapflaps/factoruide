-- Talent plan: the next talent of the class's leveling plan (TalentBuilds.lua, laid on the
-- game's trees by `fg talents`), shown by the guide while points are unspent; learned in one
-- click, or automatically.

local FG = Factoruide
local T = FG.T

local Talents = {}
FG.Talents = Talents

--- Talents of the character: "tab:tier:column" -> { tab, index, name, icon, rank, maxRank }.
local function Known()
    local out = {}
    for tab = 1, (GetNumTalentTabs and GetNumTalentTabs() or 0) do
        for i = 1, GetNumTalents(tab) do
            local name, icon, tier, column, rank, maxRank = GetTalentInfo(tab, i)
            if name then
                out[tab .. ":" .. tier .. ":" .. column] =
                    { tab = tab, index = i, name = name, icon = icon, rank = rank, maxRank = maxRank }
            end
        end
    end
    return out
end

--- The next talent of the plan not learned yet, or nil.
function Talents:Next()
    local builds = FG.TalentBuilds and FG.TalentBuilds[FG.Edition]
    local build = builds and builds[select(2, UnitClass("player"))]
    if not build then return nil end
    local known = Known()
    local wanted = {}
    for _, pick in ipairs(build.picks) do
        local key = pick[1] .. ":" .. pick[2] .. ":" .. pick[3]
        wanted[key] = (wanted[key] or 0) + 1
        local t = known[key]
        if t and t.rank < wanted[key] and t.rank < t.maxRank then return t end
    end
    return nil
end

local function Unspent() return UnitCharacterPoints and UnitCharacterPoints("player") or 0 end

--- The talent to learn now (points unspent, the advice on), nil when none.
function Talents:Pending()
    if not FG:Get("talentAdvisor") or Unspent() <= 0 then return nil end
    return self:Next()
end

function Talents:LearnNext()
    local t = self:Next()
    if t and Unspent() > 0 and not InCombatLockdown() then
        LearnTalent(t.tab, t.index)
        FG:Print(T("talent learned: %s (%d/%d)"), t.name, t.rank + 1, t.maxRank)
        return true
    end
    return false
end

function Talents:Refresh()
    if FG:Get("talentAdvisor") and FG:Get("autoLearnTalents") and not InCombatLockdown() then
        for _ = 1, Unspent() do
            if not self:LearnNext() then break end
        end
    end
    if FG.Guide then FG.Guide:Refresh() end
end

for _, event in ipairs({
    "PLAYER_ENTERING_WORLD",
    "PLAYER_LEVEL_UP",
    "CHARACTER_POINTS_CHANGED",
    "PLAYER_REGEN_ENABLED",
}) do
    FG:On(event, function()
        -- Talent data is ready a moment after these events.
        if C_Timer then
            C_Timer.After(0.5, function() Talents:Refresh() end)
        else
            Talents:Refresh()
        end
    end)
end
