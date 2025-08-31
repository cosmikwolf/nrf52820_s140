# GDB configuration for nRF52820 debugging

# Connect to probe-rs automatically
target remote 127.0.0.1:1338

# Set up breakpoints for common panic locations
break rust_begin_unwind
break panic_fmt
break __assert_func

# Break on hard fault
break HardFault

# Monitor RTT output
monitor rtt

# Continue execution
continue

# Enable pretty printing
set print pretty on
set print array on
set print array-indexes on

# Show function names in backtrace
set print frame-arguments all

echo \n=== GDB Ready - Firmware should be running ===\n
echo Use 'bt' for backtrace, 'info registers' for CPU state\n
echo Use 'monitor reset' to reset the target\n
