.PHONY: run dev build clean check kill logs sync-win win-build win-run

WIN_PROJECT = $$(powershell.exe -Command '[Environment]::GetFolderPath("UserProfile")' 2>/dev/null | tr -d '\r')\\echo-mate

# Kill any running EchoMate instances (Linux)
kill:
	-pkill -f "target/debug/echo-mate" 2>/dev/null
	@sleep 1

# Start EchoMate on Linux (dev, no hotkey on WSL2)
run: kill build
	./src-tauri/target/debug/echo-mate &
	@sleep 2
	@echo "EchoMate started (Linux). Use 'make win-run' for Windows native."

# Follow the log file
logs:
	tail -f ~/.echomate/logs/echomate.log.$$(date +%Y-%m-%d)

# Build debug binary (Linux)
build:
	cargo build --manifest-path src-tauri/Cargo.toml

# Sync source to Windows project copy
sync-win:
	@echo "Syncing sources to Windows..."
	@WIN_PROJECT_WSL="/mnt/c/Users/$$(powershell.exe -NoProfile -Command '$$env:USERNAME' 2>/dev/null | tr -d '\r')/echo-mate"; \
	mkdir -p "$$WIN_PROJECT_WSL/src-tauri/src" "$$WIN_PROJECT_WSL/frontend"; \
	cp -a src-tauri/src/. "$$WIN_PROJECT_WSL/src-tauri/src/"; \
	cp -a src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json "$$WIN_PROJECT_WSL/src-tauri/"; \
	cp -a frontend/. "$$WIN_PROJECT_WSL/frontend/"

# Build Windows binary (after sync-win)
win-build:
	powershell.exe -Command '$$env:PATH="$$env:USERPROFILE\.cargo\bin;C:\Strawberry\perl\bin;$$env:PATH"; Get-Process -Name "echo-mate" -ErrorAction SilentlyContinue | Stop-Process -Force; cd "$$env:USERPROFILE\echo-mate\src-tauri"; cargo build --release'

# Sync + build + run on Windows
win-run: sync-win win-build
	powershell.exe -Command 'Get-Process -Name "echo-mate" -ErrorAction SilentlyContinue | Stop-Process -Force; Start-Sleep 1; Start-Process "$$env:USERPROFILE\echo-mate\src-tauri\target\release\echo-mate.exe"; Write-Host "EchoMate Windows launched. Global hotkey should work now."'

# Tauri dev mode (hot-reload for UI work)
dev:
	cargo tauri dev

# Release build (Linux)
release:
	cargo tauri build

# Clean build artifacts
clean:
	cargo clean --manifest-path src-tauri/Cargo.toml

# Quick syntax / type check without full build
check:
	cargo check --manifest-path src-tauri/Cargo.toml
