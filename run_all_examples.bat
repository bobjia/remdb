@echo off
REM Batch script to run all Rust examples

setlocal enabledelayedexpansion

REM Initialize counters
set success_count=0
set failure_count=0
set total_count=0

REM Display start information
echo === RemDB Example Runner ===
echo Run Time: %date% %time%
echo ==========================
echo.

REM Iterate through all .rs files in examples directory
for %%f in (examples\*.rs) do (
    REM Extract example name (without extension)
    set "example=%%~nf"
    set /a total_count+=1
    
    echo === Running Example: !example! ===
    echo Start Time: %time%
    echo.
    
    REM Run the example
    cargo run --example !example! --release
    
    REM Check exit code
    if %errorlevel% equ 0 (
        echo.
        echo [SUCCESS] Example !example! completed successfully
        set /a success_count+=1
    ) else (
        echo.
        echo [FAILURE] Example !example! failed with exit code: %errorlevel%
        set /a failure_count+=1
    )
    
    echo End Time: %time%
    echo --------------------------
    echo.
)

REM Calculate success rate
if %total_count% gtr 0 (
    set /a success_rate=!success_count! * 100 / !total_count!
) else (
    set success_rate=0
)

REM Display summary
echo === Run Summary ===
echo Total Examples: %total_count%
echo Successful: %success_count%
echo Failed: %failure_count%
echo Success Rate: %success_rate%%%
echo.
echo === Run Completed ===

endlocal
