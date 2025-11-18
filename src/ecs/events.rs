#[derive(Clone, Copy, Hash, Eq, PartialEq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, Copy, Hash, Eq, PartialEq)]
pub enum LemgineEvent {
    Movement,
}

#[derive(Clone, Copy, Hash, Eq, PartialEq)]
pub enum LemgineEventData {
    Movement(Direction),
}
