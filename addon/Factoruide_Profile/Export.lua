-- `/fg export`: the character's profile as one text to copy into the Factoruide web app, which
-- cannot read the game's files:
--   FGP1:<LibDeflate:EncodeForPrint(LibDeflate:CompressDeflate(json))>
-- json = { format = "factoruide-profile", version = 1, edition, addon, character, profile }.
-- Built only when the command is used.

local addonName, FG = ...
local T = FG.T
local Profile = FG.Profile
local Export = {}
FG.Export = Export

Export.PREFIX = "FGP1:"

local function Deflate() return LibStub and LibStub("LibDeflate", true) end

-- JSON (RFC 8259) ------------------------------------------------------------------------------

local ESCAPES = { ['"'] = '\\"', ["\\"] = "\\\\" }

local function EscapeChar(c) return ESCAPES[c] or string.format("\\u%04x", c:byte()) end

local function Number(v)
    if v ~= v or v == math.huge or v == -math.huge then return "null" end
    if v == math.floor(v) and math.abs(v) < 2 ^ 53 then return string.format("%.0f", v) end
    -- The shortest of these that reads back as the same number.
    for _, precision in ipairs({ "%.15g", "%.16g" }) do
        local s = string.format(precision, v)
        if tonumber(s) == v then return s end
    end
    return string.format("%.17g", v)
end

--- A sequence 1..n with nothing else (an empty table counts as one).
local function IsArray(t)
    local n = 0
    for _ in pairs(t) do
        n = n + 1
    end
    for i = 1, n do
        if t[i] == nil then return false end
    end
    return true, n
end

local function Encode(v, out, depth)
    local kind = type(v)
    if kind == "string" then
        out[#out + 1] = '"' .. v:gsub('[%c"\\]', EscapeChar) .. '"'
    elseif kind == "number" then
        out[#out + 1] = Number(v)
    elseif kind == "boolean" then
        out[#out + 1] = v and "true" or "false"
    elseif kind == "table" then
        if depth > 32 then error("JSON: tables nested too deep") end
        local array, n = IsArray(v)
        if array then
            out[#out + 1] = "["
            for i = 1, n do
                if i > 1 then out[#out + 1] = "," end
                Encode(v[i], out, depth + 1)
            end
            out[#out + 1] = "]"
        else
            -- Keys sorted: the same table always gives the same text.
            local keys, values = {}, {}
            for k, value in pairs(v) do
                k = type(k) == "number" and Number(k) or tostring(k)
                keys[#keys + 1] = k
                values[k] = value
            end
            table.sort(keys)
            out[#out + 1] = "{"
            for i, k in ipairs(keys) do
                if i > 1 then out[#out + 1] = "," end
                Encode(k, out, depth + 1)
                out[#out + 1] = ":"
                Encode(values[k], out, depth + 1)
            end
            out[#out + 1] = "}"
        end
    else
        out[#out + 1] = "null" -- nil, functions...
    end
end

--- `value` as JSON text: sequences as arrays (an empty table as []), other tables as objects.
function FG.JSON(value)
    local out = {}
    Encode(value, out, 0)
    return table.concat(out)
end

-- The export ----------------------------------------------------------------------------------

--- The profile as the app reads it: the flight paths known as a sorted list of their IDs.
local function ProfileForApp(profile)
    local copy = {}
    for k, v in pairs(profile) do
        copy[k] = v
    end
    local flights = {}
    for id, known in pairs(profile.flights or {}) do
        if known and tonumber(id) then flights[#flights + 1] = tonumber(id) end
    end
    table.sort(flights)
    copy.flights = flights
    return copy
end

--- The export's JSON text for this character, nil before the login.
function Export:Json()
    local key = FG:CharacterKey()
    local profile = FactoruideDB and FactoruideDB.profiles and FactoruideDB.profiles[key]
    if not profile then return nil end
    local metadata = C_AddOns and C_AddOns.GetAddOnMetadata or GetAddOnMetadata
    return FG.JSON({
        format = "factoruide-profile",
        version = 1,
        edition = FG.Edition,
        addon = metadata and metadata(addonName, "Version") or "?",
        character = key,
        profile = ProfileForApp(profile),
    })
end

--- The text to copy: nil and why when it cannot be made.
function Export:String()
    local deflate = Deflate()
    if not deflate then return nil, "LibDeflate missing" end
    local json = self:Json()
    if not json then return nil, T("the profile is not ready yet") end
    return self.PREFIX .. deflate:EncodeForPrint(deflate:CompressDeflate(json, { level = 9 }))
end

--- `/fg export`: take the profile now and show it to copy.
function Profile:Export()
    self:Snapshot()
    local text, err = Export:String()
    if not text then return FG:Print("%s", err) end
    FG.TextWindow:Open({
        title = T("Factoruide: profile export"),
        hint = T("Copy this text (Ctrl+C) and paste it in the Factoruide web app, Characters page"),
        text = text,
    })
end
