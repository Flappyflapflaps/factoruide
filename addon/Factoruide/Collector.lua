-- Records quest data observed in game: who gives and takes quests, where, at which level,
-- how long quests take, kills (with the XP they gave), loot sources and flight masters.
-- `fg collector` imports it.
--
-- Events are stored as compact rows:
--   { seq, time, event, questID, entityType, entityID, uiMapID, x, y, level, data }
-- `level` is the character's level (for kills: before the kill's XP).

local _, FG = ...
local QuestLog = FG.QuestLog
local Collector = {}
FG.Collector = Collector

local char -- this character's saved table
local interaction -- entity the player is currently talking to
local seen = {} -- per-session dedupe for noisy events

local function Record(event, questID, entityType, entityID, data, mapID, x, y, level)
    if not char then return end
    -- An empty table would be read back as a JSON array, not an object.
    if data and next(data) == nil then data = nil end
    if not mapID then
        mapID, x, y = FG.PlayerPosition()
    end
    char.seq = char.seq + 1
    table.insert(char.events, {
        char.seq,
        time(),
        event,
        questID or false,
        entityType or false,
        entityID or false,
        mapID or false,
        x or false,
        y or false,
        level or UnitLevel("player"),
        data or false,
    })
    FG:Debug("%s q=%s %s=%s", event, tostring(questID), tostring(entityType), tostring(entityID))
end

--- Record once per session for a given key (positions rounded to ~1% of the map).
local function RecordOnce(key, ...)
    if seen[key] then return end
    seen[key] = true
    Record(...)
end

local function CurrentInteraction()
    local entityType, entityID = FG.ParseGUID(UnitGUID("npc"))
    if entityType == "npc" or entityType == "object" then return entityType, entityID end
    return nil
end

local function Remember()
    local entityType, entityID = CurrentInteraction()
    if entityType then interaction = { type = entityType, id = entityID, at = GetTime() } end
end

--- Entity the player interacted with in the last few seconds.
local function LastInteraction()
    if interaction and GetTime() - interaction.at < 10 then return interaction.type, interaction.id end
    return CurrentInteraction()
end

local function SnapshotCharacter()
    local _, raceFile, raceID = UnitRace("player")
    local _, classFile = UnitClass("player")
    char.race = raceFile
    char.raceID = raceID
    char.class = classFile
    char.faction = UnitFactionGroup("player")
    char.level = UnitLevel("player")
    local completed = QuestLog.GetAllCompletedQuestIDs and QuestLog.GetAllCompletedQuestIDs()
    if completed then char.completed = completed end
    local log = {}
    for i = 1, QuestLog.GetNumQuestLogEntries() do
        local info = QuestLog.GetInfo(i)
        if info and not info.isHeader and info.questID then table.insert(log, info.questID) end
    end
    char.questLog = log
end

FG:On("PLAYER_LOGIN", function()
    local key = FG:CharacterKey()
    FactoruideDB.characters[key] = FactoruideDB.characters[key] or { seq = 0, events = {} }
    char = FactoruideDB.characters[key]
    SnapshotCharacter()
end)

FG:On("PLAYER_LOGOUT", function()
    if char then SnapshotCharacter() end
end)

-- Quest offers: NPCs with a gossip window or a quest greeting list their available quests.
local function RecordOffers(entityType, entityID, quests)
    for _, q in ipairs(quests) do
        local mapID, x, y = FG.PlayerPosition()
        RecordOnce("offer:" .. entityID .. ":" .. q.questID, "offer", q.questID, entityType, entityID, {
            level = q.questLevel,
            trivial = q.isTrivial or nil,
            repeatable = q.repeatable or nil,
        }, mapID, x, y)
    end
end

FG:On("GOSSIP_SHOW", function()
    Remember()
    local entityType, entityID = CurrentInteraction()
    if entityType then
        RecordOffers(entityType, entityID, C_GossipInfo.GetAvailableQuests())
        for _, q in ipairs(C_GossipInfo.GetActiveQuests()) do
            RecordOnce(
                "active:" .. entityID .. ":" .. q.questID,
                "active",
                q.questID,
                entityType,
                entityID,
                { complete = q.isComplete or nil }
            )
        end
    end
end)

FG:On("QUEST_GREETING", function()
    Remember()
    local entityType, entityID = CurrentInteraction()
    if not entityType then return end
    local quests = {}
    for i = 1, GetNumAvailableQuests() do
        local isTrivial, _, isRepeatable, _, questID = GetAvailableQuestInfo(i)
        if questID then
            table.insert(quests, { questID = questID, isTrivial = isTrivial, repeatable = isRepeatable })
        end
    end
    RecordOffers(entityType, entityID, quests)
    for i = 1, GetNumActiveQuests() do
        local questID = GetActiveQuestID(i)
        if questID then RecordOnce("active:" .. entityID .. ":" .. questID, "active", questID, entityType, entityID) end
    end
end)

