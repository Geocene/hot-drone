#!/usr/bin/env python

import time
import os
import signal

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

def led_set_off():
    led_brightness_set("ACT", 0)

def led_set_on():
    led_brightness_set("ACT", 1)

def files_are_recent(path: str) -> bool:
    camera_metadata_mtime = 0
    camera_image_mtime = 0
    nexus_mtime = 0
    with os.scandir(path) as it:
        for entry in it:
            stat = entry.stat()
            if entry.name.endswith('.json'):
                if stat.st_mtime > camera_metadata_mtime:
                    camera_metadata_mtime = stat.st_mtime
            elif entry.name.endswith('.sbggr12'):
                if stat.st_mtime > camera_image_mtime:
                    camera_image_mtime = stat.st_mtime
            elif entry.name.endswith('_nexus.dat'):
                if stat.st_mtime > nexus_mtime:
                    nexus_mtime = stat.st_mtime

    time_now = time.time()

    camera_metadata_recent = (time_now - camera_metadata_mtime) < 5
    camera_image_recent = (time_now - camera_image_mtime) < 5
    nexus_recent = (time_now - nexus_mtime) < 5

    print(f"{camera_metadata_recent}:{camera_image_recent} {nexus_recent}")

    camera_recent = camera_metadata_recent and camera_image_recent
    all_recent = camera_recent and nexus_recent

    return all_recent

path_out = "/home/drone/out"

led_init()
led_set_off()

def shutdown(sig, frame):
    print("shutdown")
    led_set_off()
    sys.exit(0)

signal.signal(signal.SIGTERM, shutdown)

try:
    while True:
        if files_are_recent(path_out):
            led_set_on()
        else:
            led_set_off()

        time.sleep(0.10)
        led_set_off()
        time.sleep(0.90)
except:
    led_set_off()
