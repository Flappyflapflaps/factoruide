-- XP compared with the guide: where it expects the character at the step shown and once the
-- current tasks are done, drawn on the guide's own XP bars (Card, Mini).

local FG = Factoruide

local XpPlan = {}
FG.XpPlan = XpPlan

--- Where the guide expects the character before the current tasks (the first step of the group
--- shown): its level and XP in it.
function XpPlan:Planned()
    local route = FG.Guide and FG.Guide:Route()
    local state = FG.Guide and FG.Guide:State()
    if not route or not state then return nil end
    local steps = FG.Guide:CurrentSteps()
    local first = steps and steps[1] or state.step
    local done = route.steps[first - 1]
    if done and done.px then return done.lvl, done.px end
    if first <= 1 then return route.fromLevel or 1, 0 end
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

--- A hollow gold box on `parent`'s XP bar: the XP the current tasks give (see `XpPlan:Draw`).
function XpPlan:Box(parent)
    local box = {}
    for _, side in ipairs({ "top", "bottom", "left", "right" }) do
        box[side] = FG.Skin:Fill(parent, "OVERLAY", "gold", 1)
    end
    return box
end

--- The XP bar (`bar.track` and its textures: `fill` the character's XP, `mark` where the guide
--- expects the character before the current tasks, `box` from there to after them, hollow so
--- that the XP inside shows), `width` wide.
function XpPlan:Draw(bar, width)
    local xp, from, to = self:Shares()
    bar.fill:SetWidth(math.max(1, width * (xp or 0)))
    local boxed = from ~= nil and to ~= nil and to > from
    bar.mark:SetShown(from ~= nil and not boxed)
    if from and not boxed then
        bar.mark:ClearAllPoints()
        bar.mark:SetPoint("CENTER", bar.track, "LEFT", width * from, 0)
    end
    local box, left, right = bar.box, width * (from or 0), width * (to or 0)
    for _, t in pairs(box) do
        t:SetShown(boxed)
        t:ClearAllPoints()
    end
    if not boxed then return end
    -- 1 px lines, 2 px around the bar.
    local track = bar.track
    box.top:SetPoint("BOTTOMLEFT", track, "TOPLEFT", left - 1, 1)
    box.top:SetPoint("BOTTOMRIGHT", track, "TOPLEFT", right + 1, 1)
    box.top:SetHeight(1)
    box.bottom:SetPoint("TOPLEFT", track, "BOTTOMLEFT", left - 1, -1)
    box.bottom:SetPoint("TOPRIGHT", track, "BOTTOMLEFT", right + 1, -1)
    box.bottom:SetHeight(1)
    box.left:SetPoint("TOPRIGHT", box.top, "TOPLEFT", 1, 0)
    box.left:SetPoint("BOTTOMRIGHT", box.bottom, "BOTTOMLEFT", 1, 0)
    box.left:SetWidth(1)
    box.right:SetPoint("TOPLEFT", box.top, "TOPRIGHT", -1, 0)
    box.right:SetPoint("BOTTOMLEFT", box.bottom, "BOTTOMRIGHT", -1, 0)
    box.right:SetWidth(1)
end

for _, event in ipairs({ "PLAYER_XP_UPDATE", "PLAYER_LEVEL_UP", "PLAYER_ENTERING_WORLD" }) do
    FG:On(event, function()
        if FG.Guide then FG.Guide:Refresh() end
    end)
end
