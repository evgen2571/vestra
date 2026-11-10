# Landscape inputs

`landscape.jpg` is the 1600-pixel derivative of “Landscape in Salzburger Land”
by Dimitry Anikin, from [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Landscape_in_Salzburger_Land.jpg),
dedicated to [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/).
The Commons license statement was checked on 2026-11-06. The image was resized,
JPEG re-encoded and stripped of metadata.

Download the [original photograph](https://upload.wikimedia.org/wikipedia/commons/e/e5/Landscape_in_Salzburger_Land.jpg)
to `target/landscape-original.jpg` and verify SHA-256
`6936f552123608df0552d3b3ddaaeabdff3ed233e669b04676ac8078d6c0aad6`.
Reproduce the derivative from the repository root:

```bash
ffmpeg -i target/landscape-original.jpg -vf scale=1600:-1 -map_metadata -1 -q:v 3 assets/images/landscape.jpg
```

`landscape-study.jpg` is the distinct 640×220 crop used by the visual learning
studies. It preserves their existing input and framing; it is not another copy
of the full photograph. Reproduce it with the command in [learning-inputs.txt](learning-inputs.txt).
Encoding bytes may vary with FFmpeg versions.
