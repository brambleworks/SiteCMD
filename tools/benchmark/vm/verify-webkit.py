import sys

import gi

gi.require_version("Gtk", "3.0")
gi.require_version("WebKit2", "4.1")
from gi.repository import GLib, Gtk, WebKit2

window = Gtk.Window()
view = WebKit2.WebView()
window.add(view)
completed = False
load_finished = False


def check_title(webview, _property=None):
    global completed
    if load_finished and webview.get_title() == "SiteCMD VM rendering check":
        completed = True
        Gtk.main_quit()


def loaded(webview, event):
    global load_finished
    if event == WebKit2.LoadEvent.FINISHED:
        load_finished = True
        check_title(webview)


def expired():
    print(f"WebKit render timed out: loaded={load_finished}, title={view.get_title()!r}", file=sys.stderr)
    Gtk.main_quit()
    return False


view.connect("load-changed", loaded)
view.connect("notify::title", check_title)
view.load_html("<html><title>SiteCMD VM rendering check</title><body>Ready</body></html>", None)
window.show_all()
GLib.timeout_add_seconds(30, expired)
Gtk.main()
window.destroy()
sys.exit(0 if completed else 1)
