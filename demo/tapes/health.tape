# Every check the project's own tools can answer, run concurrently, with the
# failing one's output relayed whole.
Output "../../assets/health.gif"
Set Width 1000
Set Height 620
Source config.tape

Hide
Type "cd /tmp/opi-demo/pulse" Enter
Type "clear" Enter
Show

Sleep 800ms
Type "opi --health" Sleep 400ms Enter
Sleep 6s
