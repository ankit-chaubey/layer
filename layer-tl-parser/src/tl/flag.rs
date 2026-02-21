/// A flag reference inside a parameter type, e.g. `flags.0` in `flags.0?true`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Flag {
    /// The name of the flags field that holds this bit (usually `"flags"`).
    pub name: String,
    /// The bit index (0-based).
    pub index: u32,
}
