# Factoruide Guide

What changes for players in each version of this addon, shown on its CurseForge and Wago pages.
Same rules as the app's CHANGELOG.md: one short line per change, what the player sees. New lines go
under Unreleased; `scripts/release.sh` gives them the release's version.

## Unreleased

- A step's tooltip says why it is in the guide: the XP its quest gives, the reward planned for you, the quests it opens, or what a trainer, flight, inn or grind step is for.
- A step killing mobs below your level shows the XP each kill gives against a mob of your level ("65% XP" in green, "0% XP" in grey once they give none), with WoW Forever's lower XP for such mobs.
- Off the guide's planned walk (after a death, a detour or a hearth), the compass and the 3D arrow lead you back to the walk a bit ahead of where it is closest, up to 1000 yards away, instead of straight at the step across the ground the walk goes round; close to the step, they still go straight to it.
- Gear advice follows your talents: a balance or restoration druid, an elemental or restoration shaman, a holy or protection paladin, a holy or discipline priest and a protection warrior are told about the gear their spec wants (intellect, spell damage, healing, mana or stamina), not the leveling spec's; a reward the guide picked for another spec is no longer chosen for you.
- Logging out away from an inn or a city warns that it banks no rested XP and points the compass at the closest innkeeper (Advice, "Rested XP reminder at logout").
- WoW Forever: after a fight, a reminder of each XP bonus you are missing (Well Fed from cooked food, your Cozy Sleeping Bag when ready, a leatherworker's Camp Tent), at most every 10 minutes each (Advice, "Reminder of missing XP buffs").
- The wind guide (a gust of wind towards the step instead of the compass) is gone, with its settings and key binding; the compass stays.
- Objectives done in the same area (mobs next to each other) are shown together in Now, and a level checkpoint between them no longer cuts them apart: it comes after them (GitHub #41).
- Merchant, trainer, flight master and inn steps before the last quest you handed in are done on their own when a guide is loaded or reset, instead of each needing a right click.
- The 3D arrow next to the compass has its own size (a setting, or Ctrl + mouse wheel over it), can turn green, yellow or red as you head towards or away from the step, shows how to move it on mouse-over, and can be put back in its place from the options.

## 0.14.0 (2026-10-20)

- The guide's XP bars show a hollow gold box over the XP the current steps give, from where the guide expects you before them to after them.
- In the game's addon list, the Factoruide addons are under Quests, with their own icon, Guide and Data grouped under Profile.
- The full and minimal guides each keep their own layout: moving or resizing the quest panel or the compass in one mode no longer changes the other.
- No mark on the game's XP bar any more, and the game's XP tooltip is back: the guide's own XP bar shows where you should be (GitHub #36).
- A step to sell your loot at a vendor on the way, ticked when the merchant's window opens.
- A kind of step this version does not know yet (a guide from a newer app) is shown and passed on its own.

## 0.13.0 (2026-10-19)

- First version on CurseForge and Wago: the guide made in the Factoruide app, its compass, quest automation and fast loot. Needs Factoruide Profile.
