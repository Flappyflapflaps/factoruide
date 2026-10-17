-- Settings come from the Factoruide app (generated Config.lua). In game, the window size, the
-- tab of the next steps, the guide's mode (full or minimal), the compass shown or not and the
-- windows locked or not are saved (the same for every character); the last three chosen in the
-- app win again when they change there.

local FG = Factoruide

-- Every frame of the guide hangs on this one: `/fg hide` hides them all at once. Only visual
-- frames: the event handlers keep running while hidden.
FG.UIRoot = CreateFrame("Frame", "FactoruideUI", UIParent)
FG.UIRoot:SetAllPoints(UIParent)

FG.defaults = {
    -- The whole addon's UI (`/fg hide`, `/fg show`), for the session only.
    uiShown = true,
    guideShown = true,
    -- "full": the guide's card; "minimal": a strip at the top of the screen.
    guideMode = "full",
    -- The look of a loaded UI suite (ElvUI...) instead of Factoruide's.
    suiteSkin = false,
    -- The next steps: their tab ("next" steps or quests to "pickup").
    nextTab = "next",
    -- The quest panel: its tab ("quests", "professions", "reputation", "items") and whether it is
    -- shown (a button of the guide).
    trackerTab = "quests",
    questPanel = true,
    lockFrames = false,
    scale = 1.0,
    waypoints = true,
    autoAccept = true,
    autoTurnIn = true,
    routeQuestsOnly = true,
    autoReward = false,
    fastLoot = true,
    autoTrain = true,
    trainUseful = true,
    trackerShown = true,
    replaceTracker = true,
    trackerQuests = 8,
    trackerAll = false,
    professionsShown = true,
    targetMacro = true,
    targetMark = true,
    questItemButton = true,
    autoSellJunk = true,
    autoRepair = true,
    autoSupplies = false,
    suppliesFood = 20,
    suppliesWater = 20,
    suppliesAmmo = 1000,
    gearAdvisor = true,
    talentAdvisor = true,
    autoLearnTalents = false,
    safetyAlerts = true,
    partySync = true,
    levelTiming = true,
    xpMarker = true,
    mapPins = true,
    mapPinsCount = 15,
    autoFly = true,
    -- The compass bar at the top of the screen, and a 3D arrow too.
    compassShown = true,
    arrowShown = false,
    -- WoW Forever: the place pointed at goes to Shortest Path Forever when it is loaded, which
    -- guides there (and the game's waypoint is left to it).
    spfLink = true,
    -- The guide's walk on the compass, the way around the steep ground: "on", "off", or "auto"
    -- (unless Shortest Path Forever guides).
    pathMode = "auto",
    -- That walk dotted on the world map and the minimap too.
    pathMap = true,
    -- Places shown on the compass closer than `compassRange` yards: the class trainer, the
    -- trainers of the character's professions, flight masters, innkeepers, repairers, food and
    -- drink, bankers, auctioneers, stable masters (hunters), mailboxes.
    compassClass = true,
    compassProfession = true,
    compassFlight = true,
    compassInn = false,
    compassRepair = false,
    compassFood = false,
    compassBank = false,
    compassAuction = false,
    compassStable = false,
    compassMail = false,
    compassRange = 300,
    -- The wind instead of the compass: "off", "on", or "auto" (with Poulpix UI Immersive).
    windGuide = "auto",
    -- "wind" (trails), "mist", "leaves" or "wisps".
    windStyle = "wind",
    windInterval = 8,
    windIntensity = 1,
    -- With Poulpix UI Immersive, the windows fade with its HUD.
    poulpixFade = true,
}

local session = {}
local listeners = {}

-- Settings changed in game and kept (FactoruideDB.ui), over the app's.
local SAVED = {
    scale = true,
    nextTab = true,
    guideMode = true,
    trackerTab = true,
    questPanel = true,
    compassShown = true,
    lockFrames = true,
}
-- Of those, the ones the app's value wins again when it changes there.
local APP_WINS = { "guideMode", "compassShown", "lockFrames" }

function FG:Get(key)
    local v = session[key]
    if v == nil and SAVED[key] and FactoruideDB and FactoruideDB.ui then v = FactoruideDB.ui[key] end
    if v == nil and FG.Config then v = FG.Config[key] end
    if v == nil then v = self.defaults[key] end
    return v
end

local function Changed(key)
    local value = FG:Get(key)
    for _, fn in ipairs(listeners[key] or {}) do
        fn(value)
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

--- Keep a setting changed in game (nil: back to the app's).
function FG:Save(key, value)
    FactoruideDB.ui = FactoruideDB.ui or {}
    FactoruideDB.ui[key] = value
    session[key] = nil
    Changed(key)
end

local SCALE_MIN, SCALE_MAX = 0.6, 2

--- `/fg scale [size]`: window size (0.6 to 2; nothing: the app's).
function FG:ScaleCommand(arg)
    local v = tonumber(arg)
    self:Save("scale", v and math.max(SCALE_MIN, math.min(SCALE_MAX, v)) or nil)
    self:Print("scale %.2f", self:Get("scale"))
end

--- `/fg lock`: the windows can no longer be dragged, or can again.
function FG:LockCommand()
    local locked = not self:Get("lockFrames")
    self:Save("lockFrames", locked)
    self:Print(
        locked and FG.T("windows locked (/fg lock unlocks them)") or FG.T("windows unlocked: drag them to move them")
    )
end

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

--- The mode, the compass and the lock chosen in game are kept until the app's change (then the
--- app's apply): the app's last value is kept beside (`appGuideMode`...).
FG:On("PLAYER_LOGIN", function()
    FactoruideDB.ui = FactoruideDB.ui or {}
    for _, key in ipairs(APP_WINS) do
        local app = FG.Config and FG.Config[key]
        local seen = "app" .. key:sub(1, 1):upper() .. key:sub(2)
        if FactoruideDB.ui[seen] ~= app then
            FactoruideDB.ui[seen] = app
            FactoruideDB.ui[key] = nil
        end
    end
end)

--- Call `fn(value)` whenever setting `key` changes.
function FG:OnSetting(key, fn)
    listeners[key] = listeners[key] or {}
    table.insert(listeners[key], fn)
end

FG:OnSetting("uiShown", function(shown)
    FG.UIRoot:SetShown(shown)
    if not shown and StaticPopup_Hide then StaticPopup_Hide("FACTORUIDE_RESET") end
end)

function FG:OpenSettings() FG:Print(FG.T("settings are configured in the Factoruide app (Addon tab), then /reload")) end
