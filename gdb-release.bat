arm-none-eabi-gdb -iex "add-auto-load-safe-path ." -ex "tar ext :4242" -ex "load-reset" target/stm32f7/release/stack
