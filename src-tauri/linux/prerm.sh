#!/bin/sh
# .deb prerm / .rpm %preun: on a real removal (not an upgrade), clean every user's
# PickMyBrowser registration, autostart entry and data, then stop running copies.
# deb passes "remove"/"purge"; rpm passes the remaining install count (0 = removal).
case "$1" in
  remove|purge|0) ;;
  *) exit 0 ;;
esac

BIN=/usr/bin/pickmybrowser
pkill -x pickmybrowser 2>/dev/null || true

if [ -x "$BIN" ]; then
  getent passwd | while IFS=: read -r user _ uid _ _ home _; do
    if [ "$uid" -ge 1000 ] 2>/dev/null && [ "$uid" -lt 65534 ] && [ -d "$home" ]; then
      su -s /bin/sh "$user" -c "HOME='$home' '$BIN' --uninstall" >/dev/null 2>&1 || true
    fi
  done
fi
exit 0
