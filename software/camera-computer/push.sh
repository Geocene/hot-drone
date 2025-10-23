#!/bin/bash

TARGET=drone@drone-1

ssh $TARGET "sudo systemctl stop camera.service"
ssh $TARGET "sudo systemctl stop led-status.service"
ssh $TARGET "sudo systemctl stop nexus.service"

ssh $TARGET "sudo systemctl disable camera.service"
ssh $TARGET "sudo systemctl disable led-status.service"
ssh $TARGET "sudo systemctl disable nexus.service"

rsync -av home/*.py $TARGET:
rsync -av home/*.yaml $TARGET:
rsync -av home/bin/ $TARGET:bin/
rsync -av home/.config/ $TARGET:.config/
rsync -av etc/udev/rules.d/*.rules $TARGET:

ssh $TARGET "sudo mv .config/systemd/user/*.service /etc/systemd/system/"
ssh $TARGET "sudo chown root:root /etc/systemd/system/camera.service"
ssh $TARGET "sudo chown root:root /etc/systemd/system/led-status.service"
ssh $TARGET "sudo chown root:root /etc/systemd/system/nexus.service"
ssh $TARGET "sudo systemctl enable camera.service"
ssh $TARGET "sudo systemctl enable led-status.service"
ssh $TARGET "sudo systemctl enable nexus.service"

ssh $TARGET "sudo mv *.rules /etc/udev/rules.d/"
ssh $TARGET "sudo chown root:root /etc/udev/rules.d/*.rules"
ssh $TARGET "sudo udevadm control --reload"
