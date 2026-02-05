# PowerShell script to run all Rust examples

$ErrorActionPreference = "Stop"

# Redirect output to a log file
$LOG_FILE = "example_runner.log"
"Starting example run" | Out-File -FilePath $LOG_FILE -Encoding utf8
"Run Time: $(Get-Date)" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append

# Initialize counters
$success_count = 0
$failure_count = 0
$total_count = 0

# Display start information
"=== RemDB Example Runner ===" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
"Run Time: $(Get-Date)" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
"==========================" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
"" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
Write-Host "=== RemDB Example Runner ==="
Write-Host "Run Time: $(Get-Date)"
Write-Host "=========================="
Write-Host ""

# Get all .rs files in examples directory
$example_files = Get-ChildItem -Path "examples" -Filter "*.rs"

# Iterate through all example files
foreach ($file in $example_files) {
    # Extract example name (without extension)
    $example = $file.BaseName
    $total_count++
    
    "=== Running Example: $example ===" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
    "Start Time: $(Get-Date)" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
    "" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
    
    Write-Host "=== Running Example: $example ==="
    Write-Host "Start Time: $(Get-Date)"
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
    if ($example -eq "test_remdb_server" -or $example -eq "ha_example") {
        $features += "pubsub", "ha"
    }
    
    # Build the cargo command
    $cargo_args = @("run", "--example", $example)
    if ($features.Count -gt 0) {
        $cargo_args += "--features", ($features -join ",")
    }
    
    # Run the example with appropriate features
    $cargo_cmd = "cargo.exe"
    
    # Run the cargo command and capture all output
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
    "" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
    
    Write-Host "End Time: $(Get-Date)"
    Write-Host "--------------------------"
    Write-Host ""
}

# Calculate success rate
if ($total_count -gt 0) {
    $success_rate = [math]::Round(($success_count * 100) / $total_count, 2)
} else {
    $success_rate = 0
}

# Display summary
"=== Run Summary ===" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
"Total Examples: $total_count" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
"Successful: $success_count" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
"Failed: $failure_count" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
"Success Rate: $success_rate%" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
"" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
"=== Run Completed ===" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append
"End Time: $(Get-Date)" | Out-File -FilePath $LOG_FILE -Encoding utf8 -Append

Write-Host "=== Run Summary ==="
Write-Host "Total Examples: $total_count"
Write-Host "Successful: $success_count"
Write-Host "Failed: $failure_count"
Write-Host "Success Rate: $success_rate%"
Write-Host ""
Write-Host "=== Run Completed ==="
Write-Host "Log file created: $LOG_FILE"
Write-Host ""