FG:On("QUEST_DETAIL", function(questStartItemID)
    Remember()
    local questID = GetQuestID()
    if not questID or questID == 0 then return end
    if questStartItemID and questStartItemID ~= 0 then
        RecordOnce("detail:item:" .. questID, "detail", questID, "item", questStartItemID)
    else
        local entityType, entityID = LastInteraction()
        RecordOnce("detail:" .. tostring(entityID) .. ":" .. questID, "detail", questID, entityType, entityID)
    end
end)

FG:On("QUEST_ACCEPTED", function(questID)
    local entityType, entityID = LastInteraction()
    local info = QuestLog.GetInfo(QuestLog.GetLogIndexForQuestID(questID) or 0)
    Record("accept", questID, entityType, entityID, {
        title = info and info.title,
        level = info and info.level,
    })
    interaction = nil
end)

FG:On("QUEST_PROGRESS", Remember)
FG:On("QUEST_COMPLETE", Remember)

FG:On("QUEST_TURNED_IN", function(questID, xpReward, moneyReward)
    local entityType, entityID = LastInteraction()
    Record("turnin", questID, entityType, entityID, { xp = xpReward, money = moneyReward })
end)

FG:On("QUEST_REMOVED", function(questID, wasReplayQuest)
    -- Also fires on turn-in; the importer keeps it only when no turn-in follows.
    Record("removed", questID, nil, nil, { replay = wasReplayQuest or nil })
end)

FG:On("PLAYER_LEVEL_UP", function(level) Record("level", nil, nil, nil, { level = level }) end)

-- Creature positions: kills and NPCs the player targets or mouses over.
local function RecordUnit(unit, event)
    if UnitIsPlayer(unit) then return end
    local entityType, entityID = FG.ParseGUID(UnitGUID(unit))
    if entityType ~= "npc" then return end
    local mapID, x, y = FG.PlayerPosition()
    if not x then return end
    local key = string.format("%s:%d:%d:%d:%d", event, entityID, mapID, x, y)
    RecordOnce(key, event, nil, "npc", entityID, {
        level = FG.Readable(UnitLevel(unit)),
        name = FG.Readable(UnitName(unit)),
        reaction = FG.Readable(UnitReaction(unit, "player")),
    }, mapID, x, y)
end

-- Kills and the XP they gave, to check the kill XP formula of each edition. PARTY_KILL tells the
-- kills of the group (the character, its pet, a party member); the XP is read on the XP bar
-- (UnitXP deltas on PLAYER_XP_UPDATE), as the combat log is closed to addons on Forever and the
-- chat lines are localized. The two events come in no fixed order: kills and XP gains close in
-- time make a batch. A batch of one kill gets the XP, a batch of several gets `multi` (the XP
-- cannot be split between them), a batch next to a quest turn-in gets none (its reward is mixed
-- in). The rested bonus is what the rested pool lost: the game takes the bonus from it.
--
-- `kill` data: the mob's `level` (absent for "??"), `name`, `rank` (UnitClassification when not
-- "normal": elite, rareelite, worldboss, rare, trivial, minus); `xp` gained (0: none), `rested`
-- (the rested bonus included in `xp`); `multi` (kills in the batch) and `batchxp` (their XP
-- together) instead of `xp`; `group` (members, when grouped), `raid`; `by` who killed it ("pet",
-- "party", "unknown"; absent: the character). No `xp` either: not measured (turn-in nearby, level
-- cap). The event's level is the character's before the kill's XP.
local KILL_WINDOW = 2 -- seconds after the first kill of a batch its XP may still come
local XP_EARLY = 0.5 -- seconds an XP gain may come before its kill
local TURNIN_MARGIN = 2 -- seconds before a batch a quest turn-in still mixes its XP in

local mobs, mobCount = {}, 0 -- GUID -> the mob as last read: { id, name, level, rank }
local target = {} -- the current target: { mob = as read when targeted, done = dead or counted }
local xpBar -- last reading of the XP bar: { xp, max, rested, level }
local gains = {} -- XP gains waiting for their kill: { at, xp, rested, level }
local batch -- kills waiting for their XP: { at, kills }
local lastTurnIn

--- What can be read of a creature unit now (nil for players and their pets), and its GUID when
--- readable (Forever hides the identity of creatures in combat: GUID and name are secret).
local function ReadMob(unit)
    unit = FG.Readable(unit)
    if not unit or not UnitExists(unit) or UnitIsPlayer(unit) then return nil end
    local guid = FG.Readable(UnitGUID(unit))
    local entityType, id = FG.ParseGUID(guid)
    if guid and entityType ~= "npc" then return nil end
    local level = FG.Readable(UnitLevel(unit))
    local rank = FG.Readable(UnitClassification(unit))
    return {
        id = id,
        name = FG.Readable(UnitName(unit)),
        level = level and level > 0 and level or nil,
        rank = rank ~= "normal" and rank or nil,
    },
        guid
