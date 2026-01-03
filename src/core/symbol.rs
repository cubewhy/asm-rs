use std::cell::Cell;

// A stub for the `org.objectweb.asm.Type` class to allow the code to compile.
// In a real scenario, this would be a proper implementation.
mod r#type {
    pub(crate) fn get_arguments_and_return_sizes(_descriptor: &str) -> i32 {
        // This is a placeholder. A real implementation would parse the method descriptor.
        unimplemented!("Type::get_arguments_and_return_sizes is not implemented");
    }
}

/// An entry of the constant pool, of the BootstrapMethods attribute, or of the (ASM specific) type
/// table of a class.
///
/// See also:
/// * [JVMS 4.4](https://docs.oracle.com/javase/specs/jvms/se9/html/jvms-4.html#jvms-4.4)
/// * [JVMS 4.7.23](https://docs.oracle.com/javase/specs/jvms/se9/html/jvms-4.html#jvms-4.7.23)
///
/// Author: Eric Bruneton
pub struct Symbol {
    /// The index of this symbol in the constant pool, in the BootstrapMethods attribute, or in the
    /// (ASM specific) type table of a class (depending on the [`tag`] value).
    pub index: i32,

    /// A tag indicating the type of this symbol. Must be one of the static tag values defined in this
    /// class.
    pub tag: i32,

    /// The internal name of the owner class of this symbol. Only used for
    /// [`CONSTANT_FIELDREF_TAG`], [`CONSTANT_METHODREF_TAG`],
    /// [`CONSTANT_INTERFACE_METHODREF_TAG`], and [`CONSTANT_METHOD_HANDLE_TAG`] symbols.
    pub owner: Option<String>,

    /// The name of the class field or method corresponding to this symbol. Only used for
    /// [`CONSTANT_FIELDREF_TAG`], [`CONSTANT_METHODREF_TAG`],
    /// [`CONSTANT_INTERFACE_METHODREF_TAG`], [`CONSTANT_NAME_AND_TYPE_TAG`],
    /// [`CONSTANT_METHOD_HANDLE_TAG`], [`CONSTANT_DYNAMIC_TAG`] and
    /// [`CONSTANT_INVOKE_DYNAMIC_TAG`] symbols.
    pub name: Option<String>,

    /// The string value of this symbol. This is:
    ///
    /// *   a field or method descriptor for [`CONSTANT_FIELDREF_TAG`],
    ///     [`CONSTANT_METHODREF_TAG`], [`CONSTANT_INTERFACE_METHODREF_TAG`],
    ///     [`CONSTANT_NAME_AND_TYPE_TAG`], [`CONSTANT_METHOD_HANDLE_TAG`],
    ///     [`CONSTANT_METHOD_TYPE_TAG`], [`CONSTANT_DYNAMIC_TAG`] and
    ///     [`CONSTANT_INVOKE_DYNAMIC_TAG`] symbols,
    /// *   an arbitrary string for [`CONSTANT_UTF8_TAG`] and [`CONSTANT_STRING_TAG`]
    ///     symbols,
    /// *   an internal class name for [`CONSTANT_CLASS_TAG`], [`TYPE_TAG`],
    ///     [`UNINITIALIZED_TYPE_TAG`] and [`FORWARD_UNINITIALIZED_TYPE_TAG`] symbols,
    /// *   `None` for the other types of symbol.
    pub value: Option<String>,

