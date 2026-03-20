# wmux shell integration for Bash
# Emits OSC 7 (CWD) and OSC 133 (prompt marks)

[[ -n "$__WMUX_SHELL_INTEGRATION" ]] && return
export __WMUX_SHELL_INTEGRATION=1

__wmux_osc7() {
    local cwd
    cwd=$(pwd)
    # Percent-encode special characters
    local encoded=""
    local i c
    for (( i=0; i<${#cwd}; i++ )); do
        c="${cwd:$i:1}"
        case "$c" in
            [a-zA-Z0-9/_.-]) encoded+="$c" ;;
            *) printf -v encoded '%s%%%02X' "$encoded" "'$c" ;;
        esac
    done
    printf '\e]7;file://%s%s\a' "$(hostname)" "$encoded"
}

__wmux_prompt_command() {
    # OSC 133;D — Previous command ended
    printf '\e]133;D\a'
    # OSC 133;A — Prompt start
    printf '\e]133;A\a'
    # OSC 7 — CWD
    __wmux_osc7
}

# Append to PROMPT_COMMAND (don't overwrite)
if [[ -z "$PROMPT_COMMAND" ]]; then
    PROMPT_COMMAND="__wmux_prompt_command"
else
    PROMPT_COMMAND="__wmux_prompt_command;$PROMPT_COMMAND"
fi

# Inject OSC 133;B after PS1 renders
# We wrap PS1 to prepend the mark
__wmux_original_ps1="$PS1"
PS1='\[\e]133;B\a\]'"$PS1"
