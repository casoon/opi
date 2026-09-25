# The start screen, the tab row, one search across every group, and a script
# actually running. This is the README's first image and the one submitted
# to directories — it has to carry the whole idea on its own.
Output "../../assets/opi.gif"
Set Width 1180
Set Height 780
Source config.tape

Hide
Type "cd /tmp/opi-demo/casoon-web" Enter
Type "clear" Enter
Show

Sleep 800ms
Type "opi" Sleep 400ms Enter
Sleep 2.5s

# Twenty-three entries in ten groups: a row of tabs, not ten headings.
Right Sleep 1.2s
Right Sleep 1.2s
Left Sleep 1s

# `/` searches every group at once, including the workspace packages.
Type "/" Sleep 500ms
Type "db" Sleep 1.6s
Enter
Sleep 4s
