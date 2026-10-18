-- Look of the addon's frames, decided in one place: Factoruide's own (dark blue cards, the app's
-- colors) or, with the setting `suiteSkin`, the style of a loaded UI suite (ElvUI, EllesmereUI,
-- SpartanUI); skins registered through FactoruideAPI apply on top. Read when frames are created:
-- changing it needs a /reload.
--
-- Frame kinds: "window" (the guide: full card or minimal strip), "panel" (secondary boxes: quest
-- tracker, the minimal mode's next steps and labels), "button" (a text button), "icon" (a quest
-- item button: `.icon`, maybe `.cooldown`). Windows may have `.title`.

local FG = Factoruide

local Skin = { frames = {} }
FG.Skin = Skin

local external = {}
-- What styled each frame already: frame -> { [style name] = true }.
local done = setmetatable({}, { __mode = "k" })

--- A field the addon set on a frame (not one of its methods).
local function field(frame, key) return rawget(frame, key) end

--- r, g, b of "rrggbb".
local function rgb(hex)
    return { tonumber(hex:sub(1, 2), 16) / 255, tonumber(hex:sub(3, 4), 16) / 255, tonumber(hex:sub(5, 6), 16) / 255 }
end

-- Colors (r, g, b), the app's: its night-blue theme, or its light one, the parchment (a soft
-- yellowed ground, darker inks so that the texts stay readable on it).
local PALETTES = {
    dark = {
        bg = { 0.035, 0.067, 0.122 },
        edge = { 0.137, 0.204, 0.31 },
        line = { 0.082, 0.137, 0.227 },
        blue = { 0.227, 0.302, 1 },
        cyan = { 0.122, 0.659, 0.847 },
        accent = { 0.49, 0.553, 1 },
        text = { 1, 1, 1 },
        text2 = { 0.729, 0.769, 0.839 },
        muted = { 0.494, 0.545, 0.639 },
        dim = { 0.365, 0.416, 0.51 },
        gold = { 1, 0.82, 0 },
        green = { 0.29, 0.871, 0.502 },
        orange = { 1, 0.565, 0.251 },
        red = { 1, 0.4, 0.3 },
        purple = { 0.69, 0.486, 1 },
        lightBlue = { 0.5, 0.827, 0.941 },
        -- The XP bar: its empty part and the XP.
        xpTrack = { 0.016, 0.035, 0.071 },
        xp = { 0.616, 0.361, 1 },
    },
}
local LIGHT = {
    -- An old paper, a little yellow; amber where the other themes have blue (the step under
    -- way, the top line, the links, the XP).
    parchemin = {
        bg = "efe2c4",
        edge = "b9a47c",
        line = "e2d1ac",
        blue = "b8691e",
        cyan = "d9a441",
        accent = "8a4512",
        text = "2a2219",
        text2 = "4d4334",
        muted = "5f5341",
        dim = "7a6c58",
        gold = "7f4f00",
        green = "26672e",
        orange = "a0430a",
        red = "a63128",
        purple = "7d3f86",
        lightBlue = "2f6358",
        xpTrack = "dcc9a2",
        xp = "b5621b",
    },
}
for name, hexes in pairs(LIGHT) do
    local palette = { blue = PALETTES.dark.blue, cyan = PALETTES.dark.cyan }
    for key, hex in pairs(hexes) do
        palette[key] = rgb(hex)
    end
    PALETTES[name] = palette
end

-- The setting `uiTheme` (from the app's Config.lua), read once: changing it needs a /reload.
Skin.theme = PALETTES[FG:Get("uiTheme")] and FG:Get("uiTheme") or "dark"
Skin.light = Skin.theme ~= "dark"
Skin.colors = PALETTES[Skin.theme]

--- The same methods with the dark theme's colors and shadowed texts: for what is drawn over the
--- game world (the compass, the mark over an NPC), never on a card.
function Skin:Dark()
    local dark = setmetatable({ colors = PALETTES.dark }, { __index = Skin })
    dark.FontObject = function(size, flags) return Skin.FontObject(size, flags, true) end
    return dark
end

--- A game color ("rrggbb": a quest's difficulty, a step's kind) readable on the theme's ground:
--- as is on the dark one, half mixed with the text's ink on a light one.
function Skin.Ink(hex)
    if not Skin.light then return hex end
    local c, t = rgb(hex), Skin.colors.text
    return string.format("%02x%02x%02x", (c[1] + t[1]) / 2 * 255, (c[2] + t[2]) / 2 * 255, (c[3] + t[3]) / 2 * 255)
end

--- "|cffrrggbb" of a color of `Skin.colors`.
function Skin.Hex(name)
    local c = Skin.colors[name]
    return string.format("|cff%02x%02x%02x", c[1] * 255, c[2] * 255, c[3] * 255)
end

local WHITE = "Interface\\Buttons\\WHITE8x8"
Skin.WHITE = WHITE

--- A flat texture of a color of `Skin.colors` (or r, g, b), with alpha.
function Skin:Fill(parent, layer, color, alpha)
    local t = parent:CreateTexture(nil, layer or "ARTWORK")
    local c = type(color) == "table" and color or self.colors[color]
    t:SetColorTexture(c[1], c[2], c[3], alpha or 1)
    return t
end

--- Left-to-right fade of a color (alpha `a1` to `a2`); a flat texture of the mean alpha where the
--- client has no gradients.
function Skin:Gradient(texture, color, a1, a2)
    local c = type(color) == "table" and color or self.colors[color]
    texture:SetColorTexture(1, 1, 1, 1)
    local ok = CreateColor
        and texture.SetGradient
        and pcall(
            texture.SetGradient,
            texture,
            "HORIZONTAL",
            CreateColor(c[1], c[2], c[3], a1),
            CreateColor(c[1], c[2], c[3], a2)
        )
    if not ok then texture:SetColorTexture(c[1], c[2], c[3], (a1 + a2) / 2) end
end

--- The app's blue to cyan line (the top of the guide).
function Skin:AccentLine(texture)
    texture:SetColorTexture(1, 1, 1, 1)
    local b, c = self.colors.blue, self.colors.cyan
    local ok = CreateColor
        and texture.SetGradient
        and pcall(
            texture.SetGradient,
            texture,
            "HORIZONTAL",
            CreateColor(b[1], b[2], b[3], 1),
            CreateColor(c[1], c[2], c[3], 1)
        )
    if not ok then texture:SetColorTexture(b[1], b[2], b[3], 1) end
end

-- Fonts: the game's font (each client's own, so every language shows), in a few sizes, shadowed.
local fonts = {}

