# Shared look for every opi recording.
#
# Sourced from the top of each tape, *after* its own Output/Width/Height:
# VHS ignores a `Set` that comes after the first command, and the prompt
# setup at the bottom of this file counts as one.

Require opi

Set Shell "bash"
Set FontFamily "JetBrains Mono"
Set FontSize 18
Set LineHeight 1.25
Set Padding 24
Set Margin 24
Set MarginFill "#11111b"
Set BorderRadius 10
Set Theme "Catppuccin Mocha"
Set TypingSpeed 55ms
Set PlaybackSpeed 1
Set WindowBar Colorful
Set WindowBarSize 36

# A prompt that says nothing about whose machine this is.
Hide
Type `export PS1="$(printf '\033[38;5;147m❯\033[0m ')"` Enter
Type "clear" Enter
Show
