#!/bin/sh

# Keep running python at_hose.py unless trap signal is received

while true; do
    python at_hose.py
    sleep 1
done