--- Font object of `size` (and `flags`), cached.
function Skin.FontObject(size, flags, shadowed)
    shadowed = shadowed or not Skin.light
    local key = size .. (flags or "") .. (shadowed and "" or "flat")
    if fonts[key] then return fonts[key] end
    if not CreateFont or not GameFontNormal then return "GameFontHighlightSmall" end
    local font = CreateFont("FactoruideFont" .. key:gsub("%W", ""))
    local path = GameFontNormal:GetFont()
    font:SetFont(path, size, flags or "")
    -- A dark shadow under the light texts; none under the dark inks of a light theme.
    font:SetShadowColor(0, 0, 0, shadowed and 1 or 0)
    font:SetShadowOffset(1, -1)
    font:SetTextColor(1, 1, 1)
    fonts[key] = font
    return font
end

--- A font string of `size` in a color of `Skin.colors`, left-justified on one line.
function Skin:Text(parent, size, color, layer)
    local fs = parent:CreateFontString(nil, layer or "OVERLAY")
    fs:SetFontObject(self.FontObject(size))
    local c = self.colors[color or "text"]
    fs:SetTextColor(c[1], c[2], c[3])
    fs:SetJustifyH("LEFT")
    fs:SetWordWrap(false)
    self:Font(fs)
    return fs
end

-- Base style --------------------------------------------------------------------

local FLAT_BACKDROP = {
    bgFile = WHITE,
    edgeFile = WHITE,
    edgeSize = 1,
}

local function Base(frame, kind)
    local c = Skin.colors
    if kind == "window" or kind == "panel" then
        frame:SetBackdrop(FLAT_BACKDROP)
        local alpha = kind == "window" and 0.9 or 0.85
        if Skin.light then alpha = alpha + 0.05 end
        frame:SetBackdropColor(c.bg[1], c.bg[2], c.bg[3], alpha)
        frame:SetBackdropBorderColor(c.edge[1], c.edge[2], c.edge[3], 1)
        if kind == "window" and not field(frame, "accent") then
            frame.accent = frame:CreateTexture(nil, "BORDER")
            frame.accent:SetPoint("TOPLEFT", 1, -1)
            frame.accent:SetPoint("TOPRIGHT", -1, -1)
            frame.accent:SetHeight(2)
            Skin:AccentLine(frame.accent)
        end
    elseif kind == "icon" then
        -- A gold ring around the item, as the guide's quest items.
        if field(frame, "icon") then frame.icon:SetTexCoord(0.08, 0.92, 0.08, 0.92) end
        if not field(frame, "ring") then
            frame.ring = frame:CreateTexture(nil, "BACKGROUND")
            frame.ring:SetPoint("TOPLEFT", -2, 2)
            frame.ring:SetPoint("BOTTOMRIGHT", 2, -2)
            frame.ring:SetColorTexture(c.gold[1], c.gold[2], c.gold[3], 1)
            local inner = frame:CreateTexture(nil, "BACKGROUND", nil, 1)
            inner:SetPoint("TOPLEFT", -1, 1)
            inner:SetPoint("BOTTOMRIGHT", 1, -1)
            inner:SetColorTexture(0, 0, 0, 1)
        end
    end
end

-- UI suites ---------------------------------------------------------------------

-- Each suite: `ready` once it can style frames, `apply(frame, kind, fonts)`. Tried in order.
local suites = {}

local function Refresh()
    for _, entry in ipairs(Skin.frames) do
        Skin:Style(entry.frame, entry.kind, entry.fonts, true)
    end
end

