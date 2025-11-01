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

is_nexus = False

import platform
pc = 'x86' in platform.platform()
if pc:
    print(f"running on PC")
else:
    with usb1.USBContext() as context:
        devices = context.getDeviceList()
        for device in devices:
            if device.getVendorID() == VENDOR_ID and device.getProductID() == PRODUCT_ID:
                is_nexus = True
        print(f"is_nexus: {is_nexus}")

    if not is_nexus:
        print(f"no nexus hardware (no KB2040 attached), so shutting down")
        sys.exit(0)

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

pump = True

def received_data_callback(transfer):
    global pump

    if not pump:
        return

    if transfer.getStatus() != usb1.TRANSFER_COMPLETED:
        print("transfer.getStatus did not return usb1.TRANSFER_COMPLETED", file=sys.stderr)
        pump = False
        return

    data = transfer.getBuffer()[:transfer.getActualLength()]
    transfer.submit()

    packet_id = data[0]
    payload_length = data[1]
    timestamp = data[2:4]
    payload = data[4:]
    if len(payload) != payload_length:
        print(f"len(payload) {len(payload)} != payload_length {payload_length}, packet_id = {packet_id}", file=sys.stderr)
        pump = False
        return

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
    filename_prefix = f"{int(start_time)}"

    def streaming_start():
        handle.controlWrite(usb1.REQUEST_TYPE_VENDOR | usb1.RECIPIENT_INTERFACE, 0, 1, 0, filename_prefix.encode())

    def streaming_stop():
        print("stopping streaming", file=sys.stderr)
        handle.controlWrite(usb1.REQUEST_TYPE_VENDOR | usb1.RECIPIENT_INTERFACE, 0, 0, 0, [])

    def shutdown(sig, frame):
        print("shutdown", file=sys.stderr)
        pump = False

        try:
            streaming_stop()
        except:
            pass

        # try:
        #     handle.close()
        # except:
        #     pass

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
                    print("keyboard interrupt", file=sys.stderr)
                    pump = False
                    break
                except:
                    print(f"inner exception: {repr(sys.exception())}", file=sys.stderr)
                    pump = False
                    break

        except:
            print(f"outer exception: {repr(sys.exception())}", file=sys.stderr)

        finally:
            print("outer finally", file=sys.stderr)
            pump = False

        streaming_stop()

    print("exiting", file=sys.stderr)
    # handle.close()
