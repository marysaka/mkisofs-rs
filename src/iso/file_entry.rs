use crate::iso::utils;
use crate::iso::utils::{LOGIC_SIZE, LOGIC_SIZE_I64, LOGIC_SIZE_U32};

use byteorder::{BigEndian, LittleEndian, WriteBytesExt};
use chrono::prelude::*;

use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::io::Cursor;
use std::io::SeekFrom;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum FileType {
    Regular { path: PathBuf },
    Buffer { name: String, data: Vec<u8> },
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub file_type: FileType,
    pub size: usize,
    pub lba: u32,
    pub aligned_size: usize,
}

impl FileEntry {
    pub fn get_file_name(&self) -> String {
        match &self.file_type {
            FileType::Regular { path } => path.file_name().unwrap().to_str().unwrap().to_string(),
            FileType::Buffer { name, .. } => name.clone(),
        }
    }

    pub fn open_content_provider(&self) -> Box<dyn Read> {
        match &self.file_type {
            FileType::Regular { path } => Box::new(File::open(path).unwrap()),
            FileType::Buffer { data, .. } => Box::new(Cursor::new(data.clone())),
        }
    }

    pub fn write_entry<T>(&self, output_writter: &mut T) -> std::io::Result<()>
    where
        T: Write + Seek,
    {
        let current_pos = output_writter.seek(SeekFrom::Current(0))? as i32;
        let expected_aligned_pos = utils::align_up(current_pos, LOGIC_SIZE_U32 as i32);

        let diff_size = expected_aligned_pos - current_pos;
        let file_entry_size = self.get_entry_size() as i32;

        if file_entry_size > diff_size && diff_size != 0 {
            let mut padding: Vec<u8> = Vec::new();
            padding.resize(diff_size as usize, 0u8);
            output_writter.write_all(&padding)?;
        }

        let old_pos = output_writter.seek(SeekFrom::Current(0))? as i32;

        let file_name = self.get_file_name();
        let file_identifier = utils::convert_name(&file_name);
        let file_identifier_len = file_identifier.len() + 2;

        output_writter.write_u8(file_entry_size as u8)?;

        // Extended Attribute Record length.
        output_writter.write_u8(0u8)?;

        // Location of extent (in LB)
        write_bothendian! {
            output_writter.write_u32(self.lba)?;
        }

        // Extent size
        write_bothendian! {
            output_writter.write_u32(self.size as u32)?;
        }

        let record_datetime: DateTime<Utc> = Utc::now();
        output_writter.write_u8((record_datetime.year() - 1900) as u8)?;
        output_writter.write_u8((record_datetime.month()) as u8)?;
        output_writter.write_u8((record_datetime.day()) as u8)?;
        output_writter.write_u8((record_datetime.hour()) as u8)?;
        output_writter.write_u8((record_datetime.minute()) as u8)?;
        output_writter.write_u8((record_datetime.second()) as u8)?;
        output_writter.write_u8(0u8)?;

        // file flags
        output_writter.write_u8(0x0u8)?;

        output_writter.write_u8(0x0u8)?;
        output_writter.write_u8(0x0u8)?;

        write_bothendian! {
            output_writter.write_u16(0x1)?;
        }

        output_writter.write_u8(file_identifier_len as u8)?;
        output_writter.write_all(&file_identifier[..])?;
        output_writter.write_all(b";1")?;

        // padding if even
        if (file_identifier_len % 2) == 0 {
            output_writter.write_u8(0x0u8)?;
        }

        // RRIP 'PX' entry (IEEE P1282 4.1.1)
        output_writter.write_all(b"PX")?;
        output_writter.write_u8(0x2c)?;
        output_writter.write_u8(0x1)?;

        // file mode
        write_bothendian! {
            output_writter.write_u32(0o100_644)?; // harcoded rw-r--r--
        }

        // links
        write_bothendian! {
            output_writter.write_u32(0x1)?; // one link
        }

        // user id
        write_bothendian! {
            output_writter.write_u32(0x0)?; // root
        }

        // group id
        write_bothendian! {
            output_writter.write_u32(0x0)?; // root
        }

        // "File Serial number"
        write_bothendian! {
            // dirty way to generate an inode but I guess it's fine
            output_writter.write_u32(self.lba)?;
        }

        // RRIP 'NM' entry (IEEE P1282 4.1.4)
        output_writter.write_all(b"NM")?;
        output_writter.write_u8(0x5 + file_name.len() as u8)?;
        output_writter.write_u8(0x1)?;
        output_writter.write_u8(0x0)?; // No flags
        output_writter.write_all(file_name.as_bytes())?;

        let new_pos = output_writter.seek(SeekFrom::Current(0))? as i32;

        assert!(old_pos + file_entry_size == new_pos);

        Ok(())
    }

    pub fn get_entry_size(&self) -> u32 {
        let file_name = self.get_file_name();

        // don't miss to count the ";1"!
        utils::get_entry_size(0x21 + 2, &file_name, 0, 1)
    }

    pub fn update(&mut self) {
        match &self.file_type {
            FileType::Buffer { data, .. } => {
                self.size = data.len();
                self.aligned_size =
                    utils::align_up(self.size as i32, LOGIC_SIZE_U32 as i32) as usize;
            }
            _ => unimplemented!(),
        }
    }

    pub fn write_content<T>(&mut self, output_writter: &mut T) -> std::io::Result<()>
    where
        T: Write + Seek,
    {
        let old_pos = output_writter.seek(SeekFrom::Current(0))?;

        // Seek to the correct LBA
        output_writter.seek(SeekFrom::Start(u64::from(self.lba * LOGIC_SIZE_U32)))?;

        let mut file: Box<dyn Read> = self.open_content_provider();
        io::copy(&mut file, output_writter)?;

        let current_pos = output_writter.seek(SeekFrom::Current(0))? as usize;
        let expected_aligned_pos = ((current_pos as i64) & -LOGIC_SIZE_I64) as usize;

        let diff_size = current_pos - expected_aligned_pos;

        if diff_size != 0 {
            let mut padding: Vec<u8> = Vec::new();
            padding.resize(LOGIC_SIZE - diff_size, 0u8);
            output_writter.write_all(&padding)?;
        }

        output_writter.seek(SeekFrom::Start(old_pos))?;

        Ok(())
    }

    pub fn new_buffered(name: String) -> FileEntry {
        FileEntry {
            file_type: FileType::Buffer {
                name,
                data: Vec::new(),
            },
            lba: 0,
            size: 0,
            aligned_size: 0,
        }
    }
}
