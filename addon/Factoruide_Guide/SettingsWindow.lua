-- The settings window (`/fg config`, the guide's settings button, the game's options: AddOns,
-- Factoruide): the guides (which one the character follows, imports), every setting of the
-- schema (SettingsSchema.lua) in its group and section, and the addon's language. A change is saved (FG:Save) and applied at once; the few that need a /reload
-- say so. Mouse over a row: its help; right click: back to its default.

local FG = Factoruide
local T = FG.T
local Skin = FG.Skin
local SettingsWindow = {}
FG.SettingsWindow = SettingsWindow

local WIDTH, HEIGHT = 680, 520
local NAV = 160 -- the groups' column
local TOP, BOTTOM = 44, 40 -- header and footer
local ROW, SECTION = 26, 30
local VIEW = HEIGHT - TOP - BOTTOM

local frame, scroll, thumb, footer, reloadButton
local pages, tabs = {}, {}
local current -- the page shown
local offset = 0 -- how far it is scrolled
local reloadNeeded = false

-- Language names in their own language.
local LANGUAGE_NAMES = {
    enUS = "English",
    frFR = "Français",
    deDE = "Deutsch",
    esES = "Español (España)",
    esMX = "Español (México)",
    ptBR = "Português (Brasil)",
    ruRU = "Русский",
    koKR = "한국어",
    zhCN = "简体中文",
    zhTW = "繁體中文",
}

-- The addon's language, beside the schema's settings (the same row, its own values).
local LANGUAGE = {
    key = "locale",
    kind = "select",
    label = "Addon language",
    help = "The addon's texts. The guides keep the language they were made in.",
    reload = true,
}

--- Whether the setting `option` depends on is on (or it depends on none).
local function RequirementMet(option)
    if not option.requires then return true end
    local v = FG:Get(option.requires)
    return v ~= false and v ~= "off"
end

