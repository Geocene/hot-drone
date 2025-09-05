#!/usr/bin/env python

import time
import os
import platform

def led_trigger_set(name: str, mode: str):
    path = f"/sys/class/leds/{name}/trigger"
    with open(path, "w") as f:
        f.write(f"{mode}")

def led_brightness_set(name: str, value: int):
    path = f"/sys/class/leds/{name}/brightness"
    with open(path, "w") as f:
        f.write(f"{value}")

def led_init():
    led_trigger_set("ACT", "none")
    led_trigger_set("PWR", "none")

def led_set_off():
    led_brightness_set("ACT", 1)
    led_brightness_set("PWR", 0)

def led_set_green():
    led_brightness_set("ACT", 0)
    led_brightness_set("PWR", 0)

def led_set_red():
    led_brightness_set("ACT", 1)
    led_brightness_set("PWR", 1)

def led_set_both():
    led_brightness_set("ACT", 1)
    led_brightness_set("PWR", 0)

def files_are_recent(path, has_two_cameras=False) -> bool:
    camera_0_metadata_mtime = 0
    camera_0_image_mtime = 0
    camera_1_metadata_mtime = 0
    camera_1_image_mtime = 0
    sensors_mtime = 0
    fc_time_mtime = 0
    with os.scandir(path) as it:
        for entry in it:
            stat = entry.stat()
            if has_two_cameras:
                if entry.name.endswith('_c1.json'):
                    if stat.st_mtime > camera_1_metadata_mtime:
                        camera_1_metadata_mtime = stat.st_mtime
                elif entry.name.endswith('_c1.srggb16'):
                    if stat.st_mtime > camera_1_image_mtime:
                        camera_1_image_mtime = stat.st_mtime
                elif entry.name.endswith('_sensors.dat'):
                    if stat.st_mtime > sensors_mtime:
                        sensors_mtime = stat.st_mtime
                elif entry.name.endswith('_fc.txt'):
                    if stat.st_mtime > fc_time_mtime:
                        fc_time_mtime = stat.st_mtime

            if entry.name.endswith('_c0.json'):
                if stat.st_mtime > camera_0_metadata_mtime:
                    camera_0_metadata_mtime = stat.st_mtime
            elif entry.name.endswith('_c0.srggb16'):
                if stat.st_mtime > camera_0_image_mtime:
                    camera_0_image_mtime = stat.st_mtime

    time_now = time.time()

    camera_0_metadata_recent = (time_now - camera_0_metadata_mtime) < 5
    camera_0_image_recent = (time_now - camera_0_image_mtime) < 5
    camera_1_metadata_recent = (time_now - camera_1_metadata_mtime) < 5
    camera_1_image_recent = (time_now - camera_1_image_mtime) < 5
    sensors_recent = (time_now - sensors_mtime) < 5
    fc_time_recent = (time_now - fc_time_mtime) < 20    # Significant buffer delays flush to filesystem.

    print(f"{camera_0_metadata_recent}:{camera_0_image_recent} {camera_1_metadata_recent}:{camera_1_image_recent} {sensors_recent} {fc_time_recent}")

    camera_0_recent = camera_0_metadata_recent and camera_0_image_recent
    camera_1_recent = camera_1_metadata_recent and camera_1_image_recent

    all_recent = camera_0_recent and camera_1_recent and sensors_recent and fc_time_recent if has_two_cameras else camera_0_recent

    return all_recent

path_out = "/home/drone/out"

led_init()
led_set_off()

has_two_cameras = platform.node() == "drone-1"

fn_state = led_set_off

while True:
    for n in range(10):
        time.sleep(0.25)
        led_set_off()
        time.sleep(0.25)
        fn_state()

    if files_are_recent(path_out, has_two_cameras):
        fn_state = led_set_green
    else:
        fn_state = led_set_red
