#!/usr/bin/env python3

from fractions import Fraction
import sys
import os
import re
import json
from datetime import datetime, timedelta, timezone
from collections import defaultdict
from zoneinfo import ZoneInfo
import numpy

from lib.dng import write_dng
import lib.flight_log as flight_log
from lib.path import Flight
from lib.raw import imx477_raw_read
import lib.time_map as time_map
import lib.metadata as metadata

CAMERA_NAMES = {
    0: "v2 sony imx477c0 4056 3032 brown 0.6203",
    1: "v2 sony imx477c1 4056 3032 brown 0.6203",
    2: "v2 sony imx477c2 3032 4056 brown 0.8298",
}

RIG_CAMERAS = {
    CAMERA_NAMES[0]: {  # Down, nadir camera
        "rotation": [
            -3.14159265,
             0.0,
             0.0
        ],
        "translation": [
            0.0,    # +: To the right
            0.0,    # +: Toward Earth.
            0.0     # +: Toward front of vehicle.
        ]
    },
    CAMERA_NAMES[1]: {  # Forward
        "rotation": [
            2.35619449,
            0.0,
            0.0
        ],
        "translation": [
            0.0,    # +: To the right
            0.0,    # +: Toward Earth.
            0.0     # +: Toward front of vehicle.
        ]
    },
    CAMERA_NAMES[2]: {  # Right Side
        "rotation": [
             1.7599884,
            -1.7599884,
             0.72901107
        ],
        "translation": [
            0.0,    # +: To the right
            0.0,    # +: Toward Earth.
            0.0     # +: Toward front of vehicle.
        ]
    }
}

TIME_SYNC_MAP = [
    # These are Raspberry Pi #1 timestamps vs. flight computer timestamps, when matching
    # status text messages in both logs.

    # TODO: Find an automated, or better way to generat this data, or... just improve
    # the synchronization process.

    (1755721057.556, 1755721058.342287), # "EKF3 IMU1 MAG0 in-flight yaw alignment complete"
    (1755721062.198, 1755721062.984508), # "Mission: 1 Takeoff"
    (1755721074.101, 1755721074.887313), # "Mission: 2 WP"
    (1755721105.042, 1755721105.827057), # "Reached command #2"
    (1755721125.451, 1755721126.234538), # "Reached command #4"
    (1755721133.272, 1755721134.055284), # "Reached command #6"
    (1755721153.201, 1755721153.984489), # "Reached command #8"
    (1755721161.019, 1755721161.802059), # "Reached command #10"
    (1755721180.780, 1755721181.562304), # "Reached command #12"
    (1755721184.919, 1755721185.699968), # "Fence Breached"
    (1755721299.222, 1755721299.999963), # "Battery 1 is low 14.20V used 1284 mAh"
    (1755721399.927, 1755721400.700620), # "PreArm: Battery 1 below minimum arming voltage"
]

TZ_LOCAL = ZoneInfo("America/Los_Angeles")

# def process_images(flight: Flight, shape: tuple, format: str, dir_path_out: str):
#     for file_raw in flight.raw_files:
#         metadata_file = flight.metadata_for_raw_file(file_raw)

#         data = imx477_raw_read(file_raw.path, shape)
#         write_dng(file_raw.datetime, file_raw.cam, data, format, file_path_dng, metadata_file)

#         row = (file_name_dng, )

path_flight = sys.argv[1]
file_flight_log = sys.argv[2]
path_odm = sys.argv[3]

write_dngs = False
write_extension = ".tif"

flight = Flight(path_flight, file_flight_log)

images_path = os.path.join(path_odm, "images")
opensfm_path = os.path.join(path_odm, "opensfm")
geo_txt_path = os.path.join(path_odm, "geo.txt")

time_map_pi_to_fc = time_map.TimeSync(TIME_SYNC_MAP)

exif_metadata = []

