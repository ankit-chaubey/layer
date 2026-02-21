/// Whether a [`super::Definition`] is a data constructor or an RPC function.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    /// A concrete data constructor (the section before `---functions---`).
    Types,
    /// An RPC function definition (the section after `---functions---`).
    Functions,
}
