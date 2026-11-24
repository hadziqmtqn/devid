#!/bin/bash
# Phase 13 A1, question 5: can one type.xml serve Debian/Ubuntu and Arch? Arch keeps the same font
# files elsewhere: ttf-dejavu in /usr/share/fonts/TTF and ttf-liberation in /usr/share/fonts/liberation
# (run 5), where Debian has /usr/share/fonts/truetype/{dejavu,liberation}. ImageMagick drops an entry
# whose file is missing, so a type.xml that lists every font twice, once per layout, should work on both.
#   fonts.sh both <imagick etc dir> <out dir>   write <out dir>/ with type.xml for both layouts
#   fonts.sh draw <configure dir> <php>         draw text in DejaVu Sans, Arial and Helvetica
set -euo pipefail
case "$1" in
  both)
    mkdir -p "$3"
    cp "$2"/*.xml "$3/"
    python3 - "$2/type.xml" "$3/type.xml" <<'PY'
import re, sys
src = open(sys.argv[1]).read()
lines = [l for l in src.splitlines() if l.strip().startswith("<type ")]
arch = [l.replace("/usr/share/fonts/truetype/dejavu/", "/usr/share/fonts/TTF/")
         .replace("/usr/share/fonts/truetype/liberation/", "/usr/share/fonts/liberation/") for l in lines]
arch = [l for l in arch if l not in lines]
open(sys.argv[2], "w").write(src.replace("</typemap>", "\n".join(arch) + "\n</typemap>"))
PY
    grep -c '<type ' "$3/type.xml"
    ;;
  draw)
    # imagick throws "unable to read font" when a family has no file it can open.
    MAGICK_CONFIGURE_PATH="$2" "$3" -r '
      $i = new Imagick(); $i->newImage(500, 60, "white"); $d = new ImagickDraw();
      foreach (["DejaVu-Sans", "Arial", "Helvetica"] as $f) { $d->setFont($f); $i->annotateImage($d, 10, 40, 0, "devid $f"); }
      echo "drew DejaVu-Sans, Arial and Helvetica\n";'
    ;;
esac