for metadata_file in flight.metadata_files:
    meta = metadata_file.data
    dt_pi = metadata_file.datetime
    dt_fc = time_map_pi_to_fc.forward(dt_pi)
    if dt_fc is not None:
        vehicle_attitude = flight.log.attitude_interp(dt_fc)
        vehicle_position = flight.log.position_interp(dt_fc)
        if vehicle_position['alt'] < 190:
            continue

        file_name_raw = flight.raw_file_for_metadata(metadata_file)
        file_name_out = None
        if write_dngs:
            file_name_dng = file_name_raw.file_name_base + ".dng"
            file_path_dng = os.path.join(images_path, file_name_dng)
            shape_dng = (4064, 3040)
            format_dng = "SRGGB12"
            data = imx477_raw_read(file_name_raw.path, shape_dng)
            write_dng(dt_fc, file_name_raw.cam, data, format_dng, file_path_dng, metadata_file)
            file_name_out = file_name_dng
        else:
            file_name_out = file_name_raw.file_name_base + write_extension

        orientation = (1, "Horizontal (normal)")
        if metadata_file.cam == 2:
            orientation = (8, "Rotate 270 CW")

        lat = (-vehicle_position["lat"], "South") if vehicle_position["lat"] < 0 else (vehicle_position["lat"], "North")
        lng = (-vehicle_position["lng"], "West") if vehicle_position["lng"] < 0 else (vehicle_position["lng"], "East")
        d = {
            "SourceFile": os.path.join("images", file_name_out),
            "Directory": "images",
            "FileName": file_name_out,

            "DateTime": dt_fc.strftime("%Y:%m:%d %H:%M:%S"),
            "SubSecTime": f"{dt_fc.microsecond//1000:03d}",
            "OffsetTime": "-07:00",
            "DateTimeOriginal": dt_fc.strftime("%Y:%m:%d %H:%M:%S"),
            "SubSecTimeOriginal": f"{dt_fc.microsecond//1000:03d}",
            "OffsetTimeOriginal": "-07:00",

            "GPSLatitude": lat[0],
            "GPSLatitudeRef": lat[1],
            "GPSLongitude": lng[0],
            "GPSLongitudeRef": lng[1],
            "GPSAltitude": vehicle_position["alt"],
            "GPSAltitudeRef": "Above Sea Level",
            # "GPSPosition": "...",
            # "GPSVersionID": "2 3 0 0",

            "Make": "Sony",
            "Model": f"IMX477c{metadata_file.cam}",

            "Aperture": 2.8,
            "ExifImageWidth": 4032,
            "ExifImageHeight": 3024,
            "ExposureTime": "1/1000",
            "ShutterSpeedValue": "1/1000",
            "ShutterSpeed": "1/1000",
            "FNumber": 2.8,
            "FocalLength": "3.9 mm",
            "ISO": 100,
        }
        if write_dngs == False:
            d["Orientation"] = orientation[1]

        exif_metadata.append(d)

        # print(dt_pi, vehicle_position)
        # for camera in sorted(metadata, key=lambda d: d["cam"]):
        #     camera_ordinal = camera['cam']
        #     camera_attitude = vehicle_attitude.copy()

import csv

exiftool_column_names = [
    'SourceFile',
    "Directory",
    "FileName",

    "DateTime",
    "SubSecTime",
    "OffsetTime",
    "DateTimeOriginal",
    "SubSecTimeOriginal",
    "OffsetTimeOriginal",

    "GPSLatitude",
    "GPSLatitudeRef",
    "GPSLongitude",
    "GPSLongitudeRef",
    "GPSAltitude",
    "GPSAltitudeRef",

    "Make",
    "Model",

    "Aperture",
    "ExifImageWidth",
    "ExifImageHeight",
    "ExposureTime",
    "ShutterSpeedValue",
    "ShutterSpeed",
    "FNumber",
    "FocalLength",
    "ISO",
]
if write_dngs == False:
    exiftool_column_names.append("Orientation")

path_exiftool_csv = os.path.join(path_odm, 'exiftool.csv')
with open(path_exiftool_csv, 'w') as f:
    writer = csv.DictWriter(f, fieldnames=exiftool_column_names)
    writer.writeheader()
    writer.writerows(exif_metadata)

# To update the images with metadata from the above CSV file:
# Run `exiftool -csv=exiftool.csv images`
