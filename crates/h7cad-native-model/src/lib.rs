#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Handle(pub u64);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CadDocument {
    pub entities: usize,
}
