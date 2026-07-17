use crate::app::App;

/// A plugin encapsulates a self-contained piece of functionality that can be
/// registered with an [`App`].
///
/// Plugins are collected and built in [`App::build`]. A plugin's `build`
/// implementation can add resources, systems, and even other plugins.
pub trait Plugin: 'static {
    /// Register this plugin's resources and systems with `app`.
    fn build(&self, app: &mut App);
}
