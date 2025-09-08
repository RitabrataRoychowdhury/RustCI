@echo off
REM Windows Deployment Server Script
REM This script acts as a deployment server for receiving and running artifacts from Mac DinD runner

setlocal enabledelayedexpansion

REM Configuration
set DEPLOY_DIR=C:\temp\rustci-deployments
set SERVER_PORT=3000
set LOG_FILE=%DEPLOY_DIR%\deploy-server.log
set PID_FILE=%DEPLOY_DIR%\server.pid
set ARTIFACT_DIR=%DEPLOY_DIR%\artifacts
set CURRENT_DIR=%~dp0

REM Colors (Windows 10+ with ANSI support)
set "RED=[31m"
set "GREEN=[32m"
set "YELLOW=[33m"
set "BLUE=[34m"
set "NC=[0m"

REM Create directories
if not exist "%DEPLOY_DIR%" mkdir "%DEPLOY_DIR%"
if not exist "%ARTIFACT_DIR%" mkdir "%ARTIFACT_DIR%"

REM Logging function
:log_info
echo %BLUE%[INFO]%NC% %~1
echo [%date% %time%] [INFO] %~1 >> "%LOG_FILE%"
goto :eof

:log_success
echo %GREEN%[SUCCESS]%NC% %~1
echo [%date% %time%] [SUCCESS] %~1 >> "%LOG_FILE%"
goto :eof

:log_warning
echo %YELLOW%[WARNING]%NC% %~1
echo [%date% %time%] [WARNING] %~1 >> "%LOG_FILE%"
goto :eof

:log_error
echo %RED%[ERROR]%NC% %~1
echo [%date% %time%] [ERROR] %~1 >> "%LOG_FILE%"
goto :eof

REM Check prerequisites
:check_prerequisites
call :log_info "Checking prerequisites..."

REM Check if running as administrator (optional but recommended)
net session >nul 2>&1
if %errorLevel% == 0 (
    call :log_info "Running with administrator privileges"
) else (
    call :log_warning "Not running as administrator - some features may be limited"
)

REM Check if PowerShell is available
powershell -Command "Get-Host" >nul 2>&1
if %errorLevel% neq 0 (
    call :log_error "PowerShell is not available"
    exit /b 1
)

REM Check if curl is available (Windows 10 1803+)
curl --version >nul 2>&1
if %errorLevel% neq 0 (
    call :log_warning "curl is not available - some features may not work"
)

REM Check if tar is available (Windows 10 1803+)
tar --version >nul 2>&1
if %errorLevel% neq 0 (
    call :log_warning "tar is not available - will use PowerShell for extraction"
)

call :log_success "Prerequisites check completed"
goto :eof

REM Test network connectivity
:test_connectivity
call :log_info "Testing network connectivity..."

REM Test if port is available
netstat -an | findstr ":%SERVER_PORT%" >nul
if %errorLevel% == 0 (
    call :log_warning "Port %SERVER_PORT% is already in use"
    call :log_info "Attempting to stop existing server..."
    call :stop_server
)

REM Test internet connectivity
ping -n 1 8.8.8.8 >nul 2>&1
if %errorLevel% == 0 (
    call :log_success "Internet connectivity test passed"
) else (
    call :log_warning "Internet connectivity test failed"
)

goto :eof

REM Start the deployment server
:start_server
call :log_info "Starting Windows deployment server..."

REM Create a simple HTTP server using PowerShell
set "SERVER_SCRIPT=%DEPLOY_DIR%\server.ps1"

