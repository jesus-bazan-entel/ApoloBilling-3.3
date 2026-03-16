# 🚀 Apolo Billing - Windows PowerShell Launcher
# Run this script from Windows PowerShell to start the system in WSL

Write-Host "╔══════════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║      🚀 Apolo Billing - Windows Launcher                    ║" -ForegroundColor Cyan
Write-Host "║          Starting system in WSL Debian                       ║" -ForegroundColor Cyan
Write-Host "╚══════════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""

# Check if WSL is installed
try {
    $wslCheck = wsl --list --quiet
    if ($LASTEXITCODE -ne 0) {
        Write-Host "❌ WSL is not installed or not configured properly" -ForegroundColor Red
        Write-Host "   Please install WSL2 and Debian first" -ForegroundColor Yellow
        exit 1
    }
    Write-Host "✅ WSL is installed" -ForegroundColor Green
} catch {
    Write-Host "❌ Error checking WSL: $_" -ForegroundColor Red
    exit 1
}

# Check if project directory exists in WSL
Write-Host "📂 Checking project directory..." -ForegroundColor Yellow

$projectExists = wsl test -d ~/ApoloBilling
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ Project directory not found in WSL" -ForegroundColor Red
    Write-Host ""
    Write-Host "Please clone the repository first:" -ForegroundColor Yellow
    Write-Host "  wsl" -ForegroundColor White
    Write-Host "  cd ~" -ForegroundColor White
    Write-Host "  git clone https://github.com/jesus-bazan-entel/ApoloBilling.git" -ForegroundColor White
    Write-Host "  cd ApoloBilling" -ForegroundColor White
    Write-Host "  git checkout genspark_ai_developer" -ForegroundColor White
    exit 1
}

Write-Host "✅ Project directory found" -ForegroundColor Green
Write-Host ""

# Ask user for action
Write-Host "Select an option:" -ForegroundColor Cyan
Write-Host "  1. Quick Start (automatic setup + run)" -ForegroundColor White
Write-Host "  2. Start Backend Only (if already configured)" -ForegroundColor White
Write-Host "  3. Start Rust Billing Engine" -ForegroundColor White
Write-Host "  4. Stop All Services" -ForegroundColor White
Write-Host "  5. Check Services Status" -ForegroundColor White
Write-Host "  6. Open Dashboard in Browser" -ForegroundColor White
Write-Host "  7. Exit" -ForegroundColor White
Write-Host ""

$choice = Read-Host "Enter your choice (1-7)"

switch ($choice) {
    "1" {
        Write-Host ""
        Write-Host "🚀 Running quick start script..." -ForegroundColor Green
        Write-Host ""
        wsl bash -c "cd ~/ApoloBilling && ./quick_start_wsl.sh"
    }
    "2" {
        Write-Host ""
        Write-Host "🐍 Starting Python backend..." -ForegroundColor Green
        Write-Host ""
        wsl bash -c "cd ~/ApoloBilling/backend && source venv/bin/activate && uvicorn main:app --host 0.0.0.0 --port 8000 --reload"
    }
    "3" {
        Write-Host ""
        Write-Host "🦀 Starting Rust Billing Engine..." -ForegroundColor Green
        Write-Host ""
        wsl bash -c "cd ~/ApoloBilling/rust-billing-engine && cargo run --release"
    }
    "4" {
        Write-Host ""
        Write-Host "🛑 Stopping all services..." -ForegroundColor Yellow
        wsl bash -c "pkill -f 'uvicorn main:app'"
        wsl bash -c "pkill -f 'apolo-billing-engine'"
        Write-Host "✅ Services stopped" -ForegroundColor Green
    }
    "5" {
        Write-Host ""
        Write-Host "📊 Checking services status..." -ForegroundColor Cyan
        Write-Host ""
        
        Write-Host "PostgreSQL:" -ForegroundColor Yellow
        wsl bash -c "sudo service postgresql status | head -3"
        
        Write-Host ""
        Write-Host "Redis:" -ForegroundColor Yellow
        wsl bash -c "sudo service redis-server status | head -3"
        
        Write-Host ""
        Write-Host "Backend (Python):" -ForegroundColor Yellow
        $backendProcess = wsl bash -c "pgrep -f 'uvicorn main:app'"
        if ($backendProcess) {
            Write-Host "✅ Running (PID: $backendProcess)" -ForegroundColor Green
        } else {
            Write-Host "❌ Not running" -ForegroundColor Red
        }
        
        Write-Host ""
        Write-Host "Rust Billing Engine:" -ForegroundColor Yellow
        $rustProcess = wsl bash -c "pgrep -f 'apolo-billing-engine'"
        if ($rustProcess) {
            Write-Host "✅ Running (PID: $rustProcess)" -ForegroundColor Green
        } else {
            Write-Host "❌ Not running" -ForegroundColor Red
        }
    }
    "6" {
        Write-Host ""
        Write-Host "🌐 Opening dashboard in browser..." -ForegroundColor Green
        Start-Process "http://localhost:8000"
        Write-Host "✅ Dashboard opened" -ForegroundColor Green
    }
    "7" {
        Write-Host ""
        Write-Host "👋 Goodbye!" -ForegroundColor Cyan
        exit 0
    }
    default {
        Write-Host ""
        Write-Host "❌ Invalid choice" -ForegroundColor Red
        exit 1
    }
}

Write-Host ""
Write-Host "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" -ForegroundColor Cyan
Write-Host ""
Write-Host "📊 Access Points:" -ForegroundColor Cyan
Write-Host "   Dashboard:       http://localhost:8000" -ForegroundColor White
Write-Host "   Rate Cards UI:   http://localhost:8000/dashboard/rate-cards" -ForegroundColor White
Write-Host "   API Docs:        http://localhost:8000/docs" -ForegroundColor White
Write-Host ""
Write-Host "🔐 Default Credentials:" -ForegroundColor Cyan
Write-Host "   Username: admin" -ForegroundColor White
Write-Host "   Password: admin123" -ForegroundColor White
Write-Host ""
Write-Host "🛑 To stop: Press Ctrl+C in the WSL terminal" -ForegroundColor Yellow
Write-Host ""
Write-Host "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" -ForegroundColor Cyan
Write-Host ""
