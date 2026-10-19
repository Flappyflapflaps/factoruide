-- Writes Factoruide_Guide/SettingsSchema.lua from addon/settings.json, the one source of the
-- addon's settings: their groups, sections, kinds, defaults, bounds, choices and texts (French in
-- the schema, the other languages under `translations`). The addon shows them in English through
-- FG.T, with the translations of every language.
--
-- From the repository root:
--   luajit addon/tools/settings.lua           write the Lua file
--   luajit addon/tools/settings.lua --check   exit 1 when it differs from the JSON
-- The smoke test loads this file (`loadfile(...)("module")`) and checks the Lua file is up to date.

local JSON_PATH = "addon/settings.json"
local LUA_PATH = "addon/Factoruide_Guide/SettingsSchema.lua"
local WIDTH = 120

-- The schema's languages: its own (French) and those of `translations`, as game locales.
local LOCALES = {
    { key = "de", locale = "deDE" },
    { key = "es", locale = "esES" },
    { key = "pt", locale = "ptBR" },
    { key = "ru", locale = "ruRU" },
    { key = "ko", locale = "koKR" },
    { key = "zhCN", locale = "zhCN" },
    { key = "zhTW", locale = "zhTW" },
}

-- JSON ------------------------------------------------------------------------------------------

local function Utf8(code)
    if code < 0x80 then return string.char(code) end
    if code < 0x800 then return string.char(0xC0 + math.floor(code / 0x40), 0x80 + code % 0x40) end
    return string.char(0xE0 + math.floor(code / 0x1000), 0x80 + math.floor(code / 0x40) % 0x40, 0x80 + code % 0x40)
end

local ESCAPES = { ['"'] = '"', ["\\"] = "\\", ["/"] = "/", b = "\b", f = "\f", n = "\n", r = "\r", t = "\t" }

