use crate::iso::file_entry::{FileEntry, FileType};
use crate::iso::utils;
use crate::iso::utils::{LOGIC_SIZE, LOGIC_SIZE_I64, LOGIC_SIZE_U32};
use byteorder::{BigEndian, ByteOrder, LittleEndian, WriteBytesExt};
use chrono::prelude::*;

use std::fs;
use std::fs::DirEntry;
use std::fs::Metadata;
use std::io::prelude::*;
use std::io::Cursor;
use std::io::SeekFrom;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DirectoryEntry {
    pub path_table_index: u32,
    pub parent_index: u32,
    pub path: PathBuf,
    pub dir_childs: Vec<DirectoryEntry>,
    pub files_childs: Vec<FileEntry>,
    pub continuation_area: Option<Vec<u8>>,
    pub lba: u32,
}

impl DirectoryEntry {
    fn write_entry<T>(
        directory_entry: &DirectoryEntry,
        output_writter: &mut T,
        directory_type: u32,
    ) -> std::io::Result<()>
    where
        T: Write + Seek,
    {
        let current_pos = output_writter.seek(SeekFrom::Current(0))? as i32;
        let expected_aligned_pos = utils::align_up(current_pos, LOGIC_SIZE_U32 as i32);

        let diff_size = expected_aligned_pos - current_pos;
        let file_entry_size = directory_entry.get_entry_size(Some(directory_type)) as i32;

        if file_entry_size > diff_size && diff_size != 0 {
            let mut padding: Vec<u8> = Vec::new();
            padding.resize(diff_size as usize, 0u8);
            output_writter.write_all(&padding)?;
        }

        let old_pos = output_writter.seek(SeekFrom::Current(0))? as i32;

        let file_name = directory_entry.path.file_name().unwrap().to_str().unwrap();

        let file_name_fixed = utils::convert_name(file_name);
        let file_identifier = match directory_type {
            1 => &[0u8],
            2 => &[1u8],
            3 => &[0u8],
            4 => &[1u8],
            5 => &[0u8],
            _ => &file_name_fixed[..],
        };

        let file_identifier_len = file_identifier.len();

        output_writter.write_u8(file_entry_size as u8)?;

        // Extended Attribute Record length.
        output_writter.write_u8(0u8)?;

        // Location of extent (in LB)
        write_bothendian! {
            output_writter.write_u32(directory_entry.lba)?;
        }

        // Extent size (size of an LB)
        write_bothendian! {
            output_writter.write_u32(directory_entry.get_extent_size_in_lb() * LOGIC_SIZE_U32)?;
        }

        let record_datetime: DateTime<Utc> = Utc::now();
        output_writter.write_u8((record_datetime.year() - 1900) as u8)?;
        output_writter.write_u8((record_datetime.month()) as u8)?;
        output_writter.write_u8((record_datetime.day()) as u8)?;
        output_writter.write_u8((record_datetime.hour()) as u8)?;
        output_writter.write_u8((record_datetime.minute()) as u8)?;
        output_writter.write_u8((record_datetime.second()) as u8)?;
        output_writter.write_u8(0u8)?;

        // file flags (0x2 == directory)
        output_writter.write_u8(0x2u8)?;

        output_writter.write_u8(0x0u8)?;
        output_writter.write_u8(0x0u8)?;

        write_bothendian! {
            output_writter.write_u16(0x1)?;
        }

        output_writter.write_u8(file_identifier_len as u8)?;
        output_writter.write_all(file_identifier)?;

        // padding if even
        if (file_identifier_len % 2) == 0 {
            output_writter.write_u8(0x0u8)?;
        }

        // SUSP entries for root '.'
        if directory_type == 3 {
            // SUSP 'SP' entry (IEEE P1281 5.3)
            output_writter.write_all(b"SP")?;
            output_writter.write_u8(0x7)?;
            output_writter.write_u8(0x1)?;
            output_writter.write_u8(0xBE)?;
            output_writter.write_u8(0xEF)?;
            output_writter.write_u8(0x0)?;

            // TODO: 'CE' can be found in other entries, move this when size calculation will be good
            if directory_entry.continuation_area.is_some() {
                // SUSP 'CE' entry (IEEE P1281 5.1)
                output_writter.write_all(b"CE")?;
                output_writter.write_u8(0x1c)?;
                output_writter.write_u8(0x1)?;

                // The 'CE' logical block is just after the root directory entries
                write_bothendian! {
                    output_writter.write_u32(directory_entry.lba + directory_entry.get_extent_size_in_lb())?;
                }

                // The 'CE' offset inside the block (0 for our usecase)
                write_bothendian! {
                    output_writter.write_u32(0)?;
                }

                // 'CE' section size
                match &directory_entry.continuation_area {
                    Some(continuation_area) => {
                        write_bothendian! {
                            output_writter.write_u32(continuation_area.len() as u32)?;
                        }
                    }
                    _ => panic!(),
                }
            }
        }

        if directory_type < 5 {
            // RRIP 'PX' entry (IEEE P1282 4.1.1)
            output_writter.write_all(b"PX")?;
            output_writter.write_u8(0x2c)?;
            output_writter.write_u8(0x1)?;

            // file mode
            write_bothendian! {
                output_writter.write_u32(0o040_755)?; // harcoded drwxr-xr-x
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
                output_writter.write_u32(directory_entry.lba + directory_entry.path_table_index)?;
            }
        }

        // RRIP 'NM' entry (IEEE P1282 4.1.4)
        if directory_type == 0 {
            output_writter.write_all(b"NM")?;
            output_writter.write_u8(0x5 + file_name.len() as u8)?;
            output_writter.write_u8(0x1)?;
            output_writter.write_u8(0x0)?; // No flags
            output_writter.write_all(file_name.as_bytes())?;
        }

        let new_pos = output_writter.seek(SeekFrom::Current(0))? as i32;

        assert!(old_pos + file_entry_size == new_pos);

        Ok(())
    }

