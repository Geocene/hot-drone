#!/usr/bin/env python3

import time
import usb1
import asyncio
import struct
import os.path

VENDOR_ID = 0xc0de
PRODUCT_ID = 0xcafe
INTERFACE = 0
ENDPOINT = 1
TRANSFER_COUNT = 32
BUFFER_SIZE = 64

import platform
pc = 'x86' in platform.platform()
print(f"running on PC: {pc}")

chip = None
sync_line = None

if not pc:
    import gpiod
    chip = gpiod.Chip('gpiochip4')
    sync_line = chip.get_line(26)
    sync_line.request(consumer="SYNC", type=gpiod.LINE_REQ_DIR_OUT, flags=gpiod.LINE_REQ_FLAG_ACTIVE_LOW | gpiod.LINE_REQ_FLAG_OPEN_DRAIN | gpiod.LINE_REQ_FLAG_BIAS_DISABLE)
    sync_line.set_value(0)  # Inactive, or 3.3 V

f = None

def received_data_callback(transfer):
    if transfer.getStatus() != usb1.TRANSFER_COMPLETED:
        return

    data = transfer.getBuffer()[:transfer.getActualLength()]
    transfer.submit()

    if f is not None:
        f.write(data)

with usb1.USBContext() as context:
    handle = context.openByVendorIDAndProductID(
        VENDOR_ID,
        PRODUCT_ID,
        skip_on_error=True,
    )

    transfer_list = []

    if handle is None:
        raise RuntimeError("device not present")

    with handle.claimInterface(INTERFACE):
        if sync_line is not None:
            sync_line.set_value(1)  # Active, or 0 V
        start_time = time.time()

        path_out = "/tmp" if pc else "/home/drone/out"
        filename_out = os.path.join(path_out, f"{start_time:.3f}_sensors.dat")
        f = open(filename_out, 'wb')

        for i in range(TRANSFER_COUNT):
            transfer = handle.getTransfer()
            transfer.setBulk(
                usb1.ENDPOINT_IN | ENDPOINT,
                BUFFER_SIZE,
                callback=received_data_callback,
            )
            transfer.submit()
            transfer_list.append(transfer)

        # Start streaming.
        handle.controlWrite(usb1.REQUEST_TYPE_VENDOR | usb1.RECIPIENT_INTERFACE, 0, 1, 0, [])

        try:
            while any(x.isSubmitted() for x in transfer_list):
                try:
                    context.handleEvents()
                except KeyboardInterrupt:
                    break
                except:
                    print(repr(sys.exception()))

        except:
            print(repr(sys.exception()))

        finally:
            handle.controlWrite(usb1.REQUEST_TYPE_VENDOR | usb1.RECIPIENT_INTERFACE, 0, 0, 0, [])

    handle.close()

    if sync_line is not None:
        sync_line.set_value(0)  # Inctive, or 3.3 V
