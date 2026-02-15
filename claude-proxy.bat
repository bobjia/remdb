@echo off
set ANTHROPIC_BASE_URL=http://localhost:4000
set ANTHROPIC_MODEL=nv-glm5
set ANTHROPIC_API_KEY=sk-1234
echo Current Environment:
echo   BASE_URL=%ANTHROPIC_BASE_URL%
echo   MODEL=%ANTHROPIC_MODEL%
echo   API_KEY=%ANTHROPIC_API_KEY%
echo.
echo Press any key to launch Claude Code via LiteLLM proxy...
pause > nul
claude