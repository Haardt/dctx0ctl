use zed_extension_api::{self as zed, LanguageServerId, Result};
struct Dctx0Extension;
impl zed::Extension for Dctx0Extension {
    fn new() -> Self { Self }
    fn language_server_command(&mut self, _: &LanguageServerId, _: &zed::Worktree) -> Result<zed::Command> {
        Ok(zed::Command {
            command: __DCTX0_SERVER_PATH__.to_string(),
            args: vec!["lsp".into(), "--stdio".into()],
            env: Default::default(),
        })
    }
}
zed::register_extension!(Dctx0Extension);
