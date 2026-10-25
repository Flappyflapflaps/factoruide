-- XP buffs (WoW Forever): after a fight, a reminder of each XP bonus missing, at most every few
-- minutes each. Well Fed (any cooked food: +5% XP from kills), the Cozy Sleeping Bag when in the
-- bags and ready (up to +3%), and for leatherworkers the Camp Tent (rested XP worth 5% of a level,
-- once an hour, while there is none). Not in a rested area nor at the level cap.

local FG = Factoruide
local T = FG.T

local XpBuffs = {}
FG.XpBuffs = XpBuffs

-- Well Fed: a spell of the buff (its name in the game's language) and the icon all of them show.
local WELL_FED, FOOD_ICON = 19705, 136000
local FOOD_ICON_PATH = "interface\\icons\\spell_misc_food"
-- The Cozy Sleeping Bag and its buff (1% XP from kills a stack, 3 at most).
local SLEEPING_BAG, BAG_BUFF, BAG_STACKS = 211527, 429959, 3
-- Leatherworking, and Boosted Rest: the hour after a Camp Tent's rested XP.
local LEATHERWORKING, BOOSTED_REST = 165, 1229451
-- Seconds between two reminders of the same buff.
XpBuffs.AGAIN = 600

local last = {}

--- The character's auras (buffs, then debuffs): { name, icon, count, spell }.
function XpBuffs:Auras()
    local out = {}
    for _, filter in ipairs({ "HELPFUL", "HARMFUL" }) do
        for i = 1, 40 do
            local name, icon, count, spell
            if C_UnitAuras and C_UnitAuras.GetAuraDataByIndex then
                local a = C_UnitAuras.GetAuraDataByIndex("player", i, filter)
                if not a then break end
                name, icon, count, spell = a.name, a.icon, a.applications, a.spellId
            elseif UnitAura then
                local _
                name, icon, count, _, _, _, _, _, _, spell = UnitAura("player", i, filter)
                if not name then break end
            else
                break
            end
            table.insert(out, {
                name = FG.Readable(name),
                icon = FG.Readable(icon),
                count = FG.Readable(count) or 0,
                spell = FG.Readable(spell),
            })
        end
    end
    return out
end

local function WellFed(aura, name)
    if aura.spell == WELL_FED or (name and aura.name == name) or aura.icon == FOOD_ICON then return true end
    return type(aura.icon) == "string" and aura.icon:lower() == FOOD_ICON_PATH
end

local function ItemReady(id)
    local cooldown = (C_Container and C_Container.GetItemCooldown) or GetItemCooldown
    if not cooldown then return true end
    local start, duration = cooldown(id)
    start, duration = FG.Readable(start), FG.Readable(duration)
    return not start or start == 0 or (duration or 0) <= 0 or start + duration <= GetTime()
end

--- The reminders the character's buffs call for now: { key, text }.
function XpBuffs:Missing()
    if FG.Edition ~= "forever" or FG.Readable(IsResting()) or UnitIsDeadOrGhost("player") then return {} end
    local cap = GetMaxPlayerLevel and GetMaxPlayerLevel()
    if cap and UnitLevel("player") >= cap then return {} end
    local fed, stacks, boosted = false, 0, false
    local fedName = FG.SpellName(WELL_FED)
    for _, aura in ipairs(self:Auras()) do
        if WellFed(aura, fedName) then fed = true end
        if aura.spell == BAG_BUFF then stacks = math.max(stacks, aura.count, 1) end
        if aura.spell == BOOSTED_REST then boosted = true end
    end
    local out = {}
    if not fed then table.insert(out, { "food", T("No Well Fed buff: eat cooked food for +5% XP from kills.") }) end
    if stacks < BAG_STACKS and FG.ItemCount(SLEEPING_BAG) > 0 and ItemReady(SLEEPING_BAG) then
        table.insert(out, { "bag", T("Cozy Sleeping Bag ready: rest on it for up to +3% XP from kills.") })
    end
    if not boosted and (GetXPExhaustion() or 0) <= 0 and FG.ProfessionSkill(LEATHERWORKING) then
        table.insert(out, { "tent", T("Camp Tent ready: rest at your tent for rested XP worth 5% of a level.") })
    end
    return out
end

--- After a fight: the reminders not given in the last `AGAIN` seconds.
function XpBuffs:Check()
    if not FG:Get("xpBuffAlerts") or not FG:Get("uiShown") then return end
    local now = GetTime()
    for _, missing in ipairs(self:Missing()) do
        local key, text = missing[1], missing[2]
        if not last[key] or now - last[key] >= self.AGAIN then
            last[key] = now
            if UIErrorsFrame then UIErrorsFrame:AddMessage(text, 1, 0.82, 0) end
            FG:Print("%s", text)
        end
    end
end

--- Forget the reminders given (each comes again after the next fight).
function XpBuffs:Forget() wipe(last) end

FG:On("PLAYER_REGEN_ENABLED", function() XpBuffs:Check() end)
