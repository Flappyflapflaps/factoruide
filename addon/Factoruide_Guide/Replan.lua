-- Re-plan from here: a guide is a prediction, and real play drifts from it (more kills in the way
-- than planned, respawns, deaths, a dungeon with friends, a zone left early). The drift is watched
-- (every few seconds): the character's XP ahead of the guide's or behind it by more than the
-- setting `replanDrift` (in levels), or far from the step and from the one before it for a while.
-- The guide then offers to re-plan (a bar on it), or re-plans on its own (`replanMode` "auto").
--
-- Re-planning, in game, at once (no app needed): the guide picks up at the step where going on
-- costs the least from where the character stands, at its level. Each step it could pick up at
-- costs the trip there, less the time of the steps left out before it, plus the XP they would
-- have given (a quest left out loses its turn-in too) beyond the XP the character has ahead of the
-- plan: that XP is made up later by grinding, slower than the guide's questing. The kept
-- steps (`kp`: a chain going on, class quests, planned rewards, training) and the quests complete
-- in the log are never left out, nor content well above the character. Worked out a few steps a
-- frame. `/fg replan undo` (or the start-over button) puts the steps left out back.
--
-- With the app (it made the guide, or it installed Factoruide_Data): the request is saved
-- (FactoruideDB.replans[character]) and the game reloads so that the app reads the character as
-- it is now. The app computes the route again from it in the background, with the guide's
-- settings, and installs it; at the next /reload the character follows the new route (its
-- `replan` says for whom and which request).

local FG = Factoruide
local T = FG.T

local Replan = {}
FG.Replan = Replan

local L = {
    ahead = T("%.1f level ahead of the plan"),
    behind = T("%.1f level behind the plan"),
    far = T("Far from the guide's next step"),
    extra = T(" (+%s XP from kills off the guide)"),
    go = T("Re-plan from here"),
    tip = T(
        "The guide picks up at the best step for your level and where you stand, leaving out what is no longer worth it. Right-click: hide for now."
    ),
    tipBehind = T(
        "The guide's grind steps catch you up; the Factoruide app can also compute the route again from your character. Right-click: hide for now."
    ),
    popup = T(
        "Re-plan from here: the guide picks up at the best step for your level and where you stand, leaving out what is no longer worth it."
    ),
    popupApp = T(
        "\n\nThe Factoruide app can also compute the whole route again from your character, in the background: it needs a /reload to read it."
    ),
    here = T("Re-plan"),
    withApp = T("Re-plan + app"),
    working = T("re-planning from here…"),
    done = T("re-planned from here: %d steps left out, the guide picks up at “%s”%s (to undo: /fg replan undo)"),
    away = T(", %d yd away"),
    nothing = T("re-planned: the guide's next step is already the best place to go on from"),
    undone = T("re-plan undone: %d steps back in the guide"),
    noUndo = T("nothing to undo"),
    noRoute = T("no guide followed"),
    group = T("a group guide is followed by the whole group: make it again in the app"),
    asked = T(
        "the Factoruide app computes your route again from where you stand; once it says it is ready, type /reload"
    ),
    waiting = T("new route asked of the Factoruide app at %s: once it says it is ready, type /reload"),
    loaded = T("new route from the Factoruide app loaded: computed from level %d, where you stood"),
}

-- Far from the step: beyond these yards (from it and from the step before it), for this long.
local FAR, FAR_FOR = 1500, 120
-- Running speed (yards a second), with the first mount; a trip to another continent (seconds).
local RUN, MOUNT, OTHER_CONTINENT = 7, 1.6, 900
-- Steps looked at ahead, and per frame.
local WINDOW, PER_FRAME = 500, 40
-- A step whose content is this many levels above the character is not gone on from.
local ABOVE = 1
-- Grinding's XP a second, against the guide's questing.
local GRIND_SHARE = 0.5
-- Automatic re-plans at most this often; a bar hidden by hand stays hidden this long.
local AUTO_EVERY, HIDE_FOR = 600, 1800
-- Steps where being far is the plan (a trip), or out of the world (a dungeon).
local TRIPS = { fly = true, link = true, hearth = true, teleport = true, dungeon = true }
-- An unanswered request to the app is forgotten after a day.
local ASK_FOR = 86400

local Guide = FG.Guide
local away -- GetTime() since the character is far from the guide
local lastAuto -- GetTime() of the last automatic re-plan
local running -- the re-plan under way: { co, auto }

local function Thousands(n)
    n = math.floor((n or 0) + 0.5)
    if BreakUpLargeNumbers then return BreakUpLargeNumbers(n) end
    return tostring(n)
end

local function Route() return Guide:Route(), Guide:State() end

local function InGroupGuide() return FG.Group and FG.Group:Active() or false end

