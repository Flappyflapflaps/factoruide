-- Compass bar at the top of the screen: what is ahead in the middle, 90° each side to the ends,
-- the cardinal points. What stands out first:
--   the guide's target: a large glowing blue diamond on a beam down to the bar's line, with its
--   distance, never grouped with anything; behind, at the end it is closer to, with its
--   distance;
--   the way: where the walk the guide planned goes (Pathing.lua), a green mark (orange beside
--   the walk, going back to it), when it is not straight towards the target;
--   the other current tasks: their icon in a blue frame;
--   the close quests to pick up and the next turn-in: smaller icons, paler;
--   the places that serve the character (Nearby.lua): small faded icons.
-- Close ones are grouped with "+n", the highest kind on top; those behind at the ends (not the
-- places). Mouse over a point: what it is and how far; click: the guide points at it (and
-- Shortest Path Forever goes there when linked). Without a position (instances) or on another
-- continent it only says so.
-- Over the minimal guide's strip, else at the top of the screen. Dragged anywhere it moves
-- (unless the windows are locked), Ctrl + mouse wheel changes its size, its ends dragged its
-- width; kept for the character. The addon's options (`/fg`, Navigation) hide or show it, and put
-- it back at the top, at its first size.

local FG = Factoruide
local T = FG.T

local Compass = {}
FG.Compass = Compass

