# Aggressive PDF import tasks

Goal: replace the PDF import placeholder with a bounded real parser for simple
PDF pages, preserving basic vectors, text, page size, and embedded raster
references.

- [x] Parse PDF page dictionaries, MediaBox dimensions, and page content
  streams.
- [x] Decode plain and Flate-compressed content streams.
- [x] Extract text drawing operators into SVG text nodes.
- [x] Extract simple vector path operators into SVG path/rect nodes.
- [x] Preserve embedded image XObject references as SVG image placeholders.
- [x] Add focused PDF import tests and run IO checks.
