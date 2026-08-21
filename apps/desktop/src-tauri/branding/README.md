# Desktop branding assets

The committed icon and DMG artwork are release inputs. Release builds consume
the checked-in files and do not regenerate them implicitly.

Regeneration requires macOS, Python 3, Pillow 12.1.1, the system `tiffutil`
command, and the Arial fonts included with macOS. From this directory:

```bash
python3 -m venv /tmp/sitecmd-branding-venv
/tmp/sitecmd-branding-venv/bin/pip install "Pillow==12.1.1"
/tmp/sitecmd-branding-venv/bin/python gen_assets.py
cd ..
pnpm exec tauri icon branding/icon-source.png
```

Review every changed image before committing it. The DMG release step uses
`dmg-background.tiff`; Tauri packages the generated icon set from `icons/`.

`dmgbuild-requirements.txt` is a separate, hash-pinned dependency set used by
the release workflow when it assembles the signed installer.