-- Drift --------------------------------------------------------------------------------------

--- How far the character's XP is from where the guide expects it before the current tasks, in
--- levels: above 0 ahead of the plan, below 0 behind; nil when unknown.
function Replan:Gap()
    if not FG.XpPlan then return nil end
    local level, xp = FG.XpPlan:Planned()
    local max = UnitXPMax("player")
    if not level or not xp or not max or max <= 0 then return nil end
    return (UnitLevel("player") - level) + (UnitXP("player") - xp) / max
end

--- Whether the character stands far from the guide: from the current step's place and from the
--- last place before it (where the guide expects the character to come from), not on a trip.
local function Far()
    local route, state = Route()
    local step = route and route.steps[state.step]
    if not step or not step.m or TRIPS[step.k] then return false end
    if UnitOnTaxi("player") or IsInInstance() then return false end
    -- Where the character is unknown (a client without positions): never far.
    local map = C_Map.GetBestMapForUnit("player")
    if not map or not C_Map.GetPlayerMapPosition(map, "player") then return false end
    local d = Guide.Distance(step)
    if d and d <= FAR then return false end
    for i = state.step - 1, math.max(1, state.step - 20), -1 do
        local before = route.steps[i]
        if before.m then
            if TRIPS[before.k] then return false end
            local back = Guide.Distance(before)
            return not back or back > FAR
        end
    end
    return true
end

--- The drift from the plan worth a re-plan: { reason = "ahead", "behind" or "far", gap }, or nil.
function Replan:Drift()
    if not FG:Get("replanWatch") or InGroupGuide() then return nil end
    local route, state = Route()
    if not route or not state or not route.steps[state.step] then return nil end
    local gap = self:Gap()
    local limit = FG:Get("replanDrift") or 0.5
    if gap and gap >= limit then return { reason = "ahead", gap = gap } end
    if gap and gap <= -limit then return { reason = "behind", gap = gap } end
    if away and GetTime() - away >= FAR_FOR then return { reason = "far", gap = gap } end
    return nil
end

--- The bar offering to re-plan (see Guide:Refresh): { text, go, tip }, nil when on the plan or
--- hidden by hand.
function Replan:Banner()
    local drift = self:Drift()
    if not drift then return nil end
    local state = Guide:State()
    local hidden = state and state.replanHidden
    if hidden and hidden.reason == drift.reason and time() - (hidden.at or 0) < HIDE_FOR then return nil end
    local text
    if drift.reason == "far" then
        text = L.far
    else
        text = string.format(drift.reason == "ahead" and L.ahead or L.behind, math.abs(drift.gap))
        if drift.reason == "ahead" and FG.XpLedger then
            local got, planned = FG.XpLedger:Extra(UnitLevel("player"))
            if got > planned then text = text .. string.format(L.extra, Thousands(got - planned)) end
        end
    end
    return { text = text, go = L.go, tip = drift.reason == "behind" and L.tipBehind or L.tip, reason = drift.reason }
end

--- Hide the bar (a right-click on it) until the drift changes, or for a while.
function Replan:Hide()
    local state = Guide:State()
    local drift = self:Drift()
    if state and drift then state.replanHidden = { reason = drift.reason, at = time() } end
    Guide:Refresh()
end

-- The re-plan in game --------------------------------------------------------------------------

