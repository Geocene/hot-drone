#!/usr/bin/env python3

import sys
import os
import os.path
import json

import platform
pc = 'x86' in platform.platform()
if pc:
    print(f"running on PC")

import serial
tty_path = "/dev/ttyAMA0"
if pc:
    tty_path = "/dev/ttyUSB0"

tty = serial.Serial(tty_path, 115200, timeout=0.1, dsrdtr=False, rtscts=False, xonxoff=False)

if not pc:
    # This is necessary to set the camera sensor data received timeout very high,
    # so the software doesn't get impatient waiting for data to arrive if the external
    # trigger signal isn't yet active and pulsing.
    os.environ["LIBCAMERA_RPI_CONFIG_FILE"] = "rpi_apps.yaml"
    #os.environ["LIBCAMERA_LOG_LEVELS"] = "INFO"

    from picamera2 import Picamera2
    #import logging
    #Picamera2.set_logging(logging.INFO)

    from libcamera import controls, Transform

class CameraStill:
    def __init__(self, ordinal, frame_rate, tuning_file, output_dir):
        tuning = Picamera2.load_tuning_file(tuning_file)
        self._cam = Picamera2(ordinal, tuning=tuning)

        self._ordinal = ordinal
        self._frame_period = 1.0 / frame_rate
        self._output_dir = output_dir

        config = self._cam.create_preview_configuration(
            main={
                "size": (160, 120), # It seems I can't turn off the main stream, even if all I want is the raw stream. So at least make it small.
            },
            raw={
                "size": self._cam.sensor_resolution,
            },
            buffer_count=6,
        )
        self._cam.configure(config)

        self._cam.set_controls({
            # Auto-exposure
            "AeEnable": False,
            "AeFlickerMode": controls.AeFlickerModeEnum.Off,

            "AnalogueGain": 1.0,

            # Auto-white balance
            "AwbEnable": False,
            "AwbMode": controls.AwbModeEnum.Daylight,

            "Brightness": 0.0,  # 0.0: Normal
            "Contrast": 1.0,    # 1.0: Normal

            "ExposureTime": 1_000,  # Microseconds
            "ExposureValue": 0, # 0: "normal" exposure

            "FrameRate": frame_rate,

            "HdrMode": controls.HdrModeEnum.Off,

            "NoiseReductionMode": controls.draft.NoiseReductionModeEnum.Off,

            "Saturation": 1.0,  # 1.0: "normal" saturation
            "Sharpness": 0.0,   # 0.0: no additional sharpening performed, 1.0: "normal" level of sharpening
        })

    def run(self):
        self._cam.start()

        self._last_frame_wallclock = None

        try:
            while True:
                request = self._cam.capture_request()
                self.resolve(request)
        except KeyboardInterrupt:
            print("run: stop requested, exiting...")
        except:
            print(f"run: unexpected exception: {sys.exception()}")

        self._cam.stop()

    def resolve(self, request):
        metadata = request.get_metadata()
        sensor_timestamp = metadata["SensorTimestamp"] / 1e9    # in nanoseconds
        frame_wallclock = metadata["FrameWallClock"] / 1e9      # in nanoseconds

        # dropped_frame_str = " "
        # if self._last_frame_wallclock is not None:
        #     frame_delta = frame_wallclock - self._last_frame_wallclock
        #     frame_jitter = frame_delta - self._frame_period
        #     if abs(frame_jitter) > 0.010:   # Allow 10 milliseconds jitter.
        #         dropped_frame_str = "*"
        # self._last_frame_wallclock = frame_wallclock
        # print(f"{frame_wallclock:.3f} {dropped_frame_str} {sensor_timestamp:.3f}")

        # if "FocusFoM" in metadata:
        #     print(f"c{self._ordinal} FoM: {metadata['FocusFoM']}")

        buffer = request.make_buffer("raw")
        request.release()

        # Read the next camera filename from the KB2040 synchronization/data nexus.
        file_name_prefix = None
        while True:
            line = tty.readline()
            line = line.strip()
            if not line:
                break
            file_name_prefix = line

        if file_name_prefix is not None:
            file_name_prefix = file_name_prefix.decode()
            file_name = f"{file_name_prefix}_c{self._ordinal}"
            file_path = os.path.join(self._output_dir, file_name)

            buffer.tofile(file_path + ".sbggr12")

            with open(file_path + ".json", "w") as f:
                f.write(json.dumps(metadata))

            print(f"{file_name}")
        else:
            print("no file name prefix received, image discarded")

if pc:
    while True:
        line = tty.readline()
        line = line.strip()
        if line:
            print(f"{line}")
else:
    tuning_file = "/usr/share/libcamera/ipa/rpi/vc4/imx477_scientific.json"

    # TODO: Collect the camera ordinal for file naming purposes.
    camera_ordinal = 0

    camera = CameraStill(camera_ordinal, 1.0, tuning_file, "/home/drone/out")
    camera.run()
