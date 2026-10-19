# SavedVariables of the released addons

The addon is published on its own (CurseForge) and updates apart from the apps: **the desktop app
must keep reading what older addons saved** (`Factoruide_Profile.lua`: the profiles and the
collector's recordings). Each `X.Y.Z.lua` here is `FactoruideDB` as the addon of release X.Y.Z
left it in the smoke test (the same data the game writes, in another Lua layout).
`tests/saved_variables.rs` reads every file here.

- Never edit these files: they are what players have.
- At each addon release whose saved data changed, add one, from the repository root:
  `FG_SAVED_VARIABLES=crates/fg-contrib/tests/saved-variables/X.Y.Z.lua luajit addon/tests/smoke.lua`
