# DNxHD GUI Flatpak Installation

This is the Flatpak bundle for **DNxHD GUI**.

## Install
Download (or copy) `dnxhd-gui.flatpak` to your machine and run:

```sh
flatpak install --user ./dnxhd-gui.flatpak
flatpak run com.davinci_convert.dnxhd-gui
```

## Minimal Permissions
- GUI (Wayland/X11)
- Notifications
- Network (if required)
- D-Bus session-bus access (required for correct operation)
- GPU access to /dev/dri and /run/opengl
- No broad filesystem or system-bus privileges

## Uninstall
```sh
flatpak uninstall com.davinci_convert.dnxhd-gui
```

## Need Flatpak?
See [setup instructions for your distribution](https://flatpak.org/setup/).

## Questions or Issues?
Open an issue on the project page or contact the maintainer!
