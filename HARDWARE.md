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
- **GPIO 10:** LED PIN 1
- **GPIO 3:** LED PIN 2

NOTE! There is no Host USB on this board.
NOTE2! The UI buttons use pull-down to ground, and is NOT a matrix.