    pub fn get_path_table_size(&self) -> u32 {
        let mut res = 0u32;

        let file_name = self.path.file_name().unwrap().to_str().unwrap();

        let directory_type = if self.path_table_index == 1 { 5 } else { 6 };

        res += utils::get_entry_size(0x8, file_name, directory_type, 0);

        for entry in &self.dir_childs {
            res += entry.get_path_table_size();
        }

        res
    }

    pub fn get_extent_size_in_lb(&self) -> u32 {
        let mut res = 1u32;
        let mut size = 0u32;

        size += self.get_entry_size(Some(3)); // '.'
        size += self.get_entry_size(Some(2)); // '..'

        for entry in &self.dir_childs {
            let entry_size = entry.get_entry_size(Some(0)) as i32;
            let expected_aligned_size = utils::align_up(size as i32, LOGIC_SIZE_U32 as i32);
            let available_size_in_lb = expected_aligned_size - size as i32;

            if entry_size > available_size_in_lb && available_size_in_lb != 0 {
                size = 0;
                res += 1;
            }

            size += entry_size as u32;
        }

        for entry in &self.files_childs {
            let entry_size = entry.get_entry_size() as i32;
            let expected_aligned_size = utils::align_up(size as i32, LOGIC_SIZE_U32 as i32);
            let available_size_in_lb = expected_aligned_size - size as i32;

            if entry_size > available_size_in_lb && available_size_in_lb != 0 {
                size = 0;
                res += 1;
            }

            size += entry_size as u32;
        }

        res
    }

    pub fn get_entry_size(&self, directory_type: Option<u32>) -> u32 {
        let file_name = self.path.file_name().unwrap().to_str().unwrap();

        utils::get_entry_size(0x21, file_name, directory_type.unwrap_or(0), 1)
    }