--- The value of a JSON text (objects keep their keys' order in `[keys]`); errors on bad JSON.
local function Decode(text)
    local pos = 1
    local value

    local function Fail(what) error(string.format("settings.json: %s at byte %d", what, pos)) end
    local function Skip() pos = text:find("[^ \t\r\n]", pos) or #text + 1 end

    local function String()
        local out = {}
        pos = pos + 1
        while true do
            local c = text:sub(pos, pos)
            if c == "" then Fail("unfinished string") end
            if c == '"' then
                pos = pos + 1
                return table.concat(out)
            elseif c == "\\" then
                local e = text:sub(pos + 1, pos + 1)
                if e == "u" then
                    local code = tonumber(text:sub(pos + 2, pos + 5), 16) or Fail("bad \\u")
                    if code >= 0xD800 and code <= 0xDBFF then
                        local low = tonumber(text:sub(pos + 8, pos + 11), 16) or Fail("bad surrogate")
                        code = 0x10000 + (code - 0xD800) * 0x400 + (low - 0xDC00)
                        local b = {
                            0xF0 + math.floor(code / 0x40000),
                            0x80 + math.floor(code / 0x1000) % 0x40,
                            0x80 + math.floor(code / 0x40) % 0x40,
                            0x80 + code % 0x40,
                        }
                        out[#out + 1] = string.char(b[1], b[2], b[3], b[4])
                        pos = pos + 12
                    else
                        out[#out + 1] = Utf8(code)
                        pos = pos + 6
                    end
                else
                    out[#out + 1] = ESCAPES[e] or Fail("bad escape")
                    pos = pos + 2
                end
            else
                local stop = text:find('["\\]', pos) or #text + 1
                out[#out + 1] = text:sub(pos, stop - 1)
                pos = stop
            end
        end
    end

    function value()
        Skip()
        local c = text:sub(pos, pos)
        if c == "{" then
            local obj, keys = {}, {}
            pos = pos + 1
            Skip()
            if text:sub(pos, pos) == "}" then
                pos = pos + 1
                return obj
            end
            while true do
                Skip()
                if text:sub(pos, pos) ~= '"' then Fail("key expected") end
                local k = String()
                Skip()
                if text:sub(pos, pos) ~= ":" then Fail(": expected") end
                pos = pos + 1
                obj[k] = value()
                keys[#keys + 1] = k
                Skip()
                local sep = text:sub(pos, pos)
                pos = pos + 1
                if sep == "}" then break end
                if sep ~= "," then Fail(", or } expected") end
            end
            return obj
        elseif c == "[" then
            local arr = {}
            pos = pos + 1
            Skip()
            if text:sub(pos, pos) == "]" then
                pos = pos + 1
                return arr
            end
            while true do
                arr[#arr + 1] = value()
                Skip()
                local sep = text:sub(pos, pos)
                pos = pos + 1
                if sep == "]" then break end
                if sep ~= "," then Fail(", or ] expected") end
            end
            return arr
        elseif c == '"' then
            return String()
        elseif text:sub(pos, pos + 3) == "true" then
            pos = pos + 4
            return true
        elseif text:sub(pos, pos + 4) == "false" then
            pos = pos + 5
            return false
        elseif text:sub(pos, pos + 3) == "null" then
            pos = pos + 4
            return nil
        end
        local num = text:match("^-?%d+%.?%d*[eE]?[-+]?%d*", pos)
        if not num or num == "" then Fail("value expected") end
        pos = pos + #num
        return tonumber(num)
    end

    local result = value()
    Skip()
    if pos <= #text then Fail("text after the value") end
    return result
end

-- Lua text, as stylua formats it -------------------------------------------------------------

local function Quote(s)
    -- Double quotes, single ones when the text holds double quotes and no single quote.
    local q = (s:find('"', 1, true) and not s:find("'", 1, true)) and "'" or '"'
    s = s:gsub("\\", "\\\\"):gsub("\n", "\\n"):gsub(q, "\\" .. q)
    return q .. s .. q
end

local function Number(n)
    if n == math.floor(n) then return string.format("%d", n) end
    return string.format("%.14g", n)
end

local function Scalar(v)
    if type(v) == "string" then return Quote(v) end
    if type(v) == "number" then return Number(v) end
    return tostring(v)
end

--- Characters of a line (UTF-8 continuation bytes not counted).
local function Width(s)
    local _, continuation = s:gsub("[\128-\191]", "")
    return #s - continuation
end

-- A table to write: its fields in order, { key, value } (key nil for array items).
local function Fields(...) return { fields = { ... } } end

local Emit

local function Inline(v)
    if type(v) ~= "table" then return Scalar(v) end
    if #v.fields == 0 then return "{}" end
    local parts = {}
    for i, f in ipairs(v.fields) do
        parts[i] = (f[1] and (f[1] .. " = ") or "") .. Inline(f[2])
    end
    return "{ " .. table.concat(parts, ", ") .. " }"
end

--- Lines of `prefix .. value .. suffix` at `indent`: on one line when it fits, else one field a line.
function Emit(out, indent, prefix, v, suffix)
    local pad = string.rep(" ", indent)
    local line = pad .. prefix .. Inline(v) .. suffix
    if type(v) ~= "table" or Width(line) <= WIDTH or #v.fields == 0 then
        out[#out + 1] = line
        return
    end
    out[#out + 1] = pad .. prefix .. "{"
    for _, f in ipairs(v.fields) do
        Emit(out, indent + 4, f[1] and (f[1] .. " = ") or "", f[2], ",")
    end
    out[#out + 1] = pad .. "}" .. suffix
end

-- The Lua file ----------------------------------------------------------------------------------

local KINDS = { bool = "boolean", number = "number", select = "string" }

--- The Lua file's text for the JSON text.
local function Generate(json)
    local schema = Decode(json)
    local translations = schema.translations or {}
    local texts = {} -- French text -> true, every text shown
    local function English(fr, where)
        if fr == nil then return nil end
        local t = translations[fr]
        if type(t) ~= "table" or type(t.en) ~= "string" then
            error(string.format("settings.json: no English translation of %s %q", where, fr))
        end
        texts[fr] = true
        return t.en
    end

    local groups = {}
    local seen = {}
    for _, g in ipairs(schema.groups) do
        local sections = {}
        for _, s in ipairs(g.sections) do
            local options = {}
            for _, o in ipairs(s.options) do
                assert(type(o.key) == "string" and not seen[o.key], "settings.json: duplicate or missing key")
                seen[o.key] = true
                assert(
                    KINDS[o.kind] and type(o.default) == KINDS[o.kind],
                    "settings.json: bad kind or default " .. o.key
                )
                local fields = {
                    { "key", o.key },
                    { "kind", o.kind },
                    { "default", o.default },
                    { "label", English(o.label, o.key) },
                }
                if o.help then fields[#fields + 1] = { "help", English(o.help, o.key) } end
                if o.kind == "number" then
                    assert(o.min and o.max and o.step and o.min <= o.default and o.default <= o.max, o.key)
                    fields[#fields + 1] = { "min", o.min }
                    fields[#fields + 1] = { "max", o.max }
                    fields[#fields + 1] = { "step", o.step }
                    if o.percent then fields[#fields + 1] = { "percent", true } end
                end
                if o.kind == "select" then
                    local choices, valid = {}, false
                    for _, c in ipairs(o.options) do
                        choices[#choices + 1] =
                            { nil, Fields({ "value", c.value }, { "label", English(c.label, o.key) }) }
                        valid = valid or c.value == o.default
                    end
                    assert(valid, "settings.json: default not a choice of " .. o.key)
                    fields[#fields + 1] = { "options", { fields = choices } }
                end
                if o.only then fields[#fields + 1] = { "only", o.only } end
                if o.requires then fields[#fields + 1] = { "requires", o.requires } end
                if o.reload then fields[#fields + 1] = { "reload", true } end
                options[#options + 1] = { nil, { fields = fields } }
            end
            local section = {}
            if s.title then section[#section + 1] = { "title", English(s.title, "section") } end
            section[#section + 1] = { "options", { fields = options } }
            sections[#sections + 1] = { nil, { fields = section } }
        end
        groups[#groups + 1] = {
            nil,
            Fields({ "title", English(g.title, "group") }, { "sections", { fields = sections } }),
        }
    end
    for _, g in ipairs(schema.groups) do
        for _, s in ipairs(g.sections) do
            for _, o in ipairs(s.options) do
                assert(o.requires == nil or seen[o.requires], "settings.json: unknown requires of " .. o.key)
            end
        end
    end

    local out = {
        "-- Generated from addon/settings.json by addon/tools/settings.lua: edit the JSON, then run",
        "-- `luajit addon/tools/settings.lua` from the repository root. Do not edit.",
        "local FG = Factoruide",
        "",
        "-- The settings in groups and sections, as the settings window shows them; texts in English.",
    }
    Emit(out, 0, "FG.SettingsSchema = ", { fields = groups }, "")
    out[#out + 1] = ""
    out[#out + 1] = "-- Their texts in the other languages: English text -> translation, by game locale."
    out[#out + 1] = "FG.SettingsTexts = {"
    local all = { { key = "fr", locale = "frFR" } }
    for _, l in ipairs(LOCALES) do
        all[#all + 1] = l
    end
    local french = {}
    for fr in pairs(texts) do
        french[#french + 1] = fr
    end
    table.sort(french, function(a, b) return translations[a].en < translations[b].en end)
    for _, l in ipairs(all) do
        out[#out + 1] = "    " .. l.locale .. " = {"
        local done = {}
        for _, fr in ipairs(french) do
            local en = translations[fr].en
            local text = l.key == "fr" and fr or translations[fr][l.key]
            if text and not done[en] then
                done[en] = true
                out[#out + 1] = "        [" .. Quote(en) .. "] = " .. Quote(text) .. ","
            end
        end
        out[#out + 1] = "    },"
    end
    out[#out + 1] = "}"
    return table.concat(out, "\n") .. "\n"
end

local function Read(path)
    local f = assert(io.open(path, "rb"))
    local text = f:read("*a")
    f:close()
    return text
end

--- The Lua file's text from the JSON file, and the Lua file as it is (nil when missing).
local function Compare()
    local ok, current = pcall(Read, LUA_PATH)
    return Generate(Read(JSON_PATH)), ok and current or nil
end

local mode = ...
if mode == "module" then return { Generate = Generate, Compare = Compare, Decode = Decode } end

local generated, current = Compare()
if mode == "--check" then
    if generated ~= current then
        io.stderr:write(LUA_PATH .. " differs from " .. JSON_PATH .. ": run luajit addon/tools/settings.lua\n")
        os.exit(1)
    end
    print(LUA_PATH .. " is up to date")
else
    local f = assert(io.open(LUA_PATH, "wb"))
    f:write(generated)
    f:close()
    print("wrote " .. LUA_PATH)
end
