#!/bin/bash

TARGET=drone@drone-3.lan

# pushd ../firmware/data-logger-rs
# cargo build --release
# picotool uf2 convert target/thumbv6m-none-eabi/release/data-logger-rs -t elf kb2040.uf2 -t uf2
# popd

ssh -4 $TARGET "sudo systemctl stop camera.service"
ssh -4 $TARGET "sudo systemctl stop nexus.service"

ssh -4 $TARGET "sudo systemctl disable camera.service"
ssh -4 $TARGET "sudo systemctl disable nexus.service"

# cp ../firmware/data-logger-rs/kb2040.uf2 home/

rsync -av4 home/*.py $TARGET:
rsync -av4 home/*.yaml $TARGET:
# rsync -av4 home/*.uf2 $TARGET:
rsync -av4 home/bin/ $TARGET:bin/
rsync -av4 home/.config/ $TARGET:.config/
rsync -av4 etc/udev/rules.d/*.rules $TARGET:

ssh -4 $TARGET "sudo mv .config/systemd/user/*.service /etc/systemd/system/"
ssh -4 $TARGET "sudo chown root:root /etc/systemd/system/camera.service"
ssh -4 $TARGET "sudo chown root:root /etc/systemd/system/nexus.service"
ssh -4 $TARGET "sudo systemctl enable camera.service"
ssh -4 $TARGET "sudo systemctl enable nexus.service"

ssh -4 $TARGET "sudo mv *.rules /etc/udev/rules.d/"
ssh -4 $TARGET "sudo chown root:root /etc/udev/rules.d/*.rules"
ssh -4 $TARGET "sudo udevadm control --reload"