    /// The numeric value of this symbol. This is:
    ///
    /// *   the symbol's value for [`CONSTANT_INTEGER_TAG`],[`CONSTANT_FLOAT_TAG`],
    ///     [`CONSTANT_LONG_TAG`], [`CONSTANT_DOUBLE_TAG`],
    /// *   the CONSTANT_MethodHandle_info reference_kind field value for
    ///     [`CONSTANT_METHOD_HANDLE_TAG`] symbols (or this value left shifted by 8 bits for
    ///     reference_kind values larger than or equal to H_INVOKEVIRTUAL and if the method owner is
    ///     an interface),
    /// *   the CONSTANT_InvokeDynamic_info bootstrap_method_attr_index field value for
    ///     [`CONSTANT_INVOKE_DYNAMIC_TAG`] symbols,
    /// *   the offset of a bootstrap method in the BootstrapMethods boostrap_methods array, for
    ///     [`CONSTANT_DYNAMIC_TAG`] or [`BOOTSTRAP_METHOD_TAG`] symbols,
    /// *   the bytecode offset of the NEW instruction that created an
    ///     `Frame#ITEM_UNINITIALIZED` type for [`UNINITIALIZED_TYPE_TAG`] symbols,
    /// *   the index of the `Label` (in the `SymbolTable#labelTable` table) of the NEW
    ///     instruction that created an `Frame#ITEM_UNINITIALIZED` type for
    ///     [`FORWARD_UNINITIALIZED_TYPE_TAG`] symbols,
    /// *   the indices (in the class' type table) of two [`TYPE_TAG`] source types for
    ///     [`MERGED_TYPE_TAG`] symbols,
    /// *   0 for the other types of symbol.
    pub data: i64,

    /// Additional information about this symbol, generally computed lazily. *Warning: the value of
    /// this field is ignored when comparing Symbol instances* (to avoid duplicate entries in a
    /// SymbolTable). Therefore, this field should only contain data that can be computed from the
    /// other fields of this class. It contains:
    ///
    /// *   the `Type#getArgumentsAndReturnSizes` of the symbol's method descriptor for
    ///     [`CONSTANT_METHODREF_TAG`], [`CONSTANT_INTERFACE_METHODREF_TAG`] and
    ///     [`CONSTANT_INVOKE_DYNAMIC_TAG`] symbols,
    /// *   the index in the InnerClasses_attribute 'classes' array (plus one) corresponding to this
    ///     class, for [`CONSTANT_CLASS_TAG`] symbols,
    /// *   the index (in the class' type table) of the merged type of the two source types for
    ///     [`MERGED_TYPE_TAG`] symbols,
    /// *   0 for the other types of symbol, or if this field has not been computed yet.
    pub info: Cell<i32>,
}

// Tag values for the constant pool entries (using the same order as in the JVMS).

/// The tag value of CONSTANT_Class_info JVMS structures.
pub const CONSTANT_CLASS_TAG: u8 = 7;

/// The tag value of CONSTANT_Fieldref_info JVMS structures.
pub const CONSTANT_FIELDREF_TAG: u8 = 9;

/// The tag value of CONSTANT_Methodref_info JVMS structures.
pub const CONSTANT_METHODREF_TAG: u8 = 10;

/// The tag value of CONSTANT_InterfaceMethodref_info JVMS structures.
pub const CONSTANT_INTERFACE_METHODREF_TAG: u8 = 11;

/// The tag value of CONSTANT_String_info JVMS structures.
pub const CONSTANT_STRING_TAG: u8 = 8;

/// The tag value of CONSTANT_Integer_info JVMS structures.
pub const CONSTANT_INTEGER_TAG: u8 = 3;

/// The tag value of CONSTANT_Float_info JVMS structures.
pub const CONSTANT_FLOAT_TAG: u8 = 4;

/// The tag value of CONSTANT_Long_info JVMS structures.
pub const CONSTANT_LONG_TAG: u8 = 5;

/// The tag value of CONSTANT_Double_info JVMS structures.
pub const CONSTANT_DOUBLE_TAG: u8 = 6;

/// The tag value of CONSTANT_NameAndType_info JVMS structures.
pub const CONSTANT_NAME_AND_TYPE_TAG: u8 = 12;

/// The tag value of CONSTANT_Utf8_info JVMS structures.
pub const CONSTANT_UTF8_TAG: u8 = 1;

/// The tag value of CONSTANT_MethodHandle_info JVMS structures.
pub const CONSTANT_METHOD_HANDLE_TAG: u8 = 15;

