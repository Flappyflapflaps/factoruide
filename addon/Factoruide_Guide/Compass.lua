-- Compass bar at the top of the screen: what is ahead in the middle, 90° each side to the ends,
-- the cardinal points. The guide's target is a blue diamond with its distance; the other current
-- tasks, the close quests to pick up and the next turn-in are small icons (close ones grouped
-- with "+n", those behind at the ends). Mouse over a point: what it is and how far; click: the
-- guide points at it. Over the minimal guide's strip, else at the top of the screen (dragged
-- elsewhere unless frames are locked). Without a position (instances) or on another continent it
-- only says so.

local FG = Factoruide
local T = FG.T

local Compass = {}
FG.Compass = Compass

local L = {
    elsewhere = T("elsewhere"),
    here = T("here"),
    yards = T("%d yd"),
}
-- Every 45°, counter-clockwise from north (the game's facing).
local CARDINALS = { T("N"), T("NW"), T("W"), T("SW"), T("S"), T("SE"), T("E"), T("NE") }

local WIDTH, HEIGHT = 420, 24
local FIELD = 90 -- degrees from the middle to each end
local PPD = WIDTH / (2 * FIELD)
local FADE = WIDTH * 0.12 -- the ends fade out over this many pixels
local GROUP = 18 -- points closer than this on the bar make one
local ARRIVED = 8 -- yards
local TWO_PI = 2 * math.pi
local ICON = 11

local Skin = FG.Skin
local frame
local points = {}
local attached

--- Turn towards a point `dn` yards north and `dw` yards west of a character facing `facing` (the
--- game's facing: radians counter-clockwise from north), counter-clockwise, 0 = straight ahead.
function Compass.Angle(dn, dw, facing) return (math.atan2(dw, dn) - facing) % TWO_PI end

--- World position (continent, north, west) of a map point.
function Compass.World(map, x, y)
    if not map or not x or not C_Map.GetWorldPosFromMapPos then return nil end
    local continent, pos = C_Map.GetWorldPosFromMapPos(map, CreateVector2D(x, y))
    if not continent or not pos then return nil end
    local north, west = pos:GetXY()
    return continent, north, west
end

--- Signed degrees to the left of straight ahead (-180 to 180) of a turn from `Compass.Angle`.
function Compass.Left(angle)
    local d = math.deg(angle)
    if d > 180 then d = d - 360 end
    return d
end

local function Alpha(x) return math.max(0.15, math.min(1, math.min(x, WIDTH - x) / FADE)) end

-- Drawing ---------------------------------------------------------------------------

local function Pool(list, make)
    return function(i)
        local o = list[i]
        if not o then
            o = make()
            list[i] = o
        end
        return o
    end
end

local ticks, labels, markers = {}, {}, {}
local Tick, Label, Marker

local function ShowTooltip(self)
    if not self.group then return end
    GameTooltip:SetOwner(self, "ANCHOR_BOTTOM")
    for i, p in ipairs(self.group) do
        GameTooltip:AddLine(p.name or "?", 1, 1, 1, true)
        if p.sub then GameTooltip:AddLine(p.sub, 0.73, 0.77, 0.84, true) end
        if p.distance then
            GameTooltip:AddLine(string.format(L.yards, math.floor(p.distance + 0.5)), 0.5, 0.83, 0.94)
        end
        if i < #self.group then GameTooltip:AddLine(" ") end
    end
    GameTooltip:AddLine(FG.Guide.L.pointHint, 0.6, 0.6, 0.6, true)
    GameTooltip:Show()
end

local function MakeMarker()
    local m = CreateFrame("Button", nil, frame)
    m:SetSize(16, 16)
    m:SetScript("OnEnter", ShowTooltip)
    m:SetScript("OnLeave", function() GameTooltip:Hide() end)
    m:SetScript("OnClick", function(self)
        if self.index then FG.Guide:PointToStep(self.index) end
    end)
    -- The target: a glowing blue diamond.
    m.glow = Skin:Fill(m, "BACKGROUND", "cyan", 0.35)
    m.glow:SetSize(13, 13)
    m.glow:SetPoint("CENTER")
    m.glow:SetRotation(math.pi / 4)
    m.diamond = m:CreateTexture(nil, "ARTWORK")
    m.diamond:SetSize(9, 9)
    m.diamond:SetPoint("CENTER")
    Skin:AccentLine(m.diamond)
    m.diamond:SetRotation(math.pi / 4)
    -- The others: their icon on a black square.
    m.back = Skin:Fill(m, "BACKGROUND", { 0, 0, 0 }, 0.85)
    m.back:SetSize(ICON + 2, ICON + 2)
    m.back:SetPoint("CENTER")
    m.icon = m:CreateTexture(nil, "ARTWORK")
    m.icon:SetSize(ICON, ICON)
    m.icon:SetPoint("CENTER")
    m.icon:SetTexCoord(0.08, 0.92, 0.08, 0.92)
    m.distance = Skin:Text(m, 9, "lightBlue")
    m.distance:SetPoint("LEFT", m, "RIGHT", 1, 0)
    m.badge = Skin:Fill(m, "OVERLAY", "blue", 1)
    m.badge:SetSize(10, 8)
    m.badge:SetPoint("BOTTOMLEFT", m, "TOPRIGHT", -6, -5)
    m.more = Skin:Text(m, 7, "text", "OVERLAY")
    m.more:SetPoint("CENTER", m.badge)
    return m
end

local function Create()
    if frame or not CreateFrame then return frame end
    frame = CreateFrame("Frame", "FactoruideCompass", UIParent)
    frame:SetSize(WIDTH, HEIGHT)
    frame:SetClampedToScreen(true)
    frame:SetMovable(true)
    frame:EnableMouse(true)
    frame:RegisterForDrag("LeftButton")
    frame:SetScript("OnDragStart", function(self)
        if not attached and not FG:Get("lockFrames") then self:StartMoving() end
    end)
    frame:SetScript("OnDragStop", function(self)
        self:StopMovingOrSizing()
        local p, _, rp, x, y = self:GetPoint()
        if FG.Guide then FG.Guide:State().compassPoint = { p, "UIParent", rp, x, y } end
    end)

    -- A light veil, fading out at both ends, and a thin line at the bottom.
    local c = Skin.colors.bg
    local left = frame:CreateTexture(nil, "BACKGROUND")
    left:SetPoint("TOPLEFT")
    left:SetPoint("BOTTOMLEFT")
    left:SetWidth(FADE * 1.5)
    Skin:Gradient(left, c, 0, 0.42)
    local right = frame:CreateTexture(nil, "BACKGROUND")
    right:SetPoint("TOPRIGHT")
    right:SetPoint("BOTTOMRIGHT")
    right:SetWidth(FADE * 1.5)
    Skin:Gradient(right, c, 0.42, 0)
    local middle = Skin:Fill(frame, "BACKGROUND", c, 0.42)
    middle:SetPoint("TOPLEFT", left, "TOPRIGHT")
    middle:SetPoint("BOTTOMRIGHT", right, "BOTTOMLEFT")
    local a = Skin.colors.accent
    local lineLeft = frame:CreateTexture(nil, "BORDER")
    lineLeft:SetPoint("BOTTOMLEFT")
    lineLeft:SetSize(FADE * 1.5, 1)
    Skin:Gradient(lineLeft, a, 0, 0.45)
    local lineRight = frame:CreateTexture(nil, "BORDER")
    lineRight:SetPoint("BOTTOMRIGHT")
    lineRight:SetSize(FADE * 1.5, 1)
    Skin:Gradient(lineRight, a, 0.45, 0)
    local line = Skin:Fill(frame, "BORDER", a, 0.45)
    line:SetHeight(1)
    line:SetPoint("BOTTOMLEFT", lineLeft, "BOTTOMRIGHT")
    line:SetPoint("BOTTOMRIGHT", lineRight, "BOTTOMLEFT")
    -- Straight ahead.
    frame.caret = Skin:Fill(frame, "OVERLAY", "text", 0.8)
    frame.caret:SetSize(5, 5)
    frame.caret:SetPoint("CENTER", frame, "BOTTOM", 0, 0)
    frame.caret:SetRotation(math.pi / 4)

    frame.elsewhere = Skin:Text(frame, 9, "muted")
    frame.elsewhere:SetPoint("CENTER", 0, 2)
    frame.elsewhere:SetText(L.elsewhere)

    -- Points behind: an arrow and their icons at each end.
    frame.ends = {}
    for _, side in ipairs({ "left", "right" }) do
        local e = CreateFrame("Frame", nil, frame)
        e:SetSize(60, 14)
        e.arrow = e:CreateTexture(nil, "ARTWORK")
        e.arrow:SetTexture("Interface\\ChatFrame\\ChatFrameExpandArrow")
        e.arrow:SetSize(10, 10)
        e.arrow:SetVertexColor(0.5, 0.83, 0.94)
        e.icons = {}
        for i = 1, 3 do
            local icon = e:CreateTexture(nil, "ARTWORK")
            icon:SetSize(9, 9)
            icon:SetTexCoord(0.08, 0.92, 0.08, 0.92)
            icon:SetAlpha(0.8)
            e.icons[i] = icon
        end
        if side == "left" then
            e:SetPoint("LEFT", FADE * 0.6, 1)
            e.arrow:SetTexCoord(1, 0, 0, 1) -- pointing left
            e.arrow:SetPoint("LEFT")
            for i, icon in ipairs(e.icons) do
                icon:SetPoint("LEFT", 10 + (i - 1) * 11, 0)
            end
        else
            e:SetPoint("RIGHT", -FADE * 0.6, 1)
            e.arrow:SetPoint("RIGHT")
            for i, icon in ipairs(e.icons) do
                icon:SetPoint("RIGHT", -10 - (i - 1) * 11, 0)
            end
        end
        frame.ends[side] = e
    end

    Tick = Pool(ticks, function()
        local t = Skin:Fill(frame, "ARTWORK", "text", 0.22)
        t:SetWidth(1)
        return t
    end)
    Label = Pool(labels, function()
        local fs = Skin:Text(frame, 8, "text2")
        fs:SetJustifyH("CENTER")
        return fs
    end)
    Marker = Pool(markers, MakeMarker)

    frame:SetScript("OnUpdate", function(self, elapsed)
        self.wait = (self.wait or 0) - elapsed
        if self.wait > 0 then return end
        self.wait = 0.03
        -- Errors counted (once a frame otherwise), the compass stops drawing until shown again.
        local ok, err = pcall(Compass.Draw, Compass)
        if not ok then
            FG:Error("compass", err)
            self.wait = 5
        end
    end)
    frame:SetScale(FG:Get("scale"))
    FG:OnSetting("scale", function(v) frame:SetScale(v) end)
    frame:Hide()
    return frame
end

local function HideFrom(list, from)
    for i = from, #list do
        list[i]:Hide()
    end
end

local seen, groups = {}, {}
local behind = { left = {}, right = {} }

--- Place the points for the character's position and facing.
function Compass:Draw()
    if not frame then return end
    local map = C_Map.GetBestMapForUnit("player")
    local pos = map and C_Map.GetPlayerMapPosition(map, "player")
    local continent, pn, pw = Compass.World(map, pos and pos.x, pos and pos.y)
    local facing = GetPlayerFacing and GetPlayerFacing()
    wipe(seen)
    wipe(behind.left)
    wipe(behind.right)
    local any = false
    if continent and facing then
        for _, p in ipairs(points) do
            if p.continent == continent then
                local dn, dw = p.north - pn, p.west - pw
                p.distance = math.sqrt(dn * dn + dw * dw)
                p.left = Compass.Left(Compass.Angle(dn, dw, facing))
                any = true
                if math.abs(p.left) <= FIELD - 4 then
                    p.bar = WIDTH / 2 - p.left * PPD
                    table.insert(seen, p)
                else
                    table.insert(p.left > 0 and behind.left or behind.right, p)
                end
            else
                p.distance = nil
            end
        end
    end
    frame.elsewhere:SetShown(not any)
    if not any then
        HideFrom(ticks, 1)
        HideFrom(labels, 1)
        HideFrom(markers, 1)
        frame.caret:Hide()
        for _, e in pairs(frame.ends) do
            e:Hide()
        end
        return
    end
    frame.caret:Show()

    -- Ticks every 5° (taller every 15°), the cardinal points every 45° unless a point is there.
    table.sort(seen, function(a, b) return a.bar < b.bar end)
    wipe(groups)
    for _, p in ipairs(seen) do
        local g = groups[#groups]
        if g and p.bar - g[#g].bar < GROUP then
            table.insert(g, p)
        else
            table.insert(groups, { p })
        end
    end
    local heading = math.deg(facing)
    local nt, nl = 0, 0
    for d = math.ceil((heading - FIELD) / 5) * 5, heading + FIELD, 5 do
        local x = WIDTH / 2 - (d - heading) * PPD
        local norm = d % 360
        if norm % 45 == 0 then
            local free = true
            for _, g in ipairs(groups) do
                local gx = g[1].bar
                local main = false
                for _, p in ipairs(g) do
                    if p.main then main = true end
                end
                if x > gx - 14 and x < gx + (main and 38 or 14) then free = false end
            end
            if free then
                nl = nl + 1
                local fs = Label(nl)
                fs:ClearAllPoints()
                fs:SetPoint("TOP", frame, "TOPLEFT", x, -2)
                fs:SetText(CARDINALS[norm / 45 + 1])
                local gold = norm == 0
                fs:SetTextColor(gold and 1 or 0.73, gold and 0.82 or 0.77, gold and 0 or 0.84)
                fs:SetAlpha(Alpha(x) * 0.85)
                fs:Show()
            end
        else
            nt = nt + 1
            local t = Tick(nt)
            t:ClearAllPoints()
            t:SetPoint("BOTTOM", frame, "BOTTOMLEFT", x, 0)
            t:SetHeight(norm % 15 == 0 and 7 or 3)
            t:SetAlpha(Alpha(x))
            t:Show()
        end
    end
    HideFrom(ticks, nt + 1)
    HideFrom(labels, nl + 1)

    -- One marker a group: the target first, else the closest.
    for i, g in ipairs(groups) do
        local top = g[1]
        local x = 0
        for _, p in ipairs(g) do
            x = x + p.bar
            if p.main then
                top = p
            elseif not top.main and (p.distance or 0) < (top.distance or 0) then
                top = p
            end
        end
        x = x / #g
        local m = Marker(i)
        m.group, m.index = g, top.index
        m:ClearAllPoints()
        m:SetPoint("CENTER", frame, "TOPLEFT", x, -HEIGHT / 2 + 1)
        local main = top.main == true
        m.glow:SetShown(main)
        m.diamond:SetShown(main)
        m.back:SetShown(not main)
        m.icon:SetShown(not main)
        if not main then m.icon:SetTexture(top.icon) end
        m.distance:SetShown(main)
        if main then
            local arrived = top.distance < ARRIVED
            m.distance:SetText(arrived and L.here or string.format(L.yards, math.floor(top.distance + 0.5)))
            local c = Skin.colors[arrived and "green" or "lightBlue"]
            m.distance:SetTextColor(c[1], c[2], c[3])
        end
        m.badge:SetShown(#g > 1)
        m.more:SetShown(#g > 1)
        m.more:SetText("+" .. (#g - 1))
        -- Far ones paler, the ends faded.
        m:SetAlpha(Alpha(x) * ((main or (top.distance or 0) < 180) and 1 or 0.6))
        m:Show()
    end
    HideFrom(markers, #groups + 1)

    for side, list in pairs(behind) do
        local e = frame.ends[side]
        e:SetShown(#list > 0)
        for i, icon in ipairs(e.icons) do
            local p = list[i]
            icon:SetShown(p ~= nil)
            if p then
                if p.main then
                    icon:SetTexture(Skin.WHITE)
                    icon:SetVertexColor(0.122, 0.659, 0.847)
                else
                    icon:SetTexture(p.icon)
                    icon:SetVertexColor(1, 1, 1)
                end
            end
        end
    end
end

--- The points to show: { m, x, y (percent), index, main, kind, icon, name, sub }; nil hides
--- the compass.
function Compass:SetPoints(list)
    local f = Create()
    if not f then return end
    wipe(points)
    for _, p in ipairs(list or {}) do
        local continent, north, west = Compass.World(p.m, p.x / 100, p.y / 100)
        if continent then
            p.continent, p.north, p.west = continent, north, west
            table.insert(points, p)
        end
    end
    local shown = list ~= nil and #list > 0
    f:SetShown(shown)
    if shown then
        f.wait = 0
        self:Draw()
    end
end

--- The points shown (see `Compass:SetPoints`).
function Compass:Points() return points end

--- Over the minimal guide's strip.
function Compass:Attach(strip)
    local f = Create()
    if not f or attached == strip then return end
    attached = strip
    f:ClearAllPoints()
    f:SetPoint("BOTTOM", strip, "TOP", 0, 4)
end

--- At its own place (the full guide): where it was dragged, else the top of the screen.
function Compass:Free()
    local f = Create()
    if not f or attached == false then return end
    attached = false
    f:ClearAllPoints()
    local point = FG.Guide and FG.Guide:State() and FG.Guide:State().compassPoint
    f:SetPoint(unpack(point or { "TOP", UIParent, "TOP", 0, -14 }))
end
