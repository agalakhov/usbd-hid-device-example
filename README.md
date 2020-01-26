# Example mouse for usbd-hid-device

This example turns a [STM32F3DICOVERY](https://www.st.com/en/evaluation-tools/stm32f3discovery.html) board
into a mouse. The mouse cursor is controlled by the accelerometer, and the only and one
user button acts like left mouse button. LEDs are used to indicate the current direction.

Note that we do not do neither digital filtering nor combining data from accelerometer
and gyroscope for sake of simplicity. This results in too sensitive and "choppy" mouse
movements. If you want a *usable* accelerometer-based mouse, please add proper Kalman
or Madgwick filter.
