-- A 3D chevron turning with the character (`arrowShown`, next to the compass): towards the way
-- the guide planned while there is one to follow (Pathing:Aim), else towards the place it points
-- at; the distance to that place and its name below, a tick on arrival. The texture holds 64
-- frames of the chevron turned counter-clockwise (8 x 8, frame 0 straight ahead). Dragged
-- anywhere unless frames are locked; its own size (`arrowScale`, Ctrl + mouse wheel over it) on
-- top of the windows'; in the theme's color, or green to red as the way turns behind
-- (`arrowColor`). Without a position (instances) or on another continent it only says so.

local FG = Factoruide
local T = FG.T

local Arrow = {}
FG.Arrow = Arrow

local L = {
    elsewhere = T("elsewhere"),
    yards = T("%d yd"),
    title = T("Arrow"),
    move = T("Drag: move it"),
    size = T("Ctrl + mouse wheel: its size"),
    locked = T("Locked: unlock the windows in /fg to move it"),
    commands = T("/fg: hide it, or put it back in its place"),
    reset = T("arrow back in its place, at its first size"),
}

-- Amber on the parchment theme, blue otherwise; white, tinted, when it shows the direction.
local MEDIA = "Interface\\AddOns\\Factoruide_Guide\\Media\\"
local TEXTURE = MEDIA .. (FG.Skin.theme == "parchemin" and "ChevronParchment" or "Chevron")
local WHITE = MEDIA .. "ChevronWhite"
local FRAMES, COLS = 64, 8
local SIZE = 64
local ARRIVED = 8 -- yards
local TWO_PI = 2 * math.pi
local SCALE_MIN, SCALE_MAX = FG.bounds.arrowScale[1], FG.bounds.arrowScale[2]
local PLACE = { "TOP", "UIParent", "TOP", 0, -70 }

local Skin = FG.Skin:World()
local frame, target

--- The frame of the sprite sheet showing a turn of `angle` (radians counter-clockwise, 0 =
--- straight ahead), and its texture coordinates: left, right, top, bottom.
function Arrow.Frame(angle)
    local f = math.floor((angle % TWO_PI) / TWO_PI * FRAMES + 0.5) % FRAMES
    local row, col = math.floor(f / COLS), f % COLS
    return f, col / COLS, (col + 1) / COLS, row / COLS, (row + 1) / COLS
end

--- The color of the arrow turned by `angle` (as `Arrow.Frame`) when it shows the direction:
--- green straight ahead, yellow at a right angle, red behind.
function Arrow.Color(angle)
    local off = math.abs(FG.Compass.Left(angle)) / 180
    if off < 0.5 then return off * 2, 1, 0 end
    return 1, 2 - off * 2, 0
end

local function Scale() return FG:Get("scale") * FG:Get("arrowScale") end

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
    local angle = FG.Compass.Angle(an - pn, aw - pw, facing)
    local _, left, right, top, bottom = Arrow.Frame(angle)
    self.arrow:SetTexCoord(left, right, top, bottom)
    if FG:Get("arrowColor") == "direction" then self.arrow:SetVertexColor(Arrow.Color(angle)) end
    self.arrow:Show()
end

local function SavePoint(self)
    local p, _, rp, x, y = self:GetPoint()
    if FG.Guide and FG.Guide:State() then FG.Guide:State().arrowPoint = { p, "UIParent", rp, x, y } end
end

--- The theme's color, or a white chevron tinted by the direction (`Arrow.Color`).
local function Paint()
    local direction = FG:Get("arrowColor") == "direction"
    frame.arrow:SetTexture(direction and WHITE or TEXTURE)
    if not direction then frame.arrow:SetVertexColor(1, 1, 1) end
end

--- At its size, its middle where it was on the screen.
local function Resize()
    local x, y = frame:GetCenter()
    local before = frame:GetEffectiveScale()
    frame:SetScale(Scale())
    if not x or not y or not before then return end
    local k = before / frame:GetEffectiveScale()
    frame:ClearAllPoints()
    frame:SetPoint("CENTER", UIParent, "BOTTOMLEFT", x * k, y * k)
    SavePoint(frame)
end

-- A hint on the arrow itself: how to move, size and hide it.
local function ShowHint(self)
    if InCombatLockdown() then return end
    GameTooltip:SetOwner(self, "ANCHOR_BOTTOM")
    GameTooltip:AddLine(L.title, 1, 1, 1)
    GameTooltip:AddLine(FG:Get("lockFrames") and L.locked or L.move, 0.73, 0.77, 0.84, true)
    GameTooltip:AddLine(L.size, 0.73, 0.77, 0.84, true)
    GameTooltip:AddLine(L.commands, 0.6, 0.6, 0.6, true)
    GameTooltip:Show()
end

local function Create()
    if frame or not CreateFrame then return frame end
    frame = CreateFrame("Frame", "FactoruideArrow", FG.UIRoot)
    frame:SetSize(SIZE, SIZE)
    frame:SetClampedToScreen(true)
    FG:Draggable(frame, SavePoint)
    local point = FG.Guide and FG.Guide:State() and FG.Guide:State().arrowPoint
    frame:SetPoint(unpack(point or PLACE))

    frame.arrow = frame:CreateTexture(nil, "ARTWORK")
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
    frame:SetScript("OnEnter", ShowHint)
    frame:SetScript("OnLeave", function() GameTooltip:Hide() end)
    -- Ctrl + mouse wheel: its own size, 5% a notch.
    FG:Zoomable(frame, function(delta)
        local v = math.floor((FG:Get("arrowScale") + 0.05 * delta) * 100 + 0.5) / 100
        FG:Save("arrowScale", math.max(SCALE_MIN, math.min(SCALE_MAX, v)))
    end)
    frame:SetScale(Scale())
    FG:OnSetting("scale", Resize)
    FG:OnSetting("arrowScale", Resize)
    Paint()
    FG:OnSetting("arrowColor", Paint)
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

--- Back under the compass, at its first size.
function Arrow:ResetPlace()
    -- The size first: resized, it keeps its middle where it is (saved), forgotten just below.
    FG:Save("arrowScale", nil)
    local state = FG.Guide and FG.Guide:State()
    if state then state.arrowPoint = nil end
    if frame then
        frame:ClearAllPoints()
        frame:SetPoint(unpack(PLACE))
        frame:SetScale(Scale())
    end
    FG:Print(L.reset)
end

FG.Options:AddButton("arrowShown", {
    label = "Arrow back in its place",
    text = "Put back",
    help = "Under the compass, at its first size.",
    run = function() Arrow:ResetPlace() end,
})