REM Create PowerShell HTTP server script
(
echo $port = %SERVER_PORT%
echo $deployDir = "%DEPLOY_DIR%"
echo $artifactDir = "%ARTIFACT_DIR%"
echo $logFile = "%LOG_FILE%"
echo.
echo function Write-Log {
echo     param^([string]$message^)
echo     $timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
echo     "$timestamp [SERVER] $message" ^| Out-File -FilePath $logFile -Append
echo     Write-Host "[SERVER] $message"
echo }
echo.
echo function Handle-Request {
echo     param^($context^)
echo     
echo     $request = $context.Request
echo     $response = $context.Response
echo     $url = $request.Url.LocalPath
echo     
echo     Write-Log "Request: $($request.HttpMethod^) $url"
echo     
echo     switch ^($url^) {
echo         "/health" {
echo             $responseString = '{"status":"healthy","server":"windows-deploy-server","timestamp":"' + ^(Get-Date -Format "o"^) + '"}'
echo             $buffer = [System.Text.Encoding]::UTF8.GetBytes^($responseString^)
echo             $response.ContentLength64 = $buffer.Length
echo             $response.ContentType = "application/json"
echo             $response.OutputStream.Write^($buffer, 0, $buffer.Length^)
echo         }
echo         "/deploy" {
echo             if ^($request.HttpMethod -eq "POST"^) {
echo                 $responseString = '{"status":"success","message":"Deployment endpoint ready"}'
echo             } else {
echo                 $responseString = '{"status":"error","message":"POST method required"}'
echo                 $response.StatusCode = 405
echo             }
echo             $buffer = [System.Text.Encoding]::UTF8.GetBytes^($responseString^)
echo             $response.ContentLength64 = $buffer.Length
echo             $response.ContentType = "application/json"
echo             $response.OutputStream.Write^($buffer, 0, $buffer.Length^)
echo         }
echo         "/status" {
echo             $artifacts = Get-ChildItem -Path $artifactDir -File ^| Select-Object Name, Length, LastWriteTime
echo             $status = @{
echo                 server = "windows-deploy-server"
echo                 port = $port
echo                 deployDir = $deployDir
echo                 artifactCount = $artifacts.Count
echo                 artifacts = $artifacts
echo                 timestamp = Get-Date -Format "o"
echo             }
echo             $responseString = $status ^| ConvertTo-Json -Depth 3
echo             $buffer = [System.Text.Encoding]::UTF8.GetBytes^($responseString^)
echo             $response.ContentLength64 = $buffer.Length
echo             $response.ContentType = "application/json"
echo             $response.OutputStream.Write^($buffer, 0, $buffer.Length^)
echo         }
echo         default {
echo             $responseString = '{"status":"error","message":"Endpoint not found"}'
echo             $response.StatusCode = 404
echo             $buffer = [System.Text.Encoding]::UTF8.GetBytes^($responseString^)
echo             $response.ContentLength64 = $buffer.Length
echo             $response.ContentType = "application/json"
echo             $response.OutputStream.Write^($buffer, 0, $buffer.Length^)
echo         }
echo     }
echo     
echo     $response.Close^(^)
echo }
echo.
echo try {
echo     $listener = New-Object System.Net.HttpListener
echo     $listener.Prefixes.Add^("http://+:$port/"^)
echo     $listener.Start^(^)
echo     
echo     Write-Log "Server started on port $port"
echo     Write-Log "Deploy directory: $deployDir"
echo     Write-Log "Artifact directory: $artifactDir"
echo     
echo     while ^($listener.IsListening^) {
echo         $context = $listener.GetContext^(^)
echo         Handle-Request $context
echo     }
echo } catch {
echo     Write-Log "Server error: $($_.Exception.Message^)"
echo } finally {
echo     if ^($listener^) {
echo         $listener.Stop^(^)
echo         Write-Log "Server stopped"
echo     }
echo }
) > "%SERVER_SCRIPT%"

REM Start the server in background
start /b powershell -ExecutionPolicy Bypass -File "%SERVER_SCRIPT%"

REM Wait a moment for server to start
timeout /t 3 /nobreak >nul

REM Test if server is running
curl -s "http://localhost:%SERVER_PORT%/health" >nul 2>&1
if %errorLevel% == 0 (
    call :log_success "Deployment server started successfully on port %SERVER_PORT%"
) else (
    call :log_error "Failed to start deployment server"
    exit /b 1
)

REM Save server info
echo %date% %time% > "%PID_FILE%"

goto :eof

REM Stop the deployment server
:stop_server
call :log_info "Stopping deployment server..."

REM Kill PowerShell processes running our server script
for /f "tokens=2" %%i in ('tasklist /fi "imagename eq powershell.exe" /fo csv ^| findstr "server.ps1"') do (
    taskkill /pid %%i /f >nul 2>&1
)

REM Alternative method - kill by port
for /f "tokens=5" %%i in ('netstat -ano ^| findstr ":%SERVER_PORT%"') do (
    taskkill /pid %%i /f >nul 2>&1
)

if exist "%PID_FILE%" del "%PID_FILE%"

call :log_success "Deployment server stopped"
goto :eof

REM Deploy artifact function
:deploy_artifact
set "ARTIFACT_FILE=%~1"

if "%ARTIFACT_FILE%"=="" (
    call :log_error "No artifact file specified"
    exit /b 1
)

if not exist "C:\temp\%ARTIFACT_FILE%" (
    call :log_error "Artifact file not found: C:\temp\%ARTIFACT_FILE%"
    exit /b 1
)

call :log_info "Deploying artifact: %ARTIFACT_FILE%"

REM Copy artifact to deployment directory
copy "C:\temp\%ARTIFACT_FILE%" "%ARTIFACT_DIR%\" >nul
if %errorLevel% neq 0 (
    call :log_error "Failed to copy artifact"
    exit /b 1
)

REM Extract artifact if it's a tar.gz file
echo %ARTIFACT_FILE% | findstr "\.tar\.gz$" >nul
if %errorLevel% == 0 (
    call :log_info "Extracting tar.gz artifact..."
    
    REM Try using tar command first (Windows 10 1803+)
    tar --version >nul 2>&1
    if %errorLevel% == 0 (
        cd /d "%ARTIFACT_DIR%"
        tar -xzf "%ARTIFACT_FILE%"
        if %errorLevel% == 0 (
            call :log_success "Artifact extracted successfully using tar"
        ) else (
            call :log_warning "tar extraction failed, trying PowerShell method"
            call :extract_with_powershell "%ARTIFACT_DIR%\%ARTIFACT_FILE%"
        )
    ) else (
        call :extract_with_powershell "%ARTIFACT_DIR%\%ARTIFACT_FILE%"
    )
)

