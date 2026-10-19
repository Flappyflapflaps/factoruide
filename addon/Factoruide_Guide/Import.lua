-- `/fg import`: guides made in the Factoruide web app, which cannot write the game's files, pasted
-- in game as one text:
--   FGG1:<LibDeflate:EncodeForPrint(LibDeflate:CompressDeflate(lua))>
-- lua = the guides' Lua (`Factoruide.Routes`, `.Assignments`, `.Services`...) and maybe
-- `FactoruideSent = { ["Name-Realm"] = last seq }`, the same text the desktop app writes into the
-- data addon Factoruide_Data. It runs in a sandbox (FG.RunGuideText, the profile part's Data.lua)
-- and must leave data only. What it sets there replaces what the files set (Routes.lua and
-- Services.lua ship empty; older app versions wrote them). A `Factoruide.Config` is ignored: the
-- settings are chosen in game.
--
-- The text is kept in FactoruideDB.guideImport = { text, time }: the profile part's saved
-- variables are loaded before this addon's files run, so it applies here, right after the
-- generated files in the .toc and before every module reading FG.Routes, FG.Services...
-- Factoruide_Data's guides (FG.DataGuide, run by the profile part) apply the same way: of the two,
-- the most recent (FactoruideDataTime against the import's time) wins.

local FG = Factoruide
local T = FG.T
local Import = {}
FG.Import = Import

Import.PREFIX = "FGG1:"

--- Run a guide's Lua text in the sandbox: returns the table it filled (its `Factoruide`) and its
--- FactoruideSent, or nil and the error.
Import.Run = FG.RunGuideText

--- Copy what the guides set over the namespace (the addon's functions stay; no settings).
local function Apply(ns)
    for k, v in pairs(ns) do
        if k ~= "Config" and type(FG[k]) ~= "function" then FG[k] = v end
    end
end

--- Apply the guides kept: the imported ones or Factoruide_Data's, the most recent (the other when
--- it cannot be read). `Import.source`: "import", "data" or nil.
function Import:ApplySaved()
    local saved = FactoruideDB and FactoruideDB.guideImport
    if type(saved) ~= "table" or type(saved.text) ~= "string" then saved = nil end
    local data = FG.DataGuide
    FG.DataGuide = nil
    local order = {}
    if saved then order[#order + 1] = { source = "import", time = tonumber(saved.time) or 0 } end
    if data then order[#order + 1] = { source = "data", time = data.time } end
    table.sort(order, function(a, b) return a.time > b.time end)
    self.source, self.active, self.failed = nil, nil, nil
    for _, entry in ipairs(order) do
        if entry.source == "data" then
            Apply(data.ns)
            self.source = "data"
            return
        end
        local ns, err = Import.Run(saved.text)
        if ns then
            Apply(ns)
            self.source, self.active = "import", saved
            return
        end
        self.failed = tostring(err)
        FG:Error("import", err)
    end
end

local tried = FactoruideDB ~= nil
if tried then Import:ApplySaved() end
-- Without the saved variables yet (outside the game), as soon as they are there.
FG:On("ADDON_LOADED", function(name)
    if name ~= "Factoruide_Guide" or tried then return end
    tried = true
    Import:ApplySaved()
end)

--- The guide's Lua text out of a pasted string, or nil and a message for the player.
function Import.Decode(s)
    s = (s or ""):gsub("%s", "") -- line breaks or spaces picked up with the copy
    if s == "" then return nil, T("Paste the text first") end
    if s:sub(1, #Import.PREFIX) ~= Import.PREFIX then
        return nil, T("Not a Factoruide guide: the text should start with FGG1:")
    end
    local deflate = LibStub and LibStub("LibDeflate", true)
    if not deflate then return nil, "LibDeflate missing" end
    local ok, text = pcall(function()
        local raw = deflate:DecodeForPrint(s:sub(#Import.PREFIX + 1))
        return raw and deflate:DecompressDeflate(raw)
    end)
    if not ok or type(text) ~= "string" then return nil, T("The text is damaged: copy it again from the web app") end
    return text
end

--- Import a pasted string: keep it for the next loads. Returns the number of guides, or nil and a
--- message for the player.
function Import:FromString(s)
    local text, err = Import.Decode(s)
    if not text then return nil, err end
    local ns, sent = Import.Run(text)
    if not ns then return nil, string.format(T("The guide could not be read: %s"), tostring(sent)) end
    FactoruideDB.guideImport = { text = text, time = time(), fresh = true }
    -- What the contribution server already has: the collector drops these rows.
    FactoruideDB.sent = type(FactoruideDB.sent) == "table" and FactoruideDB.sent or {}
    for character, seq in pairs(FG.SentSeqs(sent)) do
        FactoruideDB.sent[character] = math.max(tonumber(FactoruideDB.sent[character]) or 0, seq)
    end
    local count = 0
    for _ in pairs(type(ns.Routes) == "table" and ns.Routes or {}) do
        count = count + 1
    end
    return count
end

--- Open the window to paste a guide into.
function Import:Open()
    local window = FG.TextWindow
    window:Open({
        title = T("Factoruide: guide import"),
        hint = T("Paste here (Ctrl+V) the text copied from the Factoruide web app, then click Import"),
        closeLabel = T("Cancel"),
        button = {
            label = T("Import"),
            onClick = function(text)
                local count, err = Import:FromString(text)
                if not count then return window:Status(err, true) end
                window:Status(string.format(T("guide imported (%d guides), reloading the interface"), count))
                FG:Print(T("guide imported (%d guides), reloading the interface"), count)
                ReloadUI()
            end,
        },
    })
end

--- `/fg import` opens the window; `/fg import clear` drops the imported guide.
function Import:Command(arg)
    if (arg or ""):lower() ~= "clear" then return self:Open() end
    if not FactoruideDB.guideImport then return FG:Print(T("no imported guide")) end
    FactoruideDB.guideImport = nil
    FG:Print(T("imported guide removed, reloading the interface"))
    ReloadUI()
end

FG:On("PLAYER_LOGIN", function()
    local saved = FactoruideDB.guideImport
    if Import.failed then
        FG:Print(T("the imported guide could not be read (%s): import it again with /fg import"), Import.failed)
    elseif saved and saved.fresh then
        saved.fresh = nil
        FG:Print(T("guide imported from the Factoruide web app"))
    end
end)
