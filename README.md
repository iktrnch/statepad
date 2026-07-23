## Disclaimer

> [!CAUTION]
> Use at your own risk

This project is a general-purpose programmable USB HID controller intended
for learning embedded Rust and building custom keyboard input devices.

The default configuration is designed around one physical input producing one
keyboard or mouse input. Features such as multi-key bindings, timed sequences,
repeated actions, or automated state transitions may violate the rules of
games, servers, competitions, or other services.

Do not use this project to automate gameplay or gain an unfair advantage.
Users are responsible for ensuring that their configuration complies with the
rules of any software or service in which it is used.

## Usage
Before compiling the project run the following in your terminal:
```bash
rustup target add thumbv6m-none-eabi
cargo install elf2uf2-rs
```
Afterwards, define the desired profiles in `src/profile.rs`, specifying the keyboard and mouse output for each state and any timed transitions between them.
It is recommended to flash the board before installing it in the enclosure, as the onboard **BOOT** and **RESET** buttons are difficult to access afterwards.
For the first flash, connect the board by USB, hold **BOOT**, briefly press **RESET**, then release **BOOT** and run:
```bash
cargo run --release
```
The firmware will be compiled, copied to the RP2040 bootloader drive, and started automatically. For subsequent flashes, leave the board connected and hold the **top preset button** for five seconds. The firmware will release all active HID inputs and restart the board in USB bootloader mode. Once the `RPI-RP2` device appears, run `cargo run --release` again.

## Parts used in the project
- [TENSTAR RP2040 Pro Micro Development Board 16MB](https://www.aliexpress.com/item/1005009890367599.html)
- [2pcs PBT Keycaps](https://www.aliexpress.com/item/1005009364428646.html)
	- **OR** any cherry compatible keycap works
- [2pc Mechanical Keyboard Switches](https://www.aliexpress.com/item/1005012157731141.html)
	- **OR** any cherry compatible switches
- [5 pairs JST 1.25mm](https://www.aliexpress.com/item/1005007617385129.html)
	- 1.25mm are possible but may cause clearance issues
	- Not mandatory but makes life easier
- [1 pcs 6x6x4.3MM 4PIN Push Button](https://www.aliexpress.com/item/32874867657.html)
- [SSD1306 128X32 0.91 inch OLED display](https://www.aliexpress.com/item/1005004622593515.html)
	- Has to be SSD1306
	- Has to support i2c

## Enclosure 3d models
You can find the ready for printing in the releases