end

local function LearnMob(unit)
    local mob, guid = ReadMob(unit)
    if guid then
        if mobCount >= 500 then
            wipe(mobs)
            mobCount = 0
        end
        if not mobs[guid] then mobCount = mobCount + 1 end
        mobs[guid] = mob
    end
    return mob
end

--- The target as a kill, once: when it is dead and was alive when targeted (`watched`: only if
--- it is still the same target). Its ID and name as read when targeted if they are secret now.
local function TargetKill(watched)
    if target.done or (watched and watched ~= target) or not UnitIsDead("target") then return nil end
    local mob = ReadMob("target")
    if not mob then return nil end
    target.done = true
    local seen = target.mob or {}
    return { id = mob.id or seen.id, name = mob.name or seen.name, level = mob.level or seen.level, rank = mob.rank }
end

--- The creature a PARTY_KILL is about: known by its GUID, or (secret GUID, Forever in combat) the
--- target once dead; `watch` when the target may still die from it (its death can come after the
--- event). Nil when it is not a creature.
local function KilledMob(targetGUID)
    local guid = FG.Readable(targetGUID)
    if guid then
        local entityType, id = FG.ParseGUID(guid)
        if entityType ~= "npc" then return nil end
        local mob = mobs[guid] or {}
        if FG.Readable(UnitGUID("target")) == guid then
            target.done = true
            mob = ReadMob("target") or mob
        end
        return { id = id, name = mob.name, level = mob.level, rank = mob.rank }
    end
    return TargetKill() or { watch = UnitExists("target") and target or nil }
end

local function ReadXPBar()
    local xp, max = FG.Readable(UnitXP("player")), FG.Readable(UnitXPMax("player"))
    if not xp or not max then return nil end
    return { xp = xp, max = max, rested = FG.Readable(GetXPExhaustion()) or 0, level = UnitLevel("player") }
end

local function AtLevelCap(level)
    return (IsXPUserDisabled and IsXPUserDisabled()) or (GetMaxPlayerLevel and level >= GetMaxPlayerLevel())
end

--- Record the kills of the batch once its time is over (`force`: now), with the XP gained.
local function SettleKills(now, force)
    if not batch then
        for i = #gains, 1, -1 do
            if now - gains[i].at > XP_EARLY then table.remove(gains, i) end
        end
        return
    end
    if not force and now - batch.at < KILL_WINDOW then return end
    local xp, rested, level = 0, 0, nil
    for _, g in ipairs(gains) do
        if g.at >= batch.at - XP_EARLY then
            xp, rested = xp + g.xp, rested + g.rested
            level = level or g.level
        end
    end
    wipe(gains)
    local n = #batch.kills
    local turnIn = lastTurnIn and lastTurnIn >= batch.at - TURNIN_MARGIN
    for _, kill in ipairs(batch.kills) do
        local data = kill.data
        local mob = kill.watch and TargetKill(kill.watch)
        if mob then
            kill.id, data.name, data.level, data.rank = mob.id, mob.name, mob.level, mob.rank
        end
        local before = n == 1 and level or kill.level
        if xpBar and xpBar.max > 0 and not turnIn and not AtLevelCap(before) then
            if n == 1 then
                data.xp, data.rested = xp, rested > 0 and rested or nil
            else
                data.multi, data.batchxp = n, xp
            end
        end
        Record("kill", nil, kill.id and "npc", kill.id, data, kill.mapID, kill.x, kill.y, before)
    end
    batch = nil
end

local function SettleLater()
    local ok, err = pcall(SettleKills, GetTime())
    if not ok then FG:Debug("error in kill XP: %s", tostring(err)) end
end

FG:On("PLAYER_TARGET_CHANGED", function()
    target = { mob = LearnMob("target"), done = UnitExists("target") and UnitIsDead("target") }
    if UnitExists("target") then RecordUnit("target", "seen") end
end)

FG:On("UPDATE_MOUSEOVER_UNIT", function()
    LearnMob("mouseover")
    if not InCombatLockdown() then RecordUnit("mouseover", "seen") end
end)

FG:On("NAME_PLATE_UNIT_ADDED", LearnMob)

FG:On("PLAYER_ENTERING_WORLD", function() xpBar = ReadXPBar() end)

