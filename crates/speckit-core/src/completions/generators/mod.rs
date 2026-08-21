mod bash;
mod fish;
mod powershell;
mod zsh;

pub use bash::BashGenerator;
pub use fish::FishGenerator;
pub use powershell::PowerShellGenerator;
pub use zsh::ZshGenerator;
