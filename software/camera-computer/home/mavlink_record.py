#!/usr/bin/env python3

# Setup:
# cd ~
# sudo apt install pip
# python -m venv venv
# source venv/bin/activate
# pip install pymavlink
# pip install pyserial
#
# And of course, be sure to start with created venv.

import time
import os.path

from pymavlink import mavutil

connection = mavutil.mavlink_connection("/dev/ttyAMA0", 921600, source_system=255, notimestamps=False, robust_parsing=True)

def set_message_interval(master, message_id: int, interval_usec: int):
    master.mav.command_long_send(
        master.target_system,
        master.target_component,
        mavutil.mavlink.MAV_CMD_SET_MESSAGE_INTERVAL,
        0,
        message_id, interval_usec, 0, 0, 0, 0, 0)

def request_message(master, message_id: int):
    master.mav.command_long_send(
        master.target_system,
        master.target_component,
        mavutil.mavlink.MAV_CMD_REQUEST_MESSAGE,
        0,
        message_id, 0, 0, 0, 0, 0, 0)

# request_message(connection, mavutil.mavlink.MAVLINK_MSG_ID_GPS_RAW_INT)
set_message_interval(connection, mavutil.mavlink.MAVLINK_MSG_ID_GPS_RAW_INT, 1_000_000)

path_out = "/home/drone/out"
start_time = time.time()
filename_out = os.path.join(path_out, f"{start_time:.3f}_fc.txt")
f = open(filename_out, 'w')

while True:
    d = connection.recv_match(blocking=True)
    recv_time = time.time()
    f.write(f"{recv_time:.3f} {d}\n")
