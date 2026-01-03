use std::string::FromUtf8Error;

use bytes::Bytes;

use crate::core::{constants, opcodes, symbol};

type ParseOptions = u32;

pub const SKIP_CODE: ParseOptions = 1; // 0b0001
pub const SKIP_DEBUG: ParseOptions = 2; // 0b0010
pub const SKIP_FRAMES: ParseOptions = 4; // 0b0100
pub const EXPAND_FRAMES: ParseOptions = 8; // 0b1000

pub struct ConstantDynamic {}

pub struct ClassReader {
    /// The offset in bytes of the ClassFile's access_flags field.
    pub header: usize,

    // A byte array containing the JVMS ClassFile structure to be parsed. **The content of this array
    // must not be modified. This field is intended for {@link Attribute} sub classes, and is normally
    // not needed by class visitors.**
    class_file_buffer: Bytes,
    cp_info_offsets: Vec<usize>,
    constant_utf8_values: Vec<Option<String>>,
    constant_dynamic_values: Option<Vec<ConstantDynamic>>,
    bootstrap_method_offsets: Option<Vec<usize>>,
    max_string_length: usize,
}

impl ClassReader {
    pub fn parse(
        class_file_buffer: Bytes,
        class_file_offset: usize,
        check_class_version: bool,
    ) -> Result<Self, ClassParseError> {
        // Check the class' major_version. This field is after the magic and minor_version fields,
        // which use 4 and 2 bytes respectively.
        let class_file_version = read_short(&class_file_buffer, class_file_offset + 6)?;
        if check_class_version && class_file_version as i32 > opcodes::V27 {
            return Err(ClassParseError::UnsupportedClassVersion(class_file_version));
        }

        // Create the constant pool arrays. The constant_pool_count field is after the magic,
        // minor_version and major_version fields, which use 4, 2 and 2 bytes respectively.
        let constant_pool_count =
            read_unsigned_short(&class_file_buffer, class_file_offset + 8)? as usize;

        let mut cp_info_offsets = vec![0; constant_pool_count];
        let mut constant_utf8_values = vec![None; constant_pool_count];

        let mut current_cp_info_offset = class_file_offset + 10;
        let mut current_max_string_length = 0;
        let mut has_bootstrap_methods = false;
        let mut has_constant_dynamic = false;

        let mut current_cp_info_index = 1;

        while current_cp_info_index < constant_pool_count {
            cp_info_offsets[current_cp_info_index] = current_cp_info_offset + 1;

            let cp_info_size: usize;

            match class_file_buffer
                .get(current_cp_info_offset)
                .ok_or(ClassParseError::OutOfBounds)?
                .to_owned()
            {
                symbol::CONSTANT_FIELDREF_TAG
                | symbol::CONSTANT_METHODREF_TAG
                | symbol::CONSTANT_INTERFACE_METHODREF_TAG
                | symbol::CONSTANT_INTEGER_TAG
                | symbol::CONSTANT_FLOAT_TAG
                | symbol::CONSTANT_NAME_AND_TYPE_TAG => {
                    cp_info_size = 5;
                }

                symbol::CONSTANT_DYNAMIC_TAG => {
                    cp_info_size = 5;
                    has_bootstrap_methods = true;
                    has_constant_dynamic = true;
                }
                symbol::CONSTANT_INVOKE_DYNAMIC_TAG => {
                    cp_info_size = 5;
                    has_bootstrap_methods = true;
                }
                symbol::CONSTANT_LONG_TAG | symbol::CONSTANT_DOUBLE_TAG => {
                    cp_info_size = 9;
                    current_cp_info_index += 1;
                }
                symbol::CONSTANT_UTF8_TAG => {
                    // Assuming `read_unsigned_short` is a method on `self` and returns a u16.
                    // The result is cast to usize for arithmetic with cp_info_size.
                    cp_info_size =
                        3 + read_unsigned_short(&class_file_buffer, current_cp_info_offset + 1)?
                            as usize;
                    if cp_info_size > current_max_string_length {
                        // The size in bytes of this CONSTANT_Utf8 structure provides a conservative
                        // estimate of the length in characters of the corresponding string, and is
                        // much cheaper to compute than this exact length.
                        current_max_string_length = cp_info_size;
                    }
                }
                symbol::CONSTANT_METHOD_HANDLE_TAG => {
                    cp_info_size = 4;
                }
                symbol::CONSTANT_CLASS_TAG
                | symbol::CONSTANT_STRING_TAG
                | symbol::CONSTANT_METHOD_TYPE_TAG
                | symbol::CONSTANT_PACKAGE_TAG
                | symbol::CONSTANT_MODULE_TAG => {
                    cp_info_size = 3;
                }
                unknown => return Err(ClassParseError::BadConstantPoolInfo(unknown)),
            }

            current_cp_info_offset += cp_info_size;
            current_cp_info_index += 1;
        }

        let constant_dynamic_values = if has_constant_dynamic {
            Some(Vec::with_capacity(constant_pool_count))
        } else {
            None
        };

        let bootstrap_method_offsets = if has_bootstrap_methods {
            Some(read_bootstrap_method_attribute(
                &class_file_buffer,
                current_cp_info_offset,
                &mut constant_utf8_values,
                &cp_info_offsets,
            )?)
        } else {
            None
        };

        Ok(Self {
            header: current_cp_info_offset,
            class_file_buffer,
            cp_info_offsets,
            constant_utf8_values,
            constant_dynamic_values,
            bootstrap_method_offsets,
            max_string_length: current_max_string_length,
        })
    }

