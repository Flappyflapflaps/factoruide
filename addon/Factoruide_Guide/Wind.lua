-- The wind guide, instead of the compass: now and then a gust passes by the character and goes
-- towards the guide's target (trails, low mist, leaves or will-o'-wisps), more often when facing
-- away, and swirls around the character on arrival. Addons only draw on the screen: the gusts
-- live on an imagined ground seen from a camera behind the character, so left, right, ahead and
-- behind read right even though the camera's pitch is unknown (design/immersive-wind shows the
-- same projection). On by itself with Poulpix UI Immersive ("auto").

local FG = Factoruide

local Wind = {}
FG.Wind = Wind

local MEDIA = "Interface\\AddOns\\Factoruide_Guide\\Media\\"

-- Screen fractions of its height: the horizon from the top, the focal length.
local HORIZON, FOCAL = 0.39, 0.68
local NEAR = 4 -- yards in front of the camera, closer ones not drawn
local ARRIVED = 10
local EMIT = 1.4 -- seconds a gust blows
local TICK = 0.025

local STYLES = {
    wind = { count = 22, life = { 1.8, 2.6 }, speed = { 13, 18 }, up = { 0.3, 2.6 }, trail = 10, curl = 0.35 },
    mist = { count = 14, life = { 3.0, 4.2 }, speed = { 5, 8 }, up = { 0.1, 0.9 }, trail = 1, curl = 0.15 },
    leaves = { count = 16, life = { 2.4, 3.4 }, speed = { 9, 13 }, up = { 0.4, 2.4 }, trail = 1, curl = 0.6 },
    wisps = { count = 9, life = { 3.2, 4.4 }, speed = { 6, 8 }, up = { 0.9, 1.8 }, trail = 10, curl = 0.25 },
}
local WISP_DOTS = 5

local frame
local target -- { continent, north, west, key }
local particles = {}
local pools = {}
local now, nextGust, emit, arrived = 0, nil, 0, false
local camera = { back = 9, eye = 3.2, shoulder = 0 }

local function Rand(range) return range[1] + math.random() * (range[2] - range[1]) end

local function Style()
    local s = FG:Get("windStyle")
    return STYLES[s] and s or "wind"
end

--- Whether the wind replaces the compass: "on", or "auto" with Poulpix UI Immersive loaded.
function Wind:Active()
    local mode = FG:Get("windGuide")
    if mode == "on" then return true end
    return mode == "auto" and FG.Poulpix.Loaded()
end

--- Screen point of a point `dn` yards north, `dw` west and `up` high from the character facing
--- `facing` (the game's: counter-clockwise from north), for a camera `back` yards behind and
--- `eye` high, moved `shoulder` yards to the right: x from the middle and y from the top, both in
--- screen heights, and the scale (screen heights a yard); nil behind the camera.
function Wind.Project(dn, dw, up, facing, back, eye, shoulder)
    local c, s = math.cos(facing), math.sin(facing)
    local z = dn * c + dw * s + back
    if z < NEAR then return nil end
    local k = FOCAL / z
    return (dn * s - dw * c - shoulder) * k, HORIZON + (eye - up) * k, k
end

local function Player()
    local map = C_Map.GetBestMapForUnit("player")
    local pos = map and C_Map.GetPlayerMapPosition(map, "player")
    if not pos then return nil end
    local x, y = pos:GetXY()
    x, y = FG.Readable(x), FG.Readable(y)
    local facing = GetPlayerFacing and FG.Readable(GetPlayerFacing())
    if not x or not facing then return nil end
    local continent, north, west = FG.Compass.World(map, x, y)
    if not continent then return nil end
    return continent, north, west, facing
end

-- The camera's distance (zoom) and the over-the-shoulder offset (Poulpix's situational camera).
local function ReadCamera()
    local ok, zoom = pcall(GetCameraZoom)
    zoom = ok and FG.Readable(zoom) or nil
    camera.back = math.max(3, math.min(30, zoom or 9))
    camera.eye = 1.6 + camera.back * 0.18
    local get = (C_CVar and C_CVar.GetCVar) or GetCVar
    local okCVar, shoulder = pcall(get, "test_cameraOverShoulder")
    camera.shoulder = okCVar and tonumber(shoulder) or 0
end

-- Drawing ---------------------------------------------------------------------------------

local function Texture(file, blend)
    local t = frame:CreateTexture(nil, "ARTWORK")
    t:SetTexture(MEDIA .. file)
    if blend then t:SetBlendMode(blend) end
    return t
end

