use zed_extension_api::{self as zed, Command, LanguageServerId, Result, Worktree};

struct PermissionHintExtension;

impl zed::Extension for PermissionHintExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<Command> {
        let command = worktree.which("permission-lsp").ok_or_else(|| {
            "permission-lsp not found on $PATH; install it with \
             `cargo install --path crates/permission-lsp`"
                .to_string()
        })?;
        Ok(Command {
            command,
            args: Vec::new(),
            env: worktree.shell_env(),
        })
    }
}

zed::register_extension!(PermissionHintExtension);