--- XP a second the guide plans from step `from` on (its next steps' XP over their time).
local function Rate(route, from)
    local xp, seconds = 0, 0
    for i = from, math.min(#route.steps, from + 200) do
        local s = route.steps[i]
        xp = xp + (s.rx or 0) + (s.fx or 0)
        seconds = seconds + (s.d or 0)
    end
    if xp > 0 and seconds > 0 then return xp / seconds end
    -- No planned times (older guides): a level in half an hour.
    local max = UnitXPMax("player") or 0
    return max > 0 and max / 1800 or 1
end

--- Seconds to go to a step's place from where the character stands.
local function Travel(step, speed)
    if not step or not step.m then return 0 end
    local d = Guide.Distance(step)
    if not d then return OTHER_CONTINENT end
    return d / speed
end

--- Whether a step not done yet may be left out: not kept, not along the way, not the turn-in of a
--- quest complete in the log.
local function Skippable(step, index)
    if step.kp or step.bg or Guide:StepDone(index) then return false end
    if step.k == "turnin" and step.q and FG.QuestLog.IsOnQuest(step.q) and FG.QuestLog.IsComplete(step.q) then
        return false
    end
    return true
end

--- The step to go on from and the steps to leave out before it: { to, skip = { indexes }, lost,
--- saved, travel }. `yield` is called every few steps (a frame each in game).
function Replan:Plan(yield)
    local route, state = Route()
    local from = state.step
    local last = math.min(#route.steps, from + WINDOW)
    local level = UnitLevel("player")
    -- XP lost beyond what the character has ahead of the plan: ground back later.
    local grind = Rate(route, from) * GRIND_SHARE
    local max = UnitXPMax("player") or 0
    local budget = math.max(0, (self:Gap() or 0) * max)
    local speed = level >= 40 and RUN * MOUNT or RUN
    -- The first turn-in of each quest ahead: a quest left out loses it.
    local turnin = {}
    for i = from, last do
        local s = route.steps[i]
        if s.k == "turnin" and s.q and not turnin[s.q] then turnin[s.q] = i end
    end
    local QuestLog = FG.QuestLog
    local lostQuests, lost, saved, skip = {}, 0, 0, {}
    local function Lose(q, at)
        if not q or lostQuests[q] then return end
        lostQuests[q] = true
        local t = turnin[q]
        if t and t >= at then lost = lost + (route.steps[t].rx or 0) end
    end
    -- Going on from the current step (its place, or the first one after it).
    local here = from
    while route.steps[here] and not route.steps[here].m and here < last do
        here = here + 1
    end
    local best = { to = from, cost = Travel(route.steps[here], speed), count = 0, lost = 0, saved = 0 }
    best.travel = best.cost
    for j = from + 1, last do
        local i = j - 1
        local s = route.steps[i]
        if Skippable(s, i) then
            skip[#skip + 1] = i
            saved = saved + (s.d or 0)
            lost = lost + (s.fx or 0)
            if s.k == "turnin" then
                Lose(s.q, i)
            elseif s.k == "accept" and s.q and not QuestLog.IsOnQuest(s.q) then
                Lose(s.q, i)
            elseif s.k == "objective" then
                Lose(s.q, i)
            end
        end
        local step = route.steps[j]
        if step.lvl and step.lvl > level + ABOVE then break end
        -- A step of a quest left out is passed: not one to go on from.
        if
            step.m
            and not step.bg
            and not (step.q and lostQuests[step.q])
            and Guide:GroupOf(j) == j
            and not Guide:StepDone(j)
        then
            local travel = Travel(step, speed)
            local cost = travel + math.max(0, lost - budget) / grind - saved
            if cost < best.cost then
                best = { to = j, cost = cost, count = #skip, lost = lost, saved = saved, travel = travel }
            end
        end
        if yield and (j - from) % PER_FRAME == 0 then yield() end
    end
    best.skip = {}
    for k = 1, best.count do
        best.skip[k] = skip[k]
    end
    return best
end

--- Leave out the steps of a plan (marked as done by hand) and go on from its step (`auto`: from
--- the drift watch, quiet when there is nothing to leave out).
local function Apply(plan, auto)
    local route, state = Route()
    if #plan.skip == 0 then
        if not auto then FG:Print("%s", L.nothing) end
        return
    end
    for _, index in ipairs(plan.skip) do
        state.marks[index] = "skip"
    end
    state.replanned = { at = time(), from = state.step, to = plan.to, marks = plan.skip }
    state.replanHidden = nil
    local step = route.steps[plan.to]
    local d = Guide.Distance(step)
    FG:Print(L.done, #plan.skip, step.t or "", d and string.format(L.away, d) or "")
    Guide:Resync()
end

local runner
--- Work the re-plan out a few steps a frame, then apply it.
local function Step()
    if not running then
        if runner then runner:SetScript("OnUpdate", nil) end
        return
    end
    local ok, plan = coroutine.resume(running.co)
    if not ok then
        running = nil
        FG:Try("re-plan", error, plan)
    elseif coroutine.status(running.co) == "dead" then
        local auto = running.auto
        running = nil
        if plan then FG:Try("re-plan", Apply, plan, auto) end
    end
end

--- Re-plan from here in game (`auto`: on its own, from the drift watch). Under way already: nothing.
function Replan:Start(auto)
    local route, state = Route()
    if not route or not state then return FG:Print("%s", L.noRoute) end
    if InGroupGuide() then return FG:Print("%s", L.group) end
    if running then return end
    if not auto then FG:Print("%s", L.working) end
    running = { auto = auto, co = coroutine.create(function() return Replan:Plan(coroutine.yield) end) }
    runner = runner or CreateFrame("Frame")
    runner:SetScript("OnUpdate", Step)
end

--- Finish the re-plan under way at once (before a reload; the tests).
function Replan:Finish()
    while running do
        Step()
    end
end

--- Put the steps the last re-plan left out back in the guide.
function Replan:Undo()
    local _, state = Route()
    local last = state and state.replanned
    if not last then return FG:Print("%s", L.noUndo) end
    local n = 0
    for _, index in ipairs(last.marks or {}) do
        if state.marks[index] == "skip" then
            state.marks[index] = nil
            n = n + 1
        end
    end
    state.replanned = nil
    FG:Print(L.undone, n)
    Guide:Resync()
end

-- With the app ---------------------------------------------------------------------------------

--- Whether the Factoruide app is around: it installed Factoruide_Data, or made the guide followed.
function Replan:HasApp()
    local import = FG.Import
    if import and import.data then return true end
    local name = Guide:RouteName()
    local info = name and import and import.info and import.info[name]
    return info ~= nil and info.source == "app"
end

--- Ask the app to compute the route again from the character: saved for it, read at the reload.
function Replan:AskApp()
    local route, state = Route()
    if not route or not state then return end
    FactoruideDB.replans = FactoruideDB.replans or {}
    FactoruideDB.replans[FG:CharacterKey()] = {
        asked = time(),
        route = Guide:RouteName(),
        level = UnitLevel("player"),
        step = state.step,
    }
    FG:Print("%s", L.asked)
end

StaticPopupDialogs["FACTORUIDE_REPLAN"] = {
    text = L.popup,
    button1 = L.here,
    button2 = CANCEL or "Cancel",
    OnAccept = function() Replan:Start() end,
    timeout = 0,
    whileDead = true,
    hideOnEscape = true,
}

--- The guide's "Re-plan from here" (its bar, `/fg replan`, a key binding): asks how, with the app
--- around (in game only, or the app too); else re-plans in game.
function Replan:Ask()
    local route = Route()
    if not route then return FG:Print("%s", L.noRoute) end
    if InGroupGuide() then return FG:Print("%s", L.group) end
    local dialog = StaticPopupDialogs.FACTORUIDE_REPLAN
    if self:HasApp() then
        dialog.text = L.popup .. L.popupApp
        dialog.button2 = L.withApp
        -- The second button: in game now, then the reload that hands the character to the app.
        dialog.OnCancel = function(_, _, reason)
            if reason ~= "clicked" then return end
            Replan:Start()
            Replan:Finish()
            Replan:AskApp()
            ReloadUI()
        end
    else
        dialog.text = L.popup
        dialog.button2 = CANCEL or "Cancel"
        dialog.OnCancel = nil
    end
    StaticPopup_Show("FACTORUIDE_REPLAN")
end

--- At login: the route the app computed for the request of this character, followed; else the
--- request is still waiting (said once), or forgotten after a day.
function Replan:Login()
    local key = FG:CharacterKey()
    local ask = FactoruideDB.replans and FactoruideDB.replans[key]
    if not ask then return end
    for name, r in pairs(FG.Routes or {}) do
        local made = type(r) == "table" and r.replan
        if made and made.character == key and (tonumber(made.asked) or 0) >= (ask.asked or 0) then
            FactoruideDB.replans[key] = nil
            Guide:Choose(name)
            FG:Print(L.loaded, tonumber(made.level) or UnitLevel("player"))
            return
        end
    end
    if time() - (ask.asked or 0) > ASK_FOR then
        FactoruideDB.replans[key] = nil
    else
        FG:Print(L.waiting, date("%H:%M", ask.asked))
    end
end

-- The watch ---------------------------------------------------------------------------------

--- Every few seconds: how long the character has been far from the guide, and in "auto" mode the
--- re-plan once off the plan (ahead of it or far: behind it, the guide's grind steps catch up).
function Replan:Watch()
    if not FG:Get("replanWatch") then
        away = nil
        return
    end
    if Far() then
        away = away or GetTime()
    else
        away = nil
    end
    if FG:Get("replanMode") ~= "auto" or running or UnitAffectingCombat("player") then return end
    local drift = self:Drift()
    if not drift or drift.reason == "behind" then return end
    if lastAuto and GetTime() - lastAuto < AUTO_EVERY then return end
    lastAuto = GetTime()
    self:Start(true)
end

FG:On("PLAYER_LOGIN", function()
    FG:Try("re-plan", Replan.Login, Replan)
    if C_Timer and C_Timer.NewTicker then
        C_Timer.NewTicker(5, function() FG:Try("re-plan watch", Replan.Watch, Replan) end)
    end
end)

-- `/fg replan` (`/fg replan undo`) and `/fg xp`; the rest of `/fg` opens the options.
do
    local open = SlashCmdList.FACTORUIDE
    SlashCmdList.FACTORUIDE = function(msg)
        local cmd, rest = strtrim(msg or ""):lower():match("^(%S*)%s*(.-)$")
        if cmd == "replan" then
            if rest == "undo" then return Replan:Undo() end
            return Replan:Ask()
        elseif cmd == "xp" and FG.XpLedger then
            return FG.XpLedger:Print()
        end
        if open then return open(msg) end
    end
end

BINDING_NAME_FACTORUIDE_REPLAN = T("Re-plan from here")
