# Post-Processing Utilities

## Setup

Open a console in this directory. Set up Python prerequisites:

```bash
virtualenv venv
. venv/bin/activate

pip install numpy pidng pymavlink
```

## Usage

After a flight, collect the files from the Raspberry Pi computers:

* Image files in /home/drone/out/*.srggb16
* Image metadata files in /home/drone/out/*.json
* Flight computer synchronization marks in /home/drone/out/*_fc.txt

Connect to the flight computer over USB and pull out the flight log for the flight, e.g. "log_53_2025-8-20-13-24-32.bin".

Before combining the image and image metadata files into a single directory, be aware that files for the first camera on each of the Raspberry Pis may have the same filename. It's best to rename the files on the single-camera Pi from /home/drone/out/*_c0.* to replace "_c0" with "_c2". (TODO: Change systemd services to use "2" as the camera 0 ordinal on the Pi named "drone-2", so this file renaming is no longer necessary.)

Create a directory for this flight inside the "flights" directory.

Structure the files in the flight directory ("<flight_dir>") as follows:

* <flight_dir>/raw/ contains the *.srggb16 image files
* <flight_dir>/meta/ contains the *.json image metadata files
* <flight_dir>/sync/ contains the *_fc.txt flight computer synchronization file(s)
* <flight_dir>/ contains the flight computer log(s)
* <flight_dir>/odm/ is an empty directory where ODM will do its work

Before running the image processing Python program, activate the virtual environment created during the [setup](#setup) instructions, above.

```bash
cd <directory containing this readme>
. venv/bin/activate
```

Then run the image-processing program.

```bash
./log_extract.py flights/<flight_dir> <file_name_of_flight_computer_log> flights/<flight_dir>/odm

```