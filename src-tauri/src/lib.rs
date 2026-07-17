pub mod app;
pub mod codex;
pub mod commands;
pub mod config;
pub mod deep_link;
pub mod hook;
pub mod hook_config;
pub mod model;
pub mod monitor;
pub mod registry;
pub mod tray;

#[cfg(test)]
mod tests {
    #[test]
    fn exposes_the_product_name() {
        assert_eq!(crate::app::product_name(), "Codex Pulse");
    }
}
