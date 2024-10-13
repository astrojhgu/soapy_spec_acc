# udev rule to enable ordinary user to operate airspy
```
SUBSYSTEM=="usb", ATTR{idVendor}=="1d50", ATTR{idProduct}=="*", MODE="0666", OWNER="user", GROUP="users"
```

# Usage: 
cargo run --bin channelize --release -- -f 100e6 --lna 10 --mix 10 --vga 10 -k 0.9999 -a 500 -t 8 -y 64

