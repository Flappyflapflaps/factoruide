# Changelog

The notes of each version are the message of its tag (`scripts/release.sh`), shown in the app.

## Unreleased

- Guide editor: "Edit the steps" on a guide's page opens its steps as a draft. Steps move (drag and drop, up and down, or cut and place), are removed, or are added: a quest to take (searchable list), to do or to turn in, the hearthstone bound at an inn or used (Astral Recall too), a flight between two flight paths, a flight path to learn, a dungeon run, a number of mobs to kill where the character stands. After each change the guide is replayed as the optimizer counts it: its play time and the difference with the version edited, each step's time, level and XP, what it does on the way (trips, quests taken at hand, training), and the steps that are not possible with the reason (quest not taken yet, prerequisite missing, flight path not known…). A guide with an impossible step cannot be saved; one left incomplete (a quest taken and never turned in, a final grind to reach the target level) can, with a warning. "Save" adds the edited guide as a new version.
- Guide editor: a "do the quest" step can kill every normal mob met on the way to it, beyond the share of farming each level allows, with the mobs, XP and time it would bring shown before checking it.
- Guides now keep the route they were written from, so that editing them replays it exactly; guides computed before read their route back from their steps (their time may then differ a little).

## 0.5.0 (2026-10-11)

The hearthstone used where it saves the most, mage teleports and Astral Recall, objectives done at their best place, timed quests in time, and an addon without blocked-action popups.

### Routes

