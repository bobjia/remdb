# PowerShell script to run Rust examples
# Usage: .\run_all_examples.ps1 [directory]
#   directory: Optional. Specify which directory to run examples from (api, sql, misc, or all)
#              If not specified, runs examples from all directories

param(
    [string]$Directory = ""
)

$ErrorActionPreference = "Stop"

# Validate and set directories to run
$SUBDIRS = @()
switch -Regex ($Directory.ToLower()) {
    "" {
        $SUBDIRS = @("api", "sql", "misc")
        Write-Host "No directory specified, running all examples."
    }
    "^all$" {
        $SUBDIRS = @("api", "sql", "misc")
        Write-Host "Running all examples."
    }
    "^api$" {
        $SUBDIRS = @("api")
        Write-Host "Running API examples only."
    }
    "^sql$" {
        $SUBDIRS = @("sql")
        Write-Host "Running SQL examples only."
    }
    "^misc$" {
        $SUBDIRS = @("misc")
        Write-Host "Running misc examples only."
    }
    default {
        Write-Host "Invalid directory: $Directory"
        Write-Host "Usage: .\run_all_examples.ps1 [api|sql|misc|all]"
        Write-Host ""
        Write-Host "Examples:"
        Write-Host "  .\run_all_examples.ps1          - Run all examples"
        Write-Host "  .\run_all_examples.ps1 api      - Run API examples only"
        Write-Host "  .\run_all_examples.ps1 sql      - Run SQL examples only"
        Write-Host "  .\run_all_examples.ps1 misc     - Run misc examples only"
        Write-Host "  .\run_all_examples.ps1 all      - Run all examples"
        exit 1
    }
}

# Create log file
$LOG_FILE = "example_runner.log"
"Starting example run" | Out-File -FilePath $LOG_FILE -Encoding utf8
"Run Time: $(Get-Date)" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
"Target Directories: $($SUBDIRS -join ', ')" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append

# Initialize counters
$success_count = 0
$failure_count = 0
$total_count = 0

# Display start information
"=== RemDB Example Runner ===" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
"Run Time: $(Get-Date)" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
"Target Directories: $($SUBDIRS -join ', ')" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
"==========================" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
"" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append

Write-Host "=== RemDB Example Runner ==="
Write-Host "Run Time: $(Get-Date)"
Write-Host "Target Directories: $($SUBDIRS -join ', ')"
Write-Host "=========================="
Write-Host ""

# Iterate through each subdirectory
foreach ($subdir in $SUBDIRS) {
    "=== Processing Directory: examples/$subdir ===" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
    Write-Host "=== Processing Directory: examples/$subdir ==="
    
    # Check if directory exists
    if (-not (Test-Path -Path "examples\$subdir")) {
        "[WARNING] Directory examples/$subdir does not exist, skipping..." | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
        Write-Host "[WARNING] Directory examples/$subdir does not exist, skipping..."
        continue
    }
    
    # Get all .rs files in the subdirectory
    $example_files = Get-ChildItem -Path "examples\$subdir" -Filter "*.rs" -ErrorAction SilentlyContinue
    
    if (-not $example_files) {
        "[INFO] No .rs files found in examples/$subdir" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
        Write-Host "[INFO] No .rs files found in examples/$subdir"
        continue
    }
    
    $dir_count = $example_files.Count
    "Found $dir_count example(s) in examples/$subdir" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
    Write-Host "Found $dir_count example(s) in examples/$subdir"
    
    # Iterate through all example files
    foreach ($file in $example_files) {
        # Extract example name (without extension)
        $example = $file.BaseName
        $total_count++
        
        "" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
        "=== Running Example: $example ===" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
        "Start Time: $(Get-Date)" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
        "Directory: $subdir" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
        "" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
        
        Write-Host ""
        Write-Host "=== Running Example: $example ==="
        Write-Host "Start Time: $(Get-Date)"
        Write-Host "Directory: $subdir"
        Write-Host ""
        
        # Determine which features to enable based on example name
        $features = @()
        
        # Examples requiring pubsub feature
        if ($example -eq "pubsub_example" -or 
            $example -eq "pubsub_sql_test_server" -or 
            $example -eq "pubsub_test_system_server" -or 
            $example -eq "pubsub_test_system_client" -or 
            $example -eq "pubsub_wildcard") {
            $features += "pubsub"
        }
        
        # Examples requiring ha feature
        if ($example -eq "test_remdb_server" -or 
            $example -eq "ha_example" -or 
            $example -eq "ha_example_master" -or 
            $example -eq "ha_example_slave") {
            $features += "pubsub", "ha"
        }
        
        # Examples requiring log feature
        if ($example -eq "log_example") {
            $features += "log"
        }
        
        # Build the cargo command
        $cargo_args = @("run", "--release", "--example", $example)
        if ($features.Count -gt 0) {
            $cargo_args += "--features", ($features -join ",")
        }
        
        # Log the command
        "Running: cargo $($cargo_args -join ' ')" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
        
        # Run the cargo command and capture all output
        $cargo_cmd = "cargo.exe"
        
        $process = Start-Process -FilePath $cargo_cmd -ArgumentList $cargo_args -NoNewWindow -Wait -PassThru -RedirectStandardOutput "stdout.txt" -RedirectStandardError "stderr.txt"
        
        # Read output files
        $stdout = Get-Content -Path "stdout.txt" -ErrorAction SilentlyContinue
        $stderr = Get-Content -Path "stderr.txt" -ErrorAction SilentlyContinue
        
        # Combine output
        $output = @()
        if ($stdout) { $output += $stdout }
        if ($stderr) { $output += $stderr }
        
        # Write output to log
        $output | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
        
        # Get exit code
        $exit_code = $process.ExitCode
        
        # Clean up temporary files
        Remove-Item -Path "stdout.txt" -ErrorAction SilentlyContinue
        Remove-Item -Path "stderr.txt" -ErrorAction SilentlyContinue
        
        # Check exit code
        if ($exit_code -eq 0) {
            "" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
            "[SUCCESS] Example $example completed successfully" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
            $success_count++
            Write-Host ""
            Write-Host "[SUCCESS] Example $example completed successfully"
        } else {
            "" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
            "[FAILURE] Example $example failed with exit code: $exit_code" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
            $failure_count++
            Write-Host ""
            Write-Host "[FAILURE] Example $example failed with exit code: $exit_code"
        }
        
        "End Time: $(Get-Date)" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
        "--------------------------" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
        
        Write-Host "End Time: $(Get-Date)"
        Write-Host "--------------------------"
    }
}

# Calculate success rate
if ($total_count -gt 0) {
    $success_rate = [math]::Round(($success_count * 100) / $total_count, 2)
} else {
    $success_rate = 0
}

# Display summary
"" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
"=== Run Summary ===" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
"Target Directories: $($SUBDIRS -join ', ')" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
"Total Examples: $total_count" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
"Successful: $success_count" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
"Failed: $failure_count" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
"Success Rate: $success_rate%" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
"" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
"=== Run Completed ===" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
"End Time: $(Get-Date)" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append

Write-Host ""
Write-Host "=== Run Summary ==="
Write-Host "Target Directories: $($SUBDIRS -join ', ')"
Write-Host "Total Examples: $total_count"
Write-Host "Successful: $success_count"
Write-Host "Failed: $failure_count"
Write-Host "Success Rate: $success_rate%"
Write-Host ""
Write-Host "=== Run Completed ==="
Write-Host "Log file: $LOG_FILE"
Write-Host ""
