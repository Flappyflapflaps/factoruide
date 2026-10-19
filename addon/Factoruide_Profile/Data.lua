-- Guides as Lua text, run in a sandbox: the text of a `/fg import` (Factoruide_Guide/Import.lua)
-- and the one the desktop app writes into the data addon Factoruide_Data (never published, so
-- that an update of the other addons never erases it). Factoruide_Data loads first (an optional
-- dependency) and sets two globals:
--   FactoruideData = [==[ the same Lua text as an import ]==]
--   FactoruideDataTime = unix seconds it was written
-- The text sets `Factoruide.Routes[name]` for each guide, `.Services`... and maybe
-- `FactoruideSent = { ["Name-Realm"] = last seq }`, the rows the contribution server has: the
-- collector drops them, with or without the guide part. The text runs once, here; the guide part
-- takes what it set (`FG.DataGuide`) at its load.

local addonName, FG = ...

--- Whether `v` holds a function anywhere (keys included).
local function HasFunction(v, seen)
    if type(v) == "function" then return true end
    if type(v) ~= "table" or seen[v] then return false end
    seen[v] = true
    for k, x in pairs(v) do
        if HasFunction(k, seen) or HasFunction(x, seen) then return true end
    end
    return false
end

--- Run a guide's Lua text in a sandbox holding only a fresh `Factoruide` table (no game API, no
--- globals): returns the table it filled and its FactoruideSent, or nil and the error. The text
--- may come from someone else: data only, no function.
function FG.RunGuideText(text)
    -- Precompiled chunks can get out of a sandbox: source text only.
    if type(text) ~= "string" or text:byte(1) == 27 then return nil, "not Lua source" end
    local chunk, err = loadstring(text, "=import")
    if not chunk then return nil, err end
    local ns = {}
    local env = { Factoruide = ns }
    setfenv(chunk, env)
    local ok, runErr = pcall(chunk)
    if not ok then return nil, runErr end
    local sent = type(env.FactoruideSent) == "table" and env.FactoruideSent or nil
    if HasFunction(ns, {}) or HasFunction(sent, {}) then return nil, "functions are not allowed" end
    return ns, sent
end

--- The last seq of each character the server has, from `sent` (FactoruideSent of a text), the
--- valid entries only.
function FG.SentSeqs(sent)
    local out = {}
    for character, seq in pairs(type(sent) == "table" and sent or {}) do
        seq = tonumber(seq)
        if type(character) == "string" and seq then out[character] = seq end
    end
    return out
end

-- Factoruide_Data: run now (its globals are set, the guide part is not loaded yet), its text let
-- go. FG.DataGuide = { ns, time } for the guide part, until the login; FG.DataSent = its seqs.
FG.DataSent = {}
if type(FactoruideData) == "string" then
    local ns, sent = FG.RunGuideText(FactoruideData)
    FactoruideData = nil
    if ns then
        FG.DataGuide = { ns = ns, time = tonumber(FactoruideDataTime) or 0 }
        FG.DataSent = FG.SentSeqs(sent)
    else
        FG.DataFailed = tostring(sent)
    end
end

FG:On("ADDON_LOADED", function(name)
    if name ~= addonName then return end
    if FG.DataFailed then FG:Error("Factoruide_Data", FG.DataFailed) end
end)

-- Every addon is loaded at the login: what the guide part did not take is let go.
FG:On("PLAYER_LOGIN", function() FG.DataGuide = nil end)
