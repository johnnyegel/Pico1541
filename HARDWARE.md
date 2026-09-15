# Hardware specification for the boards 

## Hardware Specification (RP235x)

### IEC Bus (Use hardware 74LS06 inverter as IEC PHY!)
- **GPIO 19:** DATA_RX (Input, 1K external pull-up)
- **GPIO 20:** DATA_TX (Output, 1K external pull-up)
- **GPIO 21:** CLK_RX (Input, 1K external pull-up)
- **GPIO 22:** CLK_TX (Output)
- **GPIO 26:** ATN_RX (Input, 1K external pull-up)
- **GPIO 27:** RESET_RX (Input, 1K external pull-up)

### UI & Peripherals
- **GPIO 2:** Piezo Speaker
- **GPIO 4:** OLED SDA
- **GPIO 5:** OLED SCL
- **GPIO 6..9:** Keyboard buttons (short to ground)

### SD Card
- **GPIO 11:** SD_DET
- **GPIO 12:** SD_DAT0
- **GPIO 13:** SD_CD
- **GPIO 14:** SD_CLK
- **GPIO 15:** SD_CMD

### Status RGB LED
- **GPIO 16:** RED
- **GPIO 17:** GREEN
- **GPIO 18:** BLUE

### Activity 2 color LED
- **GPIO 10* LED PIN 1
- **GPIO 3:** LED PIN 2

NOTE! There is no Host USB on this board.
NOTE2! The UI buttons use pull-down to ground, and is NOT a matrix.


## Hardware Specification (RP2040) - V1

Prototype which turned out to be too slow to handle the emulation cycle exact.
Has been abandoned for version 2.

### IEC Bus (Inverter Interfaced)
- **GPIO 16:** DATA_RX (Input)
- **GPIO 17:** CLK_RX (Input)
- **GPIO 18:** ATN_RX (Input)
- **GPIO 19:** DATA_TX (Output, Open Collector)
- **GPIO 20:** CLK_TX (Output, Open Collector)
- **GPIO 21:** RESET_RX (Input)

### UI & Peripherals
- **GPIO 2:** Piezo Speaker
- **GPIO 22:** Power LED (+)
- **GPIO 3:** Power LED (-)
- **GPIO 4:** OLED SDA
- **GPIO 5:** OLED SCL
- **GPIO 6, 7:** Keyboard Matrix Rows
- **GPIO 8, 9:** Keyboard Matrix Columns

### SD Card
- **GPIO 11:** SD_DET
- **GPIO 12:** SD_DATA
- **GPIO 13:** SD_CD
- **GPIO 14:** SD_CLK
- **GPIO 15:** SD_CMD

### Status RGB LED
- **GPIO 26:** RED
- **GPIO 27:** GREEN
- **GPIO 28:** BLUE

### Activity 2 color LED
- **GPIO 22:** LED PIN 1
- **GPIO 3:** LED PIN 2

### USB
- **USB_DN / USB_DP:** Connected to Host USB-A port.