# VHS tape: capture screenshots of each space against the live API.
# Run from the repo root:  vhs docs/shots.tape
Output docs/demo.gif

Set Shell bash
Set FontSize 13
Set Width 1560
Set Height 880
Set Padding 12

Hide
Type "/Users/hitensaxena/.cargo/bin/curlyos"
Enter
Sleep 6s
Show

# 1 — Home (live monitor)
Sleep 1s
Screenshot docs/home.png
Sleep 1s

# 2 — Memory · Browse
Type "2"
Sleep 3s
Screenshot docs/memory.png
Sleep 1s

# 3 — Mind · Attention  (Identity -> Principles -> Narrative -> Attention)
Type "3"
Sleep 800ms
Type "lll"
Sleep 3s
Screenshot docs/mind.png
Sleep 1s

# 4 — Graph
Type "4"
Sleep 3s
Screenshot docs/graph.png
Sleep 1s

# 5 — Systems · Agents  (Overview -> Agents)
Type "5"
Sleep 800ms
Type "l"
Sleep 3s
Screenshot docs/systems.png
Sleep 1s

Type "q"
Sleep 500ms
