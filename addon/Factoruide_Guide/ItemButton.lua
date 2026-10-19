-- Quest item button: the item a current step's quest asks to use, one click (or one key: see
-- the game's key bindings, "Factoruide"; their names in Guide.lua) away; left of the full guide, near the action bars with
-- the minimal one. Secure attributes and its place only change out of combat.

local FG = Factoruide
local T = FG.T

local ItemButton = { link = nil, pending = nil }
FG.ItemButton = ItemButton

local button

local function Create()
    if button or not CreateFrame then return button end
    button = CreateFrame("Button", "FGItemButton", FG.UIRoot, "SecureActionButtonTemplate")
    button:SetSize(36, 36)
    button:RegisterForClicks("AnyUp", "AnyDown")
    button:SetAttribute("type", "item")
    local icon = button:CreateTexture(nil, "ARTWORK")
    icon:SetAllPoints()
    button.icon = icon
    local cooldown = CreateFrame("Cooldown", nil, button, "CooldownFrameTemplate")
    cooldown:SetAllPoints()
    button.cooldown = cooldown
    button:SetScript("OnEnter", function(self)
        if ItemButton.link then
            GameTooltip:SetOwner(self, "ANCHOR_LEFT")
            GameTooltip:SetHyperlink(ItemButton.link)
            GameTooltip:Show()
        end
    end)
    button:SetScript("OnLeave", function() GameTooltip:Hide() end)
    FG.Skin:Style(button, "icon")
    button:Hide()
    return button
end

--- Show `link` on the button (nil hides it), now or after combat.
local function Apply(link, texture)
    if InCombatLockdown() then
        ItemButton.pending = { link = link or false, texture = texture }
        return
    end
    ItemButton.pending = nil
    local b = Create()
    if not b then return end
    ItemButton.link = link
    if not link then
        b:Hide()
        return
    end
    ItemButton:Place()
    b:SetAttribute("item", link)
    b.icon:SetTexture(texture)
    b:Show()
end

--- Put the button where the guide's look of the moment wants it (out of combat only).
function ItemButton:Place()
    if not button or InCombatLockdown() then return end
    local ui = FG.Guide and FG.Guide:UI()
    button:ClearAllPoints()
    if ui and ui:Frame() then
        ui:ItemAnchor(button)
    else
        button:SetPoint("CENTER", UIParent, "CENTER", -300, 0)
    end
end

--- The first quest item of these steps (quests in the log with a usable item).
function ItemButton:SetSteps(steps)
    if not GetQuestLogSpecialItemInfo then return end
    -- Turned off in the settings: the button shown goes.
    if not FG:Get("questItemButton") then
        if self.link then Apply(nil) end
        return
    end
    for _, step in ipairs(steps) do
        local index = step.q and GetQuestLogIndexByID and GetQuestLogIndexByID(step.q)
        if index and index > 0 then
            local link, texture = GetQuestLogSpecialItemInfo(index)
            if link then
                if link ~= self.link then Apply(link, texture) end
                return
            end
        end
    end
    if self.link then Apply(nil) end
end

FG:On("PLAYER_REGEN_ENABLED", function()
    local p = ItemButton.pending
    if p then
        Apply(p.link or nil, p.texture)
    elseif ItemButton.misplaced then
        ItemButton.misplaced = nil
        ItemButton:Place()
    end
end)
FG:On("PLAYER_LOGIN", function()
    FG:OnSetting("guideMode", function()
        if InCombatLockdown() then
            ItemButton.misplaced = true
        else
            ItemButton:Place()
        end
    end)
end)

FG:On("BAG_UPDATE_COOLDOWN", function()
    if button and button:IsShown() and ItemButton.link and GetItemCooldown then
        local id = tonumber(ItemButton.link:match("item:(%d+)"))
        local start, duration = GetItemCooldown(id)
        if start then button.cooldown:SetCooldown(start, duration) end
    end
end)
