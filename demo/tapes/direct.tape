# The third speed: no interface at all. A name runs it, arguments go through,
# and a workspace member's script is addressed by path.
Output "../../assets/direct.gif"
Set Width 1000
Set Height 480
Source config.tape

Hide
Type "cd /tmp/opi-demo/casoon-web" Enter
Type "clear" Enter
Show

Sleep 700ms
Type "opi dev" Sleep 400ms Enter
Sleep 4s
Ctrl+C
Sleep 800ms
Type "opi api/db:migrate" Sleep 400ms Enter
Sleep 4s
