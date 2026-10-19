-- The addon's window when the guide part is not installed (another addon shows the guides):
-- `/fg` opens it, with a button per action of the profile part (FG.Actions, Core.lua): export
-- the character, save its profile, and the recordings' advanced ones. With the guide part, its
-- settings window shows them instead. Dark like the text window (Window.lua).

local _, FG = ...
local T = FG.T
local Menu = {}
FG.Menu = Menu

local WIDTH, PAD = 440, 14
local BUTTON_W, BUTTON_H, GAP = 200, 24, 8
local SECTION = 26

local frame
local buttons = {} -- action id -> button

local function SetText(button, text) button:SetText(text) end

--- Bring every button to its action's text and whether it can be used now.
local function Refresh()
    for _, action in ipairs(FG.Actions) do
        local button = buttons[action.id]
        if button then
            if button.armed ~= true then button:SetText(FG.ActionLabel(action)) end
            local enabled = not action.enabled or action.enabled()
            if button.SetEnabled then button:SetEnabled(enabled) end
        end
    end
end

local function Tooltip(button)
    local action = button.action
    if not action.help or not GameTooltip or not GameTooltip.SetOwner then return end
    GameTooltip:SetOwner(button, "ANCHOR_RIGHT")
    GameTooltip:SetText(FG.ActionLabel(action), 1, 1, 1, 1, true)
    if GameTooltip.AddLine then GameTooltip:AddLine(T(action.help), 0.8, 0.82, 0.86, true) end
    GameTooltip:Show()
end

local function Create()
    frame = CreateFrame("Frame", "FactoruideMenu", UIParent, "BackdropTemplate")
    frame:SetPoint("CENTER")
    frame:SetFrameStrata("DIALOG")
    frame:SetClampedToScreen(true)
    frame:EnableMouse(true)
    frame:SetMovable(true)
    frame:RegisterForDrag("LeftButton")
    frame:SetScript("OnDragStart", frame.StartMoving)
    frame:SetScript("OnDragStop", frame.StopMovingOrSizing)
    if frame.SetBackdrop then
        frame:SetBackdrop({
            bgFile = "Interface\\Buttons\\WHITE8X8",
            edgeFile = "Interface\\Buttons\\WHITE8X8",
            edgeSize = 1,
        })
        frame:SetBackdropColor(0.05, 0.08, 0.13, 0.96)
        frame:SetBackdropBorderColor(0.2, 0.45, 0.8, 1)
    end
    -- Escape closes it.
    if UISpecialFrames then table.insert(UISpecialFrames, "FactoruideMenu") end

    local title = frame:CreateFontString(nil, "OVERLAY", "GameFontNormalLarge")
    title:SetPoint("TOPLEFT", PAD, -12)
    title:SetText("Factoruide")
    local close = CreateFrame("Button", nil, frame, "UIPanelCloseButton")
    close:SetPoint("TOPRIGHT", -2, -2)
    close:SetScript("OnClick", function() frame:Hide() end)

    -- The sections in their order, two buttons a row.
    local y = 40
    for _, section in ipairs(FG.ACTION_SECTIONS) do
        local actions = {}
        for _, action in ipairs(FG.Actions) do
            if action.section == section then actions[#actions + 1] = action end
        end
        if #actions > 0 then
            local head = frame:CreateFontString(nil, "OVERLAY", "GameFontNormal")
            head:SetPoint("TOPLEFT", PAD, -(y + 6))
            head:SetText(T(section))
            y = y + SECTION
            for i, action in ipairs(actions) do
                local column = (i - 1) % 2
                local button = CreateFrame("Button", nil, frame, "UIPanelButtonTemplate")
                button.action = action
                button:SetSize(BUTTON_W, BUTTON_H)
                button:SetPoint(
                    "TOPLEFT",
                    PAD + column * (BUTTON_W + GAP),
                    -(y + math.floor((i - 1) / 2) * (BUTTON_H + GAP))
                )
                button:SetScript("OnClick", function(self)
                    FG.ClickAction(action, self, SetText)
                    Refresh()
                end)
                button:SetScript("OnEnter", Tooltip)
                button:SetScript("OnLeave", function()
                    if GameTooltip and GameTooltip.Hide then GameTooltip:Hide() end
                end)
                buttons[action.id] = button
            end
            y = y + math.ceil(#actions / 2) * (BUTTON_H + GAP) + 4
        end
    end
    frame:SetSize(WIDTH, y + PAD)
    frame:Hide()
end

function Menu:Open()
    if not frame then Create() end
    Refresh()
    frame:Show()
end

function Menu:Toggle()
    if frame and frame:IsShown() then return frame:Hide() end
    self:Open()
end

--- The window and its buttons (action id -> button), nil before the first use (tests).
function Menu:Parts() return frame, buttons end
