OPENOCD_RELEASE_FILE=openocd.release
PROJ_NAME=clima-sensors
TARGET=thumbv7em-none-eabihf
BUILD_DIR=target/${TARGET}/release
ELF_NAME=${BUILD_DIR}/${PROJ_NAME}
BIN_NAME=${ELF_NAME}.bin

all: build

build:
	cargo build --release

bin: build
	arm-none-eabi-objcopy -O binary ${ELF_NAME} ${BIN_NAME}

upload: bin
	openocd -f openocd.cfg -f openocd.release \
		-c "flash write_image erase unlock ${BIN_NAME} 0x08000000; reset run; shutdown"

dfu-upload: bin
	dfu-util -a0 -s 0x08000000 -D ${BIN_NAME}

debug:
	cargo run
