#!/usr/bin/env python3

import sys
import time
import os
import os.path
import datetime
import json
import signal

import threading
import queue

import platform
pc = 'x86' in platform.platform()
if pc:
    print(f"running on PC")

import serial
tty_path = "/dev/ttyAMA0"
if pc:
    tty_path = "/dev/ttyUSB0"

tty = serial.Serial(tty_path, 115200, timeout=0.5, dsrdtr=False, rtscts=False, xonxoff=False)

if not pc:
    # This is necessary to set the camera sensor data received timeout very high,
    # so the software doesn't get impatient waiting for data to arrive if the external
    # trigger signal isn't yet active and pulsing.
    os.environ["LIBCAMERA_RPI_CONFIG_FILE"] = "/home/drone/rpi_apps.yaml"
    #os.environ["LIBCAMERA_LOG_LEVELS"] = "INFO"

    from picamera2 import Picamera2
    #import logging
    #Picamera2.set_logging(logging.INFO)

    from libcamera import controls, Transform

    # In order to control the Raspberry Pi Zero 2 W's ACT LED, the following entry must
    # be added under the `[all]` block in `/boot/firmware/config.txt`.
    # `dtparam=act_led_trigger=none`

    import RPi.GPIO as GPIO

    LED_ACT = 29

    def led_set_off():
        GPIO.output(LED_ACT, GPIO.HIGH)

    def led_set_on():
        GPIO.output(LED_ACT, GPIO.LOW)

    def led_init():
        GPIO.setmode(GPIO.BCM)
        GPIO.setup(LED_ACT, GPIO.OUT)
        led_set_off()

    led_init()

