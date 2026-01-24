@echo off
REM Batch script to run all Rust examples

setlocal enabledelayedexpansion

REM Redirect output to a log file
set LOG_FILE=example_runner.log
echo Starting example run > %LOG_FILE%
echo Run Time: %date% %time% >> %LOG_FILE%

REM Initialize counters
set success_count=0
set failure_count=0
set total_count=0

REM Display start information
echo === RemDB Example Runner === >> %LOG_FILE%
echo Run Time: %date% %time% >> %LOG_FILE%
echo ========================== >> %LOG_FILE%
echo. >> %LOG_FILE%
echo === RemDB Example Runner ===
echo Run Time: %date% %time%
echo ==========================
echo.

REM Iterate through all .rs files in examples directory
for %%f in (examples\*.rs) do (
    REM Extract example name (without extension)
    set "example=%%~nf"
    set /a total_count+=1
    
    echo === Running Example: !example! === >> %LOG_FILE%
echo Start Time: %time% >> %LOG_FILE%
echo. >> %LOG_FILE%
    
    echo === Running Example: !example! ===
echo Start Time: %time%
echo.
    
    REM Determine which features to enable based on example name
    set "features="
    
    REM Examples requiring pubsub feature
    if "!example!" == "pubsub_example" set features=--features "pubsub"
    if "!example!" == "pubsub_sql_test_server" set features=--features "pubsub"
    if "!example!" == "pubsub_test_system_server" set features=--features "pubsub"
    if "!example!" == "pubsub_test_system_client" set features=--features "pubsub"
    if "!example!" == "pubsub_wildcard" set features=--features "pubsub"
    if "!example!" == "test_remdb_server" set features=--features "pubsub ha"
    
    REM Examples requiring ha feature (which includes pubsub)
    if "!example!" == "ha_example" set features=--features "pubsub ha"
    
    REM Run the example with appropriate features
    cargo run --example !example! !features! >> %LOG_FILE% 2>&1
    
    REM Check exit code
    if !errorlevel! equ 0 (
        echo. >> %LOG_FILE%
        echo [SUCCESS] Example !example! completed successfully >> %LOG_FILE%
        set /a success_count+=1
        echo.
        echo [SUCCESS] Example !example! completed successfully
    ) else (
        echo. >> %LOG_FILE%
        echo [FAILURE] Example !example! failed with exit code: !errorlevel! >> %LOG_FILE%
        set /a failure_count+=1
        echo.
        echo [FAILURE] Example !example! failed with exit code: !errorlevel!
    )
    
    echo End Time: %time% >> %LOG_FILE%
echo -------------------------- >> %LOG_FILE%
echo. >> %LOG_FILE%
    
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
echo === Run Summary === >> %LOG_FILE%
echo Total Examples: %total_count% >> %LOG_FILE%
echo Successful: %success_count% >> %LOG_FILE%
echo Failed: %failure_count% >> %LOG_FILE%
echo Success Rate: %success_rate%%% >> %LOG_FILE%
echo. >> %LOG_FILE%
echo === Run Completed === >> %LOG_FILE%
echo End Time: %time% >> %LOG_FILE%

endlocal

echo === Run Summary ===
echo Total Examples: %total_count%
echo Successful: %success_count%
echo Failed: %failure_count%
echo Success Rate: %success_rate%%%
echo.
echo === Run Completed ===
echo Log file created: %LOG_FILE%
echo.

