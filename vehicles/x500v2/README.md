# X500 v2 Platform

## 3D Printed Components

Models are in [3d print files](3d print files).

## Flight Computer

The flight computer is a [HolyBro PixHawk 6C](https://docs.holybro.com/autopilot/pixhawk-6c) attached in the center of the top carbon fiber panel of the X500 v2 frame. The flight computer and the attached M10 GPS boom are oriented in the same direction as the vehicle-forward arrow silkscreened on the drone's top panel.

[ArduPilot Copter stable-4.5.7](https://firmware.ardupilot.org/Copter/stable-4.5.7/Pixhawk6C/) was used for the 2025/03/25 and 2025/08/20 farm flights.

The flight computer configuration "params" after the 2025/08/20 flight is in [params.txt](params.txt). It'll be kept current if there are any new flights of this vehicle where parameters are adjusted.

I run [Arch Linux](https://archlinux.org/) on my Intel 12th generation [Framework 13 Laptop](https://frame.work/laptop13). On my laptop, I have been using two different configuration and ground control programs:

* [ArduPilot Mission Planner 1.3.82](https://github.com/ArduPilot/MissionPlanner/releases/tag/MissionPlanner1.3.82) build 1.3.8979.17128
* [QGroundControl v4.4.4](https://github.com/mavlink/qgroundcontrol/releases/tag/v4.4.4)

I have preferred ArduPilot for vehicle setup, as I had some strange compatibility issues between QGroundControl and the ArduPilot firmware. But it could well be that was just operator error. QGroundControl seems nicer to use for mission planning and ground control.

## Electronic Speed Controllers

I installed BLHeli-S firmware on the electronic speed controllers. Unfortunately, I can't find any notes or assets about exactly which version.

I believe I am running the ESC interface to the flight computer in DShot 600 mode.

## Radio Control

We are flying with a RadioMaster TX16S Mark II ELRS controller. We're using [ExpressLRS 3.5.3 firmware](https://github.com/ExpressLRS/ExpressLRS/releases/tag/3.5.3).

