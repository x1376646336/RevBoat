#!/bin/sh
# Remove the `revboat` link once the binary it points to is gone (skips upgrades).
if [ -L /usr/bin/revboat ] && [ ! -e /usr/bin/RevBoat ]; then
  rm -f /usr/bin/revboat
fi
exit 0
