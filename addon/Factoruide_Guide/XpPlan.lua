-- XP compared with the guide: where it expects the character at the step shown and once the
-- current tasks are done, drawn on the guide's own XP bars (Card, Mini).

local FG = Factoruide

local XpPlan = {}
FG.XpPlan = XpPlan

--- Where the guide expects the character before the step shown: its level and XP in it.
function XpPlan:Planned()
    local route = FG.Guide and FG.Guide:Route()
    local state = FG.Guide and FG.Guide:State()
    if not route or not state then return nil end
    local done = route.steps[state.step - 1]
    if done and done.px then return done.lvl, done.px end
    if state.step <= 1 then return route.fromLevel or 1, 0 end
    return nil
end

--- XP `xp` of `level` on the current level's scale (above its size: the next level, below 0: an
--- earlier one), or nil with `level` when it is further away.
local function OnScale(level, xp)
    local here, max = UnitLevel("player"), UnitXPMax("player")
    if level == here then return xp end
    if level == here + 1 then return max + xp end
    if level == here - 1 and xp >= max then return xp - max end
    return nil, level
end

--- Where the guide expects the character once the current tasks are done: level and XP in it.
function XpPlan:PlannedAfter()
    local route = FG.Guide and FG.Guide:Route()
    local steps = FG.Guide and FG.Guide:CurrentSteps()
    local last = route and steps and route.steps[steps[#steps]]
    if not last or not last.px then return nil end
    return last.lvl, last.px
end

--- The character's XP, where the guide expects it before the step shown and once the current
--- tasks are done, as shares of the current level (0 to 1; the last two nil when unknown).
function XpPlan:Shares()
    local max = UnitXPMax("player")
    if not max or max <= 0 then return 0, nil, nil end
    local function Share(value, level)
        if value then return math.max(0, math.min(1, value / max)) end
        if level then return level > UnitLevel("player") and 1 or 0 end
        return nil
    end
    local from = Share(OnScale(self:Planned()))
    local afterLevel, afterXp = self:PlannedAfter()
    local to = afterLevel and Share(OnScale(afterLevel, afterXp)) or nil
    return math.min(1, UnitXP("player") / max), from, to
end

for _, event in ipairs({ "PLAYER_XP_UPDATE", "PLAYER_LEVEL_UP", "PLAYER_ENTERING_WORLD" }) do
    FG:On(event, function()
        if FG.Guide then FG.Guide:Refresh() end
    end)
end