FG:On("PLAYER_XP_UPDATE", function(unit)
    if unit and unit ~= "player" then return end
    local before, now = xpBar, ReadXPBar()
    xpBar = now
    if not before or not now then return end
    local gained = now.xp - before.xp
    if gained < 0 then
        -- A level up: the end of the previous level, then the start of the new one (the level
        -- read may lag behind).
        gained = before.max - before.xp + now.xp
        now.level = math.max(now.level, before.level + 1)
    end
    if gained <= 0 then return end
    local rested = math.max(0, before.rested - now.rested)
    table.insert(gains, { at = GetTime(), xp = gained, rested = rested, level = before.level })
    SettleKills(GetTime())
end)

FG:On("QUEST_TURNED_IN", function() lastTurnIn = GetTime() end)

FG:On("PARTY_KILL", function(attackerGUID, targetGUID)
    local mob = KilledMob(targetGUID)
    if not mob then return end
    local attacker, by = FG.Readable(attackerGUID), nil
    if not attacker then
        by = "unknown"
    elseif attacker ~= UnitGUID("player") then
        by = attacker == UnitGUID("pet") and "pet" or "party"
    end
    local members = GetNumGroupMembers and GetNumGroupMembers() or 0
    local now = GetTime()
    SettleKills(now)
    if not batch then
        batch = { at = now, kills = {} }
        if C_Timer and C_Timer.After then C_Timer.After(KILL_WINDOW + 0.1, SettleLater) end
    end
    local mapID, x, y = FG.PlayerPosition()
    table.insert(batch.kills, {
        id = mob.id,
        watch = mob.watch,
        mapID = mapID,
        x = x,
        y = y,
        level = UnitLevel("player"),
        data = {
            level = mob.level,
            name = mob.name,
            rank = mob.rank,
            by = by,
            group = members > 1 and members or nil,
            raid = IsInRaid and IsInRaid() or nil,
        },
    })
end)

FG:On("PLAYER_LOGOUT", function() SettleKills(GetTime(), true) end)

FG:On("LOOT_OPENED", function()
    if not GetLootSourceInfo then return end
    local items = {}
    local sourceType, sourceID
    for slot = 1, GetNumLootItems() do
        local link = GetLootSlotLink(slot)
        local itemID = link and tonumber(link:match("item:(%d+)"))
        local guid = GetLootSourceInfo(slot)
        local t, id = FG.ParseGUID(guid)
        if t == "npc" or t == "object" then
            sourceType, sourceID = t, id
        end
        if itemID then table.insert(items, itemID) end
    end
    if sourceType and #items > 0 then Record("loot", nil, sourceType, sourceID, { items = items }) end
end)

FG:On("TAXIMAP_OPENED", function()
    local mapID = GetTaxiMapID and GetTaxiMapID() or C_Map.GetBestMapForUnit("player")
    if not mapID or not C_TaxiMap.GetAllTaxiNodes then return end
    local nodes = {}
    for _, node in ipairs(C_TaxiMap.GetAllTaxiNodes(mapID)) do
        table.insert(nodes, { id = node.nodeID, name = node.name, state = node.state })
    end
    Record("taxi", nil, nil, nil, { map = mapID, nodes = nodes })
end)

-- Transport links: when the continent changes (boat, zeppelin, portal), remember where we
-- left from, where we arrived and how long it took. `fg` turns these into travel links.
local function Continent(mapID)
    local info = mapID and C_Map.GetMapInfo(mapID)
    while info and info.mapType and info.mapType > 2 and info.parentMapID and info.parentMapID ~= 0 do
        info = C_Map.GetMapInfo(info.parentMapID)
    end
    return info and info.mapID
end

local last -- last known position: { map, x, y, continent, time }

local function TrackPosition()
    if not char or UnitOnTaxi("player") then return end
    local mapID, x, y = FG.PlayerPosition()
    if not x then return end
    local continent = Continent(mapID)
    if last and continent and last.continent and continent ~= last.continent then
        Record("transit", nil, nil, nil, {
            fromMap = last.map,
            fromX = last.x,
            fromY = last.y,
            seconds = time() - last.time,
        }, mapID, x, y)
    end
    last = { map = mapID, x = x, y = y, continent = continent, time = time() }
end

FG:On("PLAYER_LOGIN", function() C_Timer.NewTicker(5, TrackPosition) end)
FG:On("ZONE_CHANGED_NEW_AREA", TrackPosition)

function Collector:PrintStatus()
    if not char then return end
    local counts = {}
    for _, e in ipairs(char.events) do
        counts[e[3]] = (counts[e[3]] or 0) + 1
    end
    local parts = {}
    for event, n in pairs(counts) do
        table.insert(parts, event .. "=" .. n)
    end
    table.sort(parts)
    FG:Print("%d events recorded (%s)", #char.events, table.concat(parts, ", "))
end

function Collector:Reset()
    if char then
        wipe(char.events)
        wipe(seen)
        FG:Print("collected events cleared")
    end
end
