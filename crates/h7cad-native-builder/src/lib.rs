use std::collections::BTreeMap;

use h7cad_native_model::{BlockRecord, CadDocument, Handle, Layout};

#[derive(Debug, Default)]
pub struct DocumentBuilder {
    next_handle: u64,
    block_templates: BTreeMap<String, BlockTemplate>,
    layout_templates: BTreeMap<String, LayoutTemplate>,
}

impl DocumentBuilder {
    pub fn new() -> Self {
        Self {
            next_handle: 1,
            block_templates: BTreeMap::new(),
            layout_templates: BTreeMap::new(),
        }
    }

    pub fn allocate_handle(&mut self) -> Handle {
        let handle = Handle(self.next_handle);
        self.next_handle += 1;
        handle
    }

    pub fn register_block_template(&mut self, name: impl Into<String>) {
        let name = name.into();
        self.block_templates
            .entry(name.clone())
            .or_insert_with(|| BlockTemplate { name });
    }

    pub fn register_layout_template(
        &mut self,
        layout_name: impl Into<String>,
        block_name: impl Into<String>,
    ) {
        let layout_name = layout_name.into();
        let block_name = block_name.into();
        self.register_block_template(block_name.clone());
        self.layout_templates
            .entry(layout_name.clone())
            .or_insert_with(|| LayoutTemplate {
                name: layout_name,
                block_name,
            });
    }

    pub fn finish(self) -> CadDocument {
        let mut document = CadDocument::new();
        let next = document.next_handle();
        if self.next_handle > next {
            let target = self.next_handle;
            while document.next_handle() < target {
                let _ = document.allocate_handle();
            }
        }

        let mut registered_blocks = BTreeMap::new();
        for template in self.block_templates.values() {
            if document.tables.block_record.entries.contains_key(&template.name) {
                continue;
            }

            let handle = document.allocate_handle();
            let block_record = BlockRecord::new(handle, template.name.clone());
            registered_blocks.insert(template.name.clone(), handle);
            document.insert_block_record(block_record);
        }

        for template in self.layout_templates.values() {
            let block_handle = document
                .tables
                .block_record
                .entries
                .get(&template.block_name)
                .copied()
                .or_else(|| registered_blocks.get(&template.block_name).copied())
                .unwrap_or_else(|| {
                    let handle = document.allocate_handle();
                    let block_record = BlockRecord::new(handle, template.block_name.clone());
                    document.insert_block_record(block_record);
                    handle
                });

            let layout = Layout::new(
                document.allocate_handle(),
                template.name.clone(),
                block_handle,
            );
            document.insert_layout(layout);
        }

        document.repair_ownership();
        document
    }
}

#[derive(Debug, Clone)]
struct BlockTemplate {
    name: String,
}

#[derive(Debug, Clone)]
struct LayoutTemplate {
    name: String,
    block_name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_finish_returns_minimal_native_document() {
        let document = DocumentBuilder::new().finish();

        assert_eq!(document.model_space_handle(), Handle::new(1));
        assert_eq!(document.paper_space_handle(), Handle::new(2));
        assert_eq!(document.root_dictionary.handle, Handle::new(5));
    }

    #[test]
    fn builder_keeps_handle_sequence_after_preallocation() {
        let mut builder = DocumentBuilder::new();

        assert_eq!(builder.allocate_handle(), Handle::new(1));
        assert_eq!(builder.allocate_handle(), Handle::new(2));

        let mut document = builder.finish();
        assert_eq!(document.next_handle(), 6);
        assert_eq!(document.allocate_handle(), Handle::new(6));
    }

    #[test]
    fn builder_registers_templates_and_repairs_owner_graph() {
        let mut builder = DocumentBuilder::new();
        builder.register_layout_template("Layout2", "*Paper_Space2");

        let document = builder.finish();
        let block_handle = *document
            .tables
            .block_record
            .entries
            .get("*Paper_Space2")
            .unwrap();
        let block = document.block_records.get(&block_handle).unwrap();
        let layout_handle = block.layout_handle.unwrap();
        let layout = document.layouts.get(&layout_handle).unwrap();

        assert_eq!(layout.name, "Layout2");
        assert_eq!(layout.owner, document.root_dictionary.handle);
        assert_eq!(layout.block_record_handle, block_handle);
        assert_eq!(
            document.root_dictionary.entries.get("LAYOUT_7"),
            Some(&layout_handle)
        );
    }
}
