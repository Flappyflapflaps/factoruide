-- A mark above the head of the NPC the guide points at (the NPC of the step, or a place picked
-- on the compass) while its nameplate is on screen: exactly where it stands, close by, when the
-- compass only knows where it was seen. Friendly NPCs have nameplates when the game's options
-- show them. Not where the game hides who a unit is (secret GUIDs in instances).

local FG = Factoruide

local NpcMark = {}
FG.NpcMark = NpcMark

-- Over the game world: the world's colors (see Skin:World).
local Skin = FG.Skin:World()
-- Nameplate units looked at when the NPC changes.
local PLATES = 40

local npc -- the NPC marked
local mark -- the mark, hung on `unit`'s nameplate
local unit

--- The NPC ID of a unit, nil when it is not an NPC or the game hides it.
local function NpcOf(u)
    local kind, id = FG.ParseGUID(UnitGUID(u))
    return kind == "npc" and id or nil
end

local function Create()
    if mark or not CreateFrame then return mark end
    mark = CreateFrame("Frame", "FactoruideNpcMark", FG.UIRoot)
    mark:SetSize(22, 30)
    mark.glow = Skin:Fill(mark, "BACKGROUND", "cyan", 0.35)
    mark.glow:SetSize(20, 20)
    mark.glow:SetPoint("BOTTOM", 0, 4)
    mark.glow:SetRotation(math.pi / 4)
    mark.diamond = mark:CreateTexture(nil, "ARTWORK")
    mark.diamond:SetSize(13, 13)
    mark.diamond:SetPoint("CENTER", mark.glow)
    Skin:AccentLine(mark.diamond)
    mark.diamond:SetRotation(math.pi / 4)
    mark.core = Skin:Fill(mark, "OVERLAY", "text", 0.9)
    mark.core:SetSize(4, 4)
    mark.core:SetPoint("CENTER", mark.glow)
    mark.core:SetRotation(math.pi / 4)
    -- A slow bob, the glow breathing.
    mark:SetScript("OnUpdate", function(self)
        local t = GetTime()
        self.glow:ClearAllPoints()
        self.glow:SetPoint("BOTTOM", 0, 4 + 3 * math.sin(t * 2.5))
        self.glow:SetAlpha(0.3 + 0.15 * math.sin(t * 3))
    end)
    mark:Hide()
    return mark
end

local function Hang(u)
    local plate = C_NamePlate and C_NamePlate.GetNamePlateForUnit and C_NamePlate.GetNamePlateForUnit(u)
    if not plate or not Create() then return false end
    unit = u
    mark:ClearAllPoints()
    mark:SetPoint("BOTTOM", plate, "TOP", 0, 2)
    mark:Show()
    return true
end

local function Unhang()
    unit = nil
    if mark then mark:Hide() end
end

--- Mark the NPC `id` (nil: none) when its nameplate shows (not on nameplate unit `skip`).
function NpcMark:SetNpc(id, skip)
    if id == npc and (unit or not id) then return end
    npc = id
    Unhang()
    if not id or not UnitExists then return end
    for i = 1, PLATES do
        local u = "nameplate" .. i
        if u ~= skip and UnitExists(u) and NpcOf(u) == id and Hang(u) then return end
    end
end

--- The nameplate unit the mark hangs on (nil: not shown).
function NpcMark:Unit() return unit end

FG:On("NAME_PLATE_UNIT_ADDED", function(u)
    if npc and not unit and NpcOf(u) == npc then Hang(u) end
end)

FG:On("NAME_PLATE_UNIT_REMOVED", function(u)
    if u == unit then
        Unhang()
        -- Another of its nameplates (the same NPC twice in sight).
        NpcMark:SetNpc(npc, u)
    end
end)
