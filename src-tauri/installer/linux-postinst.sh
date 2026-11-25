#!/bin/sh
# Expose the lowercase `revboat` command next to the bundled /usr/bin/RevBoat binary.
if [ -x /usr/bin/RevBoat ] && [ ! -e /usr/bin/revboat ]; then
  ln -s /usr/bin/RevBoat /usr/bin/revboat
fi
exit 0