local MAKE = {
    wind = function()
        local v = { lines = {} }
        for i = 1, STYLES.wind.trail - 1 do
            local l = frame:CreateLine(nil, "ARTWORK")
            l:SetColorTexture(0.93, 0.96, 1, 1)
            v.lines[i] = l
        end
        return v
    end,
    mist = function() return { tex = Texture("WindGlow.tga") } end,
    leaves = function() return { tex = Texture("WindLeaf.tga") } end,
    wisps = function()
        local v = { dots = {} }
        for i = 1, WISP_DOTS do
            local t = Texture("WindGlow.tga", "ADD")
            t:SetVertexColor(0.75, 0.94, 1)
            v.dots[i] = t
        end
        return v
    end,
}

local function HideVisual(v)
    for _, l in ipairs(v.lines or v.dots or { v.tex }) do
        l:Hide()
    end
end

local function TakeVisual(style)
    local pool = pools[style]
    if not pool then
        pool = {}
        pools[style] = pool
    end
    return table.remove(pool) or MAKE[style]()
end

local function GiveBack(p)
    HideVisual(p.visual)
    table.insert(pools[p.style], p.visual)
end

local function Clear()
    emit = 0
    for i = #particles, 1, -1 do
        GiveBack(particles[i])
        particles[i] = nil
    end
end

local function Spawn(style, pn, pw, swirl)
    local st = STYLES[style]
    local dn, dw = target.north - pn, target.west - pw
    local d = math.sqrt(dn * dn + dw * dw)
    if d < 0.01 then
        d, dn = 1, 1
    end
    local un, uw = dn / d, dw / d
    -- Behind the character on the target's line, on either side of it.
    local back, side = -7 + math.random() * 8, -5 + math.random() * 10
    local p = {
        style = style,
        visual = TakeVisual(style),
        t = 0,
        life = Rand(st.life) * (swirl and 1.3 or 1),
        north = pn + un * back - uw * side,
        west = pw + uw * back + un * side,
        up = Rand(st.up),
        dir = math.atan2(uw, un),
        speed = Rand(st.speed),
        seed = math.random() * 100,
        spin = -6 + math.random() * 12,
        rot = math.random() * 6,
        size = 0.8 + math.random() * 0.5,
        trail = {},
    }
    if swirl then
        p.orbit, p.angle, p.speed = 1.2 + math.random() * 2, math.random() * 2 * math.pi, p.speed * 0.5
        p.cn, p.cw = pn, pw
    end
    table.insert(particles, p)
end

local function Step(p, dt)
    local st = STYLES[p.style]
    p.t = p.t + dt
    if p.orbit then
        p.angle = p.angle + dt * p.speed / p.orbit
        p.orbit = p.orbit + dt * 0.6
        p.north, p.west = p.cn + math.cos(p.angle) * p.orbit, p.cw + math.sin(p.angle) * p.orbit
        p.up = p.up + dt * 0.7
    else
        local a = p.dir + math.sin(now * 1.7 + p.seed) * st.curl
        p.north = p.north + math.cos(a) * p.speed * dt
        p.west = p.west + math.sin(a) * p.speed * dt
        local lift = p.style == "leaves" and 1.6 or 0.3
        p.up = math.max(0.15, p.up + math.sin(now * 2.3 + p.seed) * dt * lift)
    end
    p.rot = p.rot + p.spin * dt
    table.insert(p.trail, { p.north, p.west, p.up })
    if #p.trail > st.trail then table.remove(p.trail, 1) end
end

-- Fades in over the first fifth of its life, out over the last third.
local function Envelope(p)
    local u = p.t / p.life
    return math.max(0, math.min(1, u / 0.2, (1 - u) / 0.35))
end

