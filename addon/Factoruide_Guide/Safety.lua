-- Safety (hardcore): warnings on dangerous targets (elites, mobs 3+ levels above, skulls) and
-- low health. The guide also tags elite objectives and mobs well above the character.

local FG = Factoruide

local fr = GetLocale() == "frFR"

local function Warn(text)
    if UIErrorsFrame then UIErrorsFrame:AddMessage(text, 1, 0.25, 0.25) end
    if PlaySound and SOUNDKIT and SOUNDKIT.RAID_WARNING then PlaySound(SOUNDKIT.RAID_WARNING) end
end

FG:On("PLAYER_TARGET_CHANGED", function()
    if
        not FG:Get("safetyAlerts")
        or not UnitExists("target")
        or FG.Readable(UnitIsDead("target"))
        or not FG.Readable(UnitCanAttack("player", "target"))
    then
        return
    end
    if FG.Readable(UnitIsPlayer("target")) then return end
    -- The target's level, kind and name can be secret on Forever: not compared then.
    local level, mine = FG.Readable(UnitLevel("target")), UnitLevel("player")
    local kind = FG.Readable(UnitClassification("target"))
    local name = FG.Readable(UnitName("target")) or ""
    if not level then return end
    if level == -1 or kind == "worldboss" then
        Warn(string.format(fr and "DANGER : %s (niveau ??)" or "DANGER: %s (level ??)", name))
    elseif kind == "elite" or kind == "rareelite" then
        Warn(string.format(fr and "Élite : %s (niveau %d)" or "Elite: %s (level %d)", name, level))
    elseif level >= mine + 3 then
        Warn(string.format(fr and "%s : niveau %d (+%d)" or "%s: level %d (+%d)", name, level, level - mine))
    end
end)

local warned = false
FG:On("UNIT_HEALTH", function(unit)
    if unit ~= "player" or not FG:Get("safetyAlerts") then return end
    local share = UnitHealth("player") / math.max(1, UnitHealthMax("player"))
    if share < 0.35 and not warned and not UnitIsDeadOrGhost("player") then
        warned = true
        Warn(string.format(fr and "Vie basse : %d %%" or "Low health: %d%%", math.floor(share * 100)))
    elseif share > 0.6 then
        warned = false
    end
end)
