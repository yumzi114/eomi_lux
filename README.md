## eomi lux module 
used ESP32S3 wroom + ssd1306 display + veml7700 sensor

This repository i2c config 600kHz setting You can change stable 100kHz setting

Im working BLE connect for android, [raspberry pi app](https://github.com/yumzi114/lighttester)

BLE 8byte protocol
Decode & Encode function

1byte 
  row or lux mod state 1bit 0 or 1
  DEVICE NUMBER<br/>
white 2byte<br/>
data 4byte<br/>

BLE Send message speed control is ble_thread_function delay ms change
## View
![Image](https://github.com/user-attachments/assets/051fa6b5-f815-4e7f-b151-e4d53986fd26)