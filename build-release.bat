set RUST_TARGET_PATH=%cd%
echo off
:loop
xargo build --release
pause
goto loop
