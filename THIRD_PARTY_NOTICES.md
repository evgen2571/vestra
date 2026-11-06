# Third-party notices

This notice accompanies Vestra software distributions, including binary wheels
and source archives. Vestra's original software is [MIT licensed](LICENSE);
bundled third-party software and fonts retain the terms and notices below.

## FFmpeg

Binary wheels statically link libraries from [FFmpeg](https://ffmpeg.org),
licensed under the GNU Lesser General Public License version 2.1 or later.
The [LGPL text](licenses/FFmpeg-LGPL-2.1.txt) accompanies Vestra distributions.
The normal build disables GPL, nonfree and version-3-only components and
external-library autodetection. The runtime `ffmpeg` and `ffprobe` executables
are separately installed tools and may have different build configurations.

Release builds preserve the exact FFmpeg checkout used by each wheel in a
separate source archive, including source modifications, revision and configure
options. These archives accompany the GitHub Release downloads. Vestra's PyPI
source distribution includes the Rust/Python source, lockfile, and patched
`vendor/ffmpeg-sys-next` build script. Together these provide the source and
build instructions for rebuilding the native module with modified FFmpeg.
Use the corresponding archive rather than assuming the upstream release branch
still has the same contents. The [release procedure](docs/development/releasing.md)
covers distribution and verification of this material.

## Rust bindings

`ffmpeg-next` and the vendored `ffmpeg-sys-next` are maintained by the
[rust-ffmpeg project](https://github.com/zmwangx/rust-ffmpeg) and its contributors,
and distributed under WTFPL. Their source/license declarations remain in the
Cargo dependencies and vendored crate. Other Rust dependencies retain the
licenses declared by their respective Cargo packages; `Cargo.lock` identifies
resolved versions.

## Oklab constants

The perceptual palette metric uses mathematical coefficients from
[Björn Ottosson's original Oklab definition](https://bottosson.github.io/posts/oklab/),
whose conversion code is offered under a public-domain option. Vestra's integer
conversion, rounding, constant generator and WGSL implementation are original
MIT-licensed code. The sRGB transfer function follows
[CSS Color 4](https://www.w3.org/TR/css-color-4/#color-conversion-code).

## Showcase assets

The current documentation checkout uses CC0 Shanghai footage, a CC0 landscape
photo and original CC0 synthesized audio. Example media is documented beside
the [showcase sources](https://github.com/evgen2571/vestra/tree/docs/learning-navigation/examples/showcase),
including Manrope's accompanying OFL copy. These inputs are not bundled in wheels.
Older Location Kenya footage copies remain CC BY 3.0; preserve their
[source, license and modification credits](https://github.com/evgen2571/vestra/blob/docs/learning-navigation/examples/showcase/real-media-edit/README.md#earlier-footage-copies)
when redistributing them.

## Bundled text and ASCII glyph font

`crates/vestra-render-common/assets/DejaVuSans.ttf` is the repository's DejaVu-derived
VestraTest face, bundled as an immutable default font for ASCII effects.
The same font is shipped as `python/vestra/fonts/VestraDefault.ttf` for high-level Python Text, with its
notice alongside it in `python/vestra/fonts/LICENSE.txt`.
The ASCII copy is embedded in the native renderer; no system font fallback is used.
The complete Bitstream Vera / DejaVu notice is distributed in
[licenses/DejaVuSans.txt](licenses/DejaVuSans.txt) and beside the font as
`crates/vestra-render-common/assets/DejaVuSans.LICENSE`. DejaVu changes are public domain;
the original font retains the Bitstream copyright and redistribution terms.
