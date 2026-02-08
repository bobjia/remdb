@echo off
REM Batch script to run all Rust examples

REM Create a unique log file name to avoid locking issues
REM Fix date and time format to avoid issues with regional settings
set "YYYY=%date:~0,4%"
set "MM=%date:~5,2%"
set "DD=%date:~8,2%"
set "HH=%time:~0,2%"
set "MI=%time:~3,2%"
set "SS=%time:~6,2%"

REM Ensure leading zero for hours (fixes issues with single-digit hours)
if "%HH:~0,1%"==" " set "HH=0%HH:~1,1%"

REM Remove slashes from date to make valid filename
set "MM=%MM:/=%%"
set "DD=%DD:/=%%"

set "LOG_FILE=example_runner_%YYYY%%MM%%DD%_%HH%%MI%%SS%.log"

REM Initialize counters outside of setlocal to preserve values
set success_count=0
set failure_count=0
set total_count=0
set success_rate=0

REM Display start information
echo Starting example run > %LOG_FILE%
echo Run Time: %date% %time% >> %LOG_FILE%
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
    
    echo === Running Example: %%~nf === >> %LOG_FILE%
echo Start Time: %time% >> %LOG_FILE%
echo. >> %LOG_FILE%
    
    echo === Running Example: %%~nf ===
echo Start Time: %time%
echo.
    
    REM Determine which features to enable based on example name
    set "features="
    
    REM Examples requiring pubsub feature
    if "%%~nf" == "pubsub_example" set features=--features "pubsub"
    if "%%~nf" == "pubsub_sql_test_server" set features=--features "pubsub"
    if "%%~nf" == "pubsub_test_system_server" set features=--features "pubsub"
    if "%%~nf" == "pubsub_test_system_client" set features=--features "pubsub"
    if "%%~nf" == "pubsub_wildcard" set features=--features "pubsub"
    if "%%~nf" == "test_remdb_server" set features=--features "pubsub ha"
    
    REM Examples requiring ha feature (which includes pubsub)
    if "%%~nf" == "ha_example" set features=--features "pubsub ha"
    
    REM Run the example with appropriate features
    REM Use separate output redirection to avoid file locking
    cargo run --example "%%~nf" %features% > temp_output.txt 2>&1
    
    REM Append temp output to log file (if it exists)
    if exist temp_output.txt (
        type temp_output.txt >> %LOG_FILE%
        del temp_output.txt > nul 2>&1
    ) else (
        echo [NOTE] No output file generated for this example >> %LOG_FILE%
    )
    
    REM Check exit code
    if %errorlevel% equ 0 (
        echo. >> %LOG_FILE%
        echo [SUCCESS] Example %%~nf completed successfully >> %LOG_FILE%
        set /a success_count+=1
        echo.
        echo [SUCCESS] Example %%~nf completed successfully
    ) else (
        echo. >> %LOG_FILE%
        echo [FAILURE] Example %%~nf failed with exit code: %errorlevel% >> %LOG_FILE%
        set /a failure_count+=1
        echo.
        echo [FAILURE] Example %%~nf failed with exit code: %errorlevel%
    )
    
    echo End Time: %time% >> %LOG_FILE%
echo -------------------------- >> %LOG_FILE%
echo. >> %LOG_FILE%
    
    echo End Time: %time%
echo --------------------------
echo.
    
    REM Add a delay to prevent file locking issues
    timeout /t 2 /nobreak > nul
)

REM Calculate success rate
if %total_count% gtr 0 (
    set /a success_rate=%success_count% * 100 / %total_count%
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

REM Display summary
echo === Run Summary ===
echo Total Examples: %total_count%
echo Successful: %success_count%
echo Failed: %failure_count%
echo Success Rate: %success_rate%%%
echo.
echo === Run Completed ===
echo Log file created: %LOG_FILE%
echo.

