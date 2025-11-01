# Setting Up From Firmware/Images

## KB2040

Installing firmware is a simple matter of connecting the KB2040 to a host PC over USB. Hold down the "BOOT" button on the KB2040 while connecting to force the KB2040 to enter the UF2 bootloader. This causes the KB2040 to appear as a USB mass storage device on your PC. From there, drag or otherwise copy the .UF2 file to to KB2040 mass storage drive. After a few seconds, the drive will disappear from your PC. Wait a few more seconds just for good measure, then detach the KB2040 and it's ready to be reconnected to the target system (the camera subsystem). One sign that the KB2040 is correctly flashed is that the onboard RGB LED will flash on and off in red. This is an indication the firmware is awaiting a USB host to connect and start streaming drone data from the KB2040.

## Raspberry Pi Zero 2 W

An image file is provided that can be written to a microSD card with the [Raspberry Pi Imager](https://www.raspberrypi.com/software/).

Inside the Raspberry Pi Imager, make the following selections:

* Raspberry Pi Device: Raspberry Pi Zero 2 W
* Operating System: choose "Use custom", and then choose the disk `.img` file provided by this `hot-drone` project.
* Storage: choose a microSD card connected to your host PC.

Choose "Next", and then "Edit Settings". Inside "OS Customization", configure the following:

* General
  * Set hostname: "drone-1" or "drone-2" or "drone-3", to make the name unique for each Pi Zero unit inside the camera assembly.
  * Set username and password:
    * Username: "drone"
    * Password: "geocene"
  * Configure wireless LAN:
    * SSID: use your prefered Wi-Fi network name
    * Password: the password for your Wi-Fi network
    * Hidden SSID: unchecked
    * Wireless LAN country: "US"
  * Set locale settings:
    * Time zone: "America/Los_Angeles"
    * Keyboard layout: "US"
* Service
  * Enable SSH: checked
  * Use password authentication (you can do public-key authentication too, if you're familiar with how to set it up properly)

Choose "Save", then choose "Yes" to continue to writing the microSD card. Once the software says the SD card is ready, remove it and install it in the appropriate Pi Zero unit.

The Pi may require a few minutes to get itself sorted out. Among other things, it will resize the disk image to take up the full amount of the microSD card it was written to. So allow each Pi a few minutes to do its thing before disconnecting power and trying to troubleshoot perceived Wi-Fi problems. (This is the voice of experience!)
