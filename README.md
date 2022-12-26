# Launcher

A Linux application launcher.

![window_manager](screenshot.png)

## Building

To build and install:

```sh
$ make && sudo make install
```

The makefile will build the program using `cargo build` and create the history
file and set its permissions.

Debug builds:

```sh
# Just build it:
$ cargo build
# Or to run it as well:
$ cargo run
```

Note that debug builds do not set the `override_redirect` flag on the window so they will appear as normal windows,
this is required for me to test it using X forwarding on Windows.

### Dependencies

- `libX11`
- `libXinerama`

## Functionality

The program searches for programs similar to the search text in both [Freedesktop Desktop Entries](https://wiki.archlinux.org/title/desktop_entries) and executables in the `PATH` environment variable.

Desktop entries are searched for the localized name, normal name, localized generic name, generic name, and the name of the desktop file.

All these ways of matching have a different priority which will give a slight boost to their score and resulting order in the results list (higher boost at the top):

- Localized name
- Localized generic name
- Normal name
- Generic name
- Name of executable in `PATH`
- Name of `.desktop` file

Additionally entries that are in the history gain a large priority bonus.

The locale used for localized name is either extracted from `LC_MESSAGES` (or `LANG` if not set) or the `locale` value in the configuration.
To disable localized names just set the `locale` value in the config to an empty string or some other invalid value.

## Controls

Initially the text entry box is focused, this supports most common text editing
controls (cursor movement, selection when holding shift, ctrl+left/right to jump words, home/end to jump to the begin or end).

Pressing the down arrow once while the input if focused changes the focus to the list view, in here the cursor can be moved using the up/down arrows, home, and end. Pressing the up arrow when the first item is selected changes focus back to the input box.

Additionally pressing tab always swap the input focus.

In the list view, pressing Enter will launch the selected program.

If the input text is empty all the items in the history are displayed, in this mode pressing
delete will remove the selected item from the history.

Clicking and item selects it and double-clicking it launches the program.

## Configuration

The configuration is a TOML file located at `~/.config/launcher.toml` with the following values:

```toml
# Dimensions of the window in percentage of the main monitor size.
window_width_percent = 50
window_height_percent = 50

# Height of the text entry box in pixels.
entry_height = 48

# Height of each item in the results list in pixels, the height of the window
# will be adjusted so that the results list height is a multiple of this value.
list_item_height = 44

# Width of the scroll bar for the results list, or 0 to disable it
scroll_bar_width = 8

# Font for the text entry
entry_font = "sans 24"

# Font for the results list
list_font = "sans 20"

# Font the "No results" banner if there are no search results, this will be
# centered inside the list view.
list_empty_font = "sans 48"

# The locale to use for localized keys, this has no default value as the current
# locale is used if not specified. Note that a lot of keys only use the LANG
# part of the locale so you can for example use "ko" instead of "ko_KR.UTF8".
locale = "no default value"

# The icon theme, this sould be the name of a folder in one of the following
# locations, containing a "48x48" folder:
#  - /usr/share/icons/
#  - ~/.local/share/icons/
#  - ~/.icons/
icon_theme = "Papirus"

# Mouse scroll speed for the results list.
scroll_speed = 10
```

The values specified here are the default values used if not defined.

## Icons

This projects contains the following icons from [Google Fonts](https://fonts.google.com/icons):

- `res/search.svg`: https://fonts.google.com/icons?selected=Material%20Symbols%20Outlined%3Asearch%3AFILL%400%3Bwght%40400%3BGRAD%400%3Bopsz%4048

- `res/history.svg`: https://fonts.google.com/icons?selected=Material%20Symbols%20Outlined%3Ahistory%3AFILL%400%3Bwght%40400%3BGRAD%400%3Bopsz%4048
