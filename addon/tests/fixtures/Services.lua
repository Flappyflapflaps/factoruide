-- A few places for the smoke test (the real Services.lua is written by the app and not
-- versioned): kind, factions, class ID or profession skill line, continent, world x, y (yards),
-- map, x, y (percent), name.
local FG = Factoruide
FG.Services = {
    { "class", "A", 8, 0, 300, 712, 1426, 30, 71.2, "Marryk Nurribit" },
    { "class", "A", 1, 0, 301, 713, 1426, 30.1, 71.3, "Thran Khorman" },
    { "class", "H", 8, 0, 302, 712, 1426, 30.2, 71.2, "Horde Mage" },
    { "profession", "A", 164, 0, 310, 712, 1426, 31, 71.2, "Smith" },
    { "profession", "A", 197, 0, 311, 712, 1426, 31.1, 71.2, "Tailor" },
    { "flight", "AH", 0, 0, 320, 712, 1426, 32, 71.2, "Flight Master" },
    { "inn", "A", 0, 0, 330, 712, 1426, 33, 71.2, "Innkeeper" },
    { "stable", "A", 0, 0, 340, 712, 1426, 34, 71.2, "Stable Master" },
    { "class", "A", 8, 0, 900, 900, 1426, 90, 90, "Far Mage Trainer" },
}
