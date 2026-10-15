//! Minimal media-host fixture for coordinator tests. This is not the editor engine.
#[derive(Clone)]
pub struct NewMedia {pub path:String}
pub enum Command {AddMedia{item:NewMedia}, Batch{commands:Vec<Command>}}
#[derive(Default)]
pub struct Project {pub media:Vec<NewMedia>}
