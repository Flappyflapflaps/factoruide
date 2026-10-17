-- Places that serve the character, for the compass (FG.Services, written by the app in
-- Services.lua): the trainer of its class, those of its professions (learned, or planned by the
-- guide), flight masters, innkeepers, repairers, food and drink, bankers, auctioneers, stable
-- masters (hunters) and mailboxes. Each kind is shown when its setting is on; the closest of each
-- closer than `compassRange` yards.

local FG = Factoruide
local T = FG.T

local Nearby = {}
FG.Nearby = Nearby

Nearby.KINDS = {
    class = { setting = "compassClass", icon = "Interface\\Icons\\INV_Misc_Book_09", label = T("Class trainer") },
    profession = {
        setting = "compassProfession",
        icon = "Interface\\GossipFrame\\TrainerGossipIcon",
        label = T("Profession trainer"),
    },
    flight = { setting = "compassFlight", icon = "Interface\\GossipFrame\\TaxiGossipIcon", label = T("Flight master") },
    inn = { setting = "compassInn", icon = "Interface\\GossipFrame\\BinderGossipIcon", label = T("Innkeeper") },
    repair = { setting = "compassRepair", icon = "Interface\\GossipFrame\\VendorGossipIcon", label = T("Repairs") },
    food = { setting = "compassFood", icon = "Interface\\Icons\\INV_Drink_04", label = T("Food and drink") },
    bank = { setting = "compassBank", icon = "Interface\\GossipFrame\\BankerGossipIcon", label = T("Banker") },
    auction = { setting = "compassAuction", icon = "Interface\\Icons\\INV_Misc_Coin_02", label = T("Auctioneer") },
    stable = {
        setting = "compassStable",
        icon = "Interface\\Icons\\Ability_Hunter_BeastTaming",
        label = T("Stable master"),
    },
    mail = { setting = "compassMail", icon = "Interface\\Icons\\INV_Letter_15", label = T("Mailbox") },
}

-- Professions by skill line: their icon.
local PROFESSIONS = {
    [171] = "Interface\\Icons\\Trade_Alchemy",
    [164] = "Interface\\Icons\\Trade_BlackSmithing",
    [185] = "Interface\\Icons\\INV_Misc_Food_15",
    [333] = "Interface\\Icons\\Trade_Engraving",
    [202] = "Interface\\Icons\\Trade_Engineering",
    [129] = "Interface\\Icons\\Spell_Holy_SealOfSacrifice",
    [356] = "Interface\\Icons\\Trade_Fishing",
    [182] = "Interface\\Icons\\Trade_Herbalism",
    [165] = "Interface\\Icons\\INV_Misc_ArmorKit_17",
    [186] = "Interface\\Icons\\Trade_Mining",
    [393] = "Interface\\Icons\\INV_Misc_Pelt_Wolf_01",
    [197] = "Interface\\Icons\\Trade_Tailoring",
    [755] = "Interface\\Icons\\INV_Misc_Gem_01",
}

local HUNTER = 3
-- The closest few of each kind; looked for again after this many seconds or yards walked.
local PER_KIND = 2
local AGAIN, MOVED = 2, 25

--- The professions whose trainers are shown: learned, or planned by the guide.
local function Professions()
    local lines = {}
    for line in pairs(PROFESSIONS) do
        if FG.ProfessionSkill(line) then lines[line] = true end
    end
    local route = FG.Guide and FG.Guide:Route()
    for _, p in ipairs(route and route.professions or {}) do
        if p.line then lines[p.line] = true end
    end
    return lines
end

--- Whether the character is served at place `p` (an entry of FG.Services) and wants it shown.
local function Wanted(p, faction, class, lines)
    local kind = Nearby.KINDS[p[1]]
    if not kind or not FG:Get(kind.setting) or not p[2]:find(faction, 1, true) then return false end
    if p[1] == "class" then return p[3] == class end
    if p[1] == "profession" then return lines[p[3]] == true end
    if p[1] == "stable" then return class == HUNTER end
    return true
end

local found, foundAt, foundN, foundW, foundContinent = {}, nil, nil, nil, nil

--- The places to show around a character at (`north`, `west`) of `continent`: { continent,
--- north, west, m, x, y (percent), name, sub, icon, service, place = true }, closest first.
function Nearby:Around(continent, north, west)
    local now = GetTime()
    if
        foundAt
        and now - foundAt < AGAIN
        and foundContinent == continent
        and (north - foundN) ^ 2 + (west - foundW) ^ 2 < MOVED * MOVED
    then
        return found
    end
    foundAt, foundN, foundW, foundContinent = now, north, west, continent
    found = {}
    local faction = UnitFactionGroup("player") == "Horde" and "H" or "A"
    local class = select(3, UnitClass("player"))
    local range = FG:Get("compassRange") or 300
    local lines = Professions()
    local close = {}
    for _, p in ipairs(FG.Services or {}) do
        if p[4] == continent then
            local d2 = (p[5] - north) ^ 2 + (p[6] - west) ^ 2
            if d2 <= range * range and Wanted(p, faction, class, lines) then
                table.insert(close, { p = p, d = math.sqrt(d2) })
            end
        end
    end
    table.sort(close, function(a, b) return a.d < b.d end)
    local count = {}
    for _, c in ipairs(close) do
        local p = c.p
        local key = p[1] .. ":" .. p[3]
        count[key] = (count[key] or 0) + 1
        if count[key] <= PER_KIND then
            local kind = Nearby.KINDS[p[1]]
            table.insert(found, {
                continent = p[4],
                north = p[5],
                west = p[6],
                m = p[7],
                x = p[8],
                y = p[9],
                name = p[10],
                sub = kind.label,
                icon = p[1] == "profession" and PROFESSIONS[p[3]] or kind.icon,
                service = p[1],
                npc = p[11] ~= 0 and p[11] or nil,
                place = true,
            })
        end
    end
    return found
end

--- Look again at the next call (a setting changed).
function Nearby:Forget() foundAt = nil end

for _, kind in pairs(Nearby.KINDS) do
    FG:OnSetting(kind.setting, function() Nearby:Forget() end)
end
FG:OnSetting("compassRange", function() Nearby:Forget() end)
