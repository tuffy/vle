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

```bash
cargo install vle
```

VLE compiles to a single self-contained binary which contains everything.
Its syntax highlighting for different languages are built-in
and it uses no configuration file; its minimal configuration options
are done via simple environment variables.

# Keybindings and Features

| Action                         | Shortcut       | Shortcut                           |
|-------------------------------:|----------------|------------------------------------|
| Open File                      | <kbd>F2</kbd>  | <kbd>Ctrl</kbd>-<kbd>O</kdb>       |
| Save File                      | <kbd>F3</kbd>  | <kbd>Ctrl</kbd>-<kbd>S</kdb>       |
| Goto Line                      | <kbd>F4</kbd>  | <kbd>Ctrl</kbd>-<kbd>T</kdb>       |
| Find Text                      | <kbd>F5</kbd>  | <kbd>Ctrl</kbd>-<kbd>F</kdb>       |
| Replace Text                   | <kbd>F6</kbd>  | <kbd>Ctrl</kbd>-<kbd>R</kdb>       |
| Goto Matching Pair             | <kbd>F7</kbd>  | <kbd>Ctrl</kbd>-<kbd>P</kdb>       |
| Select Inside Pair             | <kbd>F8</kbd>  | <kbd>Ctrl</kbd>-<kbd>E</kdb>       |
| Widen Selection to Whole Lines | <kbd>F9</kbd>  | <kbd>Ctrl</kbd>-<kbd>W</kdb>       |
| Split/Un-Split Pane            | <kbd>F10/<kbd> | <kbd>Ctrl</kbd>-<kbd>N</kdb>       |
| Reload File                    | <kbd>F11/<kbd> | <kbd>Ctrl</kbd>-<kbd>L</kdb>       |
| Quit File                      | <kbd>F12/<kbd> | <kbd>Ctrl</kbd>-<kbd>Q</kdb>       |
| Highlight Text                 |                | <kbd>Shift</kbd>-<kbd>Arrows</kbd> |
| Start                          |                | <kbd>Ctrl</kbd>-<kbd>Home<kbd>     |
| End of Selection               |                | <kbd>Ctrl</kbd>-<kbd>End</kbd>     |
| Cut                            |                | <kbd>Ctrl</kbd>-<kbd>X<kbd>        |
| Copy                           |                | <kbd>Ctrl</kbd>-<kbd>C<kbd>        |
| Paste                          |                | <kbd>Ctrl</kbd>-<kbd>V</kbd>       |
| Undo                           |                | <kbd>Ctrl</kbd>-<kbd>Z</kbd>       |
| Redo                           |                | <kbd>Ctrl</kbd>-<kbd>Y</kbd>       |
| Previous Buffer                |                | <kbd>Ctrl</kbd>-<kbd>PgUp<kbd>     |
| Next Buffer                    |                | <kbd>Ctrl</kbd>-<kbd>PgDn</kbd>    |
| Switch Pane                    |                | <kbd>Ctrl</kbd>-<kbd>Arrows</kbd/> |

Because we have so few features, non-navigational features
have alternative <kbd>Ctrl<kbd>-based and <kbd>F<kbd>-based keybindings.
This also helps maintain compatibility with terminal multiplexers
which have many of their own dedicated <kbd>Ctrl<kbd> bindings.

## Open File

Files to open can be provided on the command line,
or via the open files dialog.  A filename can be provided
if one needs to open a new file. One can select a file
by navigating the filesystem interactively. Or one can tag
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
The <kbd>Del<kbd> key can be used to cull matches from the match list.

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
paired character (such as <kbd>(</kbd>, <kbd>[</kbd>, <kbd>{</kbd>, <kbd>&lt;</kbd>),
this will jump forward or backward to its paired counterpart
(<kbd>)</kbd>, <kbd>]</kbd>, <kbd>}</kbd>, <kbd>&gt;</kbd>).

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

## Split Pane

Divides a single large window into two separate panes,
either horizontally or vertically.
Each pane may contain a different buffer, or different
locations within the same buffer.

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

# Syntax Highlighting

VLE has syntax has built-in syntax highlighting for the following
languages / file formats:

- Bourne Shell
- C
- C++
- CSS
- CSV
- Fish Shell
- Go
- HTML
- INI
- Java
- JavaScript
- JSON
- Makefile
- Markdown
- Patch
- Perl
- PHP
- Python
- Rust
- SQL
- Swift
- (La)TeX
- TOML
- XML
- YAML
- Zig

Syntax highlighting is done naively with an emphasis
on colorizing known keywords, strings, etc.
but may handle all cases since it doesn't do
rigorous syntax analysis.

# Why Another Editor?

I've tried *a lot* of different text editors over the years,
all with their own strengths and weaknesses.
But I could never find the exact text editor I was looking for.
Some were a little too overcomplicated.
Some were a little too primitive.
So rather than continue a seemingly endless search
for the exact right text editor for me, I decided to write my own
by mixing necessary features (like file saving) and those
that impressed me in other editors (like splitting panes
or selecting inside quotes).

Whether it's the editor for you depends on your needs and tastes.
But VLE has been developed exclusively with itself since version 0.2,
so I can confidently say that it's good enough for projects
at least as large as itself.