    pub fn get_access(&self) -> Result<u16, ClassParseError> {
        read_unsigned_short(&self.class_file_buffer, self.header)
    }
}

fn read_short(buffer: &Bytes, offset: usize) -> Result<i16, ClassParseError> {
    if offset + 1 >= buffer.len() {
        return Err(ClassParseError::OutOfBounds);
    }

    let byte1 = buffer[offset];
    let byte2 = buffer[offset + 1];

    Ok(i16::from_be_bytes([byte1, byte2]))
}

fn read_bootstrap_method_attribute(
    buffer: &Bytes,
    header: usize,
    constant_utf8_values: &mut Vec<Option<String>>,
    cp_info_offsets: &Vec<usize>,
) -> Result<Vec<usize>, ClassParseError> {
    let mut current_attr_offset = get_first_attribute_offset(buffer, header)?;

    for _ in (1..=read_unsigned_short(buffer, current_attr_offset - 2)?).rev() {
        let attribute_name = read_utf8(
            buffer,
            constant_utf8_values,
            cp_info_offsets,
            current_attr_offset,
        )?;
        let attribute_length = read_int(buffer, current_attr_offset + 2)? as usize;

        current_attr_offset += 6;
        if Some(constants::BOOTSTRAP_METHODS.to_string()) == attribute_name {
            let mut result: Vec<usize> =
                Vec::with_capacity(read_unsigned_short(buffer, current_attr_offset)? as usize);
            let mut current_bootstrap_method_offset = current_attr_offset + 2;
            for _ in 0..result.capacity() {
                result.push(current_bootstrap_method_offset);

                current_bootstrap_method_offset +=
                    4 + read_unsigned_short(buffer, current_bootstrap_method_offset + 2)? as usize
                        * 2;
            }
            return Ok(result);
        }

        current_attr_offset += attribute_length;
    }

    Err(ClassParseError::NoBootstrapMethods)
}

fn get_first_attribute_offset(buffer: &Bytes, header: usize) -> Result<usize, ClassParseError> {
    // Skip the access_flags, this_class, super_class, and interfaces_count fields (using 2 bytes
    // each), as well as the interfaces array field (2 bytes per interface).
    let mut current_offset = header + 8 + read_unsigned_short(buffer, header + 6)? as usize * 2;

    // Read the fields count field
    let fields_count = read_unsigned_short(buffer, current_offset)?;
    current_offset += 2;

    for _ in (1..=fields_count).rev() {
        let attributes_count = read_unsigned_short(buffer, current_offset + 6)?;
        current_offset += 8;
        for _ in (1..=attributes_count).rev() {
            // Invariant: currentOffset is the offset of an attribute_info structure.
            // Read the attribute_length field (2 bytes after the start of the attribute_info) and skip
            // this many bytes, plus 6 for the attribute_name_index and attribute_length fields
            // (yielding the total size of the attribute_info structure).
            current_offset += 6 + read_int(buffer, current_offset + 2)? as usize;
        }
    }

    // Skip the methods_count and 'methods' fields, using the same method as above.
    let methods_count = read_unsigned_short(buffer, current_offset)?;
    current_offset += 2;

    for _ in (1..=methods_count).rev() {
        let attributes_count = read_unsigned_short(buffer, current_offset + 6)?;
        current_offset += 8;
        for _ in (1..=attributes_count).rev() {
            current_offset += 6 + read_int(buffer, current_offset + 2)? as usize;
        }
    }

    // Skip the ClassFile's attributes_count field.
    Ok(current_offset + 2)
}

/// Reads a UTF-8 string from the constant pool, given an offset in the
/// buffer that points to the string's constant pool index.
///
/// Corresponds to the Java `readUTF8` method.
pub fn read_utf8(
    buffer: &Bytes,
    constant_utf8_values: &mut [Option<String>],
    cp_info_offsets: &[usize],
    offset: usize,
) -> Result<Option<String>, ClassParseError> {
    let constant_pool_entry_index = read_unsigned_short(buffer, offset)? as usize;
    if offset == 0 || constant_pool_entry_index == 0 {
        // A zero index is a convention for "no string" or null.
        return Ok(None);
    }
    // The Java code would return a reference, but to avoid complex lifetime issues
    // with the cache, we clone the string. This is a common and reasonable tradeoff.
    let value = read_utf(
        buffer,
        constant_utf8_values,
        cp_info_offsets,
        constant_pool_entry_index,
    )?;
    Ok(Some(value))
}

