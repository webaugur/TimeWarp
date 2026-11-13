# Creating a scenery theme

A scenery theme is the landscape in the upper half of the clock face. It is a layered SVG, clipped to the semicircle that sits on the 9–3 line. Clockface shows or hides named groups for the season, the time of day, and the weather. Hands, numerals, ticks, and the two lower discs are painted on top and are not part of the theme.

The theme that ships with the crate is the one to copy:

- `clockface/scenes/default/scene.svg` — the drawing
- `clockface/scenes/default/theme.toml` — which group id plays which role

The loader is `clockface/src/scene.rs`. It does not hard-code the picture. It reads the manifest, turns groups on or off, and rasterizes the SVG with [resvg](https://github.com/linebender/resvg).

## Add a theme without changing Rust

1. Copy `scenes/default/` to a new folder, for example `scenes/harbor/`.
2. Edit `scene.svg` and `theme.toml`. Keep the same manifest keys (see below). The group ids can change, as long as the SVG and the manifest agree.
3. Point the clock at that folder and take a screenshot:

```bash
CLOCKFACE_SCENE="$PWD/scenes/harbor" \
  ./target/release/clockface --screenshot harbor.png --width 1920 --height 1080
```

`CLOCKFACE_SCENE` is a directory that contains `scene.svg` and `theme.toml`. If the variable is unset, or the folder cannot be read, the clock uses the theme compiled in from `scenes/default/`. No Rust edit is required for a new theme that uses the existing roles.

A role the manifest does not know about (a sixth season, a new weather kind) does need a change in `scene.rs`, in `visible_layers`. The weather kinds the clock already understands are clear, clouds, rain, snow, storm, fog, and unknown. They come from the Open-Meteo code on the report (`clockface/src/weather.rs`).

## The view

The default SVG declares this view box, and `theme.toml` repeats it so a reader can see it without opening the drawing. Clockface does not read `view_box` or `name` when it paints. resvg uses the `viewBox` on the root `<svg>`. Keep the two in agreement. `name` is only a label.

```toml
view_box = [0.0, 0.0, 1000.0, 500.0]
```

That box is the upper half of a circle. The circle's center is the middle of the bottom edge, `(500, 500)`, and the radius is `500`. The horizon is that bottom edge. Anything in the top-left and top-right corners, outside the circle, is covered later by the brass ring, so keep the drawing inside the semicircle.

The clock maps this box onto the aperture: the full width of the upper half, and the height from the horizon up to 12 o'clock. It does not stretch a square bitmap.

## What the manifest means

`theme.toml` is the list of group ids. `scene.rs` collects every id the file mentions. For each of those groups it writes `display="inline"` or `display="none"` on the opening `<g>` tag, then asks resvg to draw the result. Groups that are not in the manifest are left as you wrote them, so a group nested inside a layer does not need an id.

```toml
name = "default"
view_box = [0.0, 0.0, 1000.0, 500.0]

always = ["land", "tree-trunk"]

[sky]
day = "sky-day"
twilight = "sky-twilight"
night = "sky-night"

[season.leaves]
spring = "leaves-spring"
summer = "leaves-summer"
autumn = "leaves-autumn"
winter = "leaves-winter"

[season.ground]
spring = "ground-spring"
summer = "ground-summer"
autumn = "ground-autumn"
winter = "ground-winter"

[weather]
clouds = ["clouds"]
rain = ["clouds", "rain"]
snow = ["clouds", "snow", "snow-cover"]
storm = ["clouds", "storm"]
fog = ["clouds", "fog"]
unknown = ["clouds"]

[time]
day = ["sun"]
twilight = ["sun"]
night = ["stars"]

hide_sun = ["storm", "fog"]
```

`always` is shown in every condition. The default uses it the way a tree is built: `tree-trunk` is the trunk and the branches, and it never turns off. The leaves are four other groups, and only the current season's group is shown.

The same split is used for the rest of the scene:

| Role | Default group ids | When it is on |
| --- | --- | --- |
| Always | `land`, `tree-trunk` | Every frame. Distant hills and the bare tree. |
| Sky | `sky-day`, `sky-twilight`, `sky-night` | One of them. Day when the sun is up, night when it is below the twilight band, twilight at dawn and dusk. |
| Leaves | `leaves-spring`, `leaves-summer`, `leaves-autumn`, `leaves-winter` | One of them, from the local date. Winter is a few clinging leaves, not a second trunk. |
| Ground | `ground-spring`, `ground-summer`, `ground-autumn`, `ground-winter` | One of them, same season as the leaves. |
| Clouds | `clouds` | Cloud, rain, snow, storm, fog, and an unknown forecast. |
| Rain | `rain` | Rain, on top of clouds. |
| Snow | `snow`, `snow-cover` | Snow. `snow` is the falling flakes. `snow-cover` is the snow on the ground and on the branches. |
| Storm | `storm` | Thunderstorm, on top of clouds. |
| Fog | `fog` | Fog, on top of clouds. |
| Sun | `sun` | Day and twilight, unless `hide_sun` names the current condition. Storm and fog hide it. |
| Stars | `stars` | Night only. A stormy or foggy night still shows the stars group, because `hide_sun` only removes the sun. |

Clear weather adds no weather group. The sun or the stars carry that scene.

Seasons follow the same dates as the sun box: winter from 21 December, spring from 20 March, summer from 21 June, autumn from 22 September. The time of day uses the local civil clock. Around the `--screenshot` instant (10:08) the band is day, not twilight.

Draw later groups further down the file. resvg paints in document order, so snow, rain, and fog should come after the tree if they are meant to sit in front of it. In the default file the order is sky, stars, sun, clouds, storm, hills, seasonal ground, snow on the ground, trunk, leaves, rain, falling snow, fog.

## The mural wheels

The two windows beside the dial are separate from the horizon theme. They are discs on the same shaft, clipped to the square corners outside the brass. The art is:

- `clockface/scenes/wheels/year.svg` — one turn per year. The lower-left window's center is clock angle `225 + year_fraction × 360` (clockwise from 12). Winter is drawn around 239°, then spring, summer, and autumn a quarter turn apart.
- `clockface/scenes/wheels/day.svg` — one turn per civil day. The lower-right window's center is `135 + day_fraction × 360`. Night is around 135°, dawn around 225°, the daytime scene around 287° (that is 10:08 in the window), dusk around 45°.

Face radius 1 is 353.6 px from the center of the 1000×1000 view. The dial covers the inside of that circle. Draw motifs between about 1.02 and 1.41 face radii, and make them large enough to read in the corner (a tree or a sun should be a sizable fraction of the window, not a speck on a gradient). Replacing either SVG replaces the mural. No Rust change is required if the view box and the angle layout stay the same.

## The moon

Do not paint the moon in the SVG. The app draws it after the theme, at a fixed place in the aperture, so a theme cannot move it by editing the file. The phase is the mean age since the new moon of 2000-01-06 18:14 UTC (synodic month 29.530588853 days): 0 is new, 0.25 is first quarter with the right side lit, 0.5 is full, 0.75 is last quarter with the left side lit. The disc is shaded (maria, crater rims, limb darkening, and a soft terminator), not a flat two-color circle. The unlit side is a darkened moon surface, day and night, so the whole round stays solid against the sky.

A group whose id is `moon` is forced to `display="none"` so a copied illustration does not sit under the real disc.

Placement, in face coordinates, is unchanged: 45° clockwise from 12, at 0.36 of the face radius, with a radius of 0.11 of the face radius. In the default view box that is about `(699, 301)` with radius `86`. Leave that patch of sky readable. The moon is painted on top of the theme, so a cloud there will not hide it, but a busy drawing behind it still looks crowded.

On Thursday 15 January 2026 at 10:08:37 the phase is about 0.89, a waning crescent, lit on the western (left) limb. The default theme at that instant shows winter, a day sky, the sun, clouds, falling snow, and snow on the ground.

## Preview

From `clockface/`:

```bash
cargo build --release
./target/release/clockface --screenshot /tmp/theme.png --width 1920 --height 1080
```

That frame is local 2026-01-15 10:08:37, Indianapolis, imperial units, and the January fixture (light snow). It does not use the network. The status line is the machine you ran it on.

To force another instant you would need a different build. The screenshot hour is fixed so themes can be compared. To see summer or night, set `CLOCKFACE_SCENE` only after checking the layer logic with a small program, or temporarily call the same `visible_layers` rules from a unit test. `scene.rs` already tests that the January fixture enables `sky-day`, `leaves-winter`, `ground-winter`, `clouds`, `snow`, `snow-cover`, and `sun`, and leaves the other seasons and the rain, storm, and fog groups off.

The live window uses the same painter. Weather from Open-Meteo selects the weather groups; the clock selects the sky and the season.

## Drawing tips

- Give every toggleable layer one `<g id="the-id">` written with a double-quoted `id`. The loader finds `id="..."` on the opening tag. Nested groups inside that layer do not need ids.
- Do not rely on `display` in the file. The loader overwrites it for every id listed in the manifest.
- Use paths, basic shapes, gradients, opacity, and stroke. resvg draws those. Avoid external images, scripts, and fonts. Text in the scene will not follow the clock's serif.
- Keep each layer meaningful on its own. A leaf layer should not contain the trunk. A snow layer should not contain the sky. Someone should be able to hide one group and still understand the rest.
- Winter leaves can be sparse. The trunk stays visible because it is in `always`.
- Put the horizon glow in the sky gradients, and the soil or grass in the ground groups, so a snow cover can sit on top of dormant grass instead of replacing the whole landscape.
- Stay inside the semicircle. The corners of the view box are hidden by brass, but a tree that crosses the horizon into the lower half is clipped.
- Keep the file hand-editable: one group per layer, gradients in `<defs>`, no flattened export from a bitmap tracer. A traced photograph is a single path with no groups to switch.
- After a change, run the screenshot and look at the upper half only. The lower quarters, the frames, and the status line are not the theme.