REM Run any deployment scripts found in the artifact
if exist "%ARTIFACT_DIR%\deploy.bat" (
    call :log_info "Running deployment script..."
    cd /d "%ARTIFACT_DIR%"
    call deploy.bat
    if %errorLevel% == 0 (
        call :log_success "Deployment script completed successfully"
    ) else (
        call :log_error "Deployment script failed"
        exit /b 1
    )
)

REM Show deployed files
call :log_info "Deployed files:"
dir "%ARTIFACT_DIR%" /b

call :log_success "Artifact deployment completed: %ARTIFACT_FILE%"
goto :eof

REM Extract using PowerShell (fallback method)
:extract_with_powershell
set "ARCHIVE_PATH=%~1"
call :log_info "Extracting using PowerShell..."

powershell -Command "& { Add-Type -AssemblyName System.IO.Compression.FileSystem; [System.IO.Compression.ZipFile]::ExtractToDirectory('%ARCHIVE_PATH%', '%ARTIFACT_DIR%'); }"
if %errorLevel% == 0 (
    call :log_success "Artifact extracted successfully using PowerShell"
) else (
    call :log_error "PowerShell extraction failed"
)
goto :eof

REM Show server status
:show_status
call :log_info "Windows Deployment Server Status"
echo ==========================================
echo Server Port: %SERVER_PORT%
echo Deploy Directory: %DEPLOY_DIR%
echo Artifact Directory: %ARTIFACT_DIR%
echo Log File: %LOG_FILE%
echo.

REM Check if server is running
curl -s "http://localhost:%SERVER_PORT%/health" >nul 2>&1
if %errorLevel% == 0 (
    echo Status: ✅ Running
    echo Health Check: http://localhost:%SERVER_PORT%/health
    echo Status Endpoint: http://localhost:%SERVER_PORT%/status
) else (
    echo Status: ❌ Not Running
)

echo.
call :log_info "Recent deployments:"
if exist "%ARTIFACT_DIR%" (
    dir "%ARTIFACT_DIR%" /b /o-d 2>nul | head -5
) else (
    echo No deployments found
)

echo.
call :log_info "Recent log entries:"
if exist "%LOG_FILE%" (
    powershell -Command "Get-Content '%LOG_FILE%' | Select-Object -Last 10"
) else (
    echo No log file found
)

goto :eof

REM Show help
:show_help
echo Windows Deployment Server Script
echo.
echo Usage: %~nx0 [COMMAND] [ARTIFACT_FILE]
echo.
echo COMMANDS:
echo   start         - Start the deployment server (default)
echo   stop          - Stop the deployment server
echo   status        - Show server status
echo   deploy FILE   - Deploy a specific artifact file
echo   test          - Test server connectivity
echo   help          - Show this help
echo.
echo EXAMPLES:
echo   %~nx0                              # Start server
echo   %~nx0 deploy app-20241209.tar.gz   # Deploy specific artifact
echo   %~nx0 status                       # Show server status
echo   %~nx0 stop                         # Stop server
echo.
echo ENDPOINTS:
echo   http://localhost:%SERVER_PORT%/health  - Health check
echo   http://localhost:%SERVER_PORT%/status  - Server status
echo   http://localhost:%SERVER_PORT%/deploy  - Deployment endpoint
echo.
goto :eof

REM Test server connectivity
:test_server
call :log_info "Testing server connectivity..."

curl -s "http://localhost:%SERVER_PORT%/health" >nul 2>&1
if %errorLevel% == 0 (
    call :log_success "Server is responding"
    curl -s "http://localhost:%SERVER_PORT%/health"
    echo.
) else (
    call :log_error "Server is not responding"
    exit /b 1
)

goto :eof

REM Main execution
:main
call :log_info "Windows Deployment Server"
echo ================================

call :check_prerequisites
call :test_connectivity
call :start_server

echo.
call :log_success "Setup completed successfully!"
echo.
call :show_status

echo.
call :log_info "Server is ready to receive deployments from Mac DinD runner"
call :log_info "Use 'curl http://localhost:%SERVER_PORT%/health' to test connectivity"

goto :eof

REM Handle command line arguments
if "%~1"=="" goto main
if "%~1"=="start" goto main
if "%~1"=="stop" (
    call :stop_server
    goto :eof
)
if "%~1"=="status" (
    call :show_status
    goto :eof
)
if "%~1"=="deploy" (
    call :deploy_artifact "%~2"
    goto :eof
)
if "%~1"=="test" (
    call :test_server
    goto :eof
)
if "%~1"=="help" (
    call :show_help
    goto :eof
)
if "%~1"=="-h" (
    call :show_help
    goto :eof
)
if "%~1"=="--help" (
    call :show_help
    goto :eof
)

call :log_error "Unknown command: %~1"
echo Use '%~nx0 help' for usage information
exit /b 1