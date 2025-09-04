# DNxHD GUI Flatpak Distribution & Cleanup Checklist

## 3. Documentation
- Confirm README-Flatpak.md includes:
  - Step-by-step install and uninstall instructions
  - Explanation of enabled permissions (filesystem, GPU, notifications, session-bus, GVFS)
  - Troubleshooting tips for codecs, file visibility, and formats
  - Link: https://flatpak.org/setup/ for end-users

## 4. Prepare for Flathub (optional)
- Add a top-level `com.davinci_convert.dnxhd-gui.metainfo.xml` (AppData/AppStream XML):
  - Name, summary, description
  - Screenshots (URLs), homepage, developer contact, license
- Ensure manifest only uses redistributable (free) software for the base bundle. If the app is GPL-incompatible, split out to a secondary repo as needed.
- Review Flatpak manifest's permissions; restrict to what's needed for Flathub acceptance.
- See: https://github.com/flathub/flathub/wiki/App-Submission

## 5. Distribution
- Publish `dnxhd-gui.flatpak` on GitHub Releases, project website, etc.
- Share the `README-Flatpak.md`, user install instructions, and support channels.
- Encourage users to test and report any sandbox/log errors for continued improvement.

## 6. Cleanup
- Remove or archive build artifacts after release:
  - `build-dir/`
  - `repo-dir/`
  - `com.davinci_convert.dnxhd-gui.minimal.json` (if obsolete)
- Store only the main manifest, bundle, and docs in your release tree.
