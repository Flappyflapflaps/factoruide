-- Target macro: a character macro kept up to date with what the current steps need (mobs to
-- kill or to loot, NPC to talk to). Bind it to a key: one press targets the nearest one.

local FG = Factoruide
local T = FG.T

local NAME = "FG Cible"
local ICON = "Ability_Hunter_SniperShot"
local MAX_BODY = 255

local Macro = { names = {}, body = nil, pending = false }
FG.TargetMacro = Macro

local function Body(names)
    local lines = { "/cleartarget" }
    for _, name in ipairs(names) do
        table.insert(lines, "/targetexact " .. name)
        table.insert(lines, "/cleartarget [dead]")
    end
    -- WoW Forever restricts SetRaidTarget: from a macro's /run the game blocks it (a popup
    -- naming "ForceTaint_Strong"), and /tm would take the skull off at the next press.
    if FG:Get("targetMark") and FG.Edition ~= "forever" then
        table.insert(
            lines,
            '/run if UnitExists("target") and not GetRaidTargetIndex("target") then SetRaidTarget("target",8) end'
        )
    end
    -- Too long for a macro: drop names from the end.
    local body = table.concat(lines, "\n")
    if #body > MAX_BODY and #names > 1 then return Body({ unpack(names, 1, #names - 1) }) end
    return body
end

--- Write the macro (out of combat), creating it when missing. Returns its index.
local function Write(body, create)
    if not GetMacroIndexByName or InCombatLockdown() then
        Macro.pending = true
        return nil
    end
    Macro.pending = false
    local index = GetMacroIndexByName(NAME)
    if index == 0 then
        if not create then return nil end
        local _, characterCount = GetNumMacros()
        if characterCount >= (MAX_CHARACTER_MACROS or 18) then
            FG:Print(T("no room for a character macro (%s)"), NAME)
            return nil
        end
        return CreateMacro(NAME, ICON, body, true)
    end
    EditMacro(index, NAME, ICON, body)
    return index
end

--- Names to target for these steps (the ones still to do).
function Macro:SetSteps(steps)
    if not FG:Get("targetMacro") then return end
    local names, seen = {}, {}
    local function add(name)
        if name and name ~= "" and not seen[name] then
            seen[name] = true
            table.insert(names, name)
        end
    end
    for _, step in ipairs(steps) do
        if step.mobs then
            for _, name in ipairs(step.mobs) do
                add(name)
            end
        elseif step.e == "npc" then
            add(step.n)
        end
    end
    local body = Body(names)
    if body ~= self.body then
        self.body = body
        self.names = names
        Write(body, false)
    end
end

--- `/fg macro`: create the macro if needed and put it on the cursor, to drop on an action bar.
function Macro:Pickup()
    local index = Write(self.body or Body(self.names), true)
    if index and PickupMacro then
        PickupMacro(index)
        FG:Print(T("macro “%s”: drop it on an action bar and bind a key"), NAME)
    elseif InCombatLockdown and InCombatLockdown() then
        FG:Print(T("not possible in combat"))
    end
end

local frame = CreateFrame("Frame")
frame:RegisterEvent("PLAYER_REGEN_ENABLED")
frame:SetScript("OnEvent", function()
    if Macro.pending and Macro.body then Write(Macro.body, false) end
end)
