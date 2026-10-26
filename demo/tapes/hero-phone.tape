# hero.tape at about 43 columns, for the project page on a phone (hero-column.tape beside the
# text on wider screens). Same keys, so both show the same run.
Output "../../assets/opi-phone.gif"
Set Width 560
Set Height 700
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
