# Required
[rsdsp](https://github.com/astrojhgu/rsdsp)

# udev rule to enable ordinary user to operate airspy
```
SUBSYSTEM=="usb", ATTR{idVendor}=="1d50", ATTR{idProduct}=="*", MODE="0666", OWNER="user", GROUP="users"
```

# Installation and usage
```
git clone https://github.com/astrojhgu/rsdsp
git clone https://github.com/astrojhgu/soapy_spec_acc
cd soapy_spec_acc
cargo run --bin channelize --release -- -f 100e6 --lna 10 --mix 10 --vga 10 -k 0.9999 -a 500 -t 8 -y 64
```
