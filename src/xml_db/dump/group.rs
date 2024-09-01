use crate::{
    crypt::ciphers::Cipher,
    db::{with_node, Entry, Group, NodePtr},
    xml_db::dump::{DumpXml, SimpleTag},
};
use xml::writer::{EventWriter, XmlEvent as WriterEvent};

impl DumpXml for Group {
    fn dump_xml<E: std::io::Write>(&self, writer: &mut EventWriter<E>, inner_cipher: &mut dyn Cipher) -> Result<(), xml::writer::Error> {
        writer.write(WriterEvent::start_element("Group"))?;

        SimpleTag("Name", self.name.as_deref().unwrap_or("")).dump_xml(writer, inner_cipher)?;
        SimpleTag("UUID", &self.uuid).dump_xml(writer, inner_cipher)?;

        if let Some(ref value) = self.notes {
            SimpleTag("Notes", value).dump_xml(writer, inner_cipher)?;
        }

        if let Some(value) = self.icon_id {
            SimpleTag("IconID", usize::from(value)).dump_xml(writer, inner_cipher)?;
        }

        if let Some(ref value) = self.custom_icon_uuid {
            SimpleTag("CustomIconUUID", value).dump_xml(writer, inner_cipher)?;
        }

        self.times.dump_xml(writer, inner_cipher)?;
        self.custom_data.dump_xml(writer, inner_cipher)?;

        SimpleTag("IsExpanded", self.is_expanded).dump_xml(writer, inner_cipher)?;

        if let Some(ref value) = self.default_autotype_sequence {
            SimpleTag("DefaultAutoTypeSequence", value).dump_xml(writer, inner_cipher)?;
        }

        if let Some(ref value) = self.enable_autotype {
            SimpleTag("EnableAutoType", value).dump_xml(writer, inner_cipher)?;
        }

        if let Some(ref value) = self.enable_searching {
            SimpleTag("EnableSearching", value).dump_xml(writer, inner_cipher)?;
        }

        if let Some(ref value) = self.last_top_visible_entry {
            SimpleTag("LastTopVisibleEntry", value).dump_xml(writer, inner_cipher)?;
        }

        for child in &self.children {
            child.dump_xml(writer, inner_cipher)?;
        }

        writer.write(WriterEvent::end_element())?; // Group

        Ok(())
    }
}

impl DumpXml for NodePtr {
    fn dump_xml<E: std::io::Write>(&self, writer: &mut EventWriter<E>, inner_cipher: &mut dyn Cipher) -> Result<(), xml::writer::Error> {
        use std::io::{Error, ErrorKind::Other};
        let err = xml::writer::Error::Io(Error::new(Other, "Node is neither an entry nor a group"));
        let _ = with_node::<Group, _, _>(self, |g| g.dump_xml(writer, inner_cipher))
            .or_else(|| with_node::<Entry, _, _>(self, |e| e.dump_xml(writer, inner_cipher)))
            .ok_or(err)?;
        Ok(())
    }
}
