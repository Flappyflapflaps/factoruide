-- A 3D chevron turning with the character (`arrowShown`, next to the compass): towards the way
-- the guide planned while there is one to follow (Pathing:Aim), else towards the place it points
-- at; the distance to that place and its name below, a tick on arrival. The texture holds 64
-- frames of the chevron turned counter-clockwise (8 x 8, frame 0 straight ahead). Dragged
-- anywhere unless frames are locked. Without a position (instances) or on another continent it
-- only says so.

local FG = Factoruide
local T = FG.T

local Arrow = {}
FG.Arrow = Arrow

local L = {
    elsewhere = T("elsewhere"),
    yards = T("%d yd"),
}

-- Amber on the parchment theme, blue otherwise.
local TEXTURE = "Interface\\AddOns\\Factoruide_Guide\\Media\\"
    .. (FG.Skin.theme == "parchemin" and "ChevronParchment" or "Chevron")
local FRAMES, COLS = 64, 8
local SIZE = 64
local ARRIVED = 8 -- yards
local TWO_PI = 2 * math.pi

local Skin = FG.Skin:World()
local frame, target

--- The frame of the sprite sheet showing a turn of `angle` (radians counter-clockwise, 0 =
--- straight ahead), and its texture coordinates: left, right, top, bottom.
function Arrow.Frame(angle)
    local f = math.floor((angle % TWO_PI) / TWO_PI * FRAMES + 0.5) % FRAMES
    local row, col = math.floor(f / COLS), f % COLS
    return f, col / COLS, (col + 1) / COLS, row / COLS, (row + 1) / COLS
end

local function Update(self, elapsed)
    self.wait = (self.wait or 0) - elapsed
    if self.wait > 0 then return end
    self.wait = 0.03
    local map = C_Map.GetBestMapForUnit("player")
    local pos = map and C_Map.GetPlayerMapPosition(map, "player")
    local continent, pn, pw = FG.Compass.World(map, pos and pos.x, pos and pos.y)
    local c2, tn, tw = FG.Compass.World(target.m, target.x / 100, target.y / 100)
    local facing = GetPlayerFacing and GetPlayerFacing()
    if not continent or continent ~= c2 or not facing then
        self.arrow:Hide()
        self.check:Hide()
        self.distance:SetText(L.elsewhere)
        return
    end
    local distance = math.sqrt((tn - pn) ^ 2 + (tw - pw) ^ 2)
    self.distance:SetText(string.format(L.yards, math.floor(distance + 0.5)))
    if distance < ARRIVED then
        self.arrow:Hide()
        self.check:Show()
        return
    end
    self.check:Hide()
    local wn, ww = nil, nil
    if FG.Pathing then
        wn, ww = FG.Pathing:Aim(continent, pn, pw)
    end
    local an, aw = wn or tn, ww or tw
    local _, left, right, top, bottom = Arrow.Frame(FG.Compass.Angle(an - pn, aw - pw, facing))
    self.arrow:SetTexCoord(left, right, top, bottom)
    self.arrow:Show()
end

local function Create()
    if frame or not CreateFrame then return frame end
    frame = CreateFrame("Frame", "FactoruideArrow", FG.UIRoot)
    frame:SetSize(SIZE, SIZE)
    frame:SetClampedToScreen(true)
    FG:Draggable(frame, function(self)
        local p, _, rp, x, y = self:GetPoint()
        if FG.Guide then FG.Guide:State().arrowPoint = { p, "UIParent", rp, x, y } end
    end)
    local point = FG.Guide and FG.Guide:State() and FG.Guide:State().arrowPoint
    frame:SetPoint(unpack(point or { "TOP", UIParent, "TOP", 0, -70 }))

    frame.arrow = frame:CreateTexture(nil, "ARTWORK")
    frame.arrow:SetTexture(TEXTURE)
    frame.arrow:SetAllPoints()
    frame.check = frame:CreateTexture(nil, "ARTWORK")
    frame.check:SetTexture("Interface\\RaidFrame\\ReadyCheck-Ready")
    frame.check:SetSize(36, 36)
    frame.check:SetPoint("CENTER")
    frame.check:Hide()
    frame.distance = Skin:Text(frame, 11, "lightBlue")
    frame.distance:SetPoint("TOP", frame, "BOTTOM", 0, 2)
    frame.title = Skin:Text(frame, 9, "text2")
    frame.title:SetPoint("TOP", frame.distance, "BOTTOM", 0, -1)
    frame.title:SetWidth(220)
    frame.title:SetWordWrap(false)
    -- Centered under the arrow (the skin's texts are left-aligned).
    frame.distance:SetJustifyH("CENTER")
    frame.title:SetJustifyH("CENTER")

    frame:SetScript("OnUpdate", function(self, elapsed)
        local ok, err = pcall(Update, self, elapsed)
        if not ok then
            FG:Error("arrow", err)
            self.wait = 5
        end
    end)
    frame:SetScale(FG:Get("scale"))
    FG:OnSetting("scale", function(v) frame:SetScale(v) end)
    frame:Hide()
    return frame
end

--- Point the arrow at a place ({ m, x, y (percent) }) called `name`; nil hides it.
function Arrow:SetTarget(place, name)
    target = place and place.m and place.x and place.y and place or nil
    local f = Create()
    if not f then return end
    if not target or not FG:Get("arrowShown") then
        f:Hide()
        return
    end
    f.title:SetText(name or "")
    f.wait = 0
    f:Show()
end

--- The place pointed at (nil: hidden).
function Arrow:Target() return frame and frame:IsShown() and target or nil end