--- The choices of a select: { value, label } (the language: the game's, then every language).
local function Choices(option)
    if option ~= LANGUAGE then return option.options end
    local game = GetLocale and GetLocale() or "enUS"
    local out = { { value = false, label = string.format(T("Game's language (%s)"), LANGUAGE_NAMES[game] or game) } }
    for _, lang in ipairs(FG.LANGUAGES) do
        out[#out + 1] = { value = lang, label = LANGUAGE_NAMES[lang] }
    end
    return out
end

local function Value(option)
    if option == LANGUAGE then return FG.ChosenLang() or false end
    return FG:Get(option.key)
end

local function Default(option)
    if option == LANGUAGE then return false end
    return option.default
end

--- A setting's value as shown.
local function Shown(option, v)
    if option.kind == "bool" then return v and T("On") or T("Off") end
    if option.kind == "number" then
        if option.percent then return string.format("%d%%", math.floor(v * 100 + 0.5)) end
        if option.step < 1 then return string.format("%.1f", v) end
        return string.format("%d", v)
    end
    for _, choice in ipairs(Choices(option)) do
        if choice.value == v then return option == LANGUAGE and choice.label or T(choice.label) end
    end
    return tostring(v)
end

--- Save a setting chosen in the window.
local function Choose(option, v)
    -- The language: false is the game's.
    if option == LANGUAGE and not v then v = nil end
    FG:Save(option.key, v)
    if option.reload then reloadNeeded = true end
    SettingsWindow:Refresh()
end

--- The next value of a number (`dir` steps) or a select (the next choice, around).
local function Step(option, dir)
    local v = Value(option)
    if option.kind == "number" then
        local n = math.floor((v - option.min) / option.step + 0.5) + dir
        v = option.min + n * option.step
        v = math.floor(v * 1e6 + 0.5) / 1e6
        return math.max(option.min, math.min(option.max, v))
    end
    local choices = Choices(option)
    for i, choice in ipairs(choices) do
        if choice.value == v then return choices[(i - 1 + dir) % #choices + 1].value end
    end
    return choices[1].value
end

-- Widgets ---------------------------------------------------------------------------------

--- A small flat button with a text.
local function SmallButton(parent, text, width)
    local b = CreateFrame("Button", nil, parent)
    b:SetSize(width or 20, 20)
    b.bg = Skin:Fill(b, "BACKGROUND", "text", 0.08)
    b.bg:SetAllPoints()
    b.label = Skin:Text(b, 12, "text")
    b.label:SetPoint("CENTER")
    b.label:SetText(text)
    b:SetHighlightTexture(Skin.WHITE, "ADD")
    local hl = b:GetHighlightTexture()
    if hl and hl.SetVertexColor then hl:SetVertexColor(1, 1, 1, 0.08) end
    return b
end

local function Tooltip(row)
    local option = row.option
    if not GameTooltip or not GameTooltip.SetOwner then return end
    GameTooltip:SetOwner(row, "ANCHOR_RIGHT")
    GameTooltip:SetText(T(option.label), 1, 1, 1, 1, true)
    local add = GameTooltip.AddLine
    if add then
        if option.help then GameTooltip:AddLine(T(option.help), 0.8, 0.82, 0.86, true) end
        if option.reload then GameTooltip:AddLine(T("Applied after /reload."), 1, 0.6, 0.3, true) end
        if not RequirementMet(option) then
            local needed = FG.settingOptions[option.requires]
            GameTooltip:AddLine(
                string.format(T("Needs: %s"), needed and T(needed.label) or option.requires),
                1,
                0.4,
                0.3,
                true
            )
        end
        GameTooltip:AddLine(string.format(T("Default: %s"), Shown(option, Default(option))), 0.6, 0.65, 0.7, true)
        GameTooltip:AddLine(T("Right click: back to the default"), 0.6, 0.65, 0.7, true)
    end
    GameTooltip:Show()
end

--- One setting's row: its label, its control (a check box, or < value > for the others).
local function Row(parent, option, y, width)
    local row = CreateFrame("Button", nil, parent)
    row.option = option
    row:SetSize(width, ROW)
    row:SetPoint("TOPLEFT", 0, -y)
    row:RegisterForClicks("LeftButtonUp", "RightButtonUp")
    row.hover = Skin:Fill(row, "BACKGROUND", "text", 0.04)
    row.hover:SetAllPoints()
    row.hover:Hide()
    row.label = Skin:Text(row, 12, "text")
    row.label:SetPoint("LEFT", 8, 0)
    row.label:SetPoint("RIGHT", -190, 0)
    local label = T(option.label)
    if option.reload then label = label .. "  " .. Skin.Hex("dim") .. "/reload|r" end
    row.label:SetText(label)

    if option.kind == "bool" then
        local box = CreateFrame("Button", nil, row)
        box:SetSize(18, 18)
        box:SetPoint("RIGHT", -12, 0)
        local edge = Skin:Fill(box, "BACKGROUND", "edge", 1)
        edge:SetAllPoints()
        local inside = Skin:Fill(box, "BORDER", "bg", 1)
        inside:SetPoint("TOPLEFT", 1, -1)
        inside:SetPoint("BOTTOMRIGHT", -1, 1)
        box.check = box:CreateTexture(nil, "ARTWORK")
        box.check:SetTexture("Interface\\Buttons\\UI-CheckBox-Check")
        box.check:SetPoint("TOPLEFT", -3, 3)
        box.check:SetPoint("BOTTOMRIGHT", 3, -3)
        box:SetScript("OnClick", function() row:Click("LeftButton") end)
        row.box = box
    else
        row.value = Skin:Text(row, 12, "accent")
        row.value:SetWidth(130)
        row.value:SetJustifyH("CENTER")
        row.value:SetPoint("RIGHT", -34, 0)
        row.prev = SmallButton(row, option.kind == "number" and "-" or "<")
        row.prev:SetPoint("RIGHT", row.value, "LEFT", -4, 0)
        row.next = SmallButton(row, option.kind == "number" and "+" or ">")
        row.next:SetPoint("RIGHT", -10, 0)
        row.prev:SetScript("OnClick", function()
            if RequirementMet(option) then Choose(option, Step(option, -1)) end
        end)
        row.next:SetScript("OnClick", function()
            if RequirementMet(option) then Choose(option, Step(option, 1)) end
        end)
    end

    row:SetScript("OnClick", function(_, button)
        if not RequirementMet(option) then return end
        if button == "RightButton" then
            Choose(option, option == LANGUAGE and false or nil)
        elseif option.kind == "bool" then
            Choose(option, not Value(option))
        else
            Choose(option, Step(option, 1))
        end
    end)
    row:SetScript("OnEnter", function(self)
        self.hover:Show()
        Tooltip(self)
    end)
    row:SetScript("OnLeave", function(self)
        self.hover:Hide()
        if GameTooltip and GameTooltip.Hide then GameTooltip:Hide() end
    end)
    return row
end

--- Update a row to its setting's value and whether it can change.
local function UpdateRow(row)
    local option = row.option
    local v = Value(option)
    if option.kind == "bool" then
        row.box.check:SetShown(v == true)
    else
        row.value:SetText(Shown(option, v))
    end
    row:SetAlpha(RequirementMet(option) and 1 or 0.4)
end

-- Pages -----------------------------------------------------------------------------------

--- A page of rows: the group's sections (the options of this edition only).
local function Page(title, sections)
    local page = CreateFrame("Frame", nil, scroll)
    local width = WIDTH - NAV - 34
    page:SetWidth(width)
    page.title, page.rows = title, {}
    local y = 4
    for _, section in ipairs(sections) do
        local options = {}
        for _, option in ipairs(section.options) do
            if not option.only or option.only == FG.Edition then options[#options + 1] = option end
        end
        if #options > 0 then
            if section.title then
                local head = Skin:Text(page, 13, "cyan")
                head:SetPoint("TOPLEFT", 8, -(y + 10))
                head:SetText(T(section.title))
                y = y + SECTION
            end
            for _, option in ipairs(options) do
                page.rows[#page.rows + 1] = Row(page, option, y, width)
                y = y + ROW
            end
            y = y + 8
        end
    end
    page:SetHeight(y)
    page.height = y
    page:Hide()
    return page
end

-- The guides page: the guides in use (imported, or written by the desktop app), the one this
-- character follows, a button to follow another, to delete an imported one, to import more.

local GUIDE_ROW = 46
local guidesPage
local CONFIRM_SECONDS = 4

--- "Race Class" of a guide (the game's names, the class in its color), its numbers when unknown.
local function RaceClass(r)
    local race = C_CreatureInfo and C_CreatureInfo.GetRaceInfo and r.race and C_CreatureInfo.GetRaceInfo(r.race)
    local class = C_CreatureInfo and C_CreatureInfo.GetClassInfo and r.class and C_CreatureInfo.GetClassInfo(r.class)
    local raceName = race and race.raceName
    local className, classFile = class and class.className, class and class.classFile
    -- Without C_CreatureInfo: the character's own names when they are its race and class.
    local myRaceName, _, myRace = UnitRace("player")
    local myClassName, myClassFile, myClass = UnitClass("player")
    if not raceName and r.race == FG.Readable(myRace) then raceName = myRaceName end
    if not className and r.class == FG.Readable(myClass) then
        className, classFile = myClassName, myClassFile
    end
    raceName = raceName or (r.race and "#" .. tostring(r.race)) or "?"
    className = className or (r.class and "#" .. tostring(r.class)) or "?"
    local color = classFile and RAID_CLASS_COLORS and RAID_CLASS_COLORS[classFile]
    if color and color.r then
        className = string.format("|cff%02x%02x%02x%s|r", color.r * 255, color.g * 255, color.b * 255, className)
    end
    return raceName .. " " .. className
end

--- The line under a guide's name: who it is for, its levels, when it was computed, where from.
local function GuideDetails(guide)
    local r = guide.route
    local parts = { RaceClass(r) }
    if r.fromLevel and r.toLevel then parts[#parts + 1] = string.format(T("levels %d-%d"), r.fromLevel, r.toLevel) end
    if guide.created and guide.created > 0 and date then
        parts[#parts + 1] = string.format(T("computed on %s"), date("%Y-%m-%d", guide.created))
    end
    if guide.version then parts[#parts + 1] = "v" .. tostring(guide.version) end
    parts[#parts + 1] = guide.source == "import" and T("imported") or T("from the app (managed by the app)")
    return table.concat(parts, "  ·  ")
end

--- A guide's row: name, details, Follow and Delete (asks again before deleting).
local function GuideRow(parent, width)
    local row = CreateFrame("Frame", nil, parent)
    row:SetSize(width, GUIDE_ROW)
    row.lit = Skin:Fill(row, "BACKGROUND", "blue", 0.12)
    row.lit:SetAllPoints()
    row.name = Skin:Text(row, 13, "text")
    row.name:SetPoint("TOPLEFT", 8, -7)
    row.name:SetPoint("RIGHT", -170, 0)
    row.details = Skin:Text(row, 10, "text2")
    row.details:SetPoint("TOPLEFT", 8, -26)
    row.details:SetPoint("RIGHT", -170, 0)
    row.follow = SmallButton(row, T("Follow"), 84)
    row.follow:SetPoint("RIGHT", -86, 0)
    row.follow.on = Skin:Fill(row.follow, "BORDER", "blue", 0.5)
    row.follow.on:SetAllPoints()
    row.follow:SetScript("OnClick", function()
        if row.guide and FG.Guide then FG.Guide:Choose(row.guide.name) end
    end)
    row.delete = SmallButton(row, T("Delete"), 74)
    row.delete:SetPoint("RIGHT", -8, 0)
    row.delete:SetScript("OnClick", function(self)
        local name = row.guide and row.guide.name
        if not name then return end
        if self.armed ~= name then
            -- First click: asks; a second one within a few seconds deletes.
            self.armed = name
            self.label:SetText(T("Sure?"))
            C_Timer.After(CONFIRM_SECONDS, function()
                if self.armed == name then
                    self.armed = false
                    self.label:SetText(T("Delete"))
                end
            end)
            return
        end
        self.armed = false
        self.label:SetText(T("Delete"))
        FG.Import:Delete(name)
        FG:Print(T("guide %s deleted"), name)
    end)
    return row
end

--- Lay the guides page out again (guides added, deleted, followed).
local function LayoutGuides()
    local page = guidesPage
    if not page then return end
    local list = FG.Import and FG.Import:List() or {}
    local followed = FG.Guide and FG.Guide.RouteName and FG.Guide:RouteName()
    local y = 40
    for i, guide in ipairs(list) do
        local row = page.guideRows[i] or GuideRow(page, page:GetWidth())
        page.guideRows[i] = row
        row.guide = guide
        row:ClearAllPoints()
        row:SetPoint("TOPLEFT", 0, -y)
        local isFollowed = guide.name == followed
        row.name:SetText(
            guide.name .. (guide.fits and "" or "  " .. Skin.Hex("dim") .. T("(another character)") .. "|r")
        )
        row.details:SetText(GuideDetails(guide))
        row.lit:SetShown(isFollowed)
        row.follow.on:SetShown(isFollowed)
        row.follow.label:SetText(isFollowed and T("Followed") or T("Follow"))
        row.delete:SetShown(guide.source == "import")
        if row.delete.armed ~= guide.name then
            row.delete.armed = false
            row.delete.label:SetText(T("Delete"))
        end
        row:Show()
        y = y + GUIDE_ROW + 4
    end
    for i = #list + 1, #page.guideRows do
        page.guideRows[i].guide = false
        page.guideRows[i]:Hide()
    end
    page.empty:SetShown(#list == 0)
    if #list == 0 then y = y + 40 end
    page:SetHeight(y)
    page.height = y
end

--- The guides page (no setting rows).
local function GuidesPage()
    local page = CreateFrame("Frame", nil, scroll)
    local width = WIDTH - NAV - 34
    page:SetWidth(width)
    page.title, page.rows, page.guideRows = "Guides", {}, {}
    page.import = SmallButton(page, T("Import a guide…"))
    page.import:SetWidth((tonumber(page.import.label:GetStringWidth()) or 100) + 20)
    page.import:SetPoint("TOPRIGHT", -8, -8)
    page.import:SetScript("OnClick", function()
        if FG.Import then FG.Import:Open() end
    end)
    page.head = Skin:Text(page, 11, "text2")
    page.head:SetPoint("TOPLEFT", 8, -12)
    page.head:SetPoint("RIGHT", page.import, "LEFT", -8, 0)
    page.head:SetText(T("The guide this character follows: click Follow."))
    page.empty = Skin:Text(page, 12, "text2")
    page.empty:SetWordWrap(true)
    page.empty:SetPoint("TOPLEFT", 8, -48)
    page.empty:SetPoint("RIGHT", -8, 0)
    page.empty:SetText(T("No guide yet: import one with the button above, or send it from the Factoruide desktop app."))
    page:Hide()
    guidesPage = page
    LayoutGuides()
    return page
end

local function UpdateThumb()
    if not current then return end
    local range = math.max(0, current.height - VIEW)
    if range <= 0 then
        thumb:Hide()
        return
    end
    local size = math.max(24, VIEW * VIEW / current.height)
    thumb:SetHeight(size)
    thumb:ClearAllPoints()
    thumb:SetPoint("TOPRIGHT", scroll, "TOPRIGHT", 10, -offset / range * (VIEW - size))
    thumb:Show()
end

--- Show the page `i` (its tab lit).
function SettingsWindow:Show(i)
    for j, page in ipairs(pages) do
        page:Hide()
        tabs[j].lit:SetShown(j == i)
        tabs[j].label:SetTextColor(unpack(Skin.colors[j == i and "text" or "text2"]))
    end
    current = pages[i]
    current:Show()
    scroll:SetScrollChild(current)
    offset = 0
    scroll:SetVerticalScroll(0)
    self.page = i
    UpdateThumb()
end

local function Create()
    frame = CreateFrame("Frame", "FactoruideSettings", UIParent, "BackdropTemplate")
    frame:SetSize(WIDTH, HEIGHT)
    frame:SetPoint("CENTER")
    -- Over the game's options too (opened from there).
    frame:SetFrameStrata("FULLSCREEN_DIALOG")
    frame:SetClampedToScreen(true)
    frame:EnableMouse(true)
    frame:SetMovable(true)
    frame:RegisterForDrag("LeftButton")
    frame:SetScript("OnDragStart", frame.StartMoving)
    frame:SetScript("OnDragStop", frame.StopMovingOrSizing)
    frame.title = Skin:Text(frame, 15, "text")
    frame.title:SetPoint("TOPLEFT", 16, -14)
    frame.title:SetText(T("Factoruide settings"))
    Skin:Style(frame, "window", { frame.title })
    -- Escape closes it.
    if UISpecialFrames then table.insert(UISpecialFrames, "FactoruideSettings") end

    local close = SmallButton(frame, "x", 22)
    close:SetPoint("TOPRIGHT", -10, -10)
    close:SetScript("OnClick", function() frame:Hide() end)

    local line = Skin:Fill(frame, "ARTWORK", "line", 1)
    line:SetPoint("TOPLEFT", NAV, -TOP + 4)
    line:SetPoint("BOTTOMLEFT", NAV, BOTTOM - 4)
    line:SetWidth(1)

    scroll = CreateFrame("ScrollFrame", nil, frame)
    scroll:SetPoint("TOPLEFT", NAV + 8, -TOP)
    scroll:SetSize(WIDTH - NAV - 34, VIEW)
    scroll:EnableMouseWheel(true)
    scroll:SetScript("OnMouseWheel", function(self, delta)
        if not current then return end
        offset = math.max(0, math.min(current.height - VIEW, offset - delta * ROW * 2))
        self:SetVerticalScroll(offset)
        UpdateThumb()
    end)
    thumb = Skin:Fill(frame, "OVERLAY", "edge", 1)
    thumb:SetWidth(4)

    -- The guides, the groups, then the language.
    local groups = { { title = "Guides", page = GuidesPage } }
    for _, group in ipairs(FG.SettingsSchema or {}) do
        groups[#groups + 1] = group
    end
    groups[#groups + 1] = { title = "Language", sections = { { options = { LANGUAGE } } } }
    for i, group in ipairs(groups) do
        pages[i] = group.page and group.page() or Page(group.title, group.sections)
        local tab = CreateFrame("Button", nil, frame)
        tab:SetSize(NAV - 16, 26)
        tab:SetPoint("TOPLEFT", 8, -TOP - (i - 1) * 28)
        tab.lit = Skin:Fill(tab, "BACKGROUND", "blue", 0.3)
        tab.lit:SetAllPoints()
        tab.label = Skin:Text(tab, 12, "text2")
        tab.label:SetPoint("LEFT", 10, 0)
        tab.label:SetText(T(group.title))
        tab:SetScript("OnClick", function() SettingsWindow:Show(i) end)
        tabs[i] = tab
    end

    reloadButton = SmallButton(frame, T("Reload the interface"))
    reloadButton:SetWidth((tonumber(reloadButton.label:GetStringWidth()) or 120) + 20)
    reloadButton:SetPoint("BOTTOMRIGHT", -14, 10)
    reloadButton:SetScript("OnClick", function() ReloadUI() end)
    footer = Skin:Text(frame, 11, "orange")
    footer:SetPoint("BOTTOMLEFT", NAV + 16, 14)
    footer:SetPoint("RIGHT", reloadButton, "LEFT", -8, 0)
    footer:SetText(T("Some changes are applied after /reload."))

    frame:Hide()
    SettingsWindow:Show(1)
end

--- Bring every row to its setting's value.
function SettingsWindow:Refresh()
    if not frame then return end
    LayoutGuides()
    for _, page in ipairs(pages) do
        for _, row in ipairs(page.rows) do
            UpdateRow(row)
        end
    end
    footer:SetShown(reloadNeeded)
    reloadButton:SetShown(reloadNeeded)
end

function SettingsWindow:Open()
    if not frame then Create() end
    self:Refresh()
    frame:Show()
end

function SettingsWindow:Close()
    if frame then frame:Hide() end
end

--- Bring the guides page to the guides in use and the one followed.
function SettingsWindow:RefreshGuides()
    LayoutGuides()
    if current == guidesPage then UpdateThumb() end
end

--- Open the window on its guides page.
function SettingsWindow:OpenGuides()
    self:Open()
    for i, page in ipairs(pages) do
        if page == guidesPage then self:Show(i) end
    end
end

--- The guides page's rows (`.guide`, `.follow`, `.delete`), nil before the first use (tests).
function SettingsWindow:GuideRows() return guidesPage and guidesPage.guideRows end

function SettingsWindow:Toggle()
    if frame and frame:IsShown() then return frame:Hide() end
    self:Open()
end

--- Whether the window asks for a /reload (a setting applied after it was changed).
function SettingsWindow:ReloadShown() return reloadNeeded end

--- The window's frame and its pages (rows: `.option`), nil before the first use (tests).
function SettingsWindow:Parts() return frame, pages end

function FG:OpenSettings() SettingsWindow:Toggle() end

-- In the game's options (AddOns): a page with a button opening the window.
FG:On("PLAYER_LOGIN", function()
    local panel = CreateFrame("Frame")
    panel.name = "Factoruide"
    local title = panel:CreateFontString(nil, "ARTWORK", "GameFontNormalLarge")
    title:SetPoint("TOPLEFT", 16, -16)
    title:SetText(T("Factoruide settings"))
    local open = CreateFrame("Button", nil, panel, "UIPanelButtonTemplate")
    open:SetSize(220, 26)
    open:SetPoint("TOPLEFT", title, "BOTTOMLEFT", 0, -12)
    open:SetText(T("Open the settings"))
    open:SetScript("OnClick", function() SettingsWindow:Open() end)
    local hint = panel:CreateFontString(nil, "ARTWORK", "GameFontHighlight")
    hint:SetPoint("TOPLEFT", open, "BOTTOMLEFT", 0, -10)
    hint:SetText(T("Also with /fg config in the chat."))
    if Settings and Settings.RegisterCanvasLayoutCategory and Settings.RegisterAddOnCategory then
        local category = Settings.RegisterCanvasLayoutCategory(panel, "Factoruide")
        Settings.RegisterAddOnCategory(category)
        SettingsWindow.category = category
    elseif InterfaceOptions_AddCategory then
        InterfaceOptions_AddCategory(panel)
    end
    SettingsWindow.panel = panel
end)
