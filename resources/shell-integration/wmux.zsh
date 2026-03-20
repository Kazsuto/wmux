# wmux shell integration for Zsh
# Emits OSC 7 (CWD) and OSC 133 (prompt marks)

[[ -n "$__WMUX_SHELL_INTEGRATION" ]] && return
export __WMUX_SHELL_INTEGRATION=1

__wmux_osc7() {
    local cwd="${PWD}"
    # Percent-encode
    local encoded="${cwd//[^a-zA-Z0-9\/_.~-]/%}"
    printf '\e]7;file://%s%s\a' "${HOST}" "${encoded}"
}

__wmux_precmd() {
    # OSC 133;D — Previous command ended
    printf '\e]133;D\a'
    # OSC 133;A — Prompt start
    printf '\e]133;A\a'
    # OSC 7
    __wmux_osc7
}

__wmux_preexec() {
    # OSC 133;C — Output start (command about to execute)
    printf '\e]133;C\a'
}

# Register hooks
autoload -Uz add-zsh-hook
add-zsh-hook precmd __wmux_precmd
add-zsh-hook preexec __wmux_preexec

# Inject OSC 133;B at end of prompt
setopt PROMPT_SUBST
PS1="%{$(printf '\e]133;B\a')%}${PS1}"
