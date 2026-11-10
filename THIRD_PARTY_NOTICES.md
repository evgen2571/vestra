# Third-party notices

This notice accompanies Vestra software distributions, including binary wheels,
native CLI executables and source archives. Vestra's original software is [MIT licensed](LICENSE);
bundled third-party software and fonts retain the terms and notices below.

## FFmpeg

Binary wheels and native CLI executables statically link libraries from [FFmpeg](https://ffmpeg.org),
licensed under the GNU Lesser General Public License version 2.1 or later.
The [LGPL text](licenses/FFmpeg-LGPL-2.1.txt) accompanies Vestra distributions.
The normal build disables GPL, nonfree and version-3-only components and
external-library autodetection. The Python runtime executable is installed
through [imageio-ffmpeg](https://github.com/imageio/imageio-ffmpeg), whose Python
code is BSD-2-Clause. Its FFmpeg binaries have separate build configurations and
licenses; the Linux x86_64 0.6.0 binary reports GPLv3-or-later and includes
libx264. These terms differ from the linked LGPL libraries above. Consult the
dependency's wheel notices and its
[binary build/source repository](https://github.com/imageio/imageio-binaries).
The macOS Intel executable in imageio-ffmpeg 0.6.0 reports `--enable-nonfree`
and explicitly states that it is not legally redistributable. Do not repackage
that executable with an application. Arrange a redistributable executable and
dependency distribution when shipping an application, and select your executable
with `IMAGEIO_FFMPEG_EXE`; setting the override alone does not remove the
dependency's installed binary. Linux and Windows binaries report GPLv3-or-later;
the macOS ARM64 binary reports GPLv2-or-later.
Vestra invokes that executable as a separate process; it is not linked into the
extension. Anyone redistributing the dependency's binaries must review their
corresponding-source, notice and license obligations, including enabled codecs.
The CLI executable and optional `ffprobe` verification tool remain separately
installed.

Release builds preserve the exact FFmpeg checkout used by each wheel and CLI build in a
separate source archive, including source modifications, revision and configure
options. These archives accompany the GitHub Release downloads. Vestra's PyPI
source distribution includes the Rust/Python source, lockfile, and patched
`vendor/ffmpeg-sys-next` build script. Together these provide the source and
build instructions for rebuilding the native module with modified FFmpeg.
CLI rebuilds use the complete matching GitHub source revision, including the
CLI crate, rather than the Python sdist. Static linking
also requires review of LGPL §6 modification/relinking obligations; supplying
source archives alone is not a compliance certification.
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

The showcases use a CC0 landscape photo and original CC0 synthesized audio. Example media is documented beside
the [showcase sources](https://github.com/evgen2571/vestra/tree/bf527c93952b258f518ea6915551437ba629b545/examples/showcase),
including Manrope's accompanying OFL copy. These inputs are not bundled in wheels.
Older Location Kenya footage copies remain CC BY 3.0; preserve their
[source, license and modification credits](https://github.com/evgen2571/vestra/blob/bf527c93952b258f518ea6915551437ba629b545/examples/showcase/real-media-edit/README.md#earlier-footage-copies)
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
