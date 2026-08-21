/// Bash completion generator.

pub struct BashGenerator;

impl BashGenerator {
    /// Generate bash completion script.
    pub fn generate(binary_name: &str) -> String {
        format!(
            r#"#!/bin/bash
# Bash completion for {binary_name}

_{binary_name}_completions() {{
    local cur prev commands
    COMPREPLY=()
    cur="${{COMP_WORDS[COMP_CWORD]}}"
    prev="${{COMP_WORDS[COMP_CWORD-1]}}"

    commands="init list show validate archive update status instructions templates schemas new completion config schema store doctor context workset feedback"

    if [[ ${{cur}} == -* || ${{COMP_CWORD}} -eq 1 ]] ; then
        COMPREPLY=( $(compgen -W "${{commands}}" -- "${{cur}}") )
        return 0
    fi
}}

complete -F _{binary_name}_completions {binary_name}
"#,
            binary_name = binary_name
        )
    }
}
