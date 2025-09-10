# Camera Computers

The X500 V2 drone platform used two Raspberry Pi 5 computers to provide three MIPI-CSI camera ports for three cameras.

## Hardware

__TODO__: Instructions for soldering battery buck regulator outputs directly to the Pi, or orient the Pis so that their USB-C power connectors are facing outward. My drone is built with the Pis oriented the same way, so that one Pi blocks the USB-C connector of the other, which necessitated soldering the regulators directly to the Pi circuit boards.

Attach the [active heat sink](https://www.raspberrypi.com/products/active-cooler/) to the Raspberry Pi and connect the fan/tachometer cable to the connector on the Raspberry Pi labeled "FAN" and "J17", at the edge of the board, behind the USB type-A connectors.

Attach the [RTC battery](https://www.raspberrypi.com/products/rtc-battery/) to the Raspberry Pi. I chose to peel the adhesive on the battery and stick it to the top of the Ethernet jack. Attach the battery cable to the connector labeled "BAT" and "J5". Route the cable through the fins of the heat sink to keep the cable from getting hooked by debris and whatnot.

Install the two Raspberry Pis on the camera mount.

One of the Pis (we'll call it "drone-1") has two cameras attached:

* MIPI-CSI port 0, labeled "CAM/DISP0" and "J3": nadir camera.
* MIPI-CSI port 1, labeled "CAM/DISP1" and "J4": forward camera.

The other Pi (call it "drone-2") has one camera attached.

* MIPI-CSI port 0, labeled "CAM/DISP0" and "J3": side camera.

Each ArduCam camera should come with two cables. One cable has 1.0 mm pitch, 15-pin connections on both ends. The other cable has a 0.5 mm pitch, 22-pin connection on one end, and a 1.0 mm pitch, 15-pin connection on the other. Older Raspberry Pi products use the 1.0 mm pitch, 15-pin connectors. The Raspberry Pi 5 uses 0.5 mm pitch, 22-pin connectors.

For each camera, use the 22-to-15 cable between the Raspberry Pi MIPI-CSI connector to a [MIPI-CSI extender](https://www.adafruit.com/product/3671). From the other side of the MIPI-CSI extender to the ArduCam IMX477 camera, use the 15-to-15 cable. Once you have the Raspberry Pis and cameras attached to the camera mount, find comfortable places to attach the MIPI-CSI extenders, using double-sided tape, so that the cables are out of the way and not pulling or scraping on anything as the camera mount and vehicle vibrate. I attached them to reasonable spots on the lower spider.

__TODO__: Find reasonable-length 22-to-22 cables, and ditch the double-cable + MIPI-CSI extender jank.

__TODO__: Describe camera spider assembly here? It looks like no instructions with the 3D prints README.

## Software

I started with the [raspios_lite_arm64-2024-11-19](https://downloads.raspberrypi.com/raspios_lite_arm64/images/raspios_lite_arm64-2024-11-19/) operating system disk image. I used the Raspberry Pi Imager to write it to a [64GB microSD card](https://www.raspberrypi.com/products/sd-cards/). I customized the operating system image:

* General -> Set hostname: `drone-1`
* General -> Set username and password: `drone`, `geocene`
* General -> Set locale settings: `America/Los_Angeles`, `us`
* Services -> Enable SSH
* Services -> Use password authentication
* Options -> Eject media when finished

## Camera Focusing

__TODO:__ Process to focus cameras, modification of camera program to mirror the figure of merit (FoM) to the console, and/or stream a tight crop of the camera over the network to a laptop.

## Image Processing Pipeline

We are flying three Sony IMX477 cameras connected to two Raspberry Pi 5s via their MIPI-CSI interfaces.

Minimal image processing is done onboard the Raspberry Pi. In part it is to provide us the opportunity to specify and tune the processing iteratively, after the flight. And in part, there is a limited amount of computing power available on the vehicle.

To the extent we have visibility on and control of the image-processing pipeline, these are the steps the image data goes through before leaving the Raspberr Pi computers:

### Sensor Exposure

The IMX477 is a rolling shutter device. It reads out 3040 rows at approximately the total link rate to the host computer. The MIPI-CSI cameras that came with the Arducam B0262 cameras only support two lanes, and the Raspberry Pi 5 link rate is claimed to be 900 Mbps according to `dmesg`. One image is 197,672,960 bits, one line is 65,024 bits. Assuming 25% overhead, we have about 1.44 Gbps data rate. So readout of one sensor line should take about 4.5 µs. The whole image should take 137 milliseconds.

### libcamera / picamera2

The sensor data is acquired by [`c.py`](home/c.py), using `libcamera` and `picamera2`.

The camera configuration requested is:

 * 4056 x 3040 Bayer elements, 12 bit sensor depth
 * Transform image by flipping horizontally and vertically, effecting a 180° rotation.
 * Raw format, SRGGB16
 * Auto-exposure off
 * Auto-exposure flicker detection disabled
 * Analog gain fixed at 1.0
 * Automatic white balance disabled
 * White balance set to "daylight"
 * Brightness "0.0" (normal)
 * Contrast "1.0" (normal)
 * Exposure time: 1.0 milliseconds
 * Frame rate: 1 Hertz
 * HDR mode disabled
 * Noise reduction off
 * Saturation "1.0" (normal)
 * Sharpness "0.0 (no additional sharpening performed)

All documentation indicates that the Bayer pattern is the same whether the readout order is normal or flipped. Experience indicates this is true. However readout order will reverse the order of the rolling shutter effect.

### Filesystem

The frame of raw samples is written to an SD card as "*.srggb16" files.

There is a narrow strip of black pixels along the right side that need to be removed during processing.

Write performance is enhanced by deleting all prior images, and then filling the filesystem with a file or files that are then deleted. This seems to clean up or defragment the filesystem and allow for the SD cards to keep up with the 24 Mbytes/camera/second data rate.
