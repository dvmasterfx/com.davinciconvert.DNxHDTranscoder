# DNxHD Transcoder (GTK4)

A simple GTK4 application to batch transcode videos into Avid DNxHD/DNxHR codecs, targeting MOV or MXF containers. It provides options for audio bit depth/channels, frame-rate control, timecode injection, and optional EBU R128 loudness normalization.

- App ID: `com.davinciconvert.DNxHDTranscoder`
- License: GPL-3.0-or-later

## Features
- Drag-and-drop or file chooser to select multiple input videos (MP4/MOV)
- Output container selection: MOV or MXF
- DNxHR profiles: LB, SQ, HQ, HQX (10-bit), 444 (10-bit)
- Audio depth: 16-bit or 24-bit PCM; channels: 2/4/8
- Preserve FPS or set a target FPS
- Optional timecode injection
- Optional EBU R128 (-23 LUFS) loudness normalization
- Live progress updates per file

Internally, the app shells out to `ffmpeg`/`ffprobe` and parses progress via `-progress pipe:1`.

## Build (native)
Prerequisites on Fedora (example):
- gtk4-devel, glib2-devel, pango-devel, cairo-devel, gdk-pixbuf2-devel
- rust/cargo

Build and run:
```
cargo build --release
./target/release/dnxhd_gui
```

Note: Native runs require `ffmpeg` and `ffprobe` available on PATH.

## Flatpak/Flathub
The repository includes a Flatpak manifest and packaging assets for Flathub.

- Manifest: `flatpak/com.davinciconvert.DNxHDTranscoder.json`
- Desktop file: `packaging/com.davinciconvert.DNxHDTranscoder.desktop`
- Icon: `packaging/com.davinciconvert.DNxHDTranscoder.svg`
- AppStream: `packaging/com.davinciconvert.DNxHDTranscoder.metainfo.xml`

Build locally with Flatpak:
```
flatpak-builder --user --force-clean build-dir flatpak/com.davinciconvert.DNxHDTranscoder.json
```
Run inside the Flatpak build:
```
flatpak-builder --run build-dir flatpak/com.davinciconvert.DNxHDTranscoder.json dnxhd_gui
```

### Runtimes
- Runtime: `org.gnome.Platform//48`
- SDK: `org.gnome.Sdk//48`
- Rust: `org.freedesktop.Sdk.Extension.rust-stable`

### Bundled dependencies
For portability and codec availability, the manifest builds and installs a minimal `ffmpeg` and `ffprobe` inside the sandbox (static, limited features: DNxHD/DNxHR encode, PCM, MOV/MXF, loudnorm filter, etc.).

### Permissions
Current `finish-args` are kept minimal and portal-friendly:
- Wayland/X11 sockets
- `RUST_LOG` env only

The GTK file chooser uses the document portal, so broad filesystem access is not required. If you prefer convenience for common downloads workflows, you can add `--filesystem=xdg-download`.

## AppStream validation
Validate the metainfo file locally:
```
appstream-util validate-relax packaging/com.davinciconvert.DNxHDTranscoder.metainfo.xml
```

## Contributing
Issues and pull requests are welcome. Please format code with `rustfmt` and follow idiomatic GTK4-Rust patterns.

## License
GPL-3.0-or-later. See the license header in source files or the project manifest.

