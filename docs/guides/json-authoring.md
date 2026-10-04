# Compose videos with JSON

Write a JSON project file and render it with `ve`. This is useful for hand-written
edits, templates or applications that generate video projects without Python.
JSON and Python use the same engine, but their authoring syntax differs: JSON
uses `layer` for stacking order, for example, while Python uses `z`.

The CLI is built separately from the Python package. Follow the
[CLI quickstart](../getting-started/cli-quickstart.md) for setup, including
`ffmpeg` and `ffprobe`. Commands below assume `ve` is on `PATH`; from a checkout,
you can replace `ve` with `cargo run -q -p vestra-cli --`.

## Start with a complete project

Save this as `project.json`. It creates a five-second blue video with no external
media files:

```json
{
  "schema_version": 1,
  "name": "My JSON video",
  "output": {
    "path": "output.mp4",
    "width": 640,
    "height": 360,
    "frame_rate": "30/1",
    "background": "#111827",
    "quality": "preview",
    "audio": false,
    "duration_mode": "explicit",
    "duration": 5
  },
  "assets": [],
  "visual": {
    "clips": [
      {
        "id": "background",
        "source": {"type": "solid_color", "colour": "#2563eb"},
        "start": 0,
        "duration": 5,
        "layer": 0,
        "opacity": {"base_value": 1}
      }
    ]
  }
}
```

Validate it, then render:

```bash
ve validate project.json
ve render project.json --output output.mp4 --render-backend cpu --overwrite
```

The result is a 640×360, 30 fps MP4. `--overwrite` allows replacement of an
existing output file. This guide only authors project files; no Python script
is needed.

The following sections build on this file. Each smaller JSON block is a fragment;
the accompanying text tells you where to insert or replace it. JSON does not
allow comments or trailing commas.

## Choose output settings

`output` describes the finished video:

| Setting | Meaning |
| --- | --- |
| `width`, `height` | Canvas size in pixels; each dimension must be even and between 2 and 8192. |
| `frame_rate` | Frames per second, as a number or a fraction such as `"30000/1001"`. |
| `duration` | Video length in seconds when `duration_mode` is `"explicit"`. |
| `background` | Canvas colour where no visual clip covers it. |
| `quality` | `"preview"`, `"balanced"` or `"high"`. |
| `audio` | Whether to include audio in the rendered file. |
| `path` | Requested MP4 output path; `--output` can override it. |

Use an explicit duration while learning. For duration inferred from the project,
set `duration_mode` to `"automatic"` and omit `duration`. Colours use `#RRGGBB`
or `#RRGGBBAA`, with the last pair controlling transparency.

## Add a layer and position it

A clip places a source on the timeline. Add this object after the background
inside `visual.clips`, separated by a comma:

```json
{
  "id": "panel",
  "source": {
    "type": "shape",
    "geometry": {"type": "rectangle", "width": 240, "height": 100},
    "fill": "#edf0e9"
  },
  "start": 0,
  "duration": 3,
  "layer": 1,
  "opacity": {"base_value": 1},
  "transform": {
    "position": {"base_value": {"x": 0.5, "y": 0.5}},
    "anchor": {"base_value": {"x": 0.5, "y": 0.5}},
    "scale": {"base_value": {"x": 1, "y": 1}}
  }
}
```

The panel appears for the first three seconds, above the background because its
`layer` is higher. Give sibling clips distinct IDs. `start` and `duration` are
seconds in the owning composition; a layer is active from its start up to its end.

The shape dimensions are pixels. Layer position uses canvas-relative coordinates:
`(0.5, 0.5)` is the centre. The anchor chooses the point within the source used
for positioning; scale `(1, 1)` keeps its size. When supplying a transform,
include `position`, `anchor` and `scale`. Rotation is optional.

