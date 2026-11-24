use crate::extensions::contracts::{FieldEdit, McpMetadata};
use crate::extensions::validate::{validate_mcp_metadata, ExtensionValidationError};

pub fn apply_mcp_metadata(
    current: &Option<McpMetadata>,
    edit: Option<&FieldEdit<McpMetadata>>,
) -> Result<Option<McpMetadata>, ExtensionValidationError> {
    let source = || current.as_ref().and_then(|metadata| metadata.registry.clone());
    match edit {
        None => Ok(current.clone()),
        Some(FieldEdit::Delete) => Ok(source().map(|registry| McpMetadata {
            registry: Some(registry),
            ..McpMetadata::default()
        })),
        Some(FieldEdit::Replace { value }) => {
            let mut next = value.clone();
            next.registry = source();
            validate_mcp_metadata(&next)?;
            Ok(Some(next))
        }
    }
}
