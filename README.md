# Clima-sensors

![](https://raw.githubusercontent.com/pferreir/clima-sensors/main/assets/image1.jpg)

This is a little something I hacked together from some loose parts I had lying around:
* [WeAct Black Pill](https://stm32-base.org/boards/STM32F401CCU6-WeAct-Black-Pill-V1.2.html) (STM32F401)
* FS1000A 433Mhz RF transmitter
* DHT11 humidity sensor
* MLX90614 temperature sensor
* MHZ19B CO2 sensor
* 128x32 SSD1306 screen

## Why?

Most importantly because it's fun, but the main motivation was that I'm using
[Home Assistant](https://www.home-assistant.io/) to keep track of the temperature in the various rooms in my apartment.
To do that, I am hijacking the sensor info from a
[TFA Klima@Home](https://www.tfa-dostmann.de/en/product/wireless-thermo-hygrometer-with-3-transmitters-klimahome-30-3060/)
I own, using an RTL-SDR dongle and [`rtl_433`](https://github.com/merbanan/rtl_433).

I was missing a sensor in the living room, though, since the Klima console doesn't emit an RF signal. I then realized I
could build one using some parts I had already got.

The values are measured and relayed through RF on 433MHz, using
[Radiohead ASK](https://www.airspayce.com/mikem/arduino/RadioHead/) encoding, as different sensors:

 * `ID = 0xed`: Temperature (signed 2-byte word, little-endian)
 * `ID = 0xee`: Humidity (unsigned 2-byte word, little-endian)
 * `ID = 0xef`: CO2 (unsigned 2-byte word, little-endian)

There is a script in the `contrib` folder which can be used together with
[`rtl_433`](https://github.com/merbanan/rtl_433) to update a MQTT queue. e.g.

```bash
$ rtl_433 -s 2.5e6 -R 67 -f 433e6 -F json | python3 contrib/mqtt.py <hostname> --username <username> --password <password>
```

## Schematic

![](https://raw.githubusercontent.com/pferreir/clima-sensors/main/assets/schematic.png)

This is what it looks like inside:

![](https://raw.githubusercontent.com/pferreir/clima-sensors/main/assets/image2.jpg)

## Firmware

The firmware was written in Rust. Many thanks to the [Embedded Rust community](https://github.com/rust-embedded/wg) for
their help. They were always very helpful whenever I got stuck along the way.

There isn't anything incredibly new in the firmware, perhaps the Radiohead ASK code will be of help to someone.

If you want to install it onto a new Black Pill, just put it in DFU mode and do:

```bash
$ make dfu-upload
```

## License

This code is made available under the [MIT License](http://github.com/pferreir/clima-sensors/blob/main/LICENSE)
