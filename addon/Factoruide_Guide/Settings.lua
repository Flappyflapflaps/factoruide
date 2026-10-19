-- The addon's settings, chosen in game (`/fg`, SettingsWindow.lua) and kept in the saved
-- variables, the same for every character: FactoruideDB.settings = { [key] = value }. A setting
-- never chosen has its default. Their schema is addon/settings.json, written into
-- SettingsSchema.lua (FG.SettingsSchema). The app's Config.lua (older versions) and the `Config` of
-- an imported guide are ignored.

local FG = Factoruide

-- Every frame of the guide hangs on this one: "Hide the addon's windows" hides them all at once. Only visual
-- frames: the event handlers keep running while hidden.
FG.UIRoot = CreateFrame("Frame", "FactoruideUI", UIParent)
FG.UIRoot:SetAllPoints(UIParent)

-- The settings' defaults, the bounds of the numbers, each setting's schema entry.
FG.defaults, FG.bounds, FG.settingOptions = {}, {}, {}
for _, group in ipairs(FG.SettingsSchema or {}) do
    for _, section in ipairs(group.sections) do
        for _, option in ipairs(section.options) do
            FG.defaults[option.key] = option.default
            FG.settingOptions[option.key] = option
            if option.min then FG.bounds[option.key] = { option.min, option.max } end
        end
    end
end

-- Their texts in every language, beside the addon's own (which win when both have one).
for locale, texts in pairs(FG.SettingsTexts or {}) do
    local strings = FG.Locales[locale] or {}
    FG.Locales[locale] = strings
    for en, text in pairs(texts) do
        if strings[en] == nil then strings[en] = text end
    end
end
FG.SettingsTexts = nil

-- Settings only changed in game, outside the settings window.
-- The whole addon's UI ("Hide the addon's windows" in `/fg`), for the session only.
FG.defaults.uiShown = true
-- The next steps: their tab ("next" steps or quests to "pickup").
FG.defaults.nextTab = "next"
-- The quest panel: its tab ("quests", "professions", "reputation", "items") and whether it is
-- shown (a button of the guide).
FG.defaults.trackerTab = "quests"
FG.defaults.questPanel = true

local session = {}
local listeners = {}

--- `value` when it fits setting `key` (its type, a choice of a select, a number within bounds),
--- else nil.
function FG.ValidSetting(key, value)
    local option = FG.settingOptions[key]
    if not option then
        local default = FG.defaults[key]
        if default ~= nil and type(value) ~= type(default) then return nil end
        return value
    end
    if option.kind == "bool" then
        if type(value) ~= "boolean" then return nil end
        return value
    end
    if option.kind == "number" then
        if type(value) ~= "number" or value ~= value then return nil end
        return math.max(option.min, math.min(option.max, value))
    end
    for _, choice in ipairs(option.options) do
        if choice.value == value then return value end
    end
    return nil
end

--- The saved settings' table, nil before the saved variables are loaded.
local function Saved()
    if type(FactoruideDB) ~= "table" then return nil end
    if type(FactoruideDB.settings) ~= "table" then FactoruideDB.settings = {} end
    return FactoruideDB.settings
end

--- A setting's value: changed for the session (`FG:Set`), else saved, else its default.
function FG:Get(key)
    local v = session[key]
    if v == nil then
        local saved = Saved()
        if saved and saved[key] ~= nil then v = FG.ValidSetting(key, saved[key]) end
    end
    if v == nil then v = self.defaults[key] end
    return v
end

local function Changed(key)
    local value = FG:Get(key)
    for _, fn in ipairs(listeners[key] or {}) do
        fn(value)
    end
    -- Then the views redrawn on any change ("*"): the setting's own listeners have run.
    for _, fn in ipairs(listeners["*"] or {}) do
        fn(key, value)
    end
end

function FG:Set(key, value)
    session[key] = value
    Changed(key)
end

-- The quest item buttons are secure: the frame holding them only shows or hides out of combat.
-- The whole change waits for the end of the fight, overlays on the game's frames included, so
-- that everything switches together; the last command wins.
local pendingUI
function FG:SetUIShown(shown)
    if InCombatLockdown() then
        pendingUI = shown
        self:Print(FG.T("the addon's windows will change when combat ends"))
        return
    end
    pendingUI = nil
    self:Set("uiShown", shown)
end

FG:On("PLAYER_REGEN_ENABLED", function()
    if pendingUI ~= nil then FG:SetUIShown(pendingUI) end
end)

--- Whether the addon's windows are shown, or will be after the fight.
function FG:UIWanted()
    if pendingUI ~= nil then return pendingUI end
    return self:Get("uiShown")
end

--- Keep a setting (nil: back to its default). Applied at once (its listeners run).
function FG:Save(key, value)
    local saved = Saved()
    if saved then saved[key] = FG.ValidSetting(key, value) end
    session[key] = nil
    Changed(key)
end

local SCALE_MIN, SCALE_MAX = FG.bounds.scale[1], FG.bounds.scale[2]

