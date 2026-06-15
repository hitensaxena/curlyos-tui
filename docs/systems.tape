# VHS tape: capture the Systems screens. Run from repo root: vhs docs/systems.tape
Output docs/_systems.gif
Set Shell bash
Set FontSize 13
Set Width 1560
Set Height 880
Set Padding 12
Hide
Type "/Users/hitensaxena/.cargo/bin/curlyos"
Enter
Sleep 5s
Type "5"
Sleep 3s
Show
Sleep 500ms
Screenshot docs/sys-overview.png
Type "ll"
Sleep 2.5s
Screenshot docs/sys-scheduler.png
Type "l"
Sleep 2.5s
Type "c"
Sleep 1.5s
Screenshot docs/sys-events-filtered.png
Type "q"