/// The tag value of CONSTANT_MethodType_info JVMS structures.
pub const CONSTANT_METHOD_TYPE_TAG: u8 = 16;

/// The tag value of CONSTANT_Dynamic_info JVMS structures.
pub const CONSTANT_DYNAMIC_TAG: u8 = 17;

/// The tag value of CONSTANT_InvokeDynamic_info JVMS structures.
pub const CONSTANT_INVOKE_DYNAMIC_TAG: u8 = 18;

/// The tag value of CONSTANT_Module_info JVMS structures.
pub const CONSTANT_MODULE_TAG: u8 = 19;

/// The tag value of CONSTANT_Package_info JVMS structures.
pub const CONSTANT_PACKAGE_TAG: u8 = 20;

// Tag values for the BootstrapMethods attribute entries (ASM specific tag).

/// The tag value of the BootstrapMethods attribute entries.
pub const BOOTSTRAP_METHOD_TAG: i32 = 64;

// Tag values for the type table entries (ASM specific tags).

/// The tag value of a normal type entry in the (ASM specific) type table of a class.
pub const TYPE_TAG: i32 = 128;

/// The tag value of an uninitialized type entry in the type table of a class. This type is used
/// for the normal case where the NEW instruction is before the `<init>` constructor call (in
/// bytecode offset order), i.e. when the label of the NEW instruction is resolved when the
/// constructor call is visited. If the NEW instruction is after the constructor call, use the
/// [`FORWARD_UNINITIALIZED_TYPE_TAG`] tag value instead.
pub const UNINITIALIZED_TYPE_TAG: i32 = 129;

/// The tag value of an uninitialized type entry in the type table of a class. This type is used
/// for the unusual case where the NEW instruction is after the `<init>` constructor call (in
/// bytecode offset order), i.e. when the label of the NEW instruction is not resolved when the
/// constructor call is visited. If the NEW instruction is before the constructor call, use the
/// [`UNINITIALIZED_TYPE_TAG`] tag value instead.
pub const FORWARD_UNINITIALIZED_TYPE_TAG: i32 = 130;

/// The tag value of a merged type entry in the (ASM specific) type table of a class.
pub const MERGED_TYPE_TAG: i32 = 131;

impl Symbol {
    /// Constructs a new Symbol. This constructor can't be used directly because the Symbol class is
    /// abstract. Instead, use the factory methods of the `SymbolTable` class.
    ///
    /// # Arguments
    ///
    /// * `index` - the symbol index in the constant pool, in the BootstrapMethods attribute, or in
    ///   the (ASM specific) type table of a class (depending on 'tag').
    /// * `tag` - the symbol type. Must be one of the static tag values defined in this class.
    /// * `owner` - The internal name of the symbol's owner class. Maybe `None`.
    /// * `name` - The name of the symbol's corresponding class field or method. Maybe `None`.
    /// * `value` - The string value of this symbol. Maybe `None`.
    /// * `data` - The numeric value of this symbol.
    pub(crate) fn new(
        index: i32,
        tag: i32,
        owner: Option<String>,
        name: Option<String>,
        value: Option<String>,
        data: i64,
    ) -> Self {
        Symbol {
            index,
            tag,
            owner,
            name,
            value,
            data,
            info: Cell::new(0),
        }
    }

    /// Returns the result of `type::get_arguments_and_return_sizes` on `value`.
    ///
    /// Returns the result of `type::get_arguments_and_return_sizes` on `value` (memoized in
    /// `info` for efficiency). This should only be used for
    /// [`CONSTANT_METHODREF_TAG`], [`CONSTANT_INTERFACE_METHODREF_TAG`] and
    /// [`CONSTANT_INVOKE_DYNAMIC_TAG`] symbols.
    pub fn get_arguments_and_return_sizes(&self) -> i32 {
        if self.info.get() == 0 {
            let info_value = r#type::get_arguments_and_return_sizes(self.value.as_ref().unwrap());
            self.info.set(info_value);
        }
        self.info.get()
    }
}
