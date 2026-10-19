-- A small window holding a text to copy or to paste (`/fg export`, `/fg import`): a title, a line
-- of explanation, a multi-line edit box, a status line and two buttons. One window, created the
-- first time it is used, dark like the app.

local _, FG = ...
local T = FG.T
local Window = {}
FG.TextWindow = Window

local frame, edit, title, hint, status, action, close
local copyText -- the text to copy, put back when the player types in the box

local function Create()
    frame = CreateFrame("Frame", "FactoruideTextWindow", UIParent, "BackdropTemplate")
    frame:SetSize(560, 340)
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
    if UISpecialFrames then table.insert(UISpecialFrames, "FactoruideTextWindow") end

    title = frame:CreateFontString(nil, "OVERLAY", "GameFontNormalLarge")
    title:SetPoint("TOPLEFT", 14, -12)
    hint = frame:CreateFontString(nil, "OVERLAY", "GameFontHighlight")
    hint:SetPoint("TOPLEFT", 14, -36)
    hint:SetPoint("TOPRIGHT", -14, -36)
    hint:SetJustifyH("LEFT")

    local box = CreateFrame("Frame", nil, frame, "BackdropTemplate")
    box:SetPoint("TOPLEFT", 12, -60)
    box:SetPoint("BOTTOMRIGHT", -12, 64)
    if box.SetBackdrop then
        box:SetBackdrop({
            bgFile = "Interface\\Buttons\\WHITE8X8",
            edgeFile = "Interface\\Buttons\\WHITE8X8",
            edgeSize = 1,
        })
        box:SetBackdropColor(0, 0, 0, 0.5)
        box:SetBackdropBorderColor(0.3, 0.3, 0.35, 1)
    end
    local scroll = CreateFrame("ScrollFrame", nil, box, "UIPanelScrollFrameTemplate")
    scroll:SetPoint("TOPLEFT", 6, -6)
    scroll:SetPoint("BOTTOMRIGHT", -28, 6)
    edit = CreateFrame("EditBox", nil, scroll)
    edit:SetMultiLine(true)
    edit:SetAutoFocus(false)
    edit:SetFontObject(ChatFontNormal or GameFontHighlightSmall)
    edit:SetWidth(500)
    -- No length limit: a guide is hundreds of kilobytes. Nothing runs per character: the text is
    -- read once, on Import.
    edit:SetMaxBytes(0)
    edit:SetMaxLetters(0)
    edit:SetScript("OnEscapePressed", function() frame:Hide() end)
    edit:SetScript("OnTextChanged", function(self, userInput)
        if userInput and copyText then
            self:SetText(copyText)
            self:HighlightText()
        end
    end)
    edit:SetScript("OnEditFocusGained", function(self)
        if copyText then self:HighlightText() end
    end)
    scroll:SetScrollChild(edit)
    -- A click anywhere in the box puts the cursor in it.
    box:EnableMouse(true)
    box:SetScript("OnMouseDown", function() edit:SetFocus() end)

    status = frame:CreateFontString(nil, "OVERLAY", "GameFontHighlight")
    status:SetPoint("BOTTOMLEFT", 14, 40)
    status:SetPoint("BOTTOMRIGHT", -14, 40)
    status:SetJustifyH("LEFT")

    close = CreateFrame("Button", nil, frame, "UIPanelButtonTemplate")
    close:SetSize(110, 24)
    close:SetPoint("BOTTOMRIGHT", -12, 10)
    close:SetScript("OnClick", function() frame:Hide() end)
    action = CreateFrame("Button", nil, frame, "UIPanelButtonTemplate")
    action:SetSize(110, 24)
    action:SetPoint("RIGHT", close, "LEFT", -8, 0)
end

--- Show the window. `opts`: title, hint, text (shown selected, to copy: typing puts it back) or
--- none (an empty box to paste into), button = { label, onClick(text) } for a second button,
--- closeLabel. Returns the frame.
function Window:Open(opts)
    if not frame then Create() end
    copyText = opts.text
    title:SetText(opts.title or "Factoruide")
    hint:SetText(opts.hint or "")
    status:SetText("")
    close:SetText(opts.closeLabel or T("Close"))
    if opts.button then
        action:SetText(opts.button.label)
        action:SetScript("OnClick", function() opts.button.onClick(edit:GetText() or "") end)
        action:Show()
    else
        action:Hide()
    end
    edit:SetText(opts.text or "")
    frame:Show()
    edit:SetFocus()
    if opts.text then
        edit:SetCursorPosition(0)
        edit:HighlightText()
    end
    return frame
end

--- A line under the box: what happened (red for an error).
function Window:Status(text, isError)
    if not frame then return end
    status:SetText(text or "")
    if isError then
        status:SetTextColor(1, 0.35, 0.3)
    else
        status:SetTextColor(0.6, 0.9, 0.6)
    end
end

function Window:Hide()
    if frame then frame:Hide() end
end

--- The window's frame, edit box, second button and status line, nil before the first use (tests).
function Window:Parts() return frame, edit, action, status end
