/// Player info.
#[derive(Debug)]
pub struct Player {
    pub name: String,
    pub clan: String,
    pub country: i32,
    pub score: i32,
    pub is_spectator: bool,
}