/// Reads a CONSTANT_Utf8 constant pool entry.
///
/// This method uses a cache to avoid decoding the same string multiple times.
///
/// Corresponds to the Java `readUtf` method.
fn read_utf(
    buffer: &Bytes,
    constant_utf8_values: &mut [Option<String>],
    cp_info_offsets: &[usize],
    constant_pool_entry_index: usize,
) -> Result<String, ClassParseError> {
    // 1. Check the cache first.
    if let Some(Some(value)) = constant_utf8_values.get(constant_pool_entry_index) {
        // The string is already in the cache, return a clone of it.
        return Ok(value.clone());
    }

    // 2. The string is not in the cache, so we need to decode it.
    // Get the offset of the CONSTANT_Utf8_info structure in the class file.
    let cp_info_offset = *cp_info_offsets
        .get(constant_pool_entry_index)
        .ok_or(ClassParseError::InvalidConstantPoolIndex)?;

    // The structure is: u1 tag, u2 length, u1[] bytes.
    // We read the length from the `cp_info_offset`. The tag is assumed to be correct.
    let len = read_unsigned_short(buffer, cp_info_offset)? as usize;
    // The actual byte data starts 2 bytes after the offset (to skip the length field).
    let string_bytes_offset = cp_info_offset + 2;

    let value = read_utf_from_buffer(buffer, string_bytes_offset, len)?;

    // 3. Store the newly decoded string in the cache.
    if let Some(Some(cache_slot)) = constant_utf8_values.get_mut(constant_pool_entry_index) {
        *cache_slot = value.clone();
    } else {
        // This would indicate a bug where the cache and offset vectors are out of sync.
        return Err(ClassParseError::InvalidConstantPoolIndex);
    }

    Ok(value)
}

/// A helper to read the raw UTF-8 bytes from the buffer and decode them.
/// This is the part that does the actual work after the offset and length are known.
///
/// NOTE: Java class files use a "modified UTF-8" (MUTF-8) encoding, which is
/// different from standard UTF-8, especially in its handling of null characters (`\0`)
/// and supplementary characters (emojis, etc.). For a truly compliant parser, you would
/// need a custom MUTF-8 decoder here instead of `String::from_utf8`. For many common
/// strings, the standard decoder will work fine.
fn read_utf_from_buffer(
    buffer: &Bytes,
    offset: usize,
    len: usize,
) -> Result<String, ClassParseError> {
    let end = offset + len;
    let bytes = buffer
        .get(offset..end)
        .ok_or(ClassParseError::OutOfBounds)?;
    String::from_utf8(bytes.to_vec()).map_err(ClassParseError::InvalidUtf8)
}

pub fn read_unsigned_short(buffer: &[u8], offset: usize) -> Result<u16, ClassParseError> {
    if offset + 2 > buffer.len() {
        return Err(ClassParseError::OutOfBounds);
    }

    let bytes_slice = &buffer[offset..offset + 2];
    let bytes_array: [u8; 2] = bytes_slice.try_into().unwrap();

    Ok(u16::from_be_bytes(bytes_array))
}

pub fn read_int(buffer: &Bytes, offset: usize) -> Result<i32, ClassParseError> {
    if offset + 4 > buffer.len() {
        return Err(ClassParseError::OutOfBounds);
    }

    let bytes_slice = &buffer[offset..offset + 4];
    let bytes_array: [u8; 4] = bytes_slice.try_into().unwrap();

    Ok(i32::from_be_bytes(bytes_array))
}

#[derive(Debug, thiserror::Error)]
pub enum ClassParseError {
    #[error("Unsupported class file major version {0}")]
    UnsupportedClassVersion(i16),

    #[error("Bad cp info: {0}")]
    BadConstantPoolInfo(u8),

    #[error("No bootstrap methods inside the class")]
    NoBootstrapMethods,

    #[error("Invalid constant pool index")]
    InvalidConstantPoolIndex,

    #[error("Out of bounds")]
    OutOfBounds,

    #[error("Invalid Utf8")]
    InvalidUtf8(#[from] FromUtf8Error),
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use crate::core::{ClassReader, opcodes};

    #[test]
    fn recognise_access() {
        let class_bytes = include_bytes!("../../resources/Main.class");
        let cr = ClassReader::parse(Bytes::from_static(class_bytes), 0, true).unwrap();

        assert_eq!(
            cr.get_access().unwrap(),
            opcodes::ACC_PUBLIC | opcodes::ACC_SUPER
        );
    }
}
