# Very Little Editor

The Very Little Editor, or VLE, is a small text editor
with Just Enough Features™ to do some actual work.

It was born of my inability to find the exact right
text editor to match my tastes and distributed in the
hope that someone else might find it useful.

# Installation

TODO

# Keybindings and Features

| Action                         | Keys                       |
|-------------------------------:|----------------------------|
| Open File                      | `Ctrl-O`                   |
| Reload File                    | `F3`                       |
| Save                           | `Ctrl-S`                   |
| Switch Buffer                  | `Ctrl-PgUp` `Ctrl-PgDn`    |
| Quit Buffer                    | `Ctrl-Q`                   |
| Open/Switch Pane               | `Ctrl-Arrows`              |
| Swap Panes                     | `F2`                       |
| Highlight Text                 | `Shift-Arrows`             |
| Widen Selection to Whole Lines | `Ctrl-W`                   |
| Start/End of Selection         | `Ctrl-Home` `Ctrl-End`     |
| Handle Enveloped Items         | `Ctrl-E`                   |
| Cut/Copy/Paste                 | `Ctrl-X` `Ctrl-C` `Ctrl-V` |
| Undo/Redo                      | `Ctrl-Z` `Ctrl-Y`          |
| Goto Line                      | `Ctrl-T`                   |
| Find Text                      | `Ctrl-F`                   |

As one can see, it's not a very long list!

Multiple simultaneous open files are supported and each is placed
in its own buffer which can be switched between using
`Ctrl-PgUp` and `Ctrl-PgDn`.

In addition, the editor's single window can be split
horizontally or vertically into two panes using `Ctrl-Arrow`,
allowing one to work on multiple buffers simltaneously.

Incremental text highlighting along with cut, copy and paste
operations are supported, naturally.

Selecting inside surrounded values is supported with a
simple keybinding (`Ctrl-E`), as is surrounding the selection
with quotes, parantheses, etc. (also `Ctrl-E`).

Jumping to a matching pair via `Ctrl-P` makes it
easy to bounce to the beginning or end of some surrounded item.

Finding text is done incrementally with `Ctrl-F`,
advancing the cursor as a longer string to match is entered.

Replacing text is done with `Ctrl-R` after a find has been performed.
Replacement text is simply inserted interactively wherever
a match was found. This preserves the instant feedback
I enjoy in multi-cursor modes without the possibility of
accidentally leaving multi-cursor mode active for too long.

The cursor will always remain centered vertically beyond
the very beginning of the file, which makes it easy to
locate at all times.

Basic syntax highlighting for various popular languages
are built-in and require no additional syntax files.

Trailing whitespace at the end of lines is highlighted.

An unlimited undo stack is provided, as is the ability to
redo changes in case one performs an undo too many.

# Non-Features

- Editing Modes

  Many editors in the Vi lineage employ different modes for
  inserting and editing text. I understand the appeal of such power -
  having used such editors for a long time myself.
  But that kind of modal operation comes at the cost of having to
  mentally context-shift every time one wants to do something.
  "Am I in typing-stuff-in mode?  Am I in issuing-commands mode?"

  I've reached the point where I'd prefer an editor where I
  can simply type in text, cut/copy/paste it around, and
  save files without having to jump back-and-forth
  between two different modes.

- Themes

  Even TUI-based editors seem to enjoy slapping their own theme
  over my terminal's perfectly good theme.
  VLE does syntax highlighting where appropriate
  but otherwise it leaves your terminal colors alone.

- A Configuration File

  With very little to configure, VLE doesn't use a config file at all.
  Any configuration is performed with two environmental variables:

  - `VLE_SPACES_PER_TAB` - the number of spaces to output per tab
  - `VLE_ALWAYS_TAB` - whether to always insert literal tabs

  No config file means there's one less thing to install,
  learn the format of, modify or break.
