#!/bin/bash

# TARGET=drone@drone-1.lan
# TARGET=drone@10.23.42.2

# TARGET=drone@drone-2.lan
TARGET=drone@10.23.42.3

ssh $TARGET "sudo ~/bin/time-sync"

ssh $TARGET "sudo systemctl stop camera-capture"
ssh $TARGET "sudo systemctl stop camera-capture-0"
ssh $TARGET "sudo systemctl stop camera-capture-1"
ssh $TARGET "sudo systemctl stop camera-fc-time"
ssh $TARGET "sudo systemctl stop camera-led-monitor"
ssh $TARGET "sudo systemctl stop camera-sensors"

ssh $TARGET "sudo systemctl disable camera-capture"
ssh $TARGET "sudo systemctl disable camera-capture-0"
ssh $TARGET "sudo systemctl disable camera-capture-1"
ssh $TARGET "sudo systemctl disable camera-fc-time"
ssh $TARGET "sudo systemctl disable camera-led-monitor"
ssh $TARGET "sudo systemctl disable camera-sensors"

rsync -av home/*.py $TARGET:
#rsync -av imx477_*.json $TARGET:
rsync -av home/bin/ $TARGET:bin/
rsync -av home/.config/ $TARGET:.config/

ssh $TARGET "sudo mv .config/systemd/user/camera-* /etc/systemd/system/"
ssh $TARGET "sudo chown root:root /etc/systemd/system/camera-*.service"

ssh $TARGET "sudo systemctl enable camera-capture-0"
ssh $TARGET "sudo systemctl enable camera-capture-1"
ssh $TARGET "sudo systemctl enable camera-fc-time"
ssh $TARGET "sudo systemctl enable camera-led-monitor"
ssh $TARGET "sudo systemctl enable camera-sensors"

ssh $TARGET "sudo ~/bin/time-sync"
