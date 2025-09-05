#!/usr/bin/env python3

import sys
import time
import argparse
import os.path
import json

from picamera2 import Picamera2

from libcamera import controls, Transform

ready_line = None

class CameraStill:
    def __init__(self, ordinal, frame_rate, tuning_file, file_type, output_dir, sync_mode_server):
        tuning = Picamera2.load_tuning_file(tuning_file)
        self._cam = Picamera2(ordinal, tuning=tuning)

        # from pprint import pprint
        # pprint(self._cam.sensor_modes)
        # pprint(self._cam.camera_controls)

        self._ordinal = ordinal
        self._file_type = file_type
        self._output_dir = output_dir
        self._sync_mode = controls.rpi.SyncModeEnum.Server if sync_mode_server else controls.rpi.SyncModeEnum.Client

        sensor_size = (4056, 3040)

        # main_config = {
        #     "format": "RGB888",
        #     "size": (507, 380),
        # }

        self._cam.configure(self._cam.create_still_configuration(
            # main=main_config,
            raw={
                "format": "SRGGB16",
                "size": sensor_size,
            },
            sensor={
                "output_size": sensor_size,
                "bit_depth": 12,
            },
            transform=Transform(hflip=True, vflip=True),
            buffer_count=5,
            # controls=ctrls,
        ))

        self._cam.set_controls({
            # Auto-exposure
            "AeEnable": False,
            "AeFlickerMode": controls.AeFlickerModeEnum.Off,

            # Auto-focus
            # "AfMetering": controls.AfMeteringEnum.Auto, # TODO: Explore the "Windows" mode.

            #"AfWindows": ...
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

            "SyncMode": self._sync_mode,
        })

    def run(self):
        from concurrent.futures import ThreadPoolExecutor
        executor = ThreadPoolExecutor(max_workers=10)

        self._cam.start()

        executor.submit(self._camera_thread)

        try:
            executor.shutdown()
        except KeyboardInterrupt:
            print("run: stop requested, exiting...")
        except:
            print(repr(sys.exception()))

        self._cam.stop()

    def await_request(self):
        return self._cam.capture_request()

    def resolve(self, request):
        metadata = request.get_metadata()
        frame_wallclock = metadata["FrameWallClock"] / 1e6

        # if "FocusFoM" in metadata:
        #     print(f"c{self._ordinal} FoM: {metadata['FocusFoM']}")

        # from pprint import pprint
        # pprint(metadata)

        if "SyncReady" in metadata and metadata["SyncReady"] == True:
            file_path = os.path.join(self._output_dir, f"{frame_wallclock:.3f}_c{self._ordinal}")

            request.make_buffer("raw").tofile(file_path + ".srggb16")

            if self._sync_mode == controls.rpi.SyncModeEnum.Server:
                ready_line.set_value(1)  # Active, or 0 V

            with open(file_path + ".json", "w") as f:
                f.write(json.dumps(metadata))

        dropped_frame_str = " "
        if self._last_frame_wallclock is not None:
            frame_delta = frame_wallclock - self._last_frame_wallclock
            frame_round = round(frame_delta * 10) / 10
            if frame_delta != 1.0:
                dropped_frame_str = "*"
        print(f"{frame_wallclock:.3f} {dropped_frame_str}")

        request.release()

    def _camera_thread(self):
        self._last_frame_wallclock = None

        try:
            while True:
                request = self.await_request()
                self.resolve(request)
        except:
            print(repr(sys.exception()))

# old_handler = None

# def handler_sigterm(signum, frame):
#     ready_line.set_value(0)
#     signal.signal(signal.SIGTERM, old_handler)

# old_handler = signal.signal(signal.SIGTERM, handler_sigterm)

parser = argparse.ArgumentParser(
    prog="c",
    description="Geocene Drone Camera Capture",
)
parser.add_argument("-o", "--ordinal", type=int, help="camera number")
parser.add_argument("-s", "--server", action='store_true', default=False, help="act as synchronization server")
args = parser.parse_args()

if args.server:
    import gpiod
    chip = gpiod.Chip('gpiochip4')
    ready_line = chip.get_line(2)
    ready_line.request(consumer="READY#", type=gpiod.LINE_REQ_DIR_OUT, flags=gpiod.LINE_REQ_FLAG_ACTIVE_LOW | gpiod.LINE_REQ_FLAG_OPEN_DRAIN | gpiod.LINE_REQ_FLAG_BIAS_DISABLE)
    ready_line.set_value(0)  # Off, or 3.3 V

tuning_file = "/usr/share/libcamera/ipa/rpi/pisp/imx477_scientific.json"

try:
    if not args.server:
        # Wait a bit, since starting both camera 0 and 1 at the same time can cause conflicts in the camera stack...?
        time.sleep(5.0)

    camera = CameraStill(args.ordinal, 1.0, tuning_file, "srggb16", "/home/drone/out", args.server)
    camera.run()
except:
    print(repr(sys.exception()))
finally:
    if ready_line:
        ready_line.set_value(0)
