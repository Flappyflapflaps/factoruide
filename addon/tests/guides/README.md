# Guides of the released apps

The addon is published on its own (CurseForge) and updates apart from the apps: a player may run
a newer addon with a guide an older app made. **A newer addon must always read the guides of the
releases before it** (the reverse is not required: a guide made for a newer addon is refused by
its `format`, see `GUIDE_FORMAT` in `crates/fg-route/src/job/routes.rs`).

Each `X.Y.Z.lua` here is the exact Lua the apps of release X.Y.Z exported for a small real guide
(Classic human warrior, levels 1 to 6): the text of an in-game import (`FGG1:`) and of the
Factoruide_Data addon. The smoke test (`addon/tests/smoke.lua`) imports every file listed in
`index.lua` both ways, follows the guide from its first step and walks it to its end.

- Never edit these files: they are what players have.
- At each release whose guide output changed, add one: `scripts/guide-fixture.sh X.Y.Z` (it
  writes `X.Y.Z.lua` and `index.lua`, and says so when the output is the same as the last one).