- Objectives whose mobs or objects live in several places: besides the main one, up to two other groups are considered (advanced setting "Other places per objective"), each with its own mobs, their level and packs, the guards around and the wait for respawns. The guide goes to the one costing the least between where the character is and the quest's turn-in, the levels to grind first included when the mobs there are too strong (undead mage 1-60, starting route: 1.8 h shorter).
- Hearthstone: bound anew as the route moves on, by bands of levels (5 levels at first, then 4, then 3 from level 40): the inns closest to where each band's time is spent, the one among most of its quest givers and the closest one next to a flight master are tried and the best kept, and the optimizer moves the binds between those inns. Starting routes used to keep the first bind for good.
- Hearthstone used where it saves the most: a step the route chooses, no longer the first trip it shortens by 3 minutes. Once it is ready, the trips of the next 20 minutes are compared (typically back to town once a zone's quests are done rather than a shortcut just before), and the optimizer moves, adds or removes uses.
- Class spells that travel: a mage teleports to its capitals once trained (Stormwind and Ironforge, Orgrimmar and Undercity at 20; Darnassus, Thunder Bluff at 30), landing at the city's mage trainer; a shaman has Astral Recall from level 30, a second hearthstone with its own cooldown (15 min, a setting). Guide steps "Teleport to Undercity", "Astral Recall to…".
- Starting routes no longer take a quest whose giver stands among mobs too strong yet when nothing else is left at the character's level: they grind to the level that quest really requires, not its minimum level (undead mage 1-60: a 52 to 60 grind for Defenders of Darrowshire). Undead mage 1-60, starting route with all the above: 142.9 h to 128.9 h.
- No more mobs to kill on the way through a capital (Stormwind, Ironforge, Orgrimmar, Shattrath...): the NPCs the data knows nothing of there were taken for mobs (Theramore Emissary in Stormwind's Old Town, the warlock trainers' demons, the Stockade's prisoners put at its door).
- Timed quests (The Rethban Gauntlet, Mist, The Affray…) keep to their time limit: the guide turns them in within three quarters of it (a margin for its estimates), says so when taking them ("timed: turn it in within 60 min"), and a route that would be late gives the quest up instead, without its reward, so the optimizer keeps the routes in time. TBC uses the limits of the quests it shares with Classic.
- Guide: a grind step is no longer placed between a dungeon run and the quests done inside it.

### Addon

- No more "blocked from an action only available to the Blizzard UI" on loading: the mobs killed on the way are counted from the game's kill event, the combat log being forbidden to addons on recent clients. During combat the boxes holding a quest item button (secure) also keep their size and visibility until the fight ends, the game's quest tracker of Classic clients is made invisible instead of hidden, and an action the game refuses names its function in the chat.
- The guide's buttons are on a bar of their own above it (steps on the left; the box under the guide, start over and settings on the right), with tooltips; the header keeps the step and the level.
- The XP bar shows the current tasks as a translucent yellow band, from the mark where the guide expects you to start them to a lighter one where it expects you once they are done; its tooltip gives both ("Current tasks: from 1,200 to 3,400 xp").
- The mobs to kill on the way to a step have a line of their own in the current tasks, before the step: their count once the step is current ("mobs on the way 5/14", the character's and its pet's kills since then, checked when reached), else how many are planned ("~14 mobs on the way").
- The box under the guide has two tabs again, "Next" (the steps that follow, as before) and "To pick up", and a button in the guide's bar shows or hides it (both kept).
- The guide's start-over button also undoes the steps and quests marked done by hand (a skip by mistake), after asking; also /fg restart.
- The quest tracker lists the guide's quests in progress from the guide itself, 8 by default: opening the game's quest log (headers collapsed or expanded) no longer changes the list, and the game's own tracker of Classic clients is hidden too.
- With "Only the guide's quests", a quest is accepted automatically only while the guide still takes it: no longer the quests it abandons (they were taken again when talking to their giver), nor those marked done by hand or left behind, nor escorts shared by the group.

### WoW Forever data

- Quest givers and turn-ins no longer have Wowhead positions that every other source, and the game, contradict (old or event places, a few to twenty map units off: Furen Longbeard, Lakeshire's NPCs, 179 NPCs in all): the arrow no longer points at a spot where the NPC is not.
- The quests new to Forever keep the prerequisites the data was missing (59 prerequisites on 41 quests): The Shamed Lieutenant after A Friend of the Family, Remember That I Love You (2/2) after its first part, the Moonbrook chain in order… A guide no longer sends you to a quest you cannot take yet.

## 0.4.0 (2026-10-11)

Stop or cancel a generation, a new guide page, dungeon quests step by step, a "To pick up" box and a guide mark on the XP bar.

### Generation

- A generation can be stopped or cancelled: "Stop and keep this guide" ends the optimization at once and saves the best guide found so far (once a first route exists); "Cancel" goes back to the setup, unchanged, without saving anything.
- The computing time chosen (10 s, 2 min, 6 min) no longer includes building the starting routes (about ten seconds on a 1-60 guide): the optimization gets its whole time.
- Walking memory: the paths around mountains worked out while building routes are kept by the app and saved with each guide (and in the .fgguide files it is shared as), so computing a guide again, or another one, does not search them again (starting routes of a 1-60 guide: 13.5 s to 9.6 s).

### Guide page

- Every step in one table by zone: expand or collapse all, filter, a color per step type, dungeon runs on their own row; on a wide window, the map of the zone in view beside it, its steps numbered as in the table.
- The guide map shows only the continents the guide goes to, the route colored from start to end, close level badges merged ("+2"), the zone in view drawn on it.
- Figures easier to read: XP sources as a bar, regular level ticks, calculation notes as a table of the quests left out, no more horizontal scrolling on narrow windows.

### Setup and settings

- The setup is laid out again: the character on one row, play style / dungeons and elites / professions and group side by side, the zones over the whole width (both continents), the computing time next to "Generate".
- Advanced settings: those changed from their default are marked, "Reset everything to defaults" at the top; professions are no longer repeated there.
- Addon tab: the save bar stays visible and flags unsaved changes; window size as a slider. Settings: "Browse…" for the World of Warcraft folder, unsupported game versions marked.
- Dates with the month in letters, numbers with the decimal separator of the app language.

### Dungeons

- The quests done in a run are steps of their own, in the app and the addon ("<quest>: <objectives>", done as the quest progresses), after the run step, which the addon marks done on entering the dungeon (guides computed before keep a single step, done once back out).

### Addon

- The "Next" box becomes "To pick up": the next quests that can be taken now (their prerequisites done, the level reached), with their distance, 10 by default. Close quests can be taken in any order, the guide skips them once in the log. A left-click on a step points the arrow at it until it is done.
- The quest tracker shows the next 10 quests of the log to do in the guide's order (setting "Quests shown"), instead of the quests with a step among the next 15.
- A yellow mark on the XP bar shows the XP the guide plans at the current step, and its tooltip how far ahead or behind you are ("Guide mark on the XP bar" in the Addon tab).
- Grind steps (checkpoints, catching up, planned grinds) count the XP of the quests complete in your log that the guide turns in before the level they aim at.

### Guides

- Before every level, a step to catch up when more than 10% of a level behind the route, at the last place of the level where its mobs live: you no longer go into content made for a higher level.
- Farm spots are measured: mobs standing in pairs or groups require more power (cautious: no longer taken on as lone mobs), and an objective asking for more kills than its spot has mobs counts the wait for respawns.
- "Crowded server" becomes a slider (players on the server, from empty to launch): other players share the mobs of small spots, and escorts and events wait longer the busier it is, less as players spread out with the levels. Settings saved with the option on read as a launch.
- Farming on the way has its own share of each level, apart from pure grinding (still 10%): 20% on Classic Era / Hardcore and TBC, 10% on Forever ("Maximum share of grinding on the way per level" in the advanced settings).

### Fixes

- WoW Forever, Redridge: Tharil'zun and Shadow Magic need Alther's Mill and Blackrock Blockade too, so a guide no longer abandons Blackrock Blockade before them.

## 0.3.0 (2026-10-11)

Guides in every language, guides started from your character's quest log.

### Languages

- Guides are no longer tied to a language: the app shows them in its own language, and the addon page chooses the language of the guides in the game, without computing them again.
- Guide sentences in 10 languages (they were in French or English only).
- Guides computed by an earlier version keep their texts: generate them again to translate them.
- Guide files carry a format number: a guide made by an earlier version is migrated when opened or imported, one from a newer version asks to update the app.

### Starting from your character

- A guide started from a character no longer has to finish the quests of its log: they count as never taken, the guide takes again those that help, and abandons the others as its first steps (the addon moves on once they have left the log).

### Fixes

- Objects gathered between kills wait for the level their own step needs (an object among stronger mobs).
- Group: a companion of a class the character's race cannot play (a human's druid) gets the class quests of a race that can, with their whole chain.
- App: ticking a box at the bottom of a long page no longer leaves the window scrolled down for good.

## 0.2.0 (2026-10-10)

Personalized leveling guides for WoW Forever, Classic Era / Hardcore and TBC Anniversary.

### Character power

- Fights depend on power: level plus gear and class spells.
- Each level of power makes fights 10% faster; XP still follows the level.
- The guide wears quest rewards and dungeon loot, and favours real upgrades.
- The addon picks the reward the guide planned.
- Training happens when the new spells are worth it.
- The addon buys only the spells useful for leveling.
- Each guide step shows the character's power and what its gear brings.

### What the character takes on

- New progression profile: cautious (hardcore survival), normal or risky, on the main settings screen.
- Cautious never fights packs of mobs more than one level above you.
- Cautious still takes yellow quests and lone targets up to two levels above.
- Normal does yellow quests, never orange.
- Risky takes orange quests and borderline elites.
- Elite camps need one more level of power per extra elite pulled, at any level.
- Quests started by an item need the power to kill the mob dropping it (no level 24 rare at level 11).
- Exploring and escorting count as fighting; talking and delivering don't.
- Givers, turn-ins and objects in the middle of a camp wait until you can fight through it (Sven's Revenge ends among level 25-27 Defias).
- New "Crowded server (launch)" option: escorts and events wait for their turn, mobs and objects with few spawns are fought over; the guide counts the wait (in full in the starting zones, half above level 20) and avoids them when other quests pay more.

### Grinding

- New "Grind on the way" option (on by default): mobs met on foot are killed on the way, as most guides ask.
- Only mobs close to your level (3 below to 1 above); the guide step says how many: "(kill ~5 mobs on the way)".
- It replaces pure grinding and never wins over a good quest: flights and walks are chosen as before.
- Grinding is limited to a share of each level, 10% by default (adjustable), mobs on the way included.
- Beyond it the guide takes quests instead, even less rewarding ones.
- New "Grinding allowed" option: without it, the guide takes more quests instead, even if longer.
- Quest objects lying among the mobs of another quest are gathered between kills, while resting.

### Start from your character

- The addon keeps a profile of your character up to date (every minute and at logout; `/fg profile` writes it at once): level, XP, quests done and in the log, position, hearthstone, flight paths, professions, gear, spells, rested XP.
- New guide: pick one of your characters in the list (no file to move); race, class and level follow it.
- The guide goes on from where it stands: quests done are left out, those in the log are finished (objectives already done count), with its hearthstone and flight paths.
- Quests the guide does not plan still take their place in the log; follow-ups of quests done long ago are known exactly.
- Rested XP doubles kill XP until it runs out (it builds up while logged out).
- Gear worn, spells learned and profession skills set the character's power, training and profession curves.
- Flight paths are only known from a flight master's map: open one in game if the list shows none.

### Professions

- Professions no longer add time or steps to the guide.
- The addon shows whether your skill is ahead or behind the curve for your level.
- The curve starts when you take the profession (level 5 by default, adjustable); taken later, it follows later.
- New curves: slow start, catching up by level 20.
- Profession quests are suggested, marked optional; the addon skips them without the skill.

### Checkpoints

- Checkpoints from level 20, where many quests open: ahead of plan, a button skips to the next one.
- Skipping keeps chains going on after it, class quests, planned rewards and training.
- Behind the route, a checkpoint step has you grind before going on, so the guide stays in sync.
- Grinding happens where mobs of your level live, with an XP target: "be level 19 and 93% (19969 xp)".

### Quests

- Dungeon quests are never planned without dungeons.
- Battleground quests are left out unless the new PvP option is on.
- Marks, tokens and crafted items never get a made-up mob source.
- Quests that can be picked up before the target level are kept, whatever their level.
- Quests are taken and turned in at hand, at quest hubs.
- The hearthstone is used only when it saves at least 3 minutes.
- A zone is done in one go instead of several short visits.
- Walks go around mountains and cliffs, from the game's terrain heights.
- Old chains no longer worth it are left out instead of coming back for them.
- Mobs of quests in the log are killed on the way.
- Class quests giving a pet or a form are done early: they count as power, like gear.
- Pet quests stay early even after a long optimization (up to 10 min slower).
- Objects of quests in the log are picked up on the way, like their mobs.
- Quests started by a rare drop (under 10%) are no longer planned: take them if it drops.
- Gray quests are no longer taken, unless they open a chain worth doing.
- A quest gone gray in the log is dropped instead of turned in many levels later.
- Quests taken but never turned in are left out of the guide.
- Long follow-ups (sixty cloth to gather) no longer make a chain worth starting.
- Fewer trips for one or two quests: changing zone counts 4 minutes (new advanced setting).
- Fixed: Deeprun Tram quests, Frostmane Hold.
- Fixed: the paladin's Tome of Divinity chain.

### In the app

- Generation: the times of the best routes are shown while they are picked, then each one's curve as it improves, in its own color; the guide kept is in bold, with why it is not always the shortest (it is scored on comfort too).
- The calculation keeps to its time: 2 minutes by default (30 s to pick the best starts, then 1 min 30 to improve them), 6 minutes for a thorough one.
- My guides: class colors, a Horde or Alliance emblem with the race, and every column sorts the list.
- The guide page shows the character the same way.
- The addon accepts only the guide's quests by default (Addon page, "Only the guide's quests"); others can still be taken by hand.

### Addon look

- The addon takes the game's look by default.
- With ElvUI, EllesmereUI or SpartanUI installed, it takes their style.
- New "Look" option: automatic, game style or dark.
- Other skins can style it through `FactoruideAPI.RegisterSkin`.
- A big arrow points to the current step: green ahead, red behind, distance below.
- Ctrl + mouse wheel over a window, or `/fg scale 1.3`, changes the size of the windows in game (kept for every character; `/fg scale` alone goes back to the app's size).
- Single scrollbar: only the content area scrolls.

### Fixes

- Quests taken or turned in on the way wait for the level their own step needs (a camp to fight through around the NPC).
- The end of the calculation no longer drops a quest turned in on the way (its XP was replaced by grinding).
- Crowded server: the wait for a contested target counts even when it is killed on the way.
- Group: the class quests required of every class in the group are required, not only the character's.
- Addon: quests marked done by hand (right click, `/fg skip`) and skipped steps belonged to the character: they also counted in other guides (quests never taken shown done). They now belong to their guide; a new version of a guide keeps the quests marked.
- Addon: opening the world map raised a Lua error (step pins).
- TBC: mobs of the blood elf and draenei starting zones give Azeroth XP (they counted as Outland: five times too much). Guides to 20 there are longer and take more quests.
- TBC: grinding XP follows where you are (Outland mobs from Hellfire Peninsula on, not from level 60); the Caverns of Time dungeons count as Outland.
- A quest dropped from the log no longer opens its follow-up (the guide asked for quests you could not get).
- Dungeon mobs keep their level: a dungeon run late no longer gives XP as if they had yours.
- Flight prices are those of the flight taken (the price of a cheaper, slower path was counted).
- Only flight masters are flight paths: no more "Flight path: Quest Path 11517… Shaman… A" steps (the nodes of quest flights, Eastern Plaguelands towers, the other faction's nodes).
- Forever: quests whose objectives or turn-in are unknown are left out instead of being taken for free (a Horde guide went to Menethil Harbor for an Alliance quest).
- The optimizer judged about a third of the changes it tried wrongly (quests taken at hand look ahead in the guide); it now evaluates them exactly.
- A new character's hearthstone takes it back to its starting area until bound at an inn (orcs were sent to Ratchet).
- Hearthstone steps name the town: "Hearthstone to Crossroads (The Barrens)".
- Ratchet and the Crossroads flight paths were shown in Durotar.
- Hearthstone, flight and boat steps show their own time (the whole trip was on the arrival step).

- The computing time is the whole time: the race between starts takes a share of it.
- A search could hand back an invalid route (when cutting the stops after the target level).
- Walks on two continents no longer share cached answers.
- The time shown during the calculation is the guide's play time, as in the result (it showed the optimizer's score, hours higher).
- Guides can be computed again with the default settings.
- Mobs killed on the way to an objective are no longer counted twice (time and XP).
- The guide and tracker windows always grow downwards: their buttons stay under the mouse.
- Objectives in progress stay in the guide until done, not only right after their step.
- A turn-in shows its quest's unfinished objectives first.
- Quest names are in English when no translation exists.
- Updates are checked only in published builds.
