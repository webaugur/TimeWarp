# Creating a scenery theme

A scenery theme is the landscape in the lower half of the clock face. It is a layered SVG, clipped to the semicircle that sits on the 9–3 line. Clockface shows or hides named groups for the season, the time of day, and the weather. Hands, numerals, and ticks are painted on top and are not part of the theme. The sun and the moon are the two upper quarters, not groups in this drawing.

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

That box is a semicircle. The circle's center is the middle of the bottom edge, `(500, 500)`, and the radius is `500`. Draw the sky toward the top of the box and the ground toward the bottom edge. The clock places the top of the box on the 9–3 line and the bottom edge toward 6 o'clock, so the landscape sits in the lower half and stays upright. Anything outside the semicircle is not drawn.

The clock maps this box onto the lower aperture: the full width, and the height from the 9–3 line down to 6 o'clock. It rasterizes the SVG at twice the aperture's on-screen pixels (and at the window's pixel scale), then keeps that image until the weather band, the time-of-day band, or the size changes. Draw in the 1000×500 view box. Do not bake a small bitmap into the file.

## What the manifest means

`theme.toml` is the list of group ids. `scene.rs` collects every id the file mentions. For each of those groups it writes `display="inline"` or `display="none"` on the opening `<g>` tag, then asks resvg to draw the result. Groups that are not in the manifest are left as you wrote them, so a group nested inside a layer does not need an id.

```toml
name = "default"
view_box = [0.0, 0.0, 1000.0, 500.0]

always = ["land", "tree-trunk"]

[sky]
day = "sky-clear"
overcast = "sky-overcast"
twilight = "tint-dusk"
night = "sky-night"

[photos]
clear = "sky-clear"
partly = "sky-partly"
overcast = "sky-overcast"
night = "sky-night"
dawn = "tint-dawn"
dusk = "tint-dusk"

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
rain = ["rain"]
snow = ["snow", "snow-cover"]
storm = ["storm"]
fog = ["fog"]

[time]
day = []
twilight = []
night = []

hide_sun = ["storm", "fog"]
```

`always` is shown in every condition. The default uses it the way a tree is built: `tree-trunk` is the trunk and the branches, and it never turns off. The leaves are four other groups, and only the current season's group is shown.

The same split is used for the rest of the scene:

| Role | Default group ids | When it is on |
| --- | --- | --- |
| Always | `land`, `tree-trunk` | Every frame. Distant hills and the bare tree. |
| Sky | `sky-clear`, `sky-partly`, `sky-overcast`, `sky-night` | One photograph. Clear under about 20% cover, partly from there to about 75%, overcast above that and for rain, snow, storm, and fog. Night uses the night plate at every cover. |
| Leaves | `leaves-spring`, `leaves-summer`, `leaves-autumn`, `leaves-winter` | One of them, from the local date. Winter is a few clinging leaves, not a second trunk. |
| Ground | `ground-spring`, `ground-summer`, `ground-autumn`, `ground-winter` | One of them, same season as the leaves. |
| Dawn, dusk | `tint-dawn`, `tint-dusk` | A translucent gradient over the daytime photograph. Not used at night. |
| Rain | `rain` | Thin strokes over the overcast photograph. |
| Snow | `snow`, `snow-cover` | `snow` is small flakes. `snow-cover` is the snow on the ground and on the branches. |
| Storm | `storm` | The lightning, over the overcast photograph. |
| Fog | `fog` | Fog, over the overcast photograph. |

The default drawing has no sun. `[time]` `day` and `twilight` are empty. A theme may name a group there; `hide_sun` still drops it for storm and fog. Do not draw a sun in a new copy of the default theme. The NOAA disc is the sun.

Seasons follow the same dates as the sun box: winter from 21 December, spring from 20 March, summer from 21 June, autumn from 22 September. The time of day uses the local civil clock. Around the `--screenshot` instant (10:08) the band is day, not twilight.

Draw later groups further down the file. resvg paints in document order. In the default file the photographs come first, then the dawn or dusk tint, then lightning, hills, seasonal ground, snow on the ground, trunk, leaves, rain, falling snow, and fog. The vector land covers the bottom of the photograph.

A `[photos]` table in `theme.toml` names those groups. Without it, the clock falls back to the `sky` gradient ids. Each plate is an `<image>` whose `href` is a file under `scenes/default/skies/`. The clock compiles those four JPEGs in, and an external theme can put its own images next to `scene.svg`. Use a public-domain or CC0 photograph, crop out any sun or moon, and keep it near 1600 pixels wide. Do not draw the clouds as turbulence filters. The clock still adds a little stable noise after the raster so a tint gradient does not band.

## The mural wheels

The two windows beside the dial are separate from the horizon theme. They are discs on the same shaft, clipped to the square corners outside the brass. The art is:

- `clockface/scenes/wheels/year.svg` — one turn per year. The lower-left window's center is clock angle `225 + year_fraction × 360` (clockwise from 12). Winter is drawn around 239°, then spring, summer, and autumn a quarter turn apart.
- `clockface/scenes/wheels/day.svg` — one turn per civil day. The lower-right window's center is `135 + day_fraction × 360`. Night is around 135°, dawn around 225°, the daytime scene around 287° (that is 10:08 in the window), dusk around 45°.

Face radius 1 is 353.6 px from the center of the 1000×1000 view. The dial covers the inside of that circle. Draw motifs between about 1.02 and 1.41 face radii, and make them large enough to read in the corner (a tree or a sun should be a sizable fraction of the window, not a speck on a gradient). Replacing either SVG replaces the mural. No Rust change is required if the view box and the angle layout stay the same.

## The sun and the moon

Do not paint either disc in the SVG. The sun is the upper-left quarter (a GOES image). The moon is the upper-right quarter, a photograph shaded to the phase.

A group whose id is `moon` is forced to `display="none"` so a copied illustration does not appear in the landscape.

On Thursday 15 January 2026 at 10:08:37 the phase is about 0.89, a waning crescent, lit on the western (left) limb. The default theme at that instant shows winter, an overcast sky, the thick cloud deck, falling snow, and snow on the ground.

## Preview

From `clockface/`:

```bash
cargo build --release
./target/release/clockface --screenshot /tmp/theme.png --width 1920 --height 1080
```

That frame is local 2026-01-15 10:08:37, Indianapolis, imperial units, and the January fixture (light snow). It does not use the network. The status line is the machine you ran it on.

To force another instant you would need a different build. The screenshot hour is fixed so themes can be compared. To see summer or night, set `CLOCKFACE_SCENE` only after checking the layer logic with a small program, or temporarily call the same `visible_layers` rules from a unit test. `scene.rs` already tests that the January fixture enables `sky-overcast`, `leaves-winter`, `ground-winter`, `snow`, and `snow-cover`, and leaves the clear, partly, and night plates, the dawn and dusk tints, and the rain, storm, and fog groups off.

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
- After a change, run the screenshot and look at the lower half only. The upper quarters, the frames, and the status line are not the theme.
