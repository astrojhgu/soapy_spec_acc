# udev rule to enable ordinary user to operate airspy
Add following line to `/dev/udev/rules.d/99-airspy.rules`
```
SUBSYSTEM=="usb", ATTR{idVendor}=="1d50", ATTR{idProduct}=="*", MODE="0666", OWNER="user", GROUP="users"
```

Then run command
```bash
sudo udevadm control --reload-rules && sudo udevadm trigger
```
to update the `udev` rules

# Installation and usage
## Install `rust` environment

### For Linux
Run command
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup default stable
```

### For Windows
TBD


### Get code and compile
```
git clone https://github.com/astrojhgu/rsdsp
git clone https://github.com/astrojhgu/soapy_spec_acc
cd soapy_spec_acc
cargo run --bin channelize --release -- -f 100e6 --lna 10 --mix 10 --vga 10 -k 0.9999 -a 500 -t 8 -y 64
```
