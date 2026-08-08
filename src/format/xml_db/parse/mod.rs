#[derive(Debug, thiserror::Error)]
pub enum XmlParseError {
    #[error(transparent)]
    Xml(#[from] quick_xml::DeError),

    #[error(transparent)]
    XmlSerialize(#[from] quick_xml::se::SeError),

    #[error("XML schema conversion error: {0}")]
    Schema(String),
}
