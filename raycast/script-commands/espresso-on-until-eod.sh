#!/bin/bash
# Required parameters:
# @raycast.schemaVersion 1
# @raycast.title Espresso: On until 18:00
# @raycast.mode silent
# @raycast.packageName EspressoMacchiato
# @raycast.icon ☕
# Adjust to your end of day; the app's own "end of day" lives in its settings.
open "espresso://on?for=$(( ($(date -j -f "%H:%M" "18:00" +%s 2>/dev/null || echo 0) - $(date +%s) + 86400) % 86400 ))"
echo "On until 18:00"
