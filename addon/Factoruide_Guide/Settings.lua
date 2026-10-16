-- Settings come from the Factoruide app (generated Config.lua). In game, the window size, the
-- tab of the next steps and the guide's mode (full or minimal) are saved (the same for every
-- character); the mode chosen in the app wins again when it changes there.

local FG = Factoruide

FG.defaults = {
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
local SAVED = { scale = true, nextTab = true, guideMode = true, trackerTab = true, questPanel = true }

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

-- Ctrl + mouse wheel over a window changes the size of the windows. The wheel is only taken
-- while Ctrl is held, so that it still zooms the camera otherwise.
local zoomable = {}

function FG:Zoomable(frame)
    table.insert(zoomable, frame)
    frame:SetScript("OnMouseWheel", function(_, delta)
        if IsControlKeyDown and not IsControlKeyDown() then return end
        local v = math.floor((FG:Get("scale") + 0.05 * delta) * 100 + 0.5) / 100
        FG:Save("scale", math.max(SCALE_MIN, math.min(SCALE_MAX, v)))
    end)
    -- Setting the script takes the wheel: given back until Ctrl is held.
    frame:EnableMouseWheel(false)
end

FG:On("MODIFIER_STATE_CHANGED", function(key, down)
    if key ~= "LCTRL" and key ~= "RCTRL" then return end
    for _, frame in ipairs(zoomable) do
        frame:EnableMouseWheel(down == 1)
    end
end)

--- The mode chosen in game is kept until the app's changes (then the app's applies).
FG:On("PLAYER_LOGIN", function()
    local app = FG.Config and FG.Config.guideMode
    FactoruideDB.ui = FactoruideDB.ui or {}
    if FactoruideDB.ui.appGuideMode ~= app then
        FactoruideDB.ui.appGuideMode = app
        FactoruideDB.ui.guideMode = nil
    end
end)

--- Call `fn(value)` whenever setting `key` changes.
function FG:OnSetting(key, fn)
    listeners[key] = listeners[key] or {}
    table.insert(listeners[key], fn)
end

function FG:OpenSettings() FG:Print(FG.T("settings are configured in the Factoruide app (Addon tab), then /reload")) end
