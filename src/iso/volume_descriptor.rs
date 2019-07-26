use byteorder::{BigEndian, LittleEndian, WriteBytesExt};
use chrono::prelude::*;

use crate::iso::directory_entry::DirectoryEntry;
use crate::iso::file_entry::FileEntry;
use crate::iso::utils::LOGIC_SIZE_U16;

use std;
use std::io::prelude::*;

#[allow(dead_code)]
#[derive(Debug)]
pub enum VolumeDescriptor {
    Boot,
    Primary,
    Supplementary,
    Volume,
    End,
}

impl VolumeDescriptor {
    fn get_type_id(&self) -> u8 {
        match self {
            VolumeDescriptor::Boot => 0,
            VolumeDescriptor::Primary => 1,
            VolumeDescriptor::Supplementary => 2,
            VolumeDescriptor::Volume => 3,
            VolumeDescriptor::End => 0xff,
        }
    }

    fn write_volume_header<T>(&mut self, output_writter: &mut T) -> std::io::Result<()>
    where
        T: Write,
    {
        let type_id = self.get_type_id();
        output_writter.write_u8(type_id)?;
        output_writter.write_all(b"CD001")?;
        output_writter.write_u8(0x1)?;
        Ok(())
    }

    pub fn write_volume<T>(
        &mut self,
        output_writter: &mut T,
        root_dir: &mut DirectoryEntry,
        path_table_start_lba: u32,
        size_in_lb: u32,
    ) -> std::io::Result<()>
    where
        T: Write + Seek,
    {
        self.write_volume_header(output_writter)?;

        match self {
            VolumeDescriptor::Boot => {
                // TODO: write it correctly
                output_writter.write_all(b"EL TORITO SPECIFICATION")?;

                let catalog_file: &FileEntry = root_dir.get_file("boot.catalog").unwrap();

                let empty_data: [u8; 0x29] = [0; 0x29];
                output_writter.write_all(&empty_data)?;

                output_writter.write_u32::<LittleEndian>(catalog_file.lba)?;

                let empty_data_2: [u8; 0x7b5] = [0; 0x7b5];
                output_writter.write_all(&empty_data_2)?;
            }
            VolumeDescriptor::Primary => {
                output_writter.write_u8(0)?;

                let system_identifier: [u8; 32] = [0x20; 32];
                output_writter.write_all(&system_identifier)?;

                output_writter.write_all(b"ISOIMAGE                        ")?;
                output_writter.write_u64::<LittleEndian>(0)?;

                // Size of the volume in LB
                write_bothendian! {
                    output_writter.write_u32(size_in_lb)?;
                }

                let zero_b32: [u8; 32] = [0; 32];
                output_writter.write_all(&zero_b32)?;

                // Disc count
                write_bothendian! {
                    output_writter.write_u16(1)?;
                }

                // Disc id
                write_bothendian! {
                    output_writter.write_u16(1)?;
                }

                // logic size: 2KB
                write_bothendian! {
                    output_writter.write_u16(LOGIC_SIZE_U16)?;
                }

                let path_table_size = root_dir.get_path_table_size();
                write_bothendian! {
                    output_writter.write_u32(path_table_size)?;
                }

                // path table location (in lba)
                let path_table_lba_le = path_table_start_lba; // System Area + Primary + End
                let path_table_lba_be = path_table_start_lba + 2; // System Area + Primary + End + Path Table LE + Spacing

                output_writter.write_u32::<LittleEndian>(path_table_lba_le)?;
                output_writter.write_u32::<LittleEndian>(0)?;
                output_writter.write_u32::<BigEndian>(path_table_lba_be)?;
                output_writter.write_u32::<BigEndian>(0)?;

                root_dir.write_as_current(output_writter, 5)?;

                let volume_set_identifier: [u8; 128] = [0x20; 128];
                let publisher_identifier: [u8; 128] = [0x20; 128];
                let data_preparer_identifier: [u8; 128] = [0x20; 128];
                let application_identifier: [u8; 128] = [0x20; 128];
                output_writter.write_all(&volume_set_identifier)?;
                output_writter.write_all(&publisher_identifier)?;
                output_writter.write_all(&data_preparer_identifier)?;
                output_writter.write_all(&application_identifier)?;

                let copyright_file_identifier: [u8; 38] = [0x20; 38];
                let abstract_file_identifier: [u8; 36] = [0x20; 36];
                let bibliographic_file_identifier: [u8; 37] = [0x20; 37];
                output_writter.write_all(&copyright_file_identifier)?;
                output_writter.write_all(&abstract_file_identifier)?;
                output_writter.write_all(&bibliographic_file_identifier)?;

                let utc: DateTime<Utc> = Utc::now();
                let creation_time: String = utc.format("%Y%m%d%H%M%S00").to_string();
                let expiration_time: [u8; 16] = [0x30; 16];

                output_writter.write_all(creation_time.as_bytes())?;
                output_writter.write_u8(0)?;
                output_writter.write_all(creation_time.as_bytes())?;
                output_writter.write_u8(0)?;
                output_writter.write_all(&expiration_time)?;
                output_writter.write_u8(0)?;
                output_writter.write_all(&expiration_time)?;
                output_writter.write_u8(0)?;

                // File structure version
                output_writter.write_u8(0x1)?;

                output_writter.write_u8(0x0)?;

                let application_used: [u8; 512] = [0x20; 512];
                output_writter.write_all(&application_used)?;

                let reserved: [u8; 653] = [0x0; 653];
                output_writter.write_all(&reserved)?;
            }
            VolumeDescriptor::End => {
                let empty_data: [u8; 2041] = [0; 2041];
                output_writter.write_all(&empty_data)?;
            }
            _ => unimplemented!(),
        }
        Ok(())
    }
}