-- ElvUI: its Skins module, once it has loaded us.
if ElvUI then
    local elv = { name = "ElvUI" }
    local E = unpack(ElvUI)
    local S = E and E.GetModule and E:GetModule("Skins", true)
    if S and S.AddCallbackForAddon then
        S:AddCallbackForAddon("Factoruide_Guide", "Factoruide", function()
            elv.ready = true
            Refresh()
        end)
        elv.apply = function(frame, kind, fonts)
            if kind == "window" or kind == "panel" then
                frame:SetTemplate("Transparent")
            elseif kind == "button" and field(frame, "Left") then
                S:HandleButton(frame)
            elseif kind == "icon" then
                S:HandleItemButton(frame, true)
            end
            for _, fs in ipairs(fonts or {}) do
                if fs.FontTemplate then
                    local _, size = fs:GetFont()
                    fs:FontTemplate(nil, size)
                end
            end
        end
        elv.font = function(fs)
            if fs.FontTemplate then
                local _, size = fs:GetFont()
                fs:FontTemplate(nil, size)
            end
        end
        suites[#suites + 1] = elv
    end
end

-- EllesmereUI: its third-party skinning API (Retail and Forever).
if EllesmereUI and EllesmereUI.RegisterSkin then
    local eui = { name = "EllesmereUI" }
    EllesmereUI.RegisterSkin("Factoruide", function(S)
        eui.S = S
        eui.ready = true
        Refresh()
    end)
    eui.apply = function(frame, kind, fonts)
        local S = eui.S
        if kind == "window" then
            S.Shell(frame, { noTopBar = true })
        elseif kind == "panel" then
            S.Panel(frame)
        elseif kind == "button" then
            S.Button(frame)
        elseif kind == "icon" and field(frame, "icon") then
            S.SquareIcon(frame.icon, frame)
        end
        for _, fs in ipairs(fonts or {}) do
            S.Font(fs)
        end
    end
    eui.font = function(fs) eui.S.Font(fs) end
    suites[#suites + 1] = eui
end

-- SpartanUI: its Skins handler.
if SUI and SUI.Skins and SUI.Skins.Register then
    local sui = { name = "SpartanUI" }
    SUI.Skins:Register("Factoruide", function()
        sui.ready = true
        Refresh()
    end)
    sui.apply = function(frame, kind)
        if kind == "window" or kind == "panel" then
            SUI.Skins.SetTemplate(frame)
        elseif kind == "button" and field(frame, "Left") then
            SUI.Skins.SkinObj("Button", frame, nil, "Factoruide")
        end
    end
    suites[#suites + 1] = sui
end

local function ActiveSuite()
    if not FG:Get("suiteSkin") then return nil end
    for _, suite in ipairs(suites) do
        if suite.ready then return suite end
    end
    return nil
end

-- Styling -------------------------------------------------------------------------

--- Style `frame` (see the kinds above), with `fonts` its font strings, and remember it for the
--- skins registered later. Safe to call again.
function Skin:Style(frame, kind, fonts, again)
    if not again then self.frames[#self.frames + 1] = { frame = frame, kind = kind, fonts = fonts } end
    local suite = ActiveSuite()
    local styled = done[frame] or {}
    done[frame] = styled
    if suite then
        if not styled[suite.name] then
            styled[suite.name] = true
            if field(frame, "accent") then frame.accent:Hide() end
            local ok, err = pcall(suite.apply, frame, kind, fonts)
            if not ok then FG:Debug("%s skin: %s", suite.name, tostring(err)) end
        end
    elseif not styled.base then
        styled.base = true
        Base(frame, kind)
    end
    for _, skin in ipairs(external) do
        if not styled[skin.name] then
            styled[skin.name] = true
            local ok, err = pcall(skin.fn, frame, kind)
            if not ok then FG:Debug("%s skin: %s", skin.name, tostring(err)) end
        end
    end
end

--- Give a font string the UI suite's font (rows created after the frame), same size.
function Skin:Font(fs)
    local suite = ActiveSuite()
    if suite and suite.font then pcall(suite.font, fs) end
end

--- Name of the UI suite styling the addon, nil when none.
function Skin:Suite()
    local suite = ActiveSuite()
    return suite and suite.name
end

-- Public API for other skins (AddOnSkins, NDui, a skin of your own) ---------------

FactoruideAPI = FactoruideAPI or {}
FactoruideAPI.version = 2

--- Call `fn(frame, kind)` on every Factoruide frame, now and when created later. Kinds:
--- "window", "panel", "button", "icon" (see Skin.lua). Frames are named FactoruideTodo (full
--- guide), FactoruideMini (minimal guide), FactoruideMiniNext, FactoruideTracker, FGItemButton,
--- FactoruideItemButton<n>.
function FactoruideAPI.RegisterSkin(name, fn)
    for _, skin in ipairs(external) do
        if skin.name == name then return end
    end
    external[#external + 1] = { name = name, fn = fn }
    Refresh()
end

--- The frames styled so far: { { frame = frame, kind = kind }, ... }.
function FactoruideAPI.GetFrames()
    local out = {}
    for _, entry in ipairs(Skin.frames) do
        out[#out + 1] = { frame = entry.frame, kind = entry.kind }
    end
    return out
end
