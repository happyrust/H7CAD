use h7cad_native_model::{CadDocument, Handle};

#[derive(Debug, Default)]
pub struct DocumentBuilder {
    next_handle: u64,
}

impl DocumentBuilder {
    pub fn new() -> Self {
        Self { next_handle: 1 }
    }

    pub fn allocate_handle(&mut self) -> Handle {
        let handle = Handle(self.next_handle);
        self.next_handle += 1;
        handle
    }

    pub fn finish(self) -> CadDocument {
        CadDocument::default()
    }
}