local L = {
    elsewhere = T("elsewhere"),
    here = T("here"),
    yards = T("%d yd"),
    placeHint = T("Left-click: the compass points at this place"),
    spfHint = T("Left-click: Shortest Path Forever goes there"),
    way = T("The planned way"),
    wayTip = T("Go this way: the guide's walk, around the steep ground"),
    back = T("Back to the planned way"),
    title = T("Compass"),
    move = T("Drag: move it"),
    size = T("Ctrl + mouse wheel: its size"),
    width = T("Drag an end: its width"),
    locked = T("Locked: unlock the windows in /fg to move it"),
    commands = T("/fg: hide it, or put it back at the top"),
    reset = T("compass back at the top of the screen, at its first size"),
}
-- Every 45°, counter-clockwise from north (the game's facing).
local CARDINALS = { T("N"), T("NW"), T("W"), T("SW"), T("S"), T("SE"), T("E"), T("NE") }

local HEIGHT = 24
-- Its width (dragged by its ends) and its size (a factor of the windows' size, Ctrl + wheel).
local WIDTH_DEFAULT, WIDTH_MIN, WIDTH_MAX = 420, 240, 1000
local SCALE_MIN, SCALE_MAX = 0.6, 2
local WIDTH = WIDTH_DEFAULT
local FIELD = 90 -- degrees from the middle to each end
local PPD = WIDTH / (2 * FIELD)
local FADE = WIDTH * 0.12 -- the ends fade out over this many pixels
local GRIP = 8 -- the ends dragged for the width
local GROUP = 18 -- points closer than this on the bar make one
local ARRIVED = 8 -- yards
local FAR = 180 -- yards: farther points paler
local SAME = 6 -- degrees: the way this close to the target is not shown
local TWO_PI = 2 * math.pi
-- What stands out first: the other current tasks, then the next steps, then the places.
local TIERS = { task = 3, next = 2, place = 1 }
local LOOKS = {
    task = { icon = 12, back = 16, alpha = 1, farAlpha = 1 },
    next = { icon = 10, back = 12, alpha = 0.8, farAlpha = 0.5 },
    place = { icon = 9, back = 11, alpha = 0.6, farAlpha = 0.45 },
}

-- Over the game world: the world's colors (see Skin:World), the line under it the cards' accent.
local Skin = FG.Skin:World()
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

local function Yards(d) return string.format(L.yards, math.floor(d + 0.5)) end

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

local function Hint()
    if FG.Pathing and FG.Pathing:SpfLinked() then return L.spfHint end
    return nil
end

local function ShowTooltip(self)
    if not self.group then return end
    GameTooltip:SetOwner(self, "ANCHOR_BOTTOM")
    for i, p in ipairs(self.group) do
        GameTooltip:AddLine(p.name or "?", 1, 1, 1, true)
        if p.sub then GameTooltip:AddLine(p.sub, 0.73, 0.77, 0.84, true) end
        if p.distance then GameTooltip:AddLine(Yards(p.distance), 0.5, 0.83, 0.94) end
        if i < #self.group then GameTooltip:AddLine(" ") end
    end
    local top = self.group[1]
    GameTooltip:AddLine(Hint() or (top.place and L.placeHint or FG.Guide.L.pointHint), 0.6, 0.6, 0.6, true)
    GameTooltip:Show()
end

local function Click(self)
    local p = self.top
    if not p then return end
    if FG.Pathing then FG.Pathing:Forget() end
    if p.place then
        FG.Guide:PointToPlace(p)
    elseif p.index then
        FG.Guide:PointToStep(p.index)
    end
end

local function MakeButton(size)
    local b = FG:DragHandle(CreateFrame("Button", nil, frame))
    b:SetSize(size, size)
    b:SetScript("OnEnter", ShowTooltip)
    b:SetScript("OnLeave", function() GameTooltip:Hide() end)
    b:SetScript("OnClick", Click)
    return b
end

-- The others: their icon on a square (blue for the current tasks), "+n" when grouped.
local function MakeMarker()
    local m = MakeButton(16)
    m.back = Skin:Fill(m, "BACKGROUND", { 0, 0, 0 }, 0.85)
    m.back:SetPoint("CENTER")
    m.icon = m:CreateTexture(nil, "ARTWORK")
    m.icon:SetPoint("CENTER")
    m.icon:SetTexCoord(0.08, 0.92, 0.08, 0.92)
    m.badge = Skin:Fill(m, "OVERLAY", "blue", 1)
    m.badge:SetSize(10, 8)
    m.badge:SetPoint("BOTTOMLEFT", m, "TOPRIGHT", -6, -5)
    m.more = Skin:Text(m, 7, "text", "OVERLAY")
    m.more:SetPoint("CENTER", m.badge)
    return m
end

-- The target: a large glowing blue diamond on a beam down to the line, its distance beside.
local function MakeTarget()
    local m = MakeButton(22)
    m:SetFrameLevel(frame:GetFrameLevel() + 6)
    m.beam = Skin:Fill(m, "BACKGROUND", "cyan", 0.7)
    m.beam:SetSize(2, HEIGHT / 2)
    m.beam:SetPoint("TOP", m, "CENTER", 0, -4)
    m.glow = Skin:Fill(m, "BACKGROUND", "cyan", 0.35)
    m.glow:SetSize(20, 20)
    m.glow:SetPoint("CENTER")
    m.glow:SetRotation(math.pi / 4)
    m.diamond = m:CreateTexture(nil, "ARTWORK")
    m.diamond:SetSize(12, 12)
    m.diamond:SetPoint("CENTER")
    Skin:AccentLine(m.diamond)
    m.diamond:SetRotation(math.pi / 4)
    m.core = Skin:Fill(m, "OVERLAY", "text", 0.9)
    m.core:SetSize(4, 4)
    m.core:SetPoint("CENTER")
    m.core:SetRotation(math.pi / 4)
    m.distance = Skin:Text(m, 10, "lightBlue")
    m.distance:SetPoint("LEFT", m, "RIGHT", -1, 0)
    return m
end

-- The way: a mark standing on the line, an arrow head on top.
local function MakeWay()
    local m = MakeButton(14)
    m:SetHeight(HEIGHT)
    m:SetFrameLevel(frame:GetFrameLevel() + 4)
    m.glow = Skin:Fill(m, "BACKGROUND", "green", 0.25)
    m.glow:SetSize(10, 18)
    m.glow:SetPoint("BOTTOM", 0, 0)
    m.bar = Skin:Fill(m, "ARTWORK", "green", 0.95)
    m.bar:SetSize(3, 12)
    m.bar:SetPoint("BOTTOM", 0, 1)
    m.head = Skin:Fill(m, "ARTWORK", "green", 0.95)
    m.head:SetSize(7, 7)
    m.head:SetPoint("CENTER", m.bar, "TOP", 0, 0)
    m.head:SetRotation(math.pi / 4)
    m:SetScript("OnEnter", function(self)
        GameTooltip:SetOwner(self, "ANCHOR_BOTTOM")
        GameTooltip:AddLine(self.back and L.back or L.way, 1, 1, 1, true)
        GameTooltip:AddLine(L.wayTip, 0.73, 0.77, 0.84, true)
        GameTooltip:Show()
    end)
    m:SetScript("OnClick", nil)
    return m
end

-- Its place, width and size, kept for the character and the guide's mode (nil before the login).
local function Saved() return FG.Guide and FG.Guide:Layout() end

local function SavePoint(self)
    local state = Saved()
    if not state then return end
    local p, _, rp, x, y = self:GetPoint()
    state.compassPoint = { p, "UIParent", rp, x, y }
end

--- Its width: the field spread over it, the faded ends.
local function Layout(width)
    WIDTH = math.floor(math.max(WIDTH_MIN, math.min(WIDTH_MAX, width)) + 0.5)
    PPD = WIDTH / (2 * FIELD)
    FADE = WIDTH * 0.12
    frame:SetWidth(WIDTH)
    for _, t in ipairs(frame.faded) do
        t:SetWidth(FADE * 1.5)
    end
    frame.ends.left:SetPoint("LEFT", FADE * 0.6, 1)
    frame.ends.right:SetPoint("RIGHT", -FADE * 0.6, 1)
end

local function Scale()
    local state = Saved()
    return FG:Get("scale") * (state and state.compassScale or 1)
end

--- Change it while its middle stays where it is on the screen (a free compass: anchored by its
--- middle from then on; over the strip it stays centered on it).
local function KeepMiddle(change)
    if attached then return change() end
    local x, y = frame:GetCenter()
    local before = frame:GetEffectiveScale()
    change()
    if not x or not y then return end
    local k = before / frame:GetEffectiveScale()
    frame:ClearAllPoints()
    frame:SetPoint("CENTER", UIParent, "BOTTOMLEFT", x * k, y * k)
    SavePoint(frame)
end

--- Ctrl + mouse wheel: its size, 5% a notch.
function Compass:Zoom(delta)
    local state = Saved()
    if not frame or not state then return end
    local v = math.floor(((state.compassScale or 1) + 0.05 * delta) * 100 + 0.5) / 100
    v = math.max(SCALE_MIN, math.min(SCALE_MAX, v))
    KeepMiddle(function()
        state.compassScale = v ~= 1 and v or nil
        frame:SetScale(Scale())
    end)
end

-- A hint on the bar itself: how to move, size and hide it.
local function ShowHint(self)
    if InCombatLockdown() then return end
    GameTooltip:SetOwner(self, "ANCHOR_BOTTOM")
    GameTooltip:AddLine(L.title, 1, 1, 1)
    if FG:Get("lockFrames") then
        GameTooltip:AddLine(L.locked, 0.73, 0.77, 0.84, true)
    else
        GameTooltip:AddLine(L.move, 0.73, 0.77, 0.84, true)
        GameTooltip:AddLine(L.width, 0.73, 0.77, 0.84, true)
    end
    GameTooltip:AddLine(L.size, 0.73, 0.77, 0.84, true)
    GameTooltip:AddLine(L.commands, 0.6, 0.6, 0.6, true)
    GameTooltip:Show()
end

local function HideHint() GameTooltip:Hide() end

-- The ends: dragged sideways, both ends move (its middle stays). Not while the windows are locked.
local function Sizing(self)
    local x = GetCursorPosition() / frame:GetEffectiveScale()
    Layout(self.from + self.sign * 2 * (x - self.startX))
end

local function SizeStop(self)
    if not self.sizing then return end
    self.sizing = nil
    self:SetScript("OnUpdate", nil)
    local state = Saved()
    if state then state.compassWidth = WIDTH ~= WIDTH_DEFAULT and WIDTH or nil end
end

local function SizeStart(self, button)
    if button ~= "LeftButton" or FG:Get("lockFrames") then return end
    KeepMiddle(function() end)
    self.sizing = true
    self.from = WIDTH
    self.startX = GetCursorPosition() / frame:GetEffectiveScale()
    self:SetScript("OnUpdate", Sizing)
end

local function MakeGrip(side)
    local g = CreateFrame("Frame", nil, frame)
    g:SetSize(GRIP, HEIGHT)
    g:SetPoint(side == "left" and "LEFT" or "RIGHT")
    g:SetFrameLevel(frame:GetFrameLevel() + 8)
    g:EnableMouse(true)
    g.sign = side == "left" and -1 or 1
    g.line = Skin:Fill(g, "OVERLAY", "accent", 0.8)
    g.line:SetSize(2, HEIGHT - 6)
    g.line:SetPoint("CENTER")
    g.line:Hide()
    g:SetScript("OnMouseDown", SizeStart)
    g:SetScript("OnMouseUp", SizeStop)
    g:SetScript("OnHide", SizeStop)
    g:SetScript("OnEnter", function(self)
        self.line:SetShown(not FG:Get("lockFrames"))
        ShowHint(self)
    end)
    g:SetScript("OnLeave", function(self)
        self.line:Hide()
        HideHint()
    end)
    return g
end

local function Create()
    if frame or not CreateFrame then return frame end
    frame = CreateFrame("Frame", "FactoruideCompass", FG.UIRoot)
    frame:SetSize(WIDTH, HEIGHT)
    frame:SetClampedToScreen(true)
    -- Over the minimal guide's strip, it moves the strip.
    FG:Draggable(frame, SavePoint, function(self) return attached or self end)
    frame:SetScript("OnEnter", ShowHint)
    frame:SetScript("OnLeave", HideHint)
    FG:Zoomable(frame, function(delta) Compass:Zoom(delta) end)

    -- A light veil, fading out at both ends, and a thin line at the bottom.
    -- Lighter on the parchment, whose texts are outlined instead.
    local c, veil = Skin.colors.bg, FG.Skin.light and 0.22 or 0.42
    local left = frame:CreateTexture(nil, "BACKGROUND")
    left:SetPoint("TOPLEFT")
    left:SetPoint("BOTTOMLEFT")
    Skin:Gradient(left, c, 0, veil)
    local right = frame:CreateTexture(nil, "BACKGROUND")
    right:SetPoint("TOPRIGHT")
    right:SetPoint("BOTTOMRIGHT")
    Skin:Gradient(right, c, veil, 0)
    local middle = Skin:Fill(frame, "BACKGROUND", c, veil)
    middle:SetPoint("TOPLEFT", left, "TOPRIGHT")
    middle:SetPoint("BOTTOMRIGHT", right, "BOTTOMLEFT")
    local a = FG.Skin.colors.accent
    local lineLeft = frame:CreateTexture(nil, "BORDER")
    lineLeft:SetPoint("BOTTOMLEFT")
    lineLeft:SetHeight(1)
    Skin:Gradient(lineLeft, a, 0, 0.45)
    local lineRight = frame:CreateTexture(nil, "BORDER")
    lineRight:SetPoint("BOTTOMRIGHT")
    lineRight:SetHeight(1)
    Skin:Gradient(lineRight, a, 0.45, 0)
    local line = Skin:Fill(frame, "BORDER", a, 0.45)
    line:SetHeight(1)
    line:SetPoint("BOTTOMLEFT", lineLeft, "BOTTOMRIGHT")
    line:SetPoint("BOTTOMRIGHT", lineRight, "BOTTOMLEFT")
    frame.faded = { left, right, lineLeft, lineRight }
    -- Straight ahead.
    frame.caret = Skin:Fill(frame, "OVERLAY", "text", 0.8)
    frame.caret:SetSize(5, 5)
    frame.caret:SetPoint("CENTER", frame, "BOTTOM", 0, 0)
    frame.caret:SetRotation(math.pi / 4)

    frame.elsewhere = Skin:Text(frame, 9, "muted")
    frame.elsewhere:SetPoint("CENTER", 0, 2)
    frame.elsewhere:SetText(L.elsewhere)

    -- Points behind: an arrow and their icons at each end, the target first with its distance.
    frame.ends = {}
    for _, side in ipairs({ "left", "right" }) do
        local e = CreateFrame("Frame", nil, frame)
        e:SetSize(90, 14)
        e.arrow = e:CreateTexture(nil, "ARTWORK")
        e.arrow:SetTexture("Interface\\ChatFrame\\ChatFrameExpandArrow")
        e.arrow:SetSize(10, 10)
        e.arrow:SetVertexColor(unpack(Skin.colors.lightBlue))
        e.target = e:CreateTexture(nil, "ARTWORK")
        e.target:SetSize(9, 9)
        Skin:AccentLine(e.target)
        e.target:SetRotation(math.pi / 4)
        e.distance = Skin:Text(e, 9, "lightBlue")
        e.icons = {}
        for i = 1, 3 do
            local icon = e:CreateTexture(nil, "ARTWORK")
            icon:SetSize(9, 9)
            icon:SetTexCoord(0.08, 0.92, 0.08, 0.92)
            icon:SetAlpha(0.8)
            e.icons[i] = icon
        end
        if side == "left" then
            e.arrow:SetTexCoord(1, 0, 0, 1) -- pointing left
            e.arrow:SetPoint("LEFT")
            e.target:SetPoint("LEFT", 12, 0)
            e.distance:SetPoint("LEFT", e.target, "RIGHT", 3, 0)
        else
            e.arrow:SetPoint("RIGHT")
            e.target:SetPoint("RIGHT", -12, 0)
            e.distance:SetPoint("RIGHT", e.target, "LEFT", -3, 0)
        end
        e.side = side
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
    frame.targetMark = MakeTarget()
    frame.wayMark = MakeWay()
    frame.grips = { MakeGrip("left"), MakeGrip("right") }
    local state = Saved()
    Layout(state and state.compassWidth or WIDTH_DEFAULT)

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
    frame:SetScale(Scale())
    FG:OnSetting("scale", function() frame:SetScale(Scale()) end)
    frame:Hide()
    return frame
end

local function HideFrom(list, from)
    for i = from, #list do
        list[i]:Hide()
    end
end

--- The ends: an arrow, the target first (with its distance), then the other points' icons.
local function DrawEnd(e, target, list)
    e:SetShown(target ~= nil or #list > 0)
    e.target:SetShown(target ~= nil)
    e.distance:SetShown(target ~= nil)
    if target then e.distance:SetText(Yards(target.distance)) end
    local from = 12 + (target and (14 + e.distance:GetStringWidth() + 3) or 0)
    for i, icon in ipairs(e.icons) do
        local p = list[i]
        icon:SetShown(p ~= nil)
        if p then
            icon:SetTexture(p.icon)
            icon:ClearAllPoints()
            local x = from + (i - 1) * 11
            if e.side == "left" then
                icon:SetPoint("LEFT", x, 0)
            else
                icon:SetPoint("RIGHT", -x, 0)
            end
        end
    end
end

local seen, groups = {}, {}
local behind = { left = {}, right = {} }

--- Where a point is for the character: its distance, its turn, its place on the bar (nil
--- behind). Nil when it is on another continent.
local function Place(p, continent, pn, pw, facing)
    if p.continent ~= continent then
        p.distance, p.bar = nil, nil
        return nil
    end
    local dn, dw = p.north - pn, p.west - pw
    p.distance = math.sqrt(dn * dn + dw * dw)
    p.left = Compass.Left(Compass.Angle(dn, dw, facing))
    p.bar = math.abs(p.left) <= FIELD - 4 and WIDTH / 2 - p.left * PPD or nil
    return p
end

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
    local target, any = nil, false
    if continent and facing then
        local function Add(p, tier)
            if not Place(p, continent, pn, pw, facing) then return end
            any = true
            if tier == "target" then
                target = p
            elseif p.bar then
                p.tier = tier
                table.insert(seen, p)
            elseif tier ~= "place" then
                table.insert(p.left > 0 and behind.left or behind.right, p)
            end
        end
        for _, p in ipairs(points) do
            Add(p, p.main and "target" or p.task and "task" or "next")
        end
        if any and FG.Nearby then
            for _, p in ipairs(FG.Nearby:Around(continent, pn, pw)) do
                Add(p, "place")
            end
        end
    end
    frame.elsewhere:SetShown(not any)
    if not any then
        HideFrom(ticks, 1)
        HideFrom(labels, 1)
        HideFrom(markers, 1)
        frame.caret:Hide()
        frame.targetMark:Hide()
        frame.wayMark:Hide()
        for _, e in pairs(frame.ends) do
            e:Hide()
        end
        return
    end
    frame.caret:Show()

    -- Groups of close points, the target apart.
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

    -- Ticks every 5° (taller every 15°), the cardinal points every 45° unless a point is there.
    local targetX = target and target.bar
    local heading = math.deg(facing)
    local nt, nl = 0, 0
    for d = math.ceil((heading - FIELD) / 5) * 5, heading + FIELD, 5 do
        local x = WIDTH / 2 - (d - heading) * PPD
        local norm = d % 360
        if norm % 45 == 0 then
            local free = not (targetX and x > targetX - 16 and x < targetX + 44)
            for _, g in ipairs(groups) do
                local gx = g[1].bar
                if x > gx - 14 and x < gx + 14 then free = false end
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

    -- One marker a group: the highest kind on top, else the closest.
    for i, g in ipairs(groups) do
        local top = g[1]
        local x = 0
        for _, p in ipairs(g) do
            x = x + p.bar
            local higher = TIERS[p.tier] - TIERS[top.tier]
            if higher > 0 or (higher == 0 and (p.distance or 0) < (top.distance or 0)) then top = p end
        end
        x = x / #g
        -- The top one first in the tooltip.
        for k, p in ipairs(g) do
            if p == top and k > 1 then
                table.remove(g, k)
                table.insert(g, 1, top)
                break
            end
        end
        local look = LOOKS[top.tier]
        local m = Marker(i)
        m.group, m.top = g, top
        m:ClearAllPoints()
        m:SetPoint("CENTER", frame, "TOPLEFT", x, -HEIGHT / 2 + 1)
        m:SetFrameLevel(frame:GetFrameLevel() + TIERS[top.tier])
        m.back:SetSize(look.back, look.back)
        local c = top.tier == "task" and Skin.colors.accent or { 0, 0, 0 }
        m.back:SetColorTexture(c[1], c[2], c[3], top.tier == "place" and 0.6 or 0.9)
        m.icon:SetSize(look.icon, look.icon)
        m.icon:SetTexture(top.icon)
        m.badge:SetShown(#g > 1)
        m.more:SetShown(#g > 1)
        m.more:SetText("+" .. (#g - 1))
        -- Far ones paler, the ends faded.
        m:SetAlpha(Alpha(x) * ((top.distance or 0) < FAR and look.alpha or look.farAlpha))
        m:Show()
    end
    HideFrom(markers, #groups + 1)

    -- The target.
    local t = frame.targetMark
    t:SetShown(targetX ~= nil)
    if targetX then
        t.group, t.top = { target }, target
        t:ClearAllPoints()
        t:SetPoint("CENTER", frame, "TOPLEFT", targetX, -HEIGHT / 2 + 1)
        local arrived = target.distance < ARRIVED
        t.distance:SetText(arrived and L.here or Yards(target.distance))
        local c = Skin.colors[arrived and "green" or "lightBlue"]
        t.distance:SetTextColor(c[1], c[2], c[3])
        -- A slow pulse of the glow.
        t.glow:SetAlpha(0.3 + 0.15 * math.sin(GetTime() * 3))
        t:SetAlpha(math.max(0.6, Alpha(targetX)))
    end

    -- The way, unless straight towards the target.
    local way = frame.wayMark
    local wn, ww, back
    if FG.Pathing then
        wn, ww, back = FG.Pathing:Aim(continent, pn, pw)
    end
    local wayLeft = wn and Compass.Left(Compass.Angle(wn - pn, ww - pw, facing))
    if wayLeft and target and math.abs(((wayLeft - target.left + 180) % 360) - 180) < SAME then wayLeft = nil end
    way:SetShown(wayLeft ~= nil)
    if wayLeft then
        -- Behind: at the end it is closer to.
        local clamped = math.max(-(FIELD - 4), math.min(FIELD - 4, wayLeft))
        local x = WIDTH / 2 - clamped * PPD
        way:ClearAllPoints()
        way:SetPoint("BOTTOM", frame, "BOTTOMLEFT", x, 0)
        way.back = back
        local c = Skin.colors[back and "orange" or "green"]
        way.bar:SetColorTexture(c[1], c[2], c[3], 0.95)
        way.head:SetColorTexture(c[1], c[2], c[3], 0.95)
        way.glow:SetColorTexture(c[1], c[2], c[3], 0.25)
        way:SetAlpha(clamped ~= wayLeft and 1 or math.max(0.7, Alpha(x)))
    end

    local targetBehind = target and not targetX and target or nil
    DrawEnd(frame.ends.left, targetBehind and target.left > 0 and target or nil, behind.left)
    DrawEnd(frame.ends.right, targetBehind and target.left <= 0 and target or nil, behind.right)
end

--- The points to show: { m, x, y (percent), index, main (the target), task (another current
--- task), kind, icon, name, sub }, or a place of the compass ({ place = true, ... }, see
--- Nearby:Around); nil hides the compass.
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
    Compass:Resize()
end

--- At its own place (the full guide): where it was dragged, else the top of the screen.
function Compass:Free()
    local f = Create()
    if not f or attached == false then return end
    attached = false
    f:ClearAllPoints()
    local state = Saved()
    f:SetPoint(unpack(state and state.compassPoint or { "TOP", UIParent, "TOP", 0, -14 }))
    Compass:Resize()
end

--- At the width and size kept for the guide's mode.
function Compass:Resize()
    local state = Saved()
    Layout(state and state.compassWidth or WIDTH_DEFAULT)
    frame:SetScale(Scale())
end

--- Back at the top of the screen, at its first width and size.
function Compass:ResetPlace()
    local state = Saved()
    if state then
        state.compassPoint, state.compassWidth, state.compassScale = nil, nil, nil
    end
    if frame then
        Layout(WIDTH_DEFAULT)
        frame:SetScale(Scale())
        if attached == false then
            attached = nil
            self:Free()
        end
    end
    FG:Print(L.reset)
end

FG.Options:AddButton("compassShown", {
    label = "Compass back to the top",
    text = "Put back",
    help = "At the top of the screen, at its first width and size.",
    run = function() Compass:ResetPlace() end,
})
