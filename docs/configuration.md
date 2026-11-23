# Configuration

oxishot keeps its settings in one TOML file:

- `$XDG_CONFIG_HOME/oxishot/config.toml` if `XDG_CONFIG_HOME` is set
- otherwise `~/.config/oxishot/config.toml`

You can change it in two ways:

- **The settings dialog:** click the gear button in the top-right toolbar. **Save** writes the file and the changes apply right away. Your comments and the file's layout stay. Values are written in the standard form (for example `"ctrl+z"` becomes `"Ctrl+Z"`).
- **By hand:** edit the file in any text editor. **Open config file** in the dialog creates it with the defaults if it doesn't exist yet. Edits apply the next time oxishot starts or you open the settings.

Anything you leave out falls back to its default. If an entry is invalid, oxishot reports it (on stderr and in the settings dialog) and uses the default for that entry only. The rest of the file still loads.

## Example

```toml
[save]
directory = "~/Pictures/Screenshots"
skip_dialog = true

[keys]
undo = "Ctrl+Z"
redo = ["Ctrl+Y", "Ctrl+Shift+Z"]
arrow = "X"
exit = "Q"
```

## `[save]`

| Key           | Type   | Default | Meaning |
| ------------- | ------ | ------- | ------- |
| `directory`   | string | `""`    | The folder screenshots are saved to. The save dialog starts here. `~` is your home directory. Leave it empty to let the dialog decide. |
| `skip_dialog` | bool   | `false` | When `true`, Save writes `oxishot-<timestamp>.png` straight into `directory` and closes, without showing a dialog. The folder is created if it doesn't exist. |

If you start oxishot with `-o <path>` while `skip_dialog` is on, it saves to that path instead of `directory`.

## `[keys]`

Each action takes one shortcut, a list of shortcuts, or `""` to leave it unbound.

```toml
save = "Ctrl+S"                    # one shortcut
delete = ["Delete", "Backspace"]   # any of these
exit = ""                          # unbound
```

### Actions

| Action       | Default                     |
| ------------ | --------------------------- |
| `undo`       | `"Ctrl+Z"`                  |
| `redo`       | `"Ctrl+Y"`                  |
| `save`       | `"Ctrl+S"`                  |
| `copy`       | `"Ctrl+C"`                  |
| `select_all` | `"Ctrl+A"`                  |
| `delete`     | `["Delete", "Backspace"]`   |
| `select`     | `"V"`                       |
| `brush`      | `"B"`                       |
| `text`       | `"T"`                       |
| `rectangle`  | `"R"`                       |
| `circle`     | `"O"`                       |
| `arrow`      | `"A"`                       |
| `blur`       | `"U"`                       |
| `eyedropper` | `"I"`                       |
| `settings`   | `""`                        |
| `exit`       | `""`                        |

`delete` only acts while the Select tool is active.

### Writing shortcuts

A shortcut is zero or more modifiers and a key, joined with `+`: `"Ctrl+Shift+Z"`, `"Alt+F4"`, `"B"`.

- **Modifiers:** `Ctrl`, `Alt`, `Shift`
- **Keys:** any single character (`"A"`, `"1"`, `"/"`, `"+"`), or one of `Delete`, `Backspace`, `Tab`, `Enter`, `Space`, `Home`, `End`, `PageUp`, `PageDown`, `Insert`, `Up`, `Down`, `Left`, `Right`, `F1` to `F12`

Names aren't case-sensitive, so `"ctrl+z"` works too. Letters ignore case unless you add `Shift`: `"Z"` is the Z key and `"Shift+Z"` is Shift and Z. For symbols you type with Shift, write the symbol itself (`"!"`, not `"Shift+1"`).

`Escape` can't be bound. It always deselects, closes popups and the settings dialog, and cancels setting a shortcut in the dialog.

If two actions share a shortcut, the one listed first in the table above wins. The settings dialog marks shared shortcuts in amber.

### In the settings dialog

Click an action's shortcut and press the keys you want. `Escape` cancels, and the × button unbinds the action. A shortcut set in the dialog replaces all of that action's shortcuts. To give one action several shortcuts, use a list in the file.