    pub fn get_file_name(&self) -> String {
        self.path.file_name().unwrap().to_str().unwrap().to_string()
    }

    fn write_path_table_entry<T, Order: ByteOrder>(
        directory_entry: &DirectoryEntry,
        output_writter: &mut T,
        directory_type: u32,
    ) -> std::io::Result<()>
    where
        T: Write,
    {
        let file_name = directory_entry.path.file_name().unwrap().to_str().unwrap();

        let file_name_fixed = utils::convert_name(file_name);

        let file_identifier = match directory_type {
            1 => &[0u8],
            _ => &file_name_fixed[..],
        };

        let file_identifier_len = file_identifier.len();

        output_writter.write_u8(file_identifier_len as u8)?;
        output_writter.write_u8(0x0u8)?;
        output_writter.write_u32::<Order>(directory_entry.lba)?;
        output_writter.write_u16::<Order>(directory_entry.parent_index as u16)?;
        output_writter.write_all(file_identifier)?;

        // padding if odd
        if (file_identifier_len % 2) != 0 {
            output_writter.write_u8(0x0u8)?;
        }

        Ok(())
    }

    fn write_path_table_childs<T, Order: ByteOrder>(
        &mut self,
        output_writter: &mut T,
    ) -> std::io::Result<()>
    where
        T: Write,
    {
        for entry in &mut self.dir_childs {
            DirectoryEntry::write_path_table_entry::<T, Order>(entry, output_writter, 0)?;
        }

        for entry in &mut self.dir_childs {
            entry.write_path_table_childs::<T, Order>(output_writter)?;
        }

        Ok(())
    }

    pub fn write_path_table<T, Order: ByteOrder>(
        &mut self,
        output_writter: &mut T,
        path_table_pos: u32,
    ) -> std::io::Result<()>
    where
        T: Write + Seek,
    {
        let old_pos = output_writter.seek(SeekFrom::Current(0))?;

        // Seek to the correct LBA
        output_writter.seek(SeekFrom::Start(u64::from(path_table_pos * LOGIC_SIZE_U32)))?;

        let old_pos_current_context = output_writter.seek(SeekFrom::Current(0))?;

        // Write root
        DirectoryEntry::write_path_table_entry::<T, Order>(self, output_writter, 1)?;

        self.write_path_table_childs::<T, Order>(output_writter)?;

        // Pad to LBA size
        let current_pos = output_writter.seek(SeekFrom::Current(0))? as usize;
        let expected_aligned_pos = ((current_pos as i64) & -LOGIC_SIZE_I64) as usize;

        let diff_size = current_pos - expected_aligned_pos;

        let written_size = current_pos - (old_pos_current_context as usize);
        assert!(written_size == (self.get_path_table_size() as usize));

        if diff_size != 0 {
            let mut padding: Vec<u8> = Vec::new();
            padding.resize(LOGIC_SIZE - diff_size, 0u8);
            output_writter.write_all(&padding)?;
        }

        // Restore old position
        output_writter.seek(SeekFrom::Start(old_pos))?;

        Ok(())
    }