--- The windows can no longer be dragged, or can again (the window's lock button, the setting).
function FG:ToggleLock()
    local locked = not self:Get("lockFrames")
    self:Save("lockFrames", locked)
    self:Print(locked and FG.T("windows locked") or FG.T("windows unlocked: drag them to move them"))
end

FG:AddAction({
    id = "hide",
    section = "Windows",
    label = function() return FG:UIWanted() and "Hide the addon's windows" or "Show the addon's windows" end,
    help = "Every window and mark of the addon, until you show them again or reload the interface.",
    run = function() FG:SetUIShown(not FG:UIWanted()) end,
})
FG:AddAction({
    id = "lock",
    section = "Windows",
    label = function() return FG:Get("lockFrames") and "Unlock the windows" or "Lock the windows" end,
    run = function() FG:ToggleLock() end,
})

-- Ctrl + mouse wheel over a window changes the size of the windows (`zoom(delta)` instead: its
-- own size). The wheel is only taken while Ctrl is held, so that it still zooms the camera
-- otherwise.
local zoomable = {}

function FG:Zoomable(frame, zoom)
    table.insert(zoomable, frame)
    frame:SetScript("OnMouseWheel", function(_, delta)
        if IsControlKeyDown and not IsControlKeyDown() then return end
        if zoom then return zoom(delta) end
        local v = math.floor((FG:Get("scale") + 0.05 * delta) * 100 + 0.5) / 100
        FG:Save("scale", math.max(SCALE_MIN, math.min(SCALE_MAX, v)))
    end)
    -- Setting the script takes the wheel: given back until Ctrl is held.
    frame:EnableMouseWheel(false)
end

-- Moving the windows: dragged from anywhere on them. Their rows and buttons pass the drag on to
-- their window (`FG:DragHandle`) and keep their clicks and tooltips. Not while the windows are
-- locked, nor during combat for a window the game protects (a secure quest item button hangs on
-- it: moving it would be blocked).

-- Per window: how it saves its place, the window moved instead, whether it is moving, when it
-- was dropped. Handles whose click was guarded.
local windows, guarded = {}, {}

local function Window(f)
    while f and not windows[f] do
        f = f:GetParent()
    end
    local target = f and windows[f].target
    if target then f = target(f) end
    return f and windows[f] and f
end

--- Whether `f`'s window is being dragged, or was dropped by this very mouse release (no click then).
function FG.JustDragged(f)
    local w = Window(f)
    local d = w and windows[w]
    return d ~= nil and (d.moving or d.dropped == GetTime())
end

-- A drag ends with the button released over the row: no click on it then.
local function NoClickOnDrop(f)
    if guarded[f] then return end
    guarded[f] = true
    local click = f:HasScript("OnClick") and f:GetScript("OnClick")
    if not click then return end
    f:SetScript("OnClick", function(self, ...)
        if FG.JustDragged(self) then return end
        return click(self, ...)
    end)
end

local function DragStart(f)
    local w = Window(f)
    local d = w and windows[w]
    if not d or d.moving or FG:Get("lockFrames") or not FG.CanLayout(w) then return end
    if f ~= w then NoClickOnDrop(f) end
    d.moving = true
    w:StartMoving()
end

local function DragStop(f)
    local w = Window(f)
    local d = w and windows[w]
    if not d or not d.moving then return end
    d.moving = false
    d.dropped = GetTime()
    w:StopMovingOrSizing()
    d.onDrop(w)
end

--- Make `frame` a window moved with the left button: `onDrop(frame)` saves where it was dropped.
--- `target(frame)` (optional): the window to move instead (another `FG:Draggable` window), nil for
--- none.
function FG:Draggable(frame, onDrop, target)
    windows[frame] = { onDrop = onDrop, target = target, moving = false }
    frame:SetMovable(true)
    frame:EnableMouse(true)
    self:DragHandle(frame)
    -- Hidden while dragged: no drag stop from the game, dropped there.
    frame:HookScript("OnHide", DragStop)
end

--- Dragging `f` (a row, a button of a window) moves its window. Not for secure buttons.
function FG:DragHandle(f)
    f:RegisterForDrag("LeftButton")
    f:SetScript("OnDragStart", DragStart)
    f:SetScript("OnDragStop", DragStop)
    return f
end

FG:On("MODIFIER_STATE_CHANGED", function(key, down)
    if key ~= "LCTRL" and key ~= "RCTRL" then return end
    for _, frame in ipairs(zoomable) do
        frame:EnableMouseWheel(down == 1)
    end
end)

--- The settings kept in game before the settings window (FactoruideDB.ui: the window size, the
--- guide's mode, the tabs...) move to FactoruideDB.settings once.
function FG.MoveOldSettings()
    local saved, old = Saved(), type(FactoruideDB) == "table" and FactoruideDB.ui
    if not saved or type(old) ~= "table" then return end
    for key, value in pairs(old) do
        if FG.defaults[key] ~= nil and key ~= "uiShown" and saved[key] == nil then
            saved[key] = FG.ValidSetting(key, value)
        end
    end
    FactoruideDB.ui = nil
end
FG.MoveOldSettings()
FG:On("ADDON_LOADED", function(name)
    if name == "Factoruide_Guide" then FG.MoveOldSettings() end
end)

--- Call `fn(value)` whenever setting `key` changes; key "*": `fn(key, value)` on any change.
function FG:OnSetting(key, fn)
    listeners[key] = listeners[key] or {}
    table.insert(listeners[key], fn)
end

FG:OnSetting("uiShown", function(shown)
    FG.UIRoot:SetShown(shown)
    if not shown and StaticPopup_Hide then StaticPopup_Hide("FACTORUIDE_RESET") end
end)
