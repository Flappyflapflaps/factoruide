-- The copy-paste strings of the addon, outside the game, with the LibDeflate the addon bundles
-- (to check another encoder against it):
--   lua5.1 addon/tests/fgstring.lua encode-guide <file.lua>     FGG1: string of a guide's Lua text
--   lua5.1 addon/tests/fgstring.lua encode-profile <file.json>  FGP1: string of a profile's JSON
--   lua5.1 addon/tests/fgstring.lua decode <string-file>        the text inside an FGG1:/FGP1: string
-- Output on stdout.

local dir = (arg and arg[0] or ""):match("^(.*)[/\\]") or "."
local LibDeflate = assert(loadfile(dir .. "/../Factoruide_Guide/Libs/LibDeflate/LibDeflate.lua"))()

local function Read(path)
    local f = assert(io.open(path, "rb"))
    local text = f:read("*a")
    f:close()
    return text
end

local function Usage()
    io.stderr:write("usage: fgstring.lua encode-guide <file.lua> | encode-profile <file.json> | decode <string-file>\n")
    os.exit(2)
end

local command, path = arg[1], arg[2]
if not path then Usage() end
if command == "encode-guide" then
    io.write("FGG1:", LibDeflate:EncodeForPrint(LibDeflate:CompressDeflate(Read(path))), "\n")
elseif command == "encode-profile" then
    io.write("FGP1:", LibDeflate:EncodeForPrint(LibDeflate:CompressDeflate(Read(path), { level = 9 })), "\n")
elseif command == "decode" then
    local s = Read(path):gsub("%s", "")
    local prefix, body = s:match("^(FG[GP]1:)(.*)$")
    if not prefix then
        io.stderr:write("not an FGG1: or FGP1: string\n")
        os.exit(1)
    end
    local raw = LibDeflate:DecodeForPrint(body)
    local text = raw and LibDeflate:DecompressDeflate(raw)
    if not text then
        io.stderr:write("damaged string: it does not decode or decompress\n")
        os.exit(1)
    end
    io.write(text)
else
    Usage()
end