    pub fn write_extent<T>(
        &mut self,
        output_writter: &mut T,
        parent_option: Option<&DirectoryEntry>,
    ) -> std::io::Result<()>
    where
        T: Write + Seek,
    {
        let old_pos = output_writter.seek(SeekFrom::Current(0))?;

        // Seek to the correct LBA
        output_writter.seek(SeekFrom::Start(u64::from(self.lba * LOGIC_SIZE_U32)))?;

        let directory_type_current = if parent_option.is_none() { 3 } else { 1 };

        self.write_as_current(output_writter, directory_type_current)?;

        let mut empty_parent_path = PathBuf::new();
        empty_parent_path.set_file_name("dummy");
        let empty_parent = DirectoryEntry {
            path_table_index: 0,
            parent_index: 0,
            path: empty_parent_path,
            dir_childs: Vec::new(),
            files_childs: Vec::new(),
            lba: self.lba,
            continuation_area: None,
        };

        let parent = match parent_option {
            Some(res) => res,
            None => &empty_parent,
        };

        parent.write_as_parent(output_writter)?;

        // FIXME: dirty
        let self_clone = self.clone();

        for child_directory in &mut self.dir_childs {
            child_directory.write_one(output_writter)?;
            child_directory.write_extent(output_writter, Some(&self_clone))?;
        }

        for child_file in &mut self.files_childs {
            child_file.write_entry(output_writter)?;
        }

        // Pad to LBA size
        let current_pos = output_writter.seek(SeekFrom::Current(0))? as usize;
        let expected_aligned_pos = ((current_pos as i64) & -LOGIC_SIZE_I64) as usize;

        let diff_size = current_pos - expected_aligned_pos;

        if diff_size != 0 {
            let mut padding: Vec<u8> = Vec::new();
            padding.resize(LOGIC_SIZE - diff_size, 0u8);
            output_writter.write_all(&padding)?;
        }

        self.write_continuation_area(output_writter)?;

        // Restore old position
        output_writter.seek(SeekFrom::Start(old_pos))?;

        Ok(())
    }

    pub fn write_files<T>(&mut self, output_writter: &mut T) -> std::io::Result<()>
    where
        T: Write + Seek,
    {
        for child_directory in &mut self.dir_childs {
            child_directory.write_files(output_writter)?;
        }

        for child_file in &mut self.files_childs {
            child_file.write_content(output_writter)?;
        }
        Ok(())
    }

    pub fn write_as_current<T>(
        &self,
        output_writter: &mut T,
        directory_type: u32,
    ) -> std::io::Result<()>
    where
        T: Write + Seek,
    {
        DirectoryEntry::write_entry(self, output_writter, directory_type)
    }

    pub fn write_as_parent<T>(&self, output_writter: &mut T) -> std::io::Result<()>
    where
        T: Write + Seek,
    {
        DirectoryEntry::write_entry(self, output_writter, 2)
    }

    fn write_one<T>(&self, output_writter: &mut T) -> std::io::Result<()>
    where
        T: Write + Seek,
    {
        DirectoryEntry::write_entry(self, output_writter, 0)
    }

    fn write_continuation_area<T>(&self, output_writter: &mut T) -> std::io::Result<()>
    where
        T: Write + Seek,
    {
        if let Some(data) = &self.continuation_area {
            let old_pos = output_writter.seek(SeekFrom::Current(0))?;

            // Seek to the correct LBA
            output_writter.seek(SeekFrom::Start(u64::from(
                (self.lba + self.get_extent_size_in_lb()) * LOGIC_SIZE_U32,
            )))?;

            let mut tmp_cursor = Cursor::new(data.clone());

            std::io::copy(&mut tmp_cursor, output_writter)?;

            // Pad to LBA size
            let current_pos = output_writter.seek(SeekFrom::Current(0))? as usize;
            let expected_aligned_pos = ((current_pos as i64) & -LOGIC_SIZE_I64) as usize;

            let diff_size = current_pos - expected_aligned_pos;

            if diff_size != 0 {
                let mut padding: Vec<u8> = Vec::new();
                padding.resize(LOGIC_SIZE - diff_size, 0u8);
                output_writter.write_all(&padding)?;
            }

            // Restore old position
            output_writter.seek(SeekFrom::Start(old_pos))?;
        }

        Ok(())
    }

    pub fn get_directory(&mut self, dir_name: &str) -> Option<&mut DirectoryEntry> {
        let mut res = None;

        for child in &mut self.dir_childs {
            let file_name = child.path.file_name().unwrap().to_str().unwrap();

            if file_name == dir_name {
                res = Some(child);
                break;
            }
        }
        res
    }

