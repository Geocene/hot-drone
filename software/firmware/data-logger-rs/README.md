# data-logger-rs

This project started as a way to collect data from a single ICM-20948 and send it to a Raspberry Pi 5 for storage during flight.

It's since evolved into a central timebase, ingesting MAVlink data from the flight computer and two ICM-20948 IMUs, while also triggering the cameras. Everything is timestamped and sent to a single Raspberry Pi Zero 2 W among the collection of three on the vehicle.

The firmware targets the [KB2040](https://www.adafruit.com/product/5302) from Adafruit, which is built around a Raspberry Pi RP2040 microcontroller.

## Setup

From https://github.com/rp-rs/rp-hal:

```

# cargo install elf2flash --locked

# To install the `ts` utility:
sudo pacman -S moreutils

# After building debug, monitor defmt debug over ttyUSB0 at 921600 baud.
socat /dev/ttyUSB0,b921600,raw,echo=0 STDOUT | defmt-print -w -e target/thumbv6m-none-eabi/debug/data-logger-rs
```

## KB2040 Pin Map

The [KB2040 pinout](https://learn.adafruit.com/adafruit-kb2040/pinouts) is mapped and connected to these off-board functions.


|    | D+    | USB.D+ | USB to one Zero                   |
|    | D-    | USB.D- | USB to one Zero                   |
|  0 | D0    | TX0    | UART transmit to all Zeros        |
|  2 | D2    | GPIO2  | Trigger to all Zeros              |
|  3 | D3    | GPIO3  | Enable camera trigger 1V8 supply  |
|  4 | D4    | GPIO4  | Camera trigger output             |
|  6 | D6    | SDA1   | SDA for I2C1                      |
|  7 | D7    | SCL1   | SCL for I2C1                      |
|  9 | D8    | TX1    | UART transmit to flight computer  |
|  9 | D9    | RX1    | UART receive from flight computer |
| 17 | LED   | PWM0B  | LED control (WS2812B)             |
| 28 | A2    | SDA0   | SDA for I2C0                      |
| 29 | A3    | SCL0   | SCL for I2C0                      |
