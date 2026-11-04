#!/bin/bash
# Render SVG gradient, pattern and stroke cases with two magick binaries, before and after
# imagemagick-svg-url-fill.patch, and print pixel colours (expected: red to blue, left to right).
# Usage: ./svg-cases.sh <magick-before> <magick-after>
set -u
cd "$(mktemp -d)"
st='<stop offset="0" stop-color="#ff0000"/><stop offset="1" stop-color="#0000ff"/>'
mk() { printf '<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100"%s>%s</svg>' "$2" "$3" > "$1.svg"; }
mk lin "" "<defs><linearGradient id=\"g\">$st</linearGradient></defs><rect width=\"200\" height=\"100\" fill=\"url(#g)\"/>"
mk pct "" "<defs><linearGradient id=\"g\" x1=\"0%\" x2=\"100%\">$st</linearGradient></defs><rect width=\"200\" height=\"100\" fill=\"url(#g)\"/>"
mk user "" "<defs><linearGradient id=\"g\" gradientUnits=\"userSpaceOnUse\" x1=\"0\" y1=\"0\" x2=\"200\" y2=\"0\">$st</linearGradient></defs><rect width=\"200\" height=\"100\" fill=\"url(#g)\"/>"
mk vb ' viewBox="0 0 20 10"' "<defs><linearGradient id=\"g\">$st</linearGradient></defs><rect width=\"20\" height=\"10\" fill=\"url(#g)\"/>"
mk vert "" "<defs><linearGradient id=\"g\" x1=\"0\" y1=\"0\" x2=\"0\" y2=\"1\">$st</linearGradient></defs><rect width=\"200\" height=\"100\" fill=\"url(#g)\"/>"
mk rad "" "<defs><radialGradient id=\"g\">$st</radialGradient></defs><rect width=\"200\" height=\"100\" fill=\"url(#g)\"/>"
mk pat "" '<defs><pattern id="p" width="20" height="20" patternUnits="userSpaceOnUse"><rect width="10" height="20" fill="#00ff00"/></pattern></defs><rect width="200" height="100" fill="url(#p)"/>'
mk stroke "" "<defs><linearGradient id=\"g\" gradientUnits=\"userSpaceOnUse\" x1=\"0\" y1=\"0\" x2=\"200\" y2=\"0\">$st</linearGradient></defs><path d=\"M0 50 H200\" stroke=\"url(#g)\" stroke-width=\"20\" fill=\"none\"/>"
mk plain "" '<rect width="200" height="100" fill="#00ff00"/>'
fmt='%[pixel:p{5,50}] %[pixel:p{100,50}] %[pixel:p{194,50}] | top %[pixel:p{100,3}] bottom %[pixel:p{100,96}]'
for f in lin pct user vb vert rad pat stroke plain; do
  printf '%-7s before  %s\n' "$f" "$("$1" -background white "msvg:$f.svg" -format "$fmt" info: 2>&1)"
  printf '%-7s patched %s\n' "$f" "$("$2" -background white "msvg:$f.svg" -format "$fmt" info: 2>&1)"
done