    pub fn get_file(&mut self, path: &str) -> Option<&mut FileEntry> {
        let mut cut_path: Vec<&str> = path.split('/').collect();

        let mut directory_entry: Option<&mut DirectoryEntry> = Some(self);

        if cut_path.len() != 1 {
            directory_entry = directory_entry.unwrap().get_directory(cut_path[0]);

            directory_entry.as_ref()?;
            cut_path.remove(0);
        }

        while cut_path.len() != 1 {
            directory_entry = directory_entry.unwrap().get_directory(cut_path[0]);

            directory_entry.as_ref()?;
            cut_path.remove(0);
        }

        let dir = directory_entry.unwrap();
        let mut res = None;

        for child in &mut dir.files_childs {
            let name = child.get_file_name();

            if name == cut_path[0] {
                res = Some(child);
                break;
            }
        }

        res
    }

    pub fn add_file(&mut self, file: FileEntry) -> &FileEntry {
        self.files_childs.push(file);
        self.files_childs.last().unwrap()
    }

    fn add_and_merge_childs_directories(
        dir_childs: &mut Vec<DirectoryEntry>,
        other: DirectoryEntry,
    ) {
        let mut new_entry = other;
        let optinal_present_entry: Option<&mut DirectoryEntry> = dir_childs
            .iter_mut()
            .filter(|in_entry| in_entry.get_file_name() == new_entry.get_file_name())
            .last();

        if let Some(present_entry) = optinal_present_entry {
            present_entry
                .files_childs
                .append(&mut new_entry.files_childs);
            present_entry.merge_child_directories(new_entry);
        } else {
            dir_childs.push(new_entry);
        }
    }

    fn merge_child_directories(&mut self, other: DirectoryEntry) {
        for mut child in other.dir_childs {
            let vec = &mut self.dir_childs;
            let optinal_present_entry: Option<&mut DirectoryEntry> = vec
                .iter_mut()
                .filter(|in_entry| in_entry.get_file_name() == child.get_file_name())
                .last();
            if let Some(present_entry) = optinal_present_entry {
                present_entry.files_childs.append(&mut child.files_childs);
                present_entry.merge_child_directories(child);
            } else {
                self.dir_childs.push(child);
            }
        }
    }

    pub fn set_path(&mut self, path: &[PathBuf]) -> std::io::Result<()> {
        let mut dir_childs: Vec<DirectoryEntry> = Vec::new();
        let mut files_childs: Vec<FileEntry> = Vec::new();

        let mut ordered_dir: Vec<DirEntry> = path
            .iter()
            .flat_map(|path| {
                let res: Vec<DirEntry> = fs::read_dir(path).unwrap().map(|r| r.unwrap()).collect();
                res
            })
            .collect();

        ordered_dir.sort_by_key(|dir| dir.path());

        for entry in ordered_dir {
            let entry_meta: Metadata = entry.metadata()?;
            if entry_meta.is_dir() {
                let path_list: Vec<PathBuf> = vec![entry.path()];
                let mut new_dir = DirectoryEntry::new()?;
                new_dir.set_path(&path_list)?;
                DirectoryEntry::add_and_merge_childs_directories(&mut dir_childs, new_dir);
            } else if entry_meta.is_file() {
                files_childs.push(FileEntry {
                    file_type: FileType::Regular { path: entry.path() },
                    size: entry_meta.len() as usize,
                    lba: 0,
                    aligned_size: utils::align_up(entry_meta.len() as i32, LOGIC_SIZE_U32 as i32)
                        as usize,
                })
            }
        }

        self.path = path[0].clone();
        self.dir_childs.append(&mut dir_childs);
        self.files_childs.append(&mut files_childs);
        Ok(())
    }

    pub fn new() -> std::io::Result<DirectoryEntry> {
        Ok(DirectoryEntry {
            path_table_index: 0,
            parent_index: 0,
            path: PathBuf::new(),
            dir_childs: Vec::new(),
            files_childs: Vec::new(),
            lba: 0,
            continuation_area: None,
        })
    }
}
