use byteorder::WriteBytesExt;
use std::io::Write;

pub const LOGIC_SIZE: usize = 0x800;
pub const LOGIC_SIZE_I64: i64 = 0x800;
pub const LOGIC_SIZE_U32: u32 = 0x800;
pub const SECTOR_SIZE: u32 = 0x200;
pub const LOGIC_SIZE_U16: u16 = 0x800;

pub fn align_up(value: i32, padding: i32) -> i32 {
    (value + (padding - 1)) & -padding
}

pub fn convert_name(value: &str) -> Vec<u8> {
    let res: Vec<&str> = value.split('.').collect();

    let file_name: &str = res[0];
    let file_truncated_size = if file_name.len() >= 8 {
        8
    } else {
        file_name.len()
    };

    let extension: &str = res.get(1).unwrap_or(&"");
    let extension_truncated_size = if extension.len() > 3 {
        3
    } else {
        extension.len()
    };

    let mut result = String::from(&file_name[0..file_truncated_size]);
    if extension_truncated_size != 0 {
        result = result + "." + &extension[0..extension_truncated_size];
    }

    result.into_bytes()
}

pub fn get_entry_size(
    base_size: u32,
    file_name: &str,
    directory_type: u32,
    padding_type: usize,
) -> u32 {
    let file_name_len = file_name.len();
    if file_name_len > 251 {
        panic!(
            "File name \"{}\" is too big (max size is 251 bytes)",
            file_name
        );
    }

    let file_name_corrected = convert_name(file_name);
    let file_identifier = match directory_type {
        1 => &[0u8],
        2 => &[1u8],
        3 => &[0u8],
        5 => &[0u8],
        _ => &file_name_corrected[..],
    };

    let mut file_identifier_len = file_identifier.len();
    let mut system_use_field_size = 0;

    if file_identifier_len % 2 != padding_type {
        file_identifier_len += 1;
    }

    // is not a path entry calculation
    if directory_type < 5 {
        // Rock Ridge 'PX' entry
        system_use_field_size += 0x2c;
    }

    // regular directory/file
    if directory_type == 0 {
        // Rock Ridge 'NM' entry
        system_use_field_size += 0x5;
        system_use_field_size += file_name_len as u32;
    }

    // root '.' has CE and SP of SUSP
    if directory_type == 3 {
        system_use_field_size += 0x7 + 0x1c; // SUSP 'SP' + SUSP 'CE'
    }

    base_size + file_identifier_len as u32 + system_use_field_size
}

pub fn write_lba_to_cls<T>(
    output_writter: &mut T,
    disk_lba: u32,
    head_count: u32,
    sector_count: u32,
) -> std::io::Result<()>
where
    T: Write,
{
    let mut sector_number = (disk_lba % sector_count) + 1;
    let tmp = disk_lba / sector_count;
    let mut head_number = tmp % head_count;
    let mut cylinder_number = tmp / head_count;

    if cylinder_number > 0x400 {
        cylinder_number = 0x3FF;
        head_number = head_count;
        sector_number = sector_count;
    }

    sector_number |= (cylinder_number & 0x300) >> 2;
    cylinder_number &= 0xFF;

    output_writter.write_u8(head_number as u8)?;
    output_writter.write_u8(sector_number as u8)?;
    output_writter.write_u8(cylinder_number as u8)?;

    Ok(())
}

macro_rules! write_bothendian {
    ($($writer:ident . $write_fn:ident($value:expr)?;)*) => {
        $($writer.$write_fn::<LittleEndian>($value)?;)*
        $($writer.$write_fn::<BigEndian>($value)?;)*
    }
}
