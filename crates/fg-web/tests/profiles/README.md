# Profile exports of the released addons

The addon is published on its own (CurseForge) and updates apart from the apps: **the apps must
keep reading what older addons produce.** Each `X.Y.Z.txt` here is the `FGP1:` text ("Export my
character") the addon of release X.Y.Z made for the smoke test's character (Brouz-Realm, a level
4 gnome mage on WoW Forever). `the_exports_of_released_addons_are_read` (`src/app/tests.rs`)
imports every file here.

- Never edit these files: they are what players paste.
- At each addon release whose export changed, add one, from the repository root:
  `FG_PROFILE_EXPORT=crates/fg-web/tests/profiles/X.Y.Z.txt luajit addon/tests/smoke.lua`
