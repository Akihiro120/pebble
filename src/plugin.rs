use crate::app::App;

pub trait Plugin: 'static {
    fn build(&self, app: &mut App);
}
