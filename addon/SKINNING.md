# Skinning the Factoruide addon

The addon draws its frames in one place, `Factoruide_Guide/Skin.lua`: dark blue cards in the
Factoruide app's colors. With the `suiteSkin` setting on ("UI suite style", Addon page of the app),
a loaded UI suite styles them instead.

## UI suites supported out of the box

| Suite | How |
|---|---|
| ElvUI | `Skins:AddCallbackForAddon("Factoruide_Guide", ...)`, then `SetTemplate("Transparent")`, `HandleButton`, `HandleItemButton`, `FontTemplate` |
| EllesmereUI | `EllesmereUI.RegisterSkin("Factoruide", ...)`: `Shell`, `Panel`, `Button`, `SquareIcon`, `Font` |
| SpartanUI | `SUI.Skins:Register("Factoruide", ...)`: `SetTemplate`, `SkinObj("Button", ...)` |

## Your own skin

```lua
if FactoruideAPI then
    FactoruideAPI.RegisterSkin("MySkin", function(frame, kind)
        -- kind: "window", "panel", "button" or "icon"
    end)
end
```

The function runs on every frame already made and on every frame made later. Frames:

| Frame | Kind | Fields |
|---|---|---|
| `FactoruideTodo` (full guide) | window | `accent` (top line), `zone`, `level`, `rows`, `nextRows` |
| `FactoruideMini` (minimal guide) | window | `accent`, `name`, `sub`, `dist` |
| `FactoruideMiniNext` (minimal guide's next steps) | panel | `title`, `rows` |
| the minimal guide's labels, the item and talent names near the action bars | panel | `text` or `name` |
| `FactoruideTracker` (quest panel) | panel | `line` (under the tabs) |
| `FGItemButton`, `FactoruideItemButton<n>` | icon | `icon`, `cooldown` |

The compass bar (`FactoruideCompass`), the arrow (`FactoruideArrow`), the mark above an NPC's
head (`FactoruideNpcMark`) and the wind that replaces the compass (`FactoruideWind`, see the
Immersion settings) are not boxes and are not skinned.

`FactoruideAPI.GetFrames()` returns `{ { frame = ..., kind = ... }, ... }` for the frames made so
far. `FactoruideAPI.version` is 2 since the frames above (1: `FactoruideNext`, `FactoruideTalents`).
