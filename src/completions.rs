//! Shell completion scripts, printed by `opi --completions <shell>`.
//!
//! Embedded as plain strings rather than generated: the parser is hand-written
//! and has one shape, so the scripts are short, and a completion framework
//! would be a dependency for forty lines of shell. Flags and workflow names are
//! static; script names are asked of `opi --list-names` each time, because they
//! belong to whichever project the shell is standing in.
//!
//! Only the first word is completed as a script. Everything after it belongs
//! to the script, so offering opi's flags there would be wrong.

const BASH: &str = r#"_opi() {
    local cur prev
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"
    case "$prev" in
        --check|--checks)
            COMPREPLY=($(compgen -W "commit push release deploy" -- "$cur"))
            return ;;
        --completions)
            COMPREPLY=($(compgen -W "bash zsh fish" -- "$cur"))
            return ;;
        --sbom)
            COMPREPLY=($(compgen -d -- "$cur"))
            return ;;
    esac
    [[ $COMP_CWORD -eq 1 || "$prev" == "--yes" || "$prev" == "-y" ]] || return
    if [[ "$cur" == -* ]]; then
        COMPREPLY=($(compgen -W "-y --yes --health --clean --security --updates --check --sbom --hooks --checks --completions -h --help -V --version" -- "$cur"))
    else
        # One name per line, each completed as a single escaped word: names
        # may contain spaces ("cargo doc") and come from files we do not trust.
        local name quoted
        while IFS= read -r name; do
            printf -v quoted '%q' "$name"
            [[ "$quoted" == "$cur"* ]] && COMPREPLY+=("$quoted")
        done < <(opi --list-names 2>/dev/null)
    fi
}
complete -F _opi opi
"#;

const ZSH: &str = r#"#compdef opi

_opi_names() {
    local -a names
    names=(${(f)"$(opi --list-names 2>/dev/null)"})
    compadd -a -- names
}

_opi() {
    case "$words[CURRENT-1]" in
        --check|--checks) compadd commit push release deploy; return ;;
        --completions) compadd bash zsh fish; return ;;
        --sbom) _directories; return ;;
    esac
    if (( CURRENT == 2 )) || [[ "$words[CURRENT-1]" == (--yes|-y) ]]; then
        if [[ "$words[CURRENT]" == -* ]]; then
            compadd -- -y --yes --health --clean --security --updates --check --sbom --hooks --checks --completions -h --help -V --version
        else
            _opi_names
        fi
    fi
}

compdef _opi opi
"#;

const FISH: &str = r#"function __opi_first_word
    set -l words (commandline -opc)
    test (count $words) -eq 1; or test (count $words) -eq 2 -a "$words[2]" = --yes; or test (count $words) -eq 2 -a "$words[2]" = -y
end

function __opi_after
    set -l words (commandline -opc)
    test "$words[-1]" = $argv[1]
end

complete -c opi -f
complete -c opi -n __opi_first_word -a '(opi --list-names 2>/dev/null)'
complete -c opi -n __opi_first_word -s y -l yes -d 'Answer a script\'s confirmation in advance'
complete -c opi -n __opi_first_word -l health -d 'Run the project\'s checks'
complete -c opi -n __opi_first_word -l clean -d 'Show and remove build artefacts'
complete -c opi -n __opi_first_word -l security -d 'Scan for secrets and vulnerable dependencies'
complete -c opi -n __opi_first_word -l updates -d 'Show dependencies with newer versions'
complete -c opi -n __opi_first_word -l check -x -d 'Run a workflow'
complete -c opi -n '__opi_after --check' -a 'commit push release deploy'
complete -c opi -n __opi_first_word -l sbom -r -d 'Write CycloneDX SBOMs'
complete -c opi -n '__opi_after --sbom' -F
complete -c opi -n __opi_first_word -l hooks -d 'Install the push workflow as the pre-push hook'
complete -c opi -n __opi_first_word -l checks -x -d 'Select checks for a workflow'
complete -c opi -n '__opi_after --checks' -a 'commit push release deploy'
complete -c opi -n __opi_first_word -l completions -x -d 'Print a completion script'
complete -c opi -n '__opi_after --completions' -a 'bash zsh fish'
complete -c opi -n __opi_first_word -s h -l help -d 'Show help'
complete -c opi -n __opi_first_word -s V -l version -d 'Show the version'
"#;

/// The completion script for `shell`, or `None` for a shell not supported.
pub fn script(shell: &str) -> Option<&'static str> {
    match shell {
        "bash" => Some(BASH),
        "zsh" => Some(ZSH),
        "fish" => Some(FISH),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_supported_shell_has_a_script_and_others_have_none() {
        for shell in ["bash", "zsh", "fish"] {
            assert!(script(shell).is_some(), "{shell}");
        }
        assert_eq!(script("powershell"), None);
    }

    #[test]
    fn every_script_names_all_flags_workflows_and_the_name_lookup() {
        for shell in ["bash", "zsh", "fish"] {
            let text = script(shell).unwrap();
            for word in [
                "health",
                "clean",
                "security",
                "updates",
                "check",
                "checks",
                "sbom",
                "hooks",
                "completions",
                "commit",
                "push",
                "release",
                "deploy",
                "opi --list-names",
            ] {
                assert!(text.contains(word), "{shell} lacks {word}");
            }
        }
    }

    #[test]
    fn script_names_are_completed_as_whole_lines_not_split_at_spaces() {
        assert!(!script("bash").unwrap().contains("-W \"$(opi"));
        assert!(script("bash").unwrap().contains("printf -v quoted '%q'"));
        assert!(script("zsh").unwrap().contains("${(f)"));
    }

    #[test]
    fn the_workflows_offered_are_the_ones_opi_parses() {
        for name in ["commit", "push", "release", "deploy"] {
            assert!(crate::workflow::Workflow::parse(name).is_some());
        }
    }
}