local function Draw(p, pn, pw, facing, w, h, intensity)
    local v = p.visual
    local a = Envelope(p) * math.min(1, intensity)
    local function At(q)
        local x, y, k = Wind.Project(q[1] - pn, q[2] - pw, q[3], facing, camera.back, camera.eye, camera.shoulder)
        if not x then return nil end
        return w / 2 + x * h, -y * h, k * h
    end
    if p.style == "wind" then
        local n = #p.trail
        for i, l in ipairs(v.lines) do
            local x0, y0 = At(p.trail[i] or p.trail[n])
            local x1, y1, k = At(p.trail[i + 1] or p.trail[n])
            if x0 and x1 and i < n then
                l:SetStartPoint("TOPLEFT", frame, x0, y0)
                l:SetEndPoint("TOPLEFT", frame, x1, y1)
                l:SetThickness(math.max(0.6, 1.4 * k / 60))
                l:SetAlpha(a * 0.6 * i / (n - 1))
                l:Show()
            else
                l:Hide()
            end
        end
        return
    elseif p.style == "wisps" then
        local n = #p.trail
        for i, t in ipairs(v.dots) do
            -- The head last, the dots before it spread along the trail.
            local q = p.trail[math.max(1, n - (WISP_DOTS - i) * 2)]
            local x, y, k = At(q)
            if x then
                local r = (0.05 + 0.1 * i / WISP_DOTS) * k * 6
                t:SetSize(r, r)
                t:SetPoint("CENTER", frame, "TOPLEFT", x, y)
                t:SetAlpha(a * (i == WISP_DOTS and 0.9 or 0.25 * i / WISP_DOTS))
                t:Show()
            else
                t:Hide()
            end
        end
        return
    end
    local x, y, k = At(p.trail[#p.trail])
    local t = v.tex
    if not x then
        t:Hide()
        return
    end
    t:ClearAllPoints()
    t:SetPoint("CENTER", frame, "TOPLEFT", x, y)
    if p.style == "mist" then
        local r = 2.2 * p.size * k
        t:SetSize(r * 3.2, r * 1.4)
        t:SetVertexColor(0.92, 0.94, 0.96)
        t:SetAlpha(a * 0.22)
    else
        local r = 0.3 * p.size * k
        t:SetSize(r * 2, r * 2 * (math.abs(math.sin(p.rot * 0.7)) * 0.8 + 0.2))
        t:SetRotation(p.rot)
        if p.seed > 50 then
            t:SetVertexColor(0.77, 0.55, 0.23)
        else
            t:SetVertexColor(0.59, 0.67, 0.27)
        end
        t:SetAlpha(a * 0.9)
    end
    t:Show()
end

-- Gusts -------------------------------------------------------------------------------------

local function Quiet() return UnitAffectingCombat("player") or UnitOnTaxi("player") or UnitIsDeadOrGhost("player") end

-- Seconds to the next gust: the setting's, longer when walking towards the target, shorter when
-- facing away.
local function Interval(left)
    local interval = FG:Get("windInterval")
    local off = math.abs(left)
    if off < 25 then return interval * 1.6 end
    if off > 90 then return interval * 0.45 end
    return interval
end

function Wind:Update(dt)
    now = now + dt
    local continent, pn, pw, facing = Player()
    if not continent or not target or target.continent ~= continent then
        Clear()
        return
    end
    local style = Style()
    local intensity = FG:Get("windIntensity")
    local dn, dw = target.north - pn, target.west - pw
    local distance = math.sqrt(dn * dn + dw * dw)

    if not arrived and distance < ARRIVED then
        arrived, emit = true, 0
        ReadCamera()
        for _ = 1, math.floor(STYLES[style].count * 1.2 * intensity) do
            Spawn(style, pn, pw, true)
        end
    end
    if not arrived then
        if Quiet() then
            emit = 0
            nextGust = math.max(nextGust or 0, now + 2)
        elseif nextGust and now >= nextGust then
            self:Gust()
            nextGust = now + Interval(FG.Compass.Left(FG.Compass.Angle(dn, dw, facing)))
        end
        if emit > 0 then
            local rate = STYLES[style].count * intensity / EMIT
            for _ = 1, math.floor(rate * math.min(emit, dt) + math.random()) do
                Spawn(style, pn, pw, false)
            end
            emit = emit - dt
        end
    end

    local w, h = tonumber(frame:GetWidth()) or 0, tonumber(frame:GetHeight()) or 0
    for i = #particles, 1, -1 do
        local p = particles[i]
        Step(p, dt)
        if p.t > p.life then
            GiveBack(p)
            table.remove(particles, i)
        else
            Draw(p, pn, pw, facing, w, h, intensity)
        end
    end
end

local function Create()
    if frame or not CreateFrame then return frame end
    frame = CreateFrame("Frame", "FactoruideWind", FG.UIRoot)
    frame:SetAllPoints(UIParent)
    frame:SetFrameStrata("BACKGROUND")
    frame:EnableMouse(false)
    local wait = 0
    frame:SetScript("OnUpdate", function(_, elapsed)
        wait = wait + elapsed
        if wait < TICK then return end
        local dt = math.min(0.1, wait)
        wait = 0
        local ok, err = pcall(Wind.Update, Wind, dt)
        if not ok then
            FG:Error("wind", err)
            frame:Hide()
        end
    end)
    frame:Hide()
    return frame
end

--- Blow towards the target now (key binding).
function Wind:Gust()
    if not target or arrived then return end
    ReadCamera()
    emit = EMIT
end

--- The place to blow towards ({ m, x, y } in percent, as the guide's steps); nil stops.
function Wind:SetTarget(place)
    local f = Create()
    if not f then return end
    local key = place and place.m and place.x and string.format("%d:%.2f:%.2f", place.m, place.x, place.y)
    if key == (target and target.key) then return end
    target = nil
    if key then
        local continent, north, west = FG.Compass.World(place.m, place.x / 100, place.y / 100)
        if continent then target = { continent = continent, north = north, west = west, key = key } end
    end
    if not target then Clear() end
    arrived, emit = false, 0
    nextGust = now + 1.5
    f:SetShown(target ~= nil)
end

--- The place blown towards ({ continent, north, west }), nil when none.
function Wind:Target() return target end

--- How many particles are blowing.
function Wind:Count() return #particles end
