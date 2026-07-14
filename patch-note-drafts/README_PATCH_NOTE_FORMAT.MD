# Porting to GH

Line spacing in the IDE is probably configured to better fit your IDE window, use pandoc or similar
to convert before pasting to GH:

### Example
```
pandoc patch-note-drafts/patch-notes-0.1.9.md \
  -f gfm -t gfm --wrap=none \
  -o patch-note-drafts/patch-notes-0.1.9.github.md
```