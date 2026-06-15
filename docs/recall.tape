# VHS tape: capture Recall states (empty / typing / loading / results).
# Run from repo root:  vhs docs/recall.tape
# Note: recall takes ~10-15s on a busy box, hence the long final Sleep.
Output docs/_recall.gif
Set Shell bash
Set FontSize 13
Set Width 1560
Set Height 880
Set Padding 12
Hide
Type "/Users/hitensaxena/.cargo/bin/curlyos"
Enter
Sleep 5s
Type "2"
Sleep 500ms
Type "ll"
Sleep 1500ms
Show
Sleep 500ms
Screenshot docs/recall-empty.png
Type "/"
Sleep 300ms
Type "who am I and what do I value"
Sleep 700ms
Screenshot docs/recall-typing.png
Enter
Sleep 1200ms
Screenshot docs/recall-loading.png
Sleep 16s
Screenshot docs/recall-results.png
Type "q"
