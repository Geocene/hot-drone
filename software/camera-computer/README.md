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

