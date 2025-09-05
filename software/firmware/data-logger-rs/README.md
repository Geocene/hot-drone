## Setup

From https://github.com/rp-rs/rp-hal:

```

# cargo install elf2uf2-rs --locked

# To install the `ts` utility:
sudo pacman -S moreutils

# After building debug, monitor defmt debug over ttyUSB0 at 921600 baud.
socat /dev/ttyUSB0,b921600,raw,echo=0 STDOUT | defmt-print -e target/thumbv6m-none-eabi/debug/data-logger-rs --log-format "{t} {L} {s}" -w --verbose | ts "%m%d%H%M%S"
```
