-- Rested XP at logout: away from an inn or a city, logging out banks no rested XP (it doubles the
-- XP of kills until used). When the logout or quit countdown starts out of a rested area, a
-- warning, and the compass and the game's waypoint pointed at the closest innkeeper. Not at the
-- level cap, nor with the rested XP already full.

local FG = Factoruide
local T = FG.T

local Rested = {}
FG.Rested = Rested

-- The Cozy Sleeping Bag (WoW Forever): lying on it rests out of an inn.
local SLEEPING_BAG = 211527
-- Rested XP stops growing at 1.5 levels' worth.
local CAP = 1.5

local function Warn(text)
    if UIErrorsFrame then UIErrorsFrame:AddMessage(text, 1, 0.82, 0) end
    FG:Print("%s", text)
end

--- Whether logging out now loses rested XP: out of a rested area, below the level cap, with room
--- left in the rested XP.
function Rested:Missed()
    if FG.Readable(IsResting()) then return false end
    local cap = GetMaxPlayerLevel and GetMaxPlayerLevel()
    if cap and UnitLevel("player") >= cap then return false end
    local max = UnitXPMax("player") or 0
    return max <= 0 or (GetXPExhaustion() or 0) < CAP * max
end

--- The closest innkeeper and its yards, from where the character stands.
function Rested:ClosestInn()
    if not FG.Nearby or not FG.Compass then return nil end
    local map = C_Map.GetBestMapForUnit("player")
    local pos = map and C_Map.GetPlayerMapPosition(map, "player")
    local x, y
    if pos then
        x, y = pos:GetXY()
    end
    local continent, north, west = FG.Compass.World(map, FG.Readable(x), FG.Readable(y))
    if not continent then return nil end
    return FG.Nearby:Closest("inn", continent, north, west)
end

--- The warning when the logout or quit countdown starts.
function Rested:Check()
    if not FG:Get("restedReminder") or not FG:Get("uiShown") or not self:Missed() then return end
    local inn, yards = self:ClosestInn()
    if inn then
        Warn(string.format(T("No rested XP here: the closest inn is %s, %d yards away."), inn.name, yards))
        if FG.Guide then FG.Guide:PointToPlace(inn) end
    else
        Warn(T("No rested XP here: log out in an inn or a city."))
    end
    if FG.ItemCount(SLEEPING_BAG) > 0 then Warn(T("Or lie on your Cozy Sleeping Bag before logging out.")) end
end

FG:On("PLAYER_CAMPING", function() Rested:Check() end)
FG:On("PLAYER_QUITING", function() Rested:Check() end)
