-- With Poulpix UI Immersive: the guide's windows fade out with its HUD while exploring (the alpha
-- of its root frame, PoulpixUIRoot) and come back with it, or under the mouse.

local FG = Factoruide

local Poulpix = {}
FG.Poulpix = Poulpix

local TICK = 0.1
local SPEED = 4 -- alpha a second

local ticker
local alphas = {}

--- Whether Poulpix UI Immersive is loaded.
function Poulpix.Loaded()
    local loaded = (C_AddOns and C_AddOns.IsAddOnLoaded) or IsAddOnLoaded
    return loaded ~= nil and loaded("Poulpix_UI_Immersive") and true or false
end

--- Alpha of a window: the HUD's, full under the mouse.
function Poulpix.Alpha(hud, hovered) return hovered and 1 or math.max(0, math.min(1, hud)) end

local function Follow()
    local root = _G.PoulpixUIRoot
    local hud = root and FG.Readable(root:GetAlpha())
    if not hud then return end
    for _, entry in ipairs(FG.Skin.frames) do
        local f = entry.frame
        -- Child frames fade with their window already (the windows hang on FG.UIRoot).
        if f:GetParent() == FG.UIRoot then
            local want = Poulpix.Alpha(hud, f:IsShown() and f:IsMouseOver())
            local a = alphas[f] or f:GetAlpha()
            if a < want then
                a = math.min(want, a + SPEED * TICK)
            else
                a = math.max(want, a - SPEED * TICK)
            end
            alphas[f] = a
            f:SetAlpha(a)
        end
    end
end

FG:On("PLAYER_LOGIN", function()
    if ticker or not FG:Get("poulpixFade") or not Poulpix.Loaded() then return end
    ticker = C_Timer.NewTicker(TICK, function()
        local ok, err = pcall(Follow)
        if not ok then
            FG:Error("poulpix", err)
            ticker:Cancel()
        end
    end)
end)
