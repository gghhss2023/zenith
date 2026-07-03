# Zenith shell integration — emits OSC 133 prompt markers.
# Installed by Zenith to ~/.config/zenith/shell-integration.sh (overwritten on update).
# Enable by adding to your ~/.zshrc or ~/.bashrc:
#   [ -n "$ZENITH_SHELL_INTEGRATION" ] && . ~/.config/zenith/shell-integration.sh
# Put that line after any prompt customization — redefining PS1/PS0 later removes the markers.

if [ -n "$ZENITH_INTEGRATION_LOADED" ]; then
    return 0
fi
ZENITH_INTEGRATION_LOADED=1

if [ -n "$ZSH_VERSION" ]; then
    _zenith_precmd() {
        local ret=$?
        printf '\033]133;D;%s\007' "$ret"
        printf '\033]133;A\007'
    }
    _zenith_preexec() {
        printf '\033]133;C\007'
    }
    typeset -ag precmd_functions preexec_functions
    precmd_functions+=(_zenith_precmd)
    preexec_functions+=(_zenith_preexec)
    PS1="$PS1"$'%{\033]133;B\007%}'
elif [ -n "$BASH_VERSION" ]; then
    _zenith_prompt_command() {
        local ret=$?
        printf '\033]133;D;%s\007' "$ret"
        printf '\033]133;A\007'
    }
    PROMPT_COMMAND="_zenith_prompt_command${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
    PS1="$PS1\[\033]133;B\007\]"
    PS0='\033]133;C\007'"$PS0"
fi
