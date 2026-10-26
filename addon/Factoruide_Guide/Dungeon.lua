-- Dungeon prep: before a dungeon the guide plans, the quests to have in the log when going in
-- (those done inside: in the log already, still to pick up on the way, or started inside), its
-- level, and a group message ready to send. Said once in the chat when the dungeon comes near
-- (within `NEAR` steps, or the character a level below it), and at any time with `/fg dungeon`;
-- `/fg lfg` puts the message in the chat's input line, to send with Enter (nothing is sent on
-- its own).

local FG = Factoruide
local T = FG.T

local Dungeon = {}
FG.Dungeon = Dungeon

local L = {
    title = T("Next dungeon: %s at level %d (~%d min), %d steps ahead"),
    inLog = T("  in your log: %s"),
    pickUp = T("  to pick up first: %s (%s)"),
    inside = T("  starts inside: %s"),
    none = T("no dungeon ahead in the guide"),
    lfg = T("/fg lfg writes the group message in the chat (Enter sends it)"),
    lfm = T("LF%dM %s, level %d"),
    lfmQuests = T(", quests: %s"),
}

-- Said when the dungeon is this many steps ahead, or the character a level below it.
Dungeon.NEAR = 25
-- Looked for this far ahead.
local AHEAD = 400
-- A group of five.
local GROUP = 5

local Guide = FG.Guide

local function Route() return Guide:Route(), Guide:State() end

--- The index of the next dungeon step from the current one, nil when none ahead.
function Dungeon:Next()
    local route, state = Route()
    if not route or not state then return nil end
    for i = state.step, math.min(#route.steps, state.step + AHEAD) do
        local s = route.steps[i]
        if s.k == "dungeon" and not Guide:StepDone(i) then return i end
    end
    return nil
end

-- A quest's title: the game's when in the log, else the text of its step inside the dungeon
-- ("{quest}: {objectives}"), else its number.
local function Title(q, step)
    local title = FG.QuestLog.GetTitleForQuestID and FG.Readable(FG.QuestLog.GetTitleForQuestID(q))
    if title then return title end
    local fromText = step and step.t and step.t:match("^(.-)%s?[:：]")
    return fromText ~= "" and fromText or tostring(q)
end

--- The quests done inside the dungeon of step `index`: { q, title, state ("log", "pickup" or
--- "inside"), npc } in the order of the guide.
function Dungeon:Quests(index)
    local route = Route()
    local out, seen = {}, {}
    local i = index + 1
    while route.steps[i] and route.steps[i].di do
        local s = route.steps[i]
        if s.q and not seen[s.q] then
            seen[s.q] = true
            local quest = { q = s.q, title = Title(s.q, s.k ~= "accept" and s or nil) }
            if s.k == "accept" then
                quest.state = "inside"
            elseif FG.QuestLog.IsOnQuest(s.q) then
                quest.state = "log"
            else
                quest.state = "pickup"
                for j = index - 1, 1, -1 do
                    local a = route.steps[j]
                    if a.k == "accept" and a.q == s.q then
                        quest.npc = a.n
                        break
                    end
                end
            end
            out[#out + 1] = quest
        end
        i = i + 1
    end
    return out
end

--- The group message for the dungeon of step `index`.
function Dungeon:Message(index)
    local route = Route()
    local step = route.steps[index]
    local members = GetNumGroupMembers and GetNumGroupMembers() or 0
    local need = GROUP - math.max(1, members)
    local text = string.format(L.lfm, math.max(1, need), step.n or "?", step.lvl or UnitLevel("player"))
    local titles = {}
    for _, quest in ipairs(self:Quests(index)) do
        if quest.state ~= "inside" then titles[#titles + 1] = quest.title end
    end
    if #titles > 0 then text = text .. string.format(L.lfmQuests, table.concat(titles, ", ")) end
    return text
end

--- The card in the chat: the dungeon, its quests, how to write the group message.
function Dungeon:Print(index)
    index = index or self:Next()
    if not index then return FG:Print("%s", L.none) end
    local route, state = Route()
    local step = route.steps[index]
    local seconds, i = 0, index + 1
    while route.steps[i] and route.steps[i].di do
        seconds = seconds + (route.steps[i].d or 0)
        i = i + 1
    end
    FG:Print(L.title, step.n or step.t, step.lvl or 0, math.floor(seconds / 60 + 0.5), index - state.step)
    for _, quest in ipairs(self:Quests(index)) do
        if quest.state == "log" then
            FG:Print(L.inLog, quest.title)
        elseif quest.state == "pickup" then
            FG:Print(L.pickUp, quest.title, quest.npc or "?")
        else
            FG:Print(L.inside, quest.title)
        end
    end
    FG:Print("%s", L.lfg)
end

--- The group message in the chat's input line (sent only with Enter).
function Dungeon:Lfg()
    local index = self:Next()
    if not index then return FG:Print("%s", L.none) end
    local text = self:Message(index)
    if ChatFrame_OpenChat then
        ChatFrame_OpenChat(text)
    else
        FG:Print("%s", text)
    end
end

--- Every few seconds: the card once when the next dungeon comes near.
function Dungeon:Watch()
    if not FG:Get("dungeonPrep") then return end
    local index = self:Next()
    if not index then return end
    local route, state = Route()
    local step = route.steps[index]
    local near = index - state.step <= self.NEAR or (step.lvl and UnitLevel("player") >= step.lvl - 1)
    if not near then return end
    state.dungeonPrep = state.dungeonPrep or {}
    if state.dungeonPrep[index] then return end
    state.dungeonPrep[index] = true
    self:Print(index)
end

FG:On("PLAYER_LOGIN", function()
    if C_Timer and C_Timer.NewTicker then
        C_Timer.NewTicker(5, function() FG:Try("dungeon prep", Dungeon.Watch, Dungeon) end)
    end
end)

-- `/fg dungeon` and `/fg lfg`; the rest of `/fg` goes on to the other commands.
do
    local open = SlashCmdList.FACTORUIDE
    SlashCmdList.FACTORUIDE = function(msg)
        local cmd = strtrim(msg or ""):lower()
        if cmd == "dungeon" then return Dungeon:Print() end
        if cmd == "lfg" then return Dungeon:Lfg() end
        if open then return open(msg) end
    end
end
