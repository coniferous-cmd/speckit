/// Command registry for completions.

/// A registered command for completion.
#[derive(Debug, Clone)]
pub struct RegisteredCommand {
    pub name: String,
    pub description: String,
    pub subcommands: Vec<RegisteredCommand>,
    pub options: Vec<RegisteredOption>,
}

/// A registered option for completion.
#[derive(Debug, Clone)]
pub struct RegisteredOption {
    pub long: String,
    pub short: Option<String>,
    pub description: String,
    pub takes_value: bool,
}

/// Registry of commands for completion.
pub struct CommandRegistry {
    commands: Vec<RegisteredCommand>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    pub fn register(&mut self, command: RegisteredCommand) {
        self.commands.push(command);
    }

    pub fn get_commands(&self) -> &[RegisteredCommand] {
        &self.commands
    }

    pub fn find_command(&self, name: &str) -> Option<&RegisteredCommand> {
        self.commands.iter().find(|c| c.name == name)
    }
}
