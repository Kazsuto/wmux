# wmux shell integration for PowerShell
# Emits OSC 7 (CWD) and OSC 133 (prompt marks)

if ($env:__WMUX_SHELL_INTEGRATION) { return }
$env:__WMUX_SHELL_INTEGRATION = "1"

# Save original prompt if exists
$__wmux_original_prompt = if (Test-Path Function:\prompt) {
    Get-Content Function:\prompt
} else { $null }

function prompt {
    # OSC 133;D — Previous command ended
    $escape = [char]0x1b
    Write-Host -NoNewline "${escape}]133;D`a"

    # OSC 133;A — Prompt start
    Write-Host -NoNewline "${escape}]133;A`a"

    # OSC 7 — Current working directory
    $cwd = (Get-Location).Path
    $uri = "file://$([System.Net.Dns]::GetHostName())/$($cwd -replace '\\','/' -replace ' ','%20')"
    Write-Host -NoNewline "${escape}]7;${uri}`a"

    # Original prompt or default
    if ($__wmux_original_prompt) {
        $result = & $__wmux_original_prompt
    } else {
        $result = "PS $($executionContext.SessionState.Path.CurrentLocation)$('>' * ($nestedPromptLevel + 1)) "
    }

    # OSC 133;B — Prompt end / Command start
    Write-Host -NoNewline "${escape}]133;B`a"

    return $result
}
