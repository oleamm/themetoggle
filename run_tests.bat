@echo off
setlocal
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
set "VCVARS=%ProgramFiles(x86)%\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
if exist "%VCVARS%" call "%VCVARS%" >nul 2>&1
cd /d "%~dp0"
cargo test --all-targets %*
