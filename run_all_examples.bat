@echo off
setlocal enabledelayedexpansion
REM Batch script to run Rust examples
REM Usage: run_all_examples.bat [directory]
REM   directory: Optional. Specify which directory to run examples from (api, sql, misc, or all)
REM              If not specified, runs examples from all directories

REM Parse command line argument
set "TARGET_DIR=%~1"

REM Validate and set directories to run
if "%TARGET_DIR%"=="" (
    set "SUBDIRS=api sql misc"
    echo No directory specified, running all examples.
) else if /i "%TARGET_DIR%"=="all" (
    set "SUBDIRS=api sql misc"
    echo Running all examples.
) else if /i "%TARGET_DIR%"=="api" (
    set "SUBDIRS=api"
    echo Running API examples only.
) else if /i "%TARGET_DIR%"=="sql" (
    set "SUBDIRS=sql"
    echo Running SQL examples only.
) else if /i "%TARGET_DIR%"=="misc" (
    set "SUBDIRS=misc"
    echo Running misc examples only.
) else (
    echo Invalid directory: %TARGET_DIR%
    echo Usage: run_all_examples.bat [api^|sql^|misc^|all]
    echo.
    echo Examples:
    echo   run_all_examples.bat          - Run all examples
    echo   run_all_examples.bat api      - Run API examples only
    echo   run_all_examples.bat sql      - Run SQL examples only
    echo   run_all_examples.bat misc     - Run misc examples only
    echo   run_all_examples.bat all      - Run all examples
    exit /b 1
)

REM Create a unique log file name
set "YYYY=%date:~0,4%"
set "MM=%date:~5,2%"
set "DD=%date:~8,2%"
set "HH=%time:~0,2%"
set "MI=%time:~3,2%"
set "SS=%time:~6,2%"

REM Ensure leading zero for hours
if "%HH:~0,1%"==" " set "HH=0%HH:~1,1%"

REM Remove slashes from date
set "MM=%MM:/=%%"
set "DD=%DD:/=%%"

set "LOG_FILE=example_runner_%YYYY%%MM%%DD%_%HH%%MI%%SS%.log"

REM Initialize counters
set success_count=0
set failure_count=0
set total_count=0

REM Display start information
echo Starting example run > %LOG_FILE%
echo Run Time: %date% %time% >> %LOG_FILE%
echo Target Directories: %SUBDIRS% >> %LOG_FILE%
echo === RemDB Example Runner === >> %LOG_FILE%
echo Run Time: %date% %time% >> %LOG_FILE%
echo Target Directories: %SUBDIRS% >> %LOG_FILE%
echo ========================== >> %LOG_FILE%
echo. >> %LOG_FILE%
echo === RemDB Example Runner ===
echo Run Time: %date% %time%
echo Target Directories: %SUBDIRS%
echo ==========================
echo.

REM Iterate through each subdirectory
for %%d in (%SUBDIRS%) do (
    echo === Processing Directory: examples/%%d === >> %LOG_FILE%
    echo === Processing Directory: examples/%%d ===
    
    REM Check if directory exists
    if exist "examples\%%d" (
        REM Iterate through all .rs files in the subdirectory
        for %%f in (examples\%%d\*.rs) do (
            set "example=%%~nf"
            set /a total_count+=1
            
            echo. >> %LOG_FILE%
            echo === Running Example: %%~nf === >> %LOG_FILE%
            echo Start Time: !time! >> %LOG_FILE%
            echo Directory: %%d >> %LOG_FILE%
            echo. >> %LOG_FILE%
            
            echo.
            echo === Running Example: %%~nf ===
            echo Start Time: !time!
            echo Directory: %%d
            echo.
            
            REM Determine which features to enable based on example name
            set "features="
            
            REM Examples requiring pubsub feature
            if "%%~nf"=="pubsub_example" set "features=--features pubsub"
            if "%%~nf"=="pubsub_sql_test_server" set "features=--features pubsub"
            if "%%~nf"=="pubsub_test_system_server" set "features=--features pubsub"
            if "%%~nf"=="pubsub_test_system_client" set "features=--features pubsub"
            if "%%~nf"=="pubsub_wildcard" set "features=--features pubsub"
            if "%%~nf"=="test_remdb_server" set "features=--features pubsub --features ha"
            
            REM Examples requiring ha feature
            if "%%~nf"=="ha_example" set "features=--features pubsub --features ha"
            if "%%~nf"=="ha_example_master" set "features=--features pubsub --features ha"
            if "%%~nf"=="ha_example_slave" set "features=--features pubsub --features ha"
            
            REM Examples requiring log feature
            if "%%~nf"=="log_example" set "features=--features log"
            
            REM Run the example
            echo Running: cargo run --release --example "%%~nf" !features! >> %LOG_FILE%
            cargo run --release --example "%%~nf" !features! > temp_output.txt 2>&1
            
            REM Append output to log
            if exist temp_output.txt (
                type temp_output.txt >> %LOG_FILE%
                del temp_output.txt > nul 2>&1
            ) else (
                echo [NOTE] No output generated >> %LOG_FILE%
            )
            
            REM Check exit code
            if !errorlevel! equ 0 (
                echo. >> %LOG_FILE%
                echo [SUCCESS] Example %%~nf completed successfully >> %LOG_FILE%
                set /a success_count+=1
                echo.
                echo [SUCCESS] Example %%~nf completed successfully
            ) else (
                echo. >> %LOG_FILE%
                echo [FAILURE] Example %%~nf failed with exit code: !errorlevel! >> %LOG_FILE%
                set /a failure_count+=1
                echo.
                echo [FAILURE] Example %%~nf failed with exit code: !errorlevel!
            )
            
            echo End Time: !time! >> %LOG_FILE%
            echo -------------------------- >> %LOG_FILE%
            echo End Time: !time!
            echo --------------------------
        )
    ) else (
        echo [WARNING] Directory examples/%%d does not exist, skipping... >> %LOG_FILE%
        echo [WARNING] Directory examples/%%d does not exist, skipping...
    )
)

REM Calculate success rate
set /a success_rate=0
if %total_count% gtr 0 (
    set /a success_rate=%success_count% * 100 / %total_count%
)

REM Display summary
echo. >> %LOG_FILE%
echo === Run Summary === >> %LOG_FILE%
echo Target Directories: %SUBDIRS% >> %LOG_FILE%
echo Total Examples: %total_count% >> %LOG_FILE%
echo Successful: %success_count% >> %LOG_FILE%
echo Failed: %failure_count% >> %LOG_FILE%
echo Success Rate: %success_rate%%% >> %LOG_FILE%
echo. >> %LOG_FILE%
echo === Run Completed === >> %LOG_FILE%
echo End Time: %time% >> %LOG_FILE%

echo.
echo === Run Summary ===
echo Target Directories: %SUBDIRS%
echo Total Examples: %total_count%
echo Successful: %success_count%
echo Failed: %failure_count%
echo Success Rate: %success_rate%%%
echo.
echo === Run Completed ===
echo Log file: %LOG_FILE%
echo.

endlocal
