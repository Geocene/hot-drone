# Camera Computer

## Operation

It is expected that you will log in to the three Raspberry Pi Zero 2 Ws via a local Wi-Fi network, using `ssh`. To do so:

```
ssh drone@drone-1.lan
```

...or `drone-2.lan` or `drone-3.lan`, depending on which Pi you want to connect to. The password, if asked, is "geocene". You may configure authorized SSH keys on each device for convenience.

## Commands

`capture-stop` Stops the capture system, shutting down the camera image capture and saving process, the nexus data logging and storage system (if the KB2040 is attached), and the LED status process.

`capture-clean` Deletes all files in the `out` directory. This is destructive, and should probably ask the user if they want to proceed, but... just be careful, OK? :-)

`capture-start` Restarts the various capture system services.
