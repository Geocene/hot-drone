#!/usr/bin/env python3

import sys
import time
import usb1
import os.path
import signal

VENDOR_ID = 0xc0de
PRODUCT_ID = 0xcafe
INTERFACE = 0
ENDPOINT = 1
TRANSFER_COUNT = 32
BUFFER_SIZE = 64

import platform
pc = 'x86' in platform.platform()
if pc:
    print(f"running on PC")

path_out = "/tmp" if pc else "/home/drone/out"

separate_output_files = False

# output_map is a set of USB record IDs associated with a particular output filename.
if separate_output_files:
    output_map = {
        'imu0':    (0xc0,),
        'imu1':    (0xc1,),
        'camera':  (0xce,),
        'mavlink': (0xcf,),
    }
else:
    output_map = {
        'nexus': (0xc0, 0xc1, 0xce, 0xcf,),
    }

output_file_map = dict([(id, None) for id in output_map.keys()])

def received_data_callback(transfer):
    if transfer.getStatus() != usb1.TRANSFER_COMPLETED:
        return

    data = transfer.getBuffer()[:transfer.getActualLength()]
    transfer.submit()

    packet_id = data[0]
    payload_length = data[1]
    timestamp = data[2:4]
    payload = data[4:]
    assert(len(payload) == payload_length)

    output_file = output_file_map.get(packet_id, None)
    if output_file is not None:
        output_file.write(data)

with usb1.USBContext() as context:
    handle = context.openByVendorIDAndProductID(
        VENDOR_ID,
        PRODUCT_ID,
        skip_on_error=True,
    )

    transfer_list = []

    if handle is None:
        raise RuntimeError("device not present")

    start_time = time.time()
    filename_prefix = f"{int(start_time)}".encode()

    def streaming_start():
        handle.controlWrite(usb1.REQUEST_TYPE_VENDOR | usb1.RECIPIENT_INTERFACE, 0, 1, 0, filename_prefix)

    def streaming_stop():
        handle.controlWrite(usb1.REQUEST_TYPE_VENDOR | usb1.RECIPIENT_INTERFACE, 0, 0, 0, [])

    def shutdown(sig, frame):
        print("shutdown")
        streaming_stop()
        handle.close()
        sys.exit(0)

    signal.signal(signal.SIGTERM, shutdown)

    with handle.claimInterface(INTERFACE):
        for name, ids in output_map.items():
            filename_out = os.path.join(path_out, f"{filename_prefix}_{name}.dat")
            f = open(filename_out, 'wb')
            for id in ids:
                output_file_map[id] = f

        for i in range(TRANSFER_COUNT):
            transfer = handle.getTransfer()
            transfer.setBulk(
                usb1.ENDPOINT_IN | ENDPOINT,
                BUFFER_SIZE,
                callback=received_data_callback,
            )
            transfer.submit()
            transfer_list.append(transfer)

        streaming_start()

        try:
            while any(x.isSubmitted() for x in transfer_list):
                try:
                    context.handleEvents()
                except KeyboardInterrupt:
                    break
                except:
                    print(repr(sys.exception()))

        except:
            print("exception", repr(sys.exception()))

        finally:
            print("finally")
            streaming_stop()

    print("exited with")
    handle.close()
