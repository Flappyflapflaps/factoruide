-- Legacy perks (WoW Forever): which to buy first to level faster, in the chat at the character's
-- first Legacy point (level 25) and with a button in the options. The game offers no way for an
-- addon to read the perks bought: the advice is the same for everyone. Perk names in English, as
-- the game's Legacy window (Y) shows them on English clients.

local FG = Factoruide
local T = FG.T

local Legacy = {}
FG.Legacy = Legacy

-- The level of the first Legacy point (from questing; 150 skill in a crafting profession too).
Legacy.FIRST_POINT = 25

-- Best first: time saved on every character for the points spent (a judgement, not measured).
Legacy.PERKS = {
    { "Frequent Flier", 1, "flights 20% faster and half price." },
    { "Well Rested", 5, "+4% rested XP gained and kept per rank." },
    { "Talented", 5, "talent points from level 9, down to level 5 with every rank." },
    { "Thrill of Adventure", 5, "health and mana back after each kill: less eating and drinking." },
    { "Gourmand", 3, "food buffs last 33% longer, Well Fed's +5% XP included." },
    { "Permanence", 2, "camp rest bonuses last 50% longer." },
}

--- The advice, a line each.
function Legacy:Lines()
    local lines = { T("Legacy perks to buy first for leveling (Y opens the Legacy window):") }
    for i, perk in ipairs(self.PERKS) do
        table.insert(lines, string.format(T("%d. %s (%d ranks): %s"), i, perk[1], perk[2], T(perk[3])))
    end
    return lines
end

function Legacy:Print()
    for _, line in ipairs(self:Lines()) do
        FG:Print("%s", line)
    end
end

FG:On("PLAYER_LEVEL_UP", function(level)
    if FG.Edition == "forever" and level == Legacy.FIRST_POINT and FG:Get("legacyAdvice") then Legacy:Print() end
end)

FG.Options:AddButton("legacyAdvice", {
    label = "Legacy perks to buy first",
    text = "Show",
    help = "In the chat: the Legacy perks that save the most leveling time, best first.",
    run = function() Legacy:Print() end,
})