class CameraStill:
    def __init__(self, camera_ordinal, frame_rate, tuning_file, tty, output_dir):
        tuning = Picamera2.load_tuning_file(tuning_file)
        self._cam = Picamera2(0, tuning=tuning)

        self._running = False
        self._error = False
        self._q = queue.Queue(maxsize=8)
        self._tty = tty
        self._tty_reset_input = True
        self._file_name_prefix = None
        self._output_dir = output_dir
        self._camera_ordinal = camera_ordinal
        self._frame_period = 1.0 / frame_rate
        self._q_in = 0
        self._q_out = 0

        config = self._cam.create_still_configuration(
            main={ "size": (0, 0), },
            raw ={ "size": self._cam.sensor_resolution, },
            buffer_count=4,
        )
        self._cam.configure(config)

        self._cam.set_controls({
            "AeEnable": False,
            "AeFlickerMode": controls.AeFlickerModeEnum.Off,
            "AnalogueGain": 1.0,
            "AwbEnable": False,
            "AwbMode": controls.AwbModeEnum.Daylight,
            "Brightness": 0.0,  # 0.0: Normal
            "Contrast": 1.0,    # 1.0: Normal
            "ExposureTime": 1_000,  # Microseconds
            "ExposureValue": 0, # 0: "normal" exposure
            "HdrMode": controls.HdrModeEnum.Off,
            "NoiseReductionMode": controls.draft.NoiseReductionModeEnum.Off,
            "Saturation": 1.0,  # 1.0: "normal" saturation
            "Sharpness": 0.0,   # 0.0: no additional sharpening performed, 1.0: "normal" level of sharpening
        })

    def read_file_name_prefix(self):
        if self._tty_reset_input:
            self._tty.reset_input_buffer()
            self._tty_reset_input = False
            return None

        # Read the next camera filename from the KB2040 synchronization/data nexus.
        file_name_prefix = None
        while True:
            try:
                line = self._tty.readline()
                line = line.strip()
                if not line:
                    break
                try:
                    file_name_prefix = line.decode()
                except:
                    print(f"{line} failed decode")
                    file_name_prefix = None
            except:
                print(f"resolve readline() loop exception: {sys.exception()}", file=sys.stderr)
                self._running = False

        return file_name_prefix

    def queue_clear(self):
        while True:
            try:
                self._q.get_nowait()
                self._q.task_done()
            except queue.Empty:
                break

        self._q_in = 0
        self._q_out = 0

    def run(self):
        self._running = True

        self._cam.start(show_preview=False)

        file_write_worker = threading.Thread(target=self.file_write_worker, daemon=True).start()
        serial_worker = threading.Thread(target=self.serial_worker, daemon=True).start()

        try:
            while self._running:
                request = self._cam.capture_request()

                if self._tty_reset_input:
                    self._tty.reset_input_buffer()
                    self._tty_reset_input = False
                    self._file_name_prefix = None

                file_name_prefix = self._file_name_prefix
                if file_name_prefix is None:
                    print("no file name prefix available")
                    self._error = True
                    request.release()

                elif self._q.full():
                    print("queue is full, not fetching image data")
                    self.queue_clear()
                    self._error = True
                    request.release()

                else:
                    led_set_on()
                    metadata = request.get_metadata()
                    buffer = request.make_buffer("raw")
                    request.release()
                    led_set_off()

                    item = (buffer, metadata, file_name_prefix)
                    try:
                        self._q.put(item, block=False)
                        self._q_in += 1
                    except queue.Full:
                        print(f"queue is full, dropping image data")
                        self._error = True
                    # Not available until Python 3.13.
                    # except queue.ShutDown:
                    #     print(f"queue indicates shutdown, exiting resolve")

        except KeyboardInterrupt:
            print("run: stop requested, exiting...", file=sys.stderr)
            self._running = False
        except:
            print(f"run: unexpected exception: {sys.exception()}", file=sys.stderr)
            self._running = False

        self._cam.stop()

    def stop(self):
        self._running = False

    def file_write_worker(self):
        last_frame_wallclock = None

        while self._running:
            buffer, metadata, file_name_prefix = self._q.get()
            self._q_out += 1

            q_count = self._q_in - self._q_out

            sensor_timestamp = metadata["SensorTimestamp"] / 1e9    # in nanoseconds
            frame_wallclock = metadata["FrameWallClock"] / 1e9      # in nanoseconds

            dropped_frame_str = " "
            if last_frame_wallclock is not None:
                frame_delta = frame_wallclock - last_frame_wallclock
                frame_jitter = frame_delta - self._frame_period
                if abs(frame_jitter) > 0.010:   # Allow 10 milliseconds jitter.
                    dropped_frame_str = "*"
            last_frame_wallclock = frame_wallclock

            focus_fom = None
            if "FocusFoM" in metadata:
                focus_fom = metadata['FocusFoM']

            if file_name_prefix is not None:
                file_name = f"{file_name_prefix}_c{self._camera_ordinal}"
                file_path = os.path.join(self._output_dir, file_name)

                buffer.tofile(file_path + ".sbggr12")
                with open(file_path + ".json", "w") as f:
                    f.write(json.dumps(metadata))

                print(f"{frame_wallclock:.3f} {dropped_frame_str} {sensor_timestamp:.3f} {file_name} {q_count} {focus_fom}")
            else:
                print(f"{datetime.datetime.now()} no file name prefix received, image discarded")

            self._q.task_done()

    def serial_worker(self):
        while self._running:
            try:
                line = self._tty.readline()
                line = line.strip()
                if line:
                    try:
                        self._file_name_prefix = line.decode()
                    except:
                        print(f"{line} failed decode")
                        self._file_name_prefix = None
            except:
                print(f"resolve readline() loop exception: {sys.exception()}", file=sys.stderr)
                self._running = False

if pc:
    while True:
        line = tty.readline()
        line = line.strip()
        if line:
            print(f"{line}")
else:
    tuning_file = "/usr/share/libcamera/ipa/rpi/vc4/imx477_scientific.json"

    camera_ordinal = None
    with open("/etc/hostname", "r") as f:
        hostname = f.readline().strip()
        camera_ordinal = int(hostname[-1]) - 1
        assert(camera_ordinal >= 0)

    camera = CameraStill(camera_ordinal, 1.0, tuning_file, tty, "/home/drone/out")

    def shutdown(sig, frame):
        print("signal shutdown")
        camera.stop()
        led_set_off()
        sys.exit(0)

    signal.signal(signal.SIGTERM, shutdown)

    camera.run()
