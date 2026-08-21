# Deterministic installer layout generated without Finder.
# release.yml supplies the app, background, and volume icon paths.
import os.path

application = defines["app"]
appname = os.path.basename(application)

volume_name = "SiteCMD"
icon = defines["volicon"]

# Zlib-compressed, read-only DMG.
format = defines.get("format", "UDZO")

files = [application]
symlinks = {"Applications": "/Applications"}

background = defines["background"]

# Positions align with the background artwork.
window_rect = ((200, 120), (660, 400))
default_view = "icon-view"
icon_size = 128
text_size = 12
show_status_bar = False
show_tab_view = False
show_toolbar = False
show_pathbar = False
show_sidebar = False
sidebar_width = 0
arrange_by = None

icon_locations = {
    appname: (165, 190),
    "Applications": (495, 190),
}