Transforms apply to images, video, shapes, text and groups. A `solid_color`
source fills the canvas and cannot have a transform. Spectrum and particle
sources have their own layout fields. See the [source reference](../reference/README.md#source-reference)
for each source's syntax.

## Animate a property

Replace the panel's `opacity` object with this fade-in:

```json
{
  "base_value": 0,
  "keyframes": [
    {"time": 0, "value": 0, "interpolation": "linear"},
    {"time": 0.5, "value": 1, "interpolation": "ease_out"}
  ]
}
```

Keyframe times are measured from the clip's beginning, so the fade lasts half a
second even if you later move the clip. `base_value` supplies the unanimated
value; `keyframes` describe changes over time. Point properties such as position
use `{"x": ..., "y": ...}` values instead of numbers. See
[animation concepts](../concepts/effects-transitions-and-animation.md) for
interpolation options.

## Add an effect

Add an `effects` field to the panel clip with this array:

```json
[
  {"id": "soften", "type": "gaussian_blur", "radius": {"base_value": 3}}
]
```

This softens the panel with a three-pixel blur. Effect parameters can use property
objects like the opacity example. Effect types use snake-case names in JSON.
Effects run in their declared order within their rendering stage; consult the
[effects reference](../reference/effects.md) for parameters and placement rules.
For coverage and cutouts, see the [masks reference](../reference/masks.md) and
[geometric mask example](../../examples/projects/geometric-masks.json).

## Transition between two clips

Add this second panel to `visual.clips`:

```json
{
  "id": "next-panel",
  "source": {
    "type": "shape",
    "geometry": {"type": "rectangle", "width": 240, "height": 100},
    "fill": "#b8d99c"
  },
  "start": 2,
  "duration": 3,
  "layer": 2,
  "opacity": {"base_value": 1},
  "transform": {
    "position": {"base_value": {"x": 0.5, "y": 0.5}},
    "anchor": {"base_value": {"x": 0.5, "y": 0.5}},
    "scale": {"base_value": {"x": 1, "y": 1}}
  }
}
```

The panels overlap from second 2 to second 3. Add `transitions` alongside `clips`
inside `visual`, using this array for a one-second crossfade:

```json
[
  {
    "id": "panel-crossfade",
    "outgoing": "panel",
    "incoming": "next-panel",
    "start": 2,
    "duration": 1,
    "definition": {
      "outgoing": {
        "opacity": {"keyframes": [
          {"progress": 0, "value": 1, "interpolation": "ease_in_out"},
          {"progress": 1, "value": 0, "interpolation": "ease_in_out"}
        ]}
      },
      "incoming": {
        "opacity": {"keyframes": [
          {"progress": 0, "value": 0, "interpolation": "ease_in_out"},
          {"progress": 1, "value": 1, "interpolation": "ease_in_out"}
        ]}
      }
    }
  }
]
```

The endpoints are IDs of clips in the same composition. The transition must fit
inside both clips. Its placement uses composition seconds, while its animation
uses normalized `progress` from 0 to 1. JSON stores the transition's channels
explicitly; it does not use Python constructor names such as `Crossfade`.
See the [transition examples](../../examples/transitions/) for more
complete projects.

## Use media assets

Images, video, audio and fonts are declared in `assets` and referenced by ID.
To try a photo instead of the original background, put `photo.png` in an `assets`
directory beside `project.json`, then replace the project's `assets` with:

```json
[
  {"id": "photo", "type": "image", "source": "assets/photo.png"}
]
```

Replace the background clip with:

```json
{
  "id": "background",
  "source": {"type": "image", "asset": "photo"},
  "start": 0,
  "duration": 5,
  "layer": 0,
  "opacity": {"base_value": 1},
  "sizing": {"mode": "cover"},
  "transform": {
    "position": {"base_value": {"x": 0.5, "y": 0.5}},
    "anchor": {"base_value": {"x": 0.5, "y": 0.5}},
    "scale": {"base_value": {"x": 1, "y": 1}}
  }
}
```

`cover` fills the canvas and crops excess edges; `fit` keeps the whole image
visible. Image clips require a transform. For CLI project files, relative asset
paths resolve from the project file's directory. Keep the JSON file and its
assets together when moving or sharing a project.

For video, declare an asset with `type: "video"` and use a source with
`type: "video"`. The optional clip field `source_start` selects where playback
begins in the media, and `playback_rate` changes its speed. See the
[video reference](../reference/sources/video.md) for the full contract.

## Add audio

Place a track of at least five seconds at `assets/music.wav`. Append this entry
to the project's `assets` array:

```json
{"id": "music", "type": "audio", "source": "assets/music.wav"}
```

Set `output.audio` to `true` and add this top-level `audio` object alongside
`output`, `assets` and `visual`:

```json
{
  "tracks": [
    {
      "id": "soundtrack",
      "clips": [
        {
          "id": "music-clip",
          "asset": "music",
          "start": 0,
          "trim_start": 0,
          "trim_end": 5,
          "gain": 0.8,
          "fade_in": 0.5,
          "fade_out": 0.5
        }
      ]
    }
  ]
}
```

`start` positions audio on the project timeline. `trim_start` and `trim_end`
select the source-media interval, in seconds. Here it plays the first five
seconds at reduced gain with half-second fades. The [audio reference](../reference/audio.md)
covers mixing, gain automation and effects; the [signals reference](../reference/signals.md)
covers binding visual properties to audio analysis.

## Nest a composition

A clip with `source.type` set to `group` contains its own `clips` and optional
`transitions`. Starting from the two-panel project, replace both panels in
`visual.clips` with this group:

```json
{
  "id": "card",
  "source": {"type": "group", "clips": [], "transitions": []},
  "start": 0,
  "duration": 5,
  "layer": 1,
  "opacity": {"base_value": 1},
  "transform": {
    "position": {"base_value": {"x": 0.5, "y": 0.5}},
    "anchor": {"base_value": {"x": 0.5, "y": 0.5}},
    "scale": {"base_value": {"x": 0.8, "y": 0.8}}
  }
}
```

Move the two panel objects into `card.source.clips`. Move the crossfade from
`visual.transitions` into `card.source.transitions`, leaving the root transition
array empty. Keep the background at the root.

The group's scale now affects both panels together. Child timing is measured
from the group's start, so moving the group moves the whole scene without
rewriting its internal timing. Child transitions can only refer to siblings
inside that group. See [nested compositions](../reference/nested-compositions.md).

## Validate edits and diagnose errors

Run `ve validate project.json` after each step. It checks the format, project
relationships and usable resources. The [JSON Schema](../../schemas/project.schema.json)
can also provide field completion and structural checks in editors that support
JSON schemas. Schema validation alone does not check asset files or all timeline
relationships.

For example, setting a clip's `duration` to `0` is invalid. Read the diagnostic
location to find the clip, then give it a positive duration. If validation says
an asset is missing, check both its ID reference and its path relative to the
project file. Unknown fields are rejected, so check spelling against the
[project-format reference](../reference/project-format.md).

```bash
ve inspect project.json
ve validate project.json --format json
```

`inspect` shows the loaded project and asset information. `--format json` makes
validation results readable by other tools. For render failures, see
[FFmpeg troubleshooting](../troubleshooting/ffmpeg.md) and
[rendering troubleshooting](../troubleshooting/rendering.md).

## Further examples

Browse the [reference examples](../../examples/README.md) for complete JSON
projects demonstrating sources, effects, masks, transitions and audio. Use the
[project-format reference](../reference/project-format.md) for field definitions
and the [CLI rendering guide](cli/rendering.md) for command options.
