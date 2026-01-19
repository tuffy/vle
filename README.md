# Very Little Editor

The Very Little Editor, or VLE, is a small text editor
with just enough features to do some actual work.

Modern full-featured text editors can be a little daunting,
complete with dozens upon dozens of different commands
to remember and modes to switch between.
It's a lot of power, but that power comes with the cognative load
of having to remember which key combination to use or what
mode the editor is in at any given time.

Let's try a different approach and see just how *few* features we need.
By restricting our feature set to less than twenty powerful features,
we can devote more mental effort to our projects and less mental
effort to our tools.

# Installation

Installing VLE from source can be done using Cargo:

    cargo install vle

VLE compiles to a single self-contained binary which contains everything.
Its syntax highlighting for different languages are built-in
and it uses no configuration file; its minimal configuration options
are done via simple environment variables.

# Keybindings and Features

| Action                         | Keys  | Keys                       |
|-------------------------------:|-------|----------------------------|
| Open File                      | `F2`  | `Ctrl-O`                   |
| Save File                      | `F3`  | `Ctrl-S`                   |
| Goto Line                      | `F4`  | `Ctrl-T`                   |
| Find Text                      | `F5`  | `Ctrl-F`                   |
| Replace Text                   | `F6`  | `Ctrl-R`                   |
| Goto Matching Pair             | `F7`  | `Ctrl-P`                   |
| Select Inside Pair             | `F8`  | `Ctrl-E`                   |
| Widen Selection to Whole Lines | `F9`  | `Ctrl-W`                   |
| Reload File                    | `F11` | `Ctrl-L`                   |
| Quit Buffer                    | `F12` | `Ctrl-Q`                   |
| Highlight Text                 |       | `Shift-Arrows`             |
| Start/End of Selection         |       | `Ctrl-Home` `Ctrl-End`     |
| Cut/Copy/Paste                 |       | `Ctrl-X` `Ctrl-C` `Ctrl-V` |
| Undo/Redo                      |       | `Ctrl-Z` `Ctrl-Y`          |
| Switch Buffer                  |       | `Ctrl-PgUp` `Ctrl-PgDn`    |
| Open/Switch Pane               |       | `Ctrl-Arrows`              |

Because we have so few features, non-navigational features
have alternative `Ctrl`-based and `F`-based keybindings.

## Open Files

Files to open can be provided on the command line,
or via the open files dialog.  A filename can be provided
if one needs to open a new file, one can select a file
by navigating the filesystem interactively, or one can tag
multiple files and open several of them simultaneously.

## Save File

Writes any changes back to the file on disk.
Will report an I/O errors during writing, and will prompt
to overwrite a file should it be changed out from under us.

## Highlight Text and Cut / Copy / Paste

Highlighted text will be overwritten by the next keystroke,
or can be cut / copied to the cut buffer where it can be pasted
somewhere else.

## Undo and Redo

VLE features a limitless undo stack, which can back up
to the state of the file when first opened if necessary.
Or, if one undoes a little too much, a redo stack
to redo those changes is also provided.

## Goto Line

Prompts one for a line number to jump to and navigates to
that position in the file.
Can also navigate directly to the first and last lines of the
file without having to type in their respective line numbers.

## Find Text

Searches incrementally forward to the next possible match
as more text is entered.
Use arrow keys to cycle forward or backward through
all possible matches in the file.
The `Del` key can be used to cull matches from the match list.

## Replace Text

Must be initiated during the "Find Text" process
(we first find the text, then replace that text).
Removes all matches from the text and prompts one for
new text which is inserted interactively at all
matches simultaneously.
Feel free to use arrow keys to cycle between matches
during the replacement process.

## Goto Matching Pair

When the cursor is positioned at some directional
paired character (such as `(`, `[`, `{`, `<`),
this will jump forward or backward to its paired counterpart
(`)`, `]`, `}`, `>`).

## Select Inside Pair

When the cursor is positioned between two paired characters,
this will select all the text between them.
Executing the command again will widen the selection to include
the paired characters themselves.
And executing the command yet again will widen the selection
steadily outward to the next set of paired characters.

If it can't determine which pair of characters you mean
automatically, it will prompt for which set to use.

## Widen Selection to Whole Lines

If the start and end points of the selection are not at
the start and end of a line, this widens the selection until it is.

## Reload File

Performs the inverse of a file save; updates our text to
reflect the contents of the file on disk.
Sometimes handy if some other tool modifies our file
and we wish to continue working on it.
Prompts whether to overwrite our buffer's contents
if it has not yet been saved.

## Quit Buffer

Closes the current file buffer, prompting for a confirmation
if its contents have not yet been saved.
The editor quits once all buffers have been closed.

# Configuration

  With very little to configure, VLE doesn't use a config file at all.
  Any configuration is performed with two environmental variables:

  - `VLE_SPACES_PER_TAB` - the number of spaces to output per tab
  - `VLE_ALWAYS_TAB` - whether to always insert literal tabs

  No config file means there's one less thing to install,
  learn the format of, modify or break.
