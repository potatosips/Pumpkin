use pumpkin_data::packet::serverbound::PLAY_EDIT_BOOK;
use pumpkin_macros::java_packet;
use pumpkin_util::version::JavaMinecraftVersion;

use crate::{
    ServerPacket, VarInt,
    ser::{NetworkReadExt, NetworkReadSliceExt, ReadingError},
};

#[derive(Debug)]
#[java_packet(PLAY_EDIT_BOOK)]
pub struct SEditBook<'a> {
    pub slot: VarInt,
    pub pages: Vec<&'a str>,
    pub title: Option<&'a str>,
}

impl<'a> ServerPacket<'a> for SEditBook<'a> {
    fn read(read: &mut &'a [u8], _version: &JavaMinecraftVersion) -> Result<Self, ReadingError> {
        let slot = read.get_var_int()?;
        let count = read.get_var_int()?.0;
        if !(0..=100).contains(&count) {
            return Err(ReadingError::Message(
                "Edit-book page count must be between 0 and 100".into(),
            ));
        }
        let count = count as usize;
        let mut pages = Vec::with_capacity(count);
        for _ in 0..count {
            pages.push(read.get_str_bounded_borrowed(1024)?);
        }
        let has_title = read.get_bool()?;
        let title = if has_title {
            Some(read.get_str_bounded_borrowed(32)?)
        } else {
            None
        };
        Ok(Self { slot, pages, title })
    }
}

#[cfg(test)]
mod tests {
    use pumpkin_util::version::JavaMinecraftVersion;

    use crate::{ServerPacket, VarInt, ser::NetworkWriteExt};

    use super::SEditBook;

    fn packet_with_page_count(count: i32) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.write_var_int(&VarInt(0)).unwrap();
        bytes.write_var_int(&VarInt(count)).unwrap();
        bytes
    }

    #[test]
    fn rejects_more_than_vanilla_maximum_pages() {
        let bytes = packet_with_page_count(101);
        assert!(SEditBook::read(&mut bytes.as_slice(), &JavaMinecraftVersion::V_1_21_4).is_err());
    }

    #[test]
    fn rejects_negative_page_count() {
        let bytes = packet_with_page_count(-1);
        assert!(SEditBook::read(&mut bytes.as_slice(), &JavaMinecraftVersion::V_1_21_4).is_err());
    }

    #[test]
    fn rejects_title_longer_than_vanilla_limit() {
        let packet = SEditBook {
            slot: VarInt(0),
            pages: Vec::new(),
            title: Some("123456789012345678901234567890123"),
        };
        let mut bytes = Vec::new();
        // Write the malformed packet manually because the bounded packet writer
        // correctly refuses to create it.
        bytes.write_var_int(&packet.slot).unwrap();
        bytes.write_var_int(&VarInt(0)).unwrap();
        bytes.write_bool(true).unwrap();
        bytes.write_string(packet.title.unwrap()).unwrap();

        assert!(SEditBook::read(&mut bytes.as_slice(), &JavaMinecraftVersion::V_1_21_4).is_err());
    }
}

impl crate::ClientPacket for SEditBook<'_> {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        use crate::ser::NetworkWriteExt;
        write.write_var_int(&self.slot)?;
        write.write_var_int(&VarInt(self.pages.len() as i32))?;
        for page in &self.pages {
            write.write_string_bounded(page, 1024)?;
        }
        if let Some(title) = self.title {
            write.write_bool(true)?;
            write.write_string_bounded(title, 128)?;
        } else {
            write.write_bool(false)?;
        }
        Ok(())
    }
}
