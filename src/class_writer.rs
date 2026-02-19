use std::collections::HashMap;

use crate::class_reader::{
    AttributeInfo, BootstrapMethod, CodeAttribute, CpInfo, ExceptionTableEntry, InnerClass,
    LineNumber, LocalVariable, MethodParameter, StackMapFrame, VerificationTypeInfo,
};
use crate::constants;
use crate::error::ClassWriteError;
use crate::insn::{
    AbstractInsnNode, FieldInsnNode, Insn, InsnNode, JumpInsnNode, JumpLabelInsnNode, Label,
    LabelNode, LdcInsnNode, LdcValue, LineNumberInsnNode, MemberRef, MethodInsnNode, NodeList,
    VarInsnNode,
};
use crate::nodes::{ClassNode, FieldNode, InnerClassNode, MethodNode};
use crate::opcodes;

/// Flag to automatically compute the stack map frames.
///
/// When this flag is passed to the `ClassWriter`, it calculates the `StackMapTable`
/// attribute based on the bytecode instructions. This requires the `compute_maxs` logic as well.
pub const COMPUTE_FRAMES: u32 = 0x1;

/// Flag to automatically compute the maximum stack size and local variables.
///
/// When this flag is set, the writer will calculate `max_stack` and `max_locals`
/// for methods, ignoring the values provided in `visit_maxs`.
pub const COMPUTE_MAXS: u32 = 0x2;

/// A builder for the constant pool of a class.
///
/// This struct manages the deduplication of constant pool entries, ensuring that
/// strings, classes, and member references are stored efficiently.
#[derive(Debug, Default)]
pub struct ConstantPoolBuilder {
    cp: Vec<CpInfo>,
    utf8: HashMap<String, u16>,
    class: HashMap<String, u16>,
    string: HashMap<String, u16>,
    name_and_type: HashMap<(String, String), u16>,
    field_ref: HashMap<(String, String, String), u16>,
    method_ref: HashMap<(String, String, String), u16>,
}

impl ConstantPoolBuilder {
    /// Creates a new, empty `ConstantPoolBuilder`.
    ///
    /// The constant pool starts with a dummy entry at index 0, as per JVM spec.
    pub fn new() -> Self {
        Self {
            cp: vec![CpInfo::Unusable],
            ..Default::default()
        }
    }

    /// Consumes the builder and returns the raw vector of `CpInfo` entries.
    pub fn into_pool(self) -> Vec<CpInfo> {
        self.cp
    }

    /// Adds a UTF-8 string to the constant pool if it doesn't exist.
    ///
    /// Returns the index of the entry.
    pub fn utf8(&mut self, value: &str) -> u16 {
        if let Some(index) = self.utf8.get(value) {
            return *index;
        }
        let index = self.push(CpInfo::Utf8(value.to_string()));
        self.utf8.insert(value.to_string(), index);
        index
    }

    /// Adds a Class constant to the pool.
    ///
    /// This will recursively add the UTF-8 name of the class.
    pub fn class(&mut self, name: &str) -> u16 {
        if let Some(index) = self.class.get(name) {
            return *index;
        }
        let name_index = self.utf8(name);
        let index = self.push(CpInfo::Class { name_index });
        self.class.insert(name.to_string(), index);
        index
    }

    /// Adds a String constant to the pool.
    ///
    /// This is for string literals (e.g., `ldc "foo"`).
    pub fn string(&mut self, value: &str) -> u16 {
        if let Some(index) = self.string.get(value) {
            return *index;
        }
        let string_index = self.utf8(value);
        let index = self.push(CpInfo::String { string_index });
        self.string.insert(value.to_string(), index);
        index
    }

    pub fn integer(&mut self, value: i32) -> u16 {
        let index = self.push(CpInfo::Integer(value));
        index
    }

    pub fn float(&mut self, value: f32) -> u16 {
        let index = self.push(CpInfo::Float(value));
        index
    }

    pub fn long(&mut self, value: i64) -> u16 {
        let index = self.push(CpInfo::Long(value));
        // Long takes two entries
        self.cp.push(CpInfo::Unusable);
        index
    }

    pub fn double(&mut self, value: f64) -> u16 {
        let index = self.push(CpInfo::Double(value));
        // Double takes two entries
        self.cp.push(CpInfo::Unusable);
        index
    }

    /// Adds a NameAndType constant to the pool.
    ///
    /// Used for field and method descriptors.
    pub fn name_and_type(&mut self, name: &str, descriptor: &str) -> u16 {
        let key = (name.to_string(), descriptor.to_string());
        if let Some(index) = self.name_and_type.get(&key) {
            return *index;
        }
        let name_index = self.utf8(name);
        let descriptor_index = self.utf8(descriptor);
        let index = self.push(CpInfo::NameAndType {
            name_index,
            descriptor_index,
        });
        self.name_and_type.insert(key, index);
        index
    }

    /// Adds a Fieldref constant to the pool.
    pub fn field_ref(&mut self, owner: &str, name: &str, descriptor: &str) -> u16 {
        let key = (owner.to_string(), name.to_string(), descriptor.to_string());
        if let Some(index) = self.field_ref.get(&key) {
            return *index;
        }
        let class_index = self.class(owner);
        let name_and_type_index = self.name_and_type(name, descriptor);
        let index = self.push(CpInfo::Fieldref {
            class_index,
            name_and_type_index,
        });
        self.field_ref.insert(key, index);
        index
    }

    /// Adds a Methodref constant to the pool.
    pub fn method_ref(&mut self, owner: &str, name: &str, descriptor: &str) -> u16 {
        let key = (owner.to_string(), name.to_string(), descriptor.to_string());
        if let Some(index) = self.method_ref.get(&key) {
            return *index;
        }
        let class_index = self.class(owner);
        let name_and_type_index = self.name_and_type(name, descriptor);
        let index = self.push(CpInfo::Methodref {
            class_index,
            name_and_type_index,
        });
        self.method_ref.insert(key, index);
        index
    }

    fn push(&mut self, entry: CpInfo) -> u16 {
        self.cp.push(entry);
        (self.cp.len() - 1) as u16
    }
}

struct FieldData {
    access_flags: u16,
    name: String,
    descriptor: String,
    attributes: Vec<AttributeInfo>,
}

struct MethodData {
    access_flags: u16,
    name: String,
    descriptor: String,
    code: Option<CodeAttribute>,
    attributes: Vec<AttributeInfo>,
}

/// A writer that generates a Java Class File structure.
///
/// This is the main entry point for creating class files programmatically.
/// It allows visiting the class header, fields, methods, and attributes.
///
/// # Example
///
/// ```rust
/// use rust_asm::{class_writer::{ClassWriter, COMPUTE_FRAMES}, opcodes};
///
/// let mut cw = ClassWriter::new(COMPUTE_FRAMES);
/// cw.visit(52, 0, 1, "com/example/MyClass", Some("java/lang/Object"), &[]);
///
/// let mut mv = cw.visit_method(1, "myMethod", "()V");
/// mv.visit_code();
/// mv.visit_insn(opcodes::RETURN);
/// mv.visit_maxs(0, 0); // Computed automatically due to COMPUTE_FRAMES
///
/// let bytes = cw.to_bytes().unwrap();
/// ```
pub struct ClassWriter {
    options: u32,
    minor_version: u16,
    major_version: u16,
    access_flags: u16,
    name: String,
    super_name: Option<String>,
    interfaces: Vec<String>,
    fields: Vec<FieldData>,
    methods: Vec<MethodData>,
    attributes: Vec<AttributeInfo>,
    source_file: Option<String>,
    cp: ConstantPoolBuilder,
}

impl ClassWriter {
    /// Creates a new `ClassWriter`.
    ///
    /// # Arguments
    ///
    /// * `options` - Bitwise flags to control generation (e.g., `COMPUTE_FRAMES`, `COMPUTE_MAXS`).
    pub fn new(options: u32) -> Self {
        Self {
            options,
            minor_version: 0,
            major_version: 52,
            access_flags: 0,
            name: String::new(),
            super_name: None,
            interfaces: Vec::new(),
            fields: Vec::new(),
            methods: Vec::new(),
            attributes: Vec::new(),
            source_file: None,
            cp: ConstantPoolBuilder::new(),
        }
    }

    /// Defines the header of the class.
    ///
    /// # Arguments
    ///
    /// * `major` - The major version (e.g., 52 for Java 8).
    /// * `minor` - The minor version.
    /// * `access_flags` - Access modifiers (e.g., public, final).
    /// * `name` - The internal name of the class (e.g., "java/lang/String").
    /// * `super_name` - The internal name of the super class (e.g., `java/lang/String`, `a/b/c`).
    ///   Use `None` for `Object`.
    /// * `interfaces` - A list of interfaces implemented by this class.
    pub fn visit(
        &mut self,
        major: u16,
        minor: u16,
        access_flags: u16,
        name: &str,
        super_name: Option<&str>,
        interfaces: &[&str],
    ) -> &mut Self {
        self.major_version = major;
        self.minor_version = minor;
        self.access_flags = access_flags;
        self.name = name.to_string();
        self.super_name = super_name.map(|value| value.to_string());
        self.interfaces = interfaces
            .iter()
            .map(|value| (*value).to_string())
            .collect();
        self
    }

    /// Sets the source file name attribute for the class.
    pub fn visit_source_file(&mut self, name: &str) -> &mut Self {
        self.source_file = Some(name.to_string());
        self
    }

    /// Adds an `InnerClasses` entry for this class.
    ///
    /// This encodes the entry into the `InnerClasses` attribute using constant pool indices.
    pub fn visit_inner_class(
        &mut self,
        name: &str,
        outer_name: Option<&str>,
        inner_name: Option<&str>,
        access_flags: u16,
    ) -> &mut Self {
        let inner_class_info_index = self.cp.class(name);
        let outer_class_info_index = match outer_name {
            Some(value) => self.cp.class(value),
            None => 0,
        };
        let inner_name_index = match inner_name {
            Some(value) => self.cp.utf8(value),
            None => 0,
        };
        let entry = InnerClass {
            inner_class_info_index,
            outer_class_info_index,
            inner_name_index,
            inner_class_access_flags: access_flags,
        };

        for attr in &mut self.attributes {
            if let AttributeInfo::InnerClasses { classes } = attr {
                classes.push(entry);
                return self;
            }
        }

        self.attributes.push(AttributeInfo::InnerClasses {
            classes: vec![entry],
        });
        self
    }

    /// Visits a method of the class.
    ///
    /// Returns a `MethodVisitor` that should be used to define the method body.
    /// The `visit_end` method of the returned visitor must be called to attach it to the class.
    pub fn visit_method(
        &mut self,
        access_flags: u16,
        name: &str,
        descriptor: &str,
    ) -> MethodVisitor {
        MethodVisitor::new(access_flags, name, descriptor)
    }

    /// Visits a field of the class.
    ///
    /// Returns a `FieldVisitor` to define field attributes.
    /// If `visit_end` is not called, the field is still committed when the visitor is dropped.
    pub fn visit_field(&mut self, access_flags: u16, name: &str, descriptor: &str) -> FieldVisitor {
        FieldVisitor::new(access_flags, name, descriptor, self as *mut ClassWriter)
    }

    /// Adds a custom attribute to the class.
    pub fn add_attribute(&mut self, attr: AttributeInfo) -> &mut Self {
        self.attributes.push(attr);
        self
    }

    /// Converts the builder state into a `ClassNode` object model.
    pub fn to_class_node(mut self) -> Result<ClassNode, String> {
        if self.name.is_empty() {
            return Err("missing class name, call visit() first".to_string());
        }

        let this_class = self.cp.class(&self.name);
        if let Some(name) = self.super_name.as_deref() {
            self.cp.class(name);
        }

        let mut interface_indices = Vec::with_capacity(self.interfaces.len());
        for name in &self.interfaces {
            interface_indices.push(self.cp.class(name));
        }

        let mut fields = Vec::with_capacity(self.fields.len());
        for field in self.fields {
            let name_index = self.cp.utf8(&field.name);
            let descriptor_index = self.cp.utf8(&field.descriptor);
            fields.push(FieldNode {
                access_flags: field.access_flags,
                name_index,
                descriptor_index,
                name: field.name,
                descriptor: field.descriptor,
                attributes: field.attributes,
            });
        }

        let mut methods = Vec::with_capacity(self.methods.len());
        for method in self.methods {
            let name_index = self.cp.utf8(&method.name);
            let descriptor_index = self.cp.utf8(&method.descriptor);
            methods.push(MethodNode {
                access_flags: method.access_flags,
                name_index,
                descriptor_index,
                name: method.name,
                descriptor: method.descriptor,
                code: method.code,
                attributes: method.attributes,
            });
        }

        if let Some(source_name) = self.source_file.as_ref() {
            let source_index = self.cp.utf8(source_name);
            self.attributes.push(AttributeInfo::SourceFile {
                sourcefile_index: source_index,
            });
        }

        let constant_pool = self.cp.into_pool();

        fn cp_utf8<'a>(cp: &'a [CpInfo], index: u16) -> Result<&'a str, String> {
            match cp.get(index as usize) {
                Some(CpInfo::Utf8(value)) => Ok(value.as_str()),
                _ => Err(format!("invalid constant pool utf8 index {}", index)),
            }
        }
        fn class_name<'a>(cp: &'a [CpInfo], index: u16) -> Result<&'a str, String> {
            match cp.get(index as usize) {
                Some(CpInfo::Class { name_index }) => cp_utf8(cp, *name_index),
                _ => Err(format!("invalid constant pool class index {}", index)),
            }
        }

        let mut inner_classes = Vec::new();
        for attr in &self.attributes {
            if let AttributeInfo::InnerClasses { classes } = attr {
                for entry in classes {
                    let name = class_name(&constant_pool, entry.inner_class_info_index)?.to_string();
                    let outer_name = if entry.outer_class_info_index == 0 {
                        None
                    } else {
                        Some(class_name(&constant_pool, entry.outer_class_info_index)?.to_string())
                    };
                    let inner_name = if entry.inner_name_index == 0 {
                        None
                    } else {
                        Some(cp_utf8(&constant_pool, entry.inner_name_index)?.to_string())
                    };
                    inner_classes.push(InnerClassNode {
                        name,
                        outer_name,
                        inner_name,
                        access_flags: entry.inner_class_access_flags,
                    });
                }
            }
        }

        let mut outer_class = String::new();
        if let Some(class_index) = self.attributes.iter().find_map(|attr| match attr {
            AttributeInfo::EnclosingMethod { class_index, .. } => Some(*class_index),
            _ => None,
        }) {
            outer_class = class_name(&constant_pool, class_index)?.to_string();
        }
        if outer_class.is_empty() {
            for attr in &self.attributes {
                if let AttributeInfo::InnerClasses { classes } = attr {
                    if let Some(entry) = classes.iter().find(|entry| {
                        entry.inner_class_info_index == this_class
                            && entry.outer_class_info_index != 0
                    }) {
                        outer_class =
                            class_name(&constant_pool, entry.outer_class_info_index)?.to_string();
                        break;
                    }
                }
            }
        }

        Ok(ClassNode {
            minor_version: self.minor_version,
            major_version: self.major_version,
            access_flags: self.access_flags,
            constant_pool,
            this_class,
            name: self.name,
            super_name: self.super_name,
            source_file: self.source_file.clone(),
            interfaces: self.interfaces,
            interface_indices,
            fields,
            methods,
            attributes: self.attributes,
            inner_classes,
            outer_class,
        })
    }

    /// Generates the raw byte vector representing the .class file.
    ///
    /// This method performs all necessary computations (stack map frames, max stack size)
    /// based on the options provided in `new`.
    pub fn to_bytes(self) -> Result<Vec<u8>, ClassWriteError> {
        let options = self.options;
        let class_node = self
            .to_class_node()
            .map_err(ClassWriteError::FrameComputation)?;
        ClassFileWriter::new(options).to_bytes(&class_node)
    }

    pub fn write_class_node(
        class_node: &ClassNode,
        options: u32,
    ) -> Result<Vec<u8>, ClassWriteError> {
        ClassFileWriter::new(options).to_bytes(class_node)
    }
}

/// A visitor to visit a Java method.
///
/// Used to generate the bytecode instructions, exception tables, and attributes
/// for a specific method.
pub struct MethodVisitor {
    access_flags: u16,
    name: String,
    descriptor: String,
    has_code: bool,
    max_stack: u16,
    max_locals: u16,
    insns: NodeList,
    exception_table: Vec<ExceptionTableEntry>,
    code_attributes: Vec<AttributeInfo>,
    attributes: Vec<AttributeInfo>,
}

impl MethodVisitor {
    pub fn new(access_flags: u16, name: &str, descriptor: &str) -> Self {
        Self {
            access_flags,
            name: name.to_string(),
            descriptor: descriptor.to_string(),
            has_code: false,
            max_stack: 0,
            max_locals: 0,
            insns: NodeList::new(),
            exception_table: Vec::new(),
            code_attributes: Vec::new(),
            attributes: Vec::new(),
        }
    }

    /// Starts the visit of the method's code.
    pub fn visit_code(&mut self) -> &mut Self {
        self.has_code = true;
        self
    }

    /// Visits a zero-operand instruction (e.g., NOP, RETURN).
    pub fn visit_insn(&mut self, opcode: u8) -> &mut Self {
        self.insns.add(Insn::from(Into::<InsnNode>::into(opcode)));
        self
    }

    /// Visits a local variable instruction (e.g., ILOAD, ASTORE).
    pub fn visit_var_insn(&mut self, opcode: u8, var_index: u16) -> &mut Self {
        self.insns.add(Insn::Var(VarInsnNode {
            insn: opcode.into(),
            var_index,
        }));
        self
    }

    /// Visits a field instruction (e.g., GETFIELD, PUTSTATIC).
    pub fn visit_field_insn(
        &mut self,
        opcode: u8,
        owner: &str,
        name: &str,
        descriptor: &str,
    ) -> &mut Self {
        self.insns.add(Insn::Field(FieldInsnNode::new(
            opcode, owner, name, descriptor,
        )));
        self
    }

    /// Visits a method instruction (e.g., INVOKEVIRTUAL).
    pub fn visit_method_insn(
        &mut self,
        opcode: u8,
        owner: &str,
        name: &str,
        descriptor: &str,
        _is_interface: bool,
    ) -> &mut Self {
        self.insns.add(Insn::Method(MethodInsnNode::new(
            opcode, owner, name, descriptor,
        )));
        self
    }

    pub fn visit_jump_insn(&mut self, opcode: u8, target: Label) -> &mut Self {
        self.insns.add(JumpLabelInsnNode {
            insn: opcode.into(),
            target: LabelNode::from_label(target),
        });
        self
    }

    pub fn visit_label(&mut self, label: Label) -> &mut Self {
        self.insns.add(LabelNode::from_label(label));
        self
    }

    pub fn visit_line_number(&mut self, line: u16, start: LabelNode) -> &mut Self {
        self.insns.add(LineNumberInsnNode::new(line, start));
        self
    }

    /// Visits a constant instruction (LDC).
    pub fn visit_ldc_insn(&mut self, value: LdcInsnNode) -> &mut Self {
        self.insns.add(Insn::Ldc(value));
        self
    }

    /// Visits the maximum stack size and number of local variables.
    ///
    /// If `COMPUTE_MAXS` or `COMPUTE_FRAMES` was passed to the ClassWriter,
    /// these values may be ignored or recomputed.
    pub fn visit_maxs(&mut self, max_stack: u16, max_locals: u16) -> &mut Self {
        self.max_stack = max_stack;
        self.max_locals = max_locals;
        self
    }

    /// Finalizes the method and attaches it to the parent `ClassWriter`.
    pub fn visit_end(mut self, class: &mut ClassWriter) {
        let code = if self.has_code || !self.insns.nodes().is_empty() {
            Some(build_code_attribute(
                self.max_stack,
                self.max_locals,
                self.insns,
                &mut class.cp,
                std::mem::take(&mut self.exception_table),
                std::mem::take(&mut self.code_attributes),
            ))
        } else {
            None
        };
        class.methods.push(MethodData {
            access_flags: self.access_flags,
            name: self.name,
            descriptor: self.descriptor,
            code,
            attributes: std::mem::take(&mut self.attributes),
        });
    }
}

/// A visitor to visit a Java field.
pub struct FieldVisitor {
    access_flags: u16,
    name: String,
    descriptor: String,
    attributes: Vec<AttributeInfo>,
    class_ptr: Option<*mut ClassWriter>,
    committed: bool,
}

impl FieldVisitor {
    pub fn new(
        access_flags: u16,
        name: &str,
        descriptor: &str,
        class_ptr: *mut ClassWriter,
    ) -> Self {
        Self {
            access_flags,
            name: name.to_string(),
            descriptor: descriptor.to_string(),
            attributes: Vec::new(),
            class_ptr: Some(class_ptr),
            committed: false,
        }
    }

    /// Adds an attribute to the field.
    pub fn add_attribute(&mut self, attr: AttributeInfo) -> &mut Self {
        self.attributes.push(attr);
        self
    }

    /// Finalizes the field and attaches it to the parent `ClassWriter`.
    /// If you don't call this, the field is still attached when the visitor is dropped.
    pub fn visit_end(mut self, class: &mut ClassWriter) {
        class.fields.push(FieldData {
            access_flags: self.access_flags,
            name: std::mem::take(&mut self.name),
            descriptor: std::mem::take(&mut self.descriptor),
            attributes: std::mem::take(&mut self.attributes),
        });
        self.committed = true;
        self.class_ptr = None;
    }
}

impl Drop for FieldVisitor {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        let Some(ptr) = self.class_ptr else {
            return;
        };
        // Safety: FieldVisitor is expected to be dropped before the ClassWriter it was created from.
        unsafe {
            let class = &mut *ptr;
            class.fields.push(FieldData {
                access_flags: self.access_flags,
                name: std::mem::take(&mut self.name),
                descriptor: std::mem::take(&mut self.descriptor),
                attributes: std::mem::take(&mut self.attributes),
            });
        }
        self.committed = true;
        self.class_ptr = None;
    }
}

struct CodeBody {
    max_stack: u16,
    max_locals: u16,
    insns: NodeList,
    exception_table: Vec<ExceptionTableEntry>,
    attributes: Vec<AttributeInfo>,
}

impl CodeBody {
    fn new(max_stack: u16, max_locals: u16, insns: NodeList) -> Self {
        Self {
            max_stack,
            max_locals,
            insns,
            exception_table: Vec::new(),
            attributes: Vec::new(),
        }
    }

    fn build(self, cp: &mut ConstantPoolBuilder) -> CodeAttribute {
        let mut code = Vec::new();
        let mut instructions = Vec::new();
        let mut insn_nodes = Vec::new();
        let mut label_offsets: HashMap<usize, u16> = HashMap::new();
        let mut pending_lines: Vec<LineNumberInsnNode> = Vec::new();
        let mut jump_fixups: Vec<JumpFixup> = Vec::new();
        for node in self.insns.into_nodes() {
            match node {
                AbstractInsnNode::Insn(insn) => {
                    let resolved = emit_insn(&mut code, insn, cp);
                    instructions.push(resolved.clone());
                    insn_nodes.push(AbstractInsnNode::Insn(resolved));
                }
                AbstractInsnNode::JumpLabel(node) => {
                    let opcode = node.insn.opcode;
                    let start = code.len();
                    code.push(opcode);
                    if is_wide_jump(opcode) {
                        write_i4(&mut code, 0);
                    } else {
                        write_i2(&mut code, 0);
                    }
                    let insn = Insn::Jump(JumpInsnNode {
                        insn: InsnNode { opcode },
                        offset: 0,
                    });
                    instructions.push(insn.clone());
                    insn_nodes.push(AbstractInsnNode::Insn(insn.clone()));
                    jump_fixups.push(JumpFixup {
                        start,
                        opcode,
                        target: node.target,
                        insn_index: instructions.len() - 1,
                        node_index: insn_nodes.len() - 1,
                    });
                }
                AbstractInsnNode::Label(label) => {
                    let offset = code.len();
                    if offset <= u16::MAX as usize {
                        label_offsets.insert(label.id, offset as u16);
                    }
                    insn_nodes.push(AbstractInsnNode::Label(label));
                }
                AbstractInsnNode::LineNumber(line) => {
                    pending_lines.push(line);
                    insn_nodes.push(AbstractInsnNode::LineNumber(line));
                }
            }
        }
        for fixup in jump_fixups {
            if let Some(target_offset) = label_offsets.get(&fixup.target.id) {
                let offset = *target_offset as i32 - fixup.start as i32;
                if is_wide_jump(fixup.opcode) {
                    write_i4_at(&mut code, fixup.start + 1, offset);
                } else {
                    write_i2_at(&mut code, fixup.start + 1, offset as i16);
                }
                let resolved = Insn::Jump(JumpInsnNode {
                    insn: InsnNode {
                        opcode: fixup.opcode,
                    },
                    offset,
                });
                instructions[fixup.insn_index] = resolved.clone();
                insn_nodes[fixup.node_index] = AbstractInsnNode::Insn(resolved);
            }
        }
        let mut attributes = self.attributes;
        if !pending_lines.is_empty() {
            let mut entries = Vec::new();
            for line in pending_lines {
                if let Some(start_pc) = label_offsets.get(&line.start.id) {
                    entries.push(LineNumber {
                        start_pc: *start_pc,
                        line_number: line.line,
                    });
                }
            }
            if !entries.is_empty() {
                attributes.push(AttributeInfo::LineNumberTable { entries });
            }
        }
        CodeAttribute {
            max_stack: self.max_stack,
            max_locals: self.max_locals,
            code,
            instructions,
            insn_nodes,
            exception_table: self.exception_table,
            try_catch_blocks: Vec::new(),
            attributes,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct JumpFixup {
    start: usize,
    opcode: u8,
    target: LabelNode,
    insn_index: usize,
    node_index: usize,
}

fn is_wide_jump(opcode: u8) -> bool {
    matches!(opcode, opcodes::GOTO_W | opcodes::JSR_W)
}

fn jump_size(opcode: u8) -> usize {
    if is_wide_jump(opcode) { 5 } else { 3 }
}
fn build_code_attribute(
    max_stack: u16,
    max_locals: u16,
    insns: NodeList,
    cp: &mut ConstantPoolBuilder,
    exception_table: Vec<ExceptionTableEntry>,
    attributes: Vec<AttributeInfo>,
) -> CodeAttribute {
    CodeBody {
        max_stack,
        max_locals,
        insns,
        exception_table,
        attributes,
    }
    .build(cp)
}

fn emit_insn(code: &mut Vec<u8>, insn: Insn, cp: &mut ConstantPoolBuilder) -> Insn {
    let offset = code.len();
    match insn {
        Insn::Simple(node) => {
            code.push(node.opcode);
            Insn::Simple(node)
        }
        Insn::Int(node) => {
            code.push(node.insn.opcode);
            match node.insn.opcode {
                opcodes::BIPUSH => write_i1(code, node.operand as i8),
                opcodes::SIPUSH => write_i2(code, node.operand as i16),
                opcodes::NEWARRAY => write_u1(code, node.operand as u8),
                _ => write_i1(code, node.operand as i8),
            }
            Insn::Int(node)
        }
        Insn::Var(node) => {
            code.push(node.insn.opcode);
            write_u1(code, node.var_index as u8);
            Insn::Var(node)
        }
        Insn::Type(node) => {
            code.push(node.insn.opcode);
            write_u2(code, node.type_index);
            Insn::Type(node)
        }
        Insn::Field(node) => {
            code.push(node.insn.opcode);
            let (index, resolved) = resolve_field_ref(node, cp);
            write_u2(code, index);
            Insn::Field(resolved)
        }
        Insn::Method(node) => {
            code.push(node.insn.opcode);
            let (index, resolved) = resolve_method_ref(node, cp);
            write_u2(code, index);
            Insn::Method(resolved)
        }
        Insn::InvokeInterface(node) => {
            code.push(node.insn.opcode);
            write_u2(code, node.method_index);
            write_u1(code, node.count);
            write_u1(code, 0);
            Insn::InvokeInterface(node)
        }
        Insn::InvokeDynamic(node) => {
            code.push(node.insn.opcode);
            write_u2(code, node.method_index);
            write_u2(code, 0);
            Insn::InvokeDynamic(node)
        }
        Insn::Jump(node) => {
            code.push(node.insn.opcode);
            match node.insn.opcode {
                opcodes::GOTO_W | opcodes::JSR_W => write_i4(code, node.offset),
                _ => write_i2(code, node.offset as i16),
            }
            Insn::Jump(node)
        }
        Insn::Ldc(node) => {
            let (opcode, index, resolved) = resolve_ldc(node, cp);
            code.push(opcode);
            if opcode == opcodes::LDC {
                write_u1(code, index as u8);
            } else {
                write_u2(code, index);
            }
            Insn::Ldc(resolved)
        }
        Insn::Iinc(node) => {
            code.push(node.insn.opcode);
            write_u1(code, node.var_index as u8);
            write_i1(code, node.increment as i8);
            Insn::Iinc(node)
        }
        Insn::TableSwitch(node) => {
            code.push(node.insn.opcode);
            write_switch_padding(code, offset);
            write_i4(code, node.default_offset);
            write_i4(code, node.low);
            write_i4(code, node.high);
            for value in &node.offsets {
                write_i4(code, *value);
            }
            Insn::TableSwitch(node)
        }
        Insn::LookupSwitch(node) => {
            code.push(node.insn.opcode);
            write_switch_padding(code, offset);
            write_i4(code, node.default_offset);
            write_i4(code, node.pairs.len() as i32);
            for (key, value) in &node.pairs {
                write_i4(code, *key);
                write_i4(code, *value);
            }
            Insn::LookupSwitch(node)
        }
        Insn::MultiANewArray(node) => {
            code.push(node.insn.opcode);
            write_u2(code, node.type_index);
            write_u1(code, node.dimensions);
            Insn::MultiANewArray(node)
        }
    }
}

fn resolve_field_ref(node: FieldInsnNode, cp: &mut ConstantPoolBuilder) -> (u16, FieldInsnNode) {
    match node.field_ref {
        MemberRef::Index(index) => (index, node),
        MemberRef::Symbolic {
            owner,
            name,
            descriptor,
        } => {
            let index = cp.field_ref(&owner, &name, &descriptor);
            (
                index,
                FieldInsnNode {
                    insn: node.insn,
                    field_ref: MemberRef::Index(index),
                },
            )
        }
    }
}

fn resolve_method_ref(node: MethodInsnNode, cp: &mut ConstantPoolBuilder) -> (u16, MethodInsnNode) {
    match node.method_ref {
        MemberRef::Index(index) => (index, node),
        MemberRef::Symbolic {
            owner,
            name,
            descriptor,
        } => {
            let index = cp.method_ref(&owner, &name, &descriptor);
            (
                index,
                MethodInsnNode {
                    insn: node.insn,
                    method_ref: MemberRef::Index(index),
                },
            )
        }
    }
}

fn resolve_ldc(node: LdcInsnNode, cp: &mut ConstantPoolBuilder) -> (u8, u16, LdcInsnNode) {
    match node.value {
        LdcValue::Index(index) => {
            let opcode = if index <= 0xFF {
                opcodes::LDC
            } else {
                opcodes::LDC_W
            };
            (
                opcode,
                index,
                LdcInsnNode {
                    insn: opcode.into(),
                    value: LdcValue::Index(index),
                },
            )
        }
        LdcValue::String(value) => {
            let index = cp.string(&value);
            let opcode = if index <= 0xFF {
                opcodes::LDC
            } else {
                opcodes::LDC_W
            };
            (
                opcode,
                index,
                LdcInsnNode {
                    insn: opcode.into(),
                    value: LdcValue::Index(index),
                },
            )
        }
        LdcValue::Int(value) => {
            let index = cp.integer(value);
            let opcode = if index <= 0xFF {
                opcodes::LDC
            } else {
                opcodes::LDC_W
            };
            (
                opcode,
                index,
                LdcInsnNode {
                    insn: opcode.into(),
                    value: LdcValue::Index(index),
                },
            )
        }
        LdcValue::Float(value) => {
            let index = cp.float(value);
            let opcode = if index <= 0xFF {
                opcodes::LDC
            } else {
                opcodes::LDC_W
            };
            (
                opcode,
                index,
                LdcInsnNode {
                    insn: opcode.into(),
                    value: LdcValue::Index(index),
                },
            )
        }
        LdcValue::Long(value) => {
            let index = cp.long(value);
            (
                opcodes::LDC2_W,
                index,
                LdcInsnNode {
                    insn: opcodes::LDC2_W.into(),
                    value: LdcValue::Index(index),
                },
            )
        }
        LdcValue::Double(value) => {
            let index = cp.double(value);
            (
                opcodes::LDC2_W,
                index,
                LdcInsnNode {
                    insn: opcodes::LDC2_W.into(),
                    value: LdcValue::Index(index),
                },
            )
        }
    }
}

struct ClassFileWriter {
    options: u32,
}

impl ClassFileWriter {
    fn new(options: u32) -> Self {
        Self { options }
    }

    fn to_bytes(&self, class_node: &ClassNode) -> Result<Vec<u8>, ClassWriteError> {
        if class_node.constant_pool.is_empty() {
            return Err(ClassWriteError::MissingConstantPool);
        }

        let mut cp = class_node.constant_pool.clone();
        let mut out = Vec::new();
        write_u4(&mut out, 0xCAFEBABE);
        write_u2(&mut out, class_node.minor_version);
        write_u2(&mut out, class_node.major_version);

        let mut class_attributes = class_node.attributes.clone();
        if let Some(source_file) = &class_node.source_file {
            class_attributes.retain(|attr| !matches!(attr, AttributeInfo::SourceFile { .. }));
            let source_index = ensure_utf8(&mut cp, source_file);
            class_attributes.push(AttributeInfo::SourceFile {
                sourcefile_index: source_index,
            });
        }

        let mut attribute_names = Vec::new();
        collect_attribute_names(&class_attributes, &mut attribute_names);
        for field in &class_node.fields {
            collect_attribute_names(&field.attributes, &mut attribute_names);
        }
        for method in &class_node.methods {
            collect_attribute_names(&method.attributes, &mut attribute_names);
            if let Some(code) = &method.code {
                attribute_names.push("Code".to_string());
                collect_attribute_names(&code.attributes, &mut attribute_names);
            }
        }
        for name in attribute_names {
            ensure_utf8(&mut cp, &name);
        }

        let mut precomputed_stack_maps: Vec<Option<Vec<StackMapFrame>>> =
            Vec::with_capacity(class_node.methods.len());
        let mut precomputed_maxs: Vec<Option<(u16, u16)>> =
            Vec::with_capacity(class_node.methods.len());
        let compute_frames = self.options & COMPUTE_FRAMES != 0;
        let compute_maxs_flag = self.options & COMPUTE_MAXS != 0;
        if compute_frames {
            ensure_utf8(&mut cp, "StackMapTable");
            for method in &class_node.methods {
                if let Some(code) = &method.code {
                    let maxs = if compute_maxs_flag {
                        Some(compute_maxs(method, class_node, code, &cp)?)
                    } else {
                        None
                    };
                    let max_locals = maxs.map(|item| item.1).unwrap_or(code.max_locals);
                    let stack_map =
                        compute_stack_map_table(method, class_node, code, &mut cp, max_locals)?;
                    precomputed_stack_maps.push(Some(stack_map));
                    precomputed_maxs.push(maxs);
                } else {
                    precomputed_stack_maps.push(None);
                    precomputed_maxs.push(None);
                }
            }
        } else if compute_maxs_flag {
            for method in &class_node.methods {
                if let Some(code) = &method.code {
                    precomputed_maxs.push(Some(compute_maxs(method, class_node, code, &cp)?));
                } else {
                    precomputed_maxs.push(None);
                }
            }
            precomputed_stack_maps.resize(class_node.methods.len(), None);
        } else {
            precomputed_stack_maps.resize(class_node.methods.len(), None);
            precomputed_maxs.resize(class_node.methods.len(), None);
        }

        write_constant_pool(&mut out, &cp)?;
        write_u2(&mut out, class_node.access_flags);
        write_u2(&mut out, class_node.this_class);
        let super_class = match class_node.super_name.as_deref() {
            Some(name) => ensure_class(&mut cp, name),
            None => 0,
        };
        write_u2(&mut out, super_class);
        write_u2(&mut out, class_node.interface_indices.len() as u16);
        for index in &class_node.interface_indices {
            write_u2(&mut out, *index);
        }

        write_u2(&mut out, class_node.fields.len() as u16);
        for field in &class_node.fields {
            write_field(&mut out, field, &mut cp)?;
        }

        write_u2(&mut out, class_node.methods.len() as u16);
        for (index, method) in class_node.methods.iter().enumerate() {
            let stack_map = precomputed_stack_maps
                .get(index)
                .and_then(|item| item.as_ref());
            let maxs = precomputed_maxs.get(index).and_then(|item| *item);
            write_method(
                &mut out,
                method,
                class_node,
                &mut cp,
                self.options,
                stack_map,
                maxs,
            )?;
        }

        write_u2(&mut out, class_attributes.len() as u16);
        for attr in &class_attributes {
            write_attribute(&mut out, attr, &mut cp, None, self.options, None, None)?;
        }

        Ok(out)
    }
}

fn write_field(
    out: &mut Vec<u8>,
    field: &FieldNode,
    cp: &mut Vec<CpInfo>,
) -> Result<(), ClassWriteError> {
    write_u2(out, field.access_flags);
    write_u2(out, field.name_index);
    write_u2(out, field.descriptor_index);
    write_u2(out, field.attributes.len() as u16);
    for attr in &field.attributes {
        write_attribute(out, attr, cp, None, 0, None, None)?;
    }
    Ok(())
}

fn write_method(
    out: &mut Vec<u8>,
    method: &MethodNode,
    class_node: &ClassNode,
    cp: &mut Vec<CpInfo>,
    options: u32,
    precomputed_stack_map: Option<&Vec<StackMapFrame>>,
    precomputed_maxs: Option<(u16, u16)>,
) -> Result<(), ClassWriteError> {
    write_u2(out, method.access_flags);
    write_u2(out, method.name_index);
    write_u2(out, method.descriptor_index);

    let mut attributes = method.attributes.clone();
    if let Some(code) = &method.code {
        attributes.retain(|attr| !matches!(attr, AttributeInfo::Code(_)));
        attributes.push(AttributeInfo::Code(code.clone()));
    }

    write_u2(out, attributes.len() as u16);
    for attr in &attributes {
        write_attribute(
            out,
            attr,
            cp,
            Some((method, class_node)),
            options,
            precomputed_stack_map,
            precomputed_maxs,
        )?;
    }
    Ok(())
}

fn write_attribute(
    out: &mut Vec<u8>,
    attr: &AttributeInfo,
    cp: &mut Vec<CpInfo>,
    method_ctx: Option<(&MethodNode, &ClassNode)>,
    options: u32,
    precomputed_stack_map: Option<&Vec<StackMapFrame>>,
    precomputed_maxs: Option<(u16, u16)>,
) -> Result<(), ClassWriteError> {
    match attr {
        AttributeInfo::Code(code) => {
            let name_index = ensure_utf8(cp, "Code");
            let mut info = Vec::new();
            let mut code_attributes = code.attributes.clone();
            let (max_stack, max_locals) =
                precomputed_maxs.unwrap_or((code.max_stack, code.max_locals));
            if options & COMPUTE_FRAMES != 0 {
                code_attributes.retain(|item| !matches!(item, AttributeInfo::StackMapTable { .. }));
                let stack_map = if let Some(precomputed) = precomputed_stack_map {
                    precomputed.clone()
                } else {
                    let (method, class_node) = method_ctx.ok_or_else(|| {
                        ClassWriteError::FrameComputation("missing method".to_string())
                    })?;
                    compute_stack_map_table(method, class_node, code, cp, max_locals)?
                };
                code_attributes.push(AttributeInfo::StackMapTable { entries: stack_map });
            }

            write_u2(&mut info, max_stack);
            write_u2(&mut info, max_locals);
            write_u4(&mut info, code.code.len() as u32);
            info.extend_from_slice(&code.code);
            write_u2(&mut info, code.exception_table.len() as u16);
            for entry in &code.exception_table {
                write_exception_table_entry(&mut info, entry);
            }
            write_u2(&mut info, code_attributes.len() as u16);
            for nested in &code_attributes {
                write_attribute(&mut info, nested, cp, method_ctx, options, None, None)?;
            }
            write_attribute_with_info(out, name_index, &info);
        }
        AttributeInfo::ConstantValue {
            constantvalue_index,
        } => {
            let name_index = ensure_utf8(cp, "ConstantValue");
            let mut info = Vec::new();
            write_u2(&mut info, *constantvalue_index);
            write_attribute_with_info(out, name_index, &info);
        }
        AttributeInfo::Exceptions {
            exception_index_table,
        } => {
            let name_index = ensure_utf8(cp, "Exceptions");
            let mut info = Vec::new();
            write_u2(&mut info, exception_index_table.len() as u16);
            for index in exception_index_table {
                write_u2(&mut info, *index);
            }
            write_attribute_with_info(out, name_index, &info);
        }
        AttributeInfo::SourceFile { sourcefile_index } => {
            let name_index = ensure_utf8(cp, "SourceFile");
            let mut info = Vec::new();
            write_u2(&mut info, *sourcefile_index);
            write_attribute_with_info(out, name_index, &info);
        }
        AttributeInfo::LineNumberTable { entries } => {
            let name_index = ensure_utf8(cp, "LineNumberTable");
            let mut info = Vec::new();
            write_u2(&mut info, entries.len() as u16);
            for entry in entries {
                write_line_number(&mut info, entry);
            }
            write_attribute_with_info(out, name_index, &info);
        }
        AttributeInfo::LocalVariableTable { entries } => {
            let name_index = ensure_utf8(cp, "LocalVariableTable");
            let mut info = Vec::new();
            write_u2(&mut info, entries.len() as u16);
            for entry in entries {
                write_local_variable(&mut info, entry);
            }
            write_attribute_with_info(out, name_index, &info);
        }
        AttributeInfo::Signature { signature_index } => {
            let name_index = ensure_utf8(cp, "Signature");
            let mut info = Vec::new();
            write_u2(&mut info, *signature_index);
            write_attribute_with_info(out, name_index, &info);
        }
        AttributeInfo::StackMapTable { entries } => {
            let name_index = ensure_utf8(cp, "StackMapTable");
            let mut info = Vec::new();
            write_u2(&mut info, entries.len() as u16);
            for entry in entries {
                write_stack_map_frame(&mut info, entry);
            }
            write_attribute_with_info(out, name_index, &info);
        }
        AttributeInfo::Deprecated => {
            let name_index = ensure_utf8(cp, "Deprecated");
            write_attribute_with_info(out, name_index, &[]);
        }
        AttributeInfo::Synthetic => {
            let name_index = ensure_utf8(cp, "Synthetic");
            write_attribute_with_info(out, name_index, &[]);
        }
        AttributeInfo::InnerClasses { classes } => {
            let name_index = ensure_utf8(cp, "InnerClasses");
            let mut info = Vec::new();
            write_u2(&mut info, classes.len() as u16);
            for class in classes {
                write_inner_class(&mut info, class);
            }
            write_attribute_with_info(out, name_index, &info);
        }
        AttributeInfo::EnclosingMethod {
            class_index,
            method_index,
        } => {
            let name_index = ensure_utf8(cp, "EnclosingMethod");
            let mut info = Vec::new();
            write_u2(&mut info, *class_index);
            write_u2(&mut info, *method_index);
            write_attribute_with_info(out, name_index, &info);
        }
        AttributeInfo::BootstrapMethods { methods } => {
            let name_index = ensure_utf8(cp, "BootstrapMethods");
            let mut info = Vec::new();
            write_u2(&mut info, methods.len() as u16);
            for method in methods {
                write_bootstrap_method(&mut info, method);
            }
            write_attribute_with_info(out, name_index, &info);
        }
        AttributeInfo::MethodParameters { parameters } => {
            let name_index = ensure_utf8(cp, "MethodParameters");
            let mut info = Vec::new();
            write_u1(&mut info, parameters.len() as u8);
            for parameter in parameters {
                write_method_parameter(&mut info, parameter);
            }
            write_attribute_with_info(out, name_index, &info);
        }
        AttributeInfo::Unknown { name, info } => {
            let name_index = ensure_utf8(cp, name);
            write_attribute_with_info(out, name_index, info);
        }
    }

    Ok(())
}

fn write_attribute_with_info(out: &mut Vec<u8>, name_index: u16, info: &[u8]) {
    write_u2(out, name_index);
    write_u4(out, info.len() as u32);
    out.extend_from_slice(info);
}

fn write_exception_table_entry(out: &mut Vec<u8>, entry: &ExceptionTableEntry) {
    write_u2(out, entry.start_pc);
    write_u2(out, entry.end_pc);
    write_u2(out, entry.handler_pc);
    write_u2(out, entry.catch_type);
}

fn write_line_number(out: &mut Vec<u8>, entry: &LineNumber) {
    write_u2(out, entry.start_pc);
    write_u2(out, entry.line_number);
}

fn write_local_variable(out: &mut Vec<u8>, entry: &LocalVariable) {
    write_u2(out, entry.start_pc);
    write_u2(out, entry.length);
    write_u2(out, entry.name_index);
    write_u2(out, entry.descriptor_index);
    write_u2(out, entry.index);
}

fn write_inner_class(out: &mut Vec<u8>, entry: &InnerClass) {
    write_u2(out, entry.inner_class_info_index);
    write_u2(out, entry.outer_class_info_index);
    write_u2(out, entry.inner_name_index);
    write_u2(out, entry.inner_class_access_flags);
}

fn write_bootstrap_method(out: &mut Vec<u8>, entry: &BootstrapMethod) {
    write_u2(out, entry.bootstrap_method_ref);
    write_u2(out, entry.bootstrap_arguments.len() as u16);
    for arg in &entry.bootstrap_arguments {
        write_u2(out, *arg);
    }
}

fn write_method_parameter(out: &mut Vec<u8>, entry: &MethodParameter) {
    write_u2(out, entry.name_index);
    write_u2(out, entry.access_flags);
}

fn write_stack_map_frame(out: &mut Vec<u8>, frame: &StackMapFrame) {
    match frame {
        StackMapFrame::SameFrame { offset_delta } => {
            write_u1(out, *offset_delta as u8);
        }
        StackMapFrame::SameLocals1StackItemFrame {
            offset_delta,
            stack,
        } => {
            write_u1(out, (*offset_delta as u8) + 64);
            write_verification_type(out, stack);
        }
        StackMapFrame::SameLocals1StackItemFrameExtended {
            offset_delta,
            stack,
        } => {
            write_u1(out, 247);
            write_u2(out, *offset_delta);
            write_verification_type(out, stack);
        }
        StackMapFrame::ChopFrame { offset_delta, k } => {
            write_u1(out, 251 - *k);
            write_u2(out, *offset_delta);
        }
        StackMapFrame::SameFrameExtended { offset_delta } => {
            write_u1(out, 251);
            write_u2(out, *offset_delta);
        }
        StackMapFrame::AppendFrame {
            offset_delta,
            locals,
        } => {
            write_u1(out, 251 + locals.len() as u8);
            write_u2(out, *offset_delta);
            for local in locals {
                write_verification_type(out, local);
            }
        }
        StackMapFrame::FullFrame {
            offset_delta,
            locals,
            stack,
        } => {
            write_u1(out, 255);
            write_u2(out, *offset_delta);
            write_u2(out, locals.len() as u16);
            for local in locals {
                write_verification_type(out, local);
            }
            write_u2(out, stack.len() as u16);
            for value in stack {
                write_verification_type(out, value);
            }
        }
    }
}

fn write_verification_type(out: &mut Vec<u8>, value: &VerificationTypeInfo) {
    match value {
        VerificationTypeInfo::Top => write_u1(out, 0),
        VerificationTypeInfo::Integer => write_u1(out, 1),
        VerificationTypeInfo::Float => write_u1(out, 2),
        VerificationTypeInfo::Double => write_u1(out, 3),
        VerificationTypeInfo::Long => write_u1(out, 4),
        VerificationTypeInfo::Null => write_u1(out, 5),
        VerificationTypeInfo::UninitializedThis => write_u1(out, 6),
        VerificationTypeInfo::Object { cpool_index } => {
            write_u1(out, 7);
            write_u2(out, *cpool_index);
        }
        VerificationTypeInfo::Uninitialized { offset } => {
            write_u1(out, 8);
            write_u2(out, *offset);
        }
    }
}

fn collect_attribute_names(attributes: &[AttributeInfo], names: &mut Vec<String>) {
    for attr in attributes {
        match attr {
            AttributeInfo::Code(_) => names.push("Code".to_string()),
            AttributeInfo::ConstantValue { .. } => names.push("ConstantValue".to_string()),
            AttributeInfo::Exceptions { .. } => names.push("Exceptions".to_string()),
            AttributeInfo::SourceFile { .. } => names.push("SourceFile".to_string()),
            AttributeInfo::LineNumberTable { .. } => names.push("LineNumberTable".to_string()),
            AttributeInfo::LocalVariableTable { .. } => {
                names.push("LocalVariableTable".to_string())
            }
            AttributeInfo::Signature { .. } => names.push("Signature".to_string()),
            AttributeInfo::StackMapTable { .. } => names.push("StackMapTable".to_string()),
            AttributeInfo::Deprecated => names.push("Deprecated".to_string()),
            AttributeInfo::Synthetic => names.push("Synthetic".to_string()),
            AttributeInfo::InnerClasses { .. } => names.push("InnerClasses".to_string()),
            AttributeInfo::EnclosingMethod { .. } => names.push("EnclosingMethod".to_string()),
            AttributeInfo::BootstrapMethods { .. } => names.push("BootstrapMethods".to_string()),
            AttributeInfo::MethodParameters { .. } => names.push("MethodParameters".to_string()),
            AttributeInfo::Unknown { name, .. } => names.push(name.clone()),
        }
    }
}

fn write_constant_pool(out: &mut Vec<u8>, cp: &[CpInfo]) -> Result<(), ClassWriteError> {
    write_u2(out, cp.len() as u16);
    for entry in cp.iter().skip(1) {
        match entry {
            CpInfo::Unusable => {}
            CpInfo::Utf8(value) => {
                let bytes = encode_modified_utf8(value);
                write_u1(out, 1);
                write_u2(out, bytes.len() as u16);
                out.extend_from_slice(&bytes);
            }
            CpInfo::Integer(value) => {
                write_u1(out, 3);
                write_u4(out, *value as u32);
            }
            CpInfo::Float(value) => {
                write_u1(out, 4);
                write_u4(out, value.to_bits());
            }
            CpInfo::Long(value) => {
                write_u1(out, 5);
                write_u8(out, *value as u64);
            }
            CpInfo::Double(value) => {
                write_u1(out, 6);
                write_u8(out, value.to_bits());
            }
            CpInfo::Class { name_index } => {
                write_u1(out, 7);
                write_u2(out, *name_index);
            }
            CpInfo::String { string_index } => {
                write_u1(out, 8);
                write_u2(out, *string_index);
            }
            CpInfo::Fieldref {
                class_index,
                name_and_type_index,
            } => {
                write_u1(out, 9);
                write_u2(out, *class_index);
                write_u2(out, *name_and_type_index);
            }
            CpInfo::Methodref {
                class_index,
                name_and_type_index,
            } => {
                write_u1(out, 10);
                write_u2(out, *class_index);
                write_u2(out, *name_and_type_index);
            }
            CpInfo::InterfaceMethodref {
                class_index,
                name_and_type_index,
            } => {
                write_u1(out, 11);
                write_u2(out, *class_index);
                write_u2(out, *name_and_type_index);
            }
            CpInfo::NameAndType {
                name_index,
                descriptor_index,
            } => {
                write_u1(out, 12);
                write_u2(out, *name_index);
                write_u2(out, *descriptor_index);
            }
            CpInfo::MethodHandle {
                reference_kind,
                reference_index,
            } => {
                write_u1(out, 15);
                write_u1(out, *reference_kind);
                write_u2(out, *reference_index);
            }
            CpInfo::MethodType { descriptor_index } => {
                write_u1(out, 16);
                write_u2(out, *descriptor_index);
            }
            CpInfo::Dynamic {
                bootstrap_method_attr_index,
                name_and_type_index,
            } => {
                write_u1(out, 17);
                write_u2(out, *bootstrap_method_attr_index);
                write_u2(out, *name_and_type_index);
            }
            CpInfo::InvokeDynamic {
                bootstrap_method_attr_index,
                name_and_type_index,
            } => {
                write_u1(out, 18);
                write_u2(out, *bootstrap_method_attr_index);
                write_u2(out, *name_and_type_index);
            }
            CpInfo::Module { name_index } => {
                write_u1(out, 19);
                write_u2(out, *name_index);
            }
            CpInfo::Package { name_index } => {
                write_u1(out, 20);
                write_u2(out, *name_index);
            }
        }
    }
    Ok(())
}

fn encode_modified_utf8(value: &str) -> Vec<u8> {
    let mut out = Vec::new();
    for ch in value.chars() {
        let code = ch as u32;
        if code == 0 {
            out.push(0xC0);
            out.push(0x80);
        } else if code <= 0x7F {
            out.push(code as u8);
        } else if code <= 0x7FF {
            out.push((0xC0 | ((code >> 6) & 0x1F)) as u8);
            out.push((0x80 | (code & 0x3F)) as u8);
        } else if code <= 0xFFFF {
            out.push((0xE0 | ((code >> 12) & 0x0F)) as u8);
            out.push((0x80 | ((code >> 6) & 0x3F)) as u8);
            out.push((0x80 | (code & 0x3F)) as u8);
        } else {
            let u = code - 0x10000;
            let high = 0xD800 + ((u >> 10) & 0x3FF);
            let low = 0xDC00 + (u & 0x3FF);
            for cu in [high, low] {
                out.push((0xE0 | ((cu >> 12) & 0x0F)) as u8);
                out.push((0x80 | ((cu >> 6) & 0x3F)) as u8);
                out.push((0x80 | (cu & 0x3F)) as u8);
            }
        }
    }
    out
}

fn ensure_utf8(cp: &mut Vec<CpInfo>, value: &str) -> u16 {
    if let Some(index) = cp_find_utf8(cp, value) {
        return index;
    }
    cp.push(CpInfo::Utf8(value.to_string()));
    (cp.len() - 1) as u16
}

fn ensure_class(cp: &mut Vec<CpInfo>, name: &str) -> u16 {
    for (index, entry) in cp.iter().enumerate() {
        if let CpInfo::Class { name_index } = entry
            && let Some(CpInfo::Utf8(value)) = cp.get(*name_index as usize)
            && value == name
        {
            return index as u16;
        }
    }
    let name_index = ensure_utf8(cp, name);
    cp.push(CpInfo::Class { name_index });
    (cp.len() - 1) as u16
}

fn cp_find_utf8(cp: &[CpInfo], value: &str) -> Option<u16> {
    for (index, entry) in cp.iter().enumerate() {
        if let CpInfo::Utf8(existing) = entry
            && existing == value
        {
            return Some(index as u16);
        }
    }
    None
}

fn write_u1(out: &mut Vec<u8>, value: u8) {
    out.push(value);
}

fn write_u2(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn write_u4(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn write_i1(out: &mut Vec<u8>, value: i8) {
    out.push(value as u8);
}

fn write_i2(out: &mut Vec<u8>, value: i16) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn write_i4(out: &mut Vec<u8>, value: i32) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn write_i2_at(out: &mut [u8], pos: usize, value: i16) {
    let bytes = value.to_be_bytes();
    out[pos] = bytes[0];
    out[pos + 1] = bytes[1];
}

fn write_i4_at(out: &mut [u8], pos: usize, value: i32) {
    let bytes = value.to_be_bytes();
    out[pos] = bytes[0];
    out[pos + 1] = bytes[1];
    out[pos + 2] = bytes[2];
    out[pos + 3] = bytes[3];
}

fn write_switch_padding(out: &mut Vec<u8>, opcode_offset: usize) {
    let mut padding = (4 - ((opcode_offset + 1) % 4)) % 4;
    while padding > 0 {
        out.push(0);
        padding -= 1;
    }
}

fn write_u8(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_be_bytes());
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FrameType {
    Top,
    Integer,
    Float,
    Long,
    Double,
    Null,
    UninitializedThis,
    Object(String),
    Uninitialized(u16),
}

fn compute_stack_map_table(
    method: &MethodNode,
    class_node: &ClassNode,
    code: &CodeAttribute,
    cp: &mut Vec<CpInfo>,
    max_locals: u16,
) -> Result<Vec<StackMapFrame>, ClassWriteError> {
    let insns = parse_instructions(&code.code)?;
    if insns.is_empty() {
        return Ok(Vec::new());
    }

    let mut insn_index = std::collections::HashMap::new();
    for (index, insn) in insns.iter().enumerate() {
        insn_index.insert(insn.offset, index);
    }

    let handlers = build_exception_handlers(code, cp)?;
    let handler_common = handler_common_types(&handlers);
    let mut frames: std::collections::HashMap<u16, FrameState> = std::collections::HashMap::new();
    let mut worklist = std::collections::VecDeque::new();
    let mut in_worklist = std::collections::HashSet::new();

    let mut initial = initial_frame(method, class_node)?;
    pad_locals(&mut initial.locals, max_locals);
    frames.insert(0, initial.clone());
    worklist.push_back(0u16);
    in_worklist.insert(0u16);

    let mut max_iterations = 0usize;
    while let Some(offset) = worklist.pop_front() {
        in_worklist.remove(&offset);
        max_iterations += 1;
        if max_iterations > 100000 {
            return Err(ClassWriteError::FrameComputation(
                "frame analysis exceeded iteration limit".to_string(),
            ));
        }
        let index = *insn_index.get(&offset).ok_or_else(|| {
            ClassWriteError::FrameComputation(format!("missing instruction at {offset}"))
        })?;
        let insn = &insns[index];
        let frame = frames
            .get(&offset)
            .ok_or_else(|| ClassWriteError::FrameComputation(format!("missing frame at {offset}")))?
            .clone();
        let insn1 = &insn;
        let out_frame = execute_instruction(insn1, &frame, class_node, cp)?;

        for succ in instruction_successors(insn) {
            if let Some(next_frame) = merge_frame(&out_frame, frames.get(&succ)) {
                let changed = match frames.get(&succ) {
                    Some(existing) => existing != &next_frame,
                    None => true,
                };
                if changed {
                    frames.insert(succ, next_frame);
                    if in_worklist.insert(succ) {
                        worklist.push_back(succ);
                    }
                }
            }
        }

        for handler in handlers.iter().filter(|item| item.covers(offset)) {
            let mut handler_frame = FrameState {
                locals: frame.locals.clone(),
                stack: Vec::new(),
            };
            let exception_type = handler_common
                .get(&handler.handler_pc)
                .cloned()
                .unwrap_or_else(|| handler.exception_type.clone());
            handler_frame.stack.push(exception_type);
            if let Some(next_frame) = merge_frame(&handler_frame, frames.get(&handler.handler_pc)) {
                let changed = match frames.get(&handler.handler_pc) {
                    Some(existing) => existing != &next_frame,
                    None => true,
                };
                if changed {
                    frames.insert(handler.handler_pc, next_frame);
                    if in_worklist.insert(handler.handler_pc) {
                        worklist.push_back(handler.handler_pc);
                    }
                }
            }
        }
    }

    let mut frame_offsets: Vec<u16> = frames.keys().copied().collect();
    frame_offsets.sort_unstable();
    let mut result = Vec::new();
    let mut previous_offset: i32 = -1;
    for offset in frame_offsets {
        if offset == 0 {
            continue;
        }
        let frame = frames
            .get(&offset)
            .ok_or_else(|| ClassWriteError::FrameComputation(format!("missing frame at {offset}")))?
            .clone();
        let locals = compact_locals(&frame.locals);
        let stack = frame.stack;
        let offset_delta = (offset as i32 - previous_offset - 1) as u16;
        previous_offset = offset as i32;
        let locals_info = locals
            .iter()
            .map(|value| to_verification_type(value, cp))
            .collect();
        let stack_info = stack
            .iter()
            .map(|value| to_verification_type(value, cp))
            .collect();
        result.push(StackMapFrame::FullFrame {
            offset_delta,
            locals: locals_info,
            stack: stack_info,
        });
    }

    Ok(result)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FrameState {
    locals: Vec<FrameType>,
    stack: Vec<FrameType>,
}

fn merge_frame(frame: &FrameState, existing: Option<&FrameState>) -> Option<FrameState> {
    match existing {
        None => Some(frame.clone()),
        Some(other) => {
            let merged = FrameState {
                locals: merge_vec(&frame.locals, &other.locals),
                stack: merge_vec(&frame.stack, &other.stack),
            };
            if merged == *other { None } else { Some(merged) }
        }
    }
}

fn merge_vec(a: &[FrameType], b: &[FrameType]) -> Vec<FrameType> {
    let len = a.len().max(b.len());
    let mut merged = Vec::with_capacity(len);
    for i in 0..len {
        let left = a.get(i).cloned().unwrap_or(FrameType::Top);
        let right = b.get(i).cloned().unwrap_or(FrameType::Top);
        merged.push(merge_type(&left, &right));
    }
    merged
}

fn merge_type(a: &FrameType, b: &FrameType) -> FrameType {
    if a == b {
        return a.clone();
    }
    match (a, b) {
        (FrameType::Top, _) => FrameType::Top,
        (_, FrameType::Top) => FrameType::Top,
        (FrameType::Null, FrameType::Object(name)) | (FrameType::Object(name), FrameType::Null) => {
            FrameType::Object(name.clone())
        }
        (FrameType::Object(left), FrameType::Object(right)) => {
            FrameType::Object(common_superclass(left, right))
        }
        (FrameType::Object(_), FrameType::Uninitialized(_))
        | (FrameType::Uninitialized(_), FrameType::Object(_))
        | (FrameType::UninitializedThis, FrameType::Object(_))
        | (FrameType::Object(_), FrameType::UninitializedThis) => {
            FrameType::Object("java/lang/Object".to_string())
        }
        _ => FrameType::Top,
    }
}

fn common_superclass(left: &str, right: &str) -> String {
    if left == right {
        return left.to_string();
    }
    if left.starts_with('[') || right.starts_with('[') {
        return "java/lang/Object".to_string();
    }

    let mut ancestors = std::collections::HashSet::new();
    let mut current = left;
    ancestors.insert(current.to_string());
    while let Some(parent) = known_superclass(current) {
        if ancestors.insert(parent.to_string()) {
            current = parent;
        } else {
            break;
        }
    }
    ancestors.insert("java/lang/Object".to_string());

    current = right;
    if ancestors.contains(current) {
        return current.to_string();
    }
    while let Some(parent) = known_superclass(current) {
        if ancestors.contains(parent) {
            return parent.to_string();
        }
        current = parent;
    }
    "java/lang/Object".to_string()
}

fn known_superclass(name: &str) -> Option<&'static str> {
    match name {
        "java/lang/Throwable" => Some("java/lang/Object"),
        "java/lang/Exception" => Some("java/lang/Throwable"),
        "java/lang/RuntimeException" => Some("java/lang/Exception"),
        "java/lang/IllegalArgumentException" => Some("java/lang/RuntimeException"),
        "java/lang/IllegalStateException" => Some("java/lang/RuntimeException"),
        "java/security/GeneralSecurityException" => Some("java/lang/Exception"),
        "java/security/NoSuchAlgorithmException" => Some("java/security/GeneralSecurityException"),
        "java/security/InvalidKeyException" => Some("java/security/GeneralSecurityException"),
        "javax/crypto/NoSuchPaddingException" => Some("java/security/GeneralSecurityException"),
        "javax/crypto/IllegalBlockSizeException" => Some("java/security/GeneralSecurityException"),
        "javax/crypto/BadPaddingException" => Some("java/security/GeneralSecurityException"),
        _ => None,
    }
}

fn pad_locals(locals: &mut Vec<FrameType>, max_locals: u16) {
    while locals.len() < max_locals as usize {
        locals.push(FrameType::Top);
    }
}

fn compute_maxs(
    method: &MethodNode,
    class_node: &ClassNode,
    code: &CodeAttribute,
    cp: &[CpInfo],
) -> Result<(u16, u16), ClassWriteError> {
    let insns = parse_instructions(&code.code)?;
    if insns.is_empty() {
        let initial = initial_frame(method, class_node)?;
        return Ok((0, initial.locals.len() as u16));
    }

    let mut insn_index = std::collections::HashMap::new();
    for (index, insn) in insns.iter().enumerate() {
        insn_index.insert(insn.offset, index);
    }

    let handlers = build_exception_handlers(code, cp)?;
    let mut frames: std::collections::HashMap<u16, FrameState> = std::collections::HashMap::new();
    let mut worklist = std::collections::VecDeque::new();
    let mut in_worklist = std::collections::HashSet::new();

    let initial = initial_frame(method, class_node)?;
    frames.insert(0, initial.clone());
    worklist.push_back(0u16);
    in_worklist.insert(0u16);

    let mut max_stack = initial.stack.len();
    let mut max_locals = initial.locals.len();
    let mut max_iterations = 0usize;
    let mut offset_hits: std::collections::HashMap<u16, u32> = std::collections::HashMap::new();
    while let Some(offset) = worklist.pop_front() {
        in_worklist.remove(&offset);
        max_iterations += 1;
        *offset_hits.entry(offset).or_insert(0) += 1;
        if max_iterations > 100000 {
            return Err(ClassWriteError::FrameComputation(
                "frame analysis exceeded iteration limit".to_string(),
            ));
        }
        let index = *insn_index.get(&offset).ok_or_else(|| {
            ClassWriteError::FrameComputation(format!("missing instruction at {offset}"))
        })?;
        let insn = &insns[index];
        let frame = frames.get(&offset).cloned().ok_or_else(|| {
            ClassWriteError::FrameComputation(format!("missing frame at {offset}"))
        })?;
        max_stack = max_stack.max(stack_slots(&frame.stack));
        max_locals = max_locals.max(frame.locals.len());

        let out_frame = execute_instruction(insn, &frame, class_node, cp)?;
        max_stack = max_stack.max(stack_slots(&out_frame.stack));
        max_locals = max_locals.max(out_frame.locals.len());

        for succ in instruction_successors(insn) {
            if let Some(next_frame) = merge_frame(&out_frame, frames.get(&succ)) {
                let changed = match frames.get(&succ) {
                    Some(existing) => existing != &next_frame,
                    None => true,
                };
                if changed {
                    frames.insert(succ, next_frame);
                    if in_worklist.insert(succ) {
                        worklist.push_back(succ);
                    }
                }
            }
        }

        for handler in handlers.iter().filter(|item| item.covers(offset)) {
            let mut handler_frame = FrameState {
                locals: frame.locals.clone(),
                stack: Vec::new(),
            };
            handler_frame.stack.push(handler.exception_type.clone());
            max_stack = max_stack.max(stack_slots(&handler_frame.stack));
            max_locals = max_locals.max(handler_frame.locals.len());
            if let Some(next_frame) = merge_frame(&handler_frame, frames.get(&handler.handler_pc)) {
                let changed = match frames.get(&handler.handler_pc) {
                    Some(existing) => existing != &next_frame,
                    None => true,
                };
                if changed {
                    frames.insert(handler.handler_pc, next_frame);
                    if in_worklist.insert(handler.handler_pc) {
                        worklist.push_back(handler.handler_pc);
                    }
                }
            }
        }
    }

    Ok((max_stack as u16, max_locals as u16))
}

fn stack_slots(stack: &[FrameType]) -> usize {
    let mut slots = 0usize;
    for value in stack {
        slots += if is_category2(value) { 2 } else { 1 };
    }
    slots
}

fn compact_locals(locals: &[FrameType]) -> Vec<FrameType> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < locals.len() {
        match locals[i] {
            FrameType::Top => {
                if i > 0 && matches!(locals[i - 1], FrameType::Long | FrameType::Double) {
                    i += 1;
                    continue;
                }
                out.push(FrameType::Top);
            }
            FrameType::Long | FrameType::Double => {
                out.push(locals[i].clone());
                if i + 1 < locals.len() && matches!(locals[i + 1], FrameType::Top) {
                    i += 1;
                }
            }
            _ => out.push(locals[i].clone()),
        }
        i += 1;
    }

    while matches!(out.last(), Some(FrameType::Top)) {
        out.pop();
    }
    out
}

fn to_verification_type(value: &FrameType, cp: &mut Vec<CpInfo>) -> VerificationTypeInfo {
    match value {
        FrameType::Top => VerificationTypeInfo::Top,
        FrameType::Integer => VerificationTypeInfo::Integer,
        FrameType::Float => VerificationTypeInfo::Float,
        FrameType::Long => VerificationTypeInfo::Long,
        FrameType::Double => VerificationTypeInfo::Double,
        FrameType::Null => VerificationTypeInfo::Null,
        FrameType::UninitializedThis => VerificationTypeInfo::UninitializedThis,
        FrameType::Uninitialized(offset) => VerificationTypeInfo::Uninitialized { offset: *offset },
        FrameType::Object(name) => {
            let index = ensure_class(cp, name);
            VerificationTypeInfo::Object { cpool_index: index }
        }
    }
}

fn initial_frame(
    method: &MethodNode,
    class_node: &ClassNode,
) -> Result<FrameState, ClassWriteError> {
    let mut locals = Vec::new();
    let is_static = method.access_flags & constants::ACC_STATIC != 0;
    if !is_static {
        if method.name == "<init>" {
            locals.push(FrameType::UninitializedThis);
        } else {
            locals.push(FrameType::Object(class_node.name.clone()));
        }
    }
    let (params, _) = parse_method_descriptor(&method.descriptor)?;
    for param in params {
        push_local_type(&mut locals, param);
    }
    Ok(FrameState {
        locals,
        stack: Vec::new(),
    })
}

fn push_local_type(locals: &mut Vec<FrameType>, ty: FieldType) {
    match ty {
        FieldType::Long => {
            locals.push(FrameType::Long);
            locals.push(FrameType::Top);
        }
        FieldType::Double => {
            locals.push(FrameType::Double);
            locals.push(FrameType::Top);
        }
        FieldType::Float => locals.push(FrameType::Float),
        FieldType::Boolean
        | FieldType::Byte
        | FieldType::Char
        | FieldType::Short
        | FieldType::Int => locals.push(FrameType::Integer),
        FieldType::Object(name) => locals.push(FrameType::Object(name)),
        FieldType::Array(desc) => locals.push(FrameType::Object(desc)),
        FieldType::Void => {}
    }
}

#[derive(Debug, Clone)]
struct ExceptionHandlerInfo {
    start_pc: u16,
    end_pc: u16,
    handler_pc: u16,
    exception_type: FrameType,
}

impl ExceptionHandlerInfo {
    fn covers(&self, offset: u16) -> bool {
        offset >= self.start_pc && offset < self.end_pc
    }
}

fn build_exception_handlers(
    code: &CodeAttribute,
    cp: &[CpInfo],
) -> Result<Vec<ExceptionHandlerInfo>, ClassWriteError> {
    let mut handlers = Vec::new();
    for entry in &code.exception_table {
        let exception_type = if entry.catch_type == 0 {
            FrameType::Object("java/lang/Throwable".to_string())
        } else {
            let class_name = cp_class_name(cp, entry.catch_type)?;
            FrameType::Object(class_name.to_string())
        };
        handlers.push(ExceptionHandlerInfo {
            start_pc: entry.start_pc,
            end_pc: entry.end_pc,
            handler_pc: entry.handler_pc,
            exception_type,
        });
    }
    Ok(handlers)
}

fn handler_common_types(
    handlers: &[ExceptionHandlerInfo],
) -> std::collections::HashMap<u16, FrameType> {
    let mut map: std::collections::HashMap<u16, FrameType> = std::collections::HashMap::new();
    for handler in handlers {
        map.entry(handler.handler_pc)
            .and_modify(|existing| {
                *existing = merge_exception_type(existing, &handler.exception_type);
            })
            .or_insert_with(|| handler.exception_type.clone());
    }
    map
}

fn merge_exception_type(left: &FrameType, right: &FrameType) -> FrameType {
    match (left, right) {
        (FrameType::Object(l), FrameType::Object(r)) => FrameType::Object(common_superclass(l, r)),
        _ if left == right => left.clone(),
        _ => FrameType::Object("java/lang/Object".to_string()),
    }
}

fn dump_frame_debug(
    method: &MethodNode,
    label: &str,
    iterations: usize,
    hits: &std::collections::HashMap<u16, u32>,
) {
    let mut entries: Vec<(u16, u32)> = hits.iter().map(|(k, v)| (*k, *v)).collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1));
    let top = entries.into_iter().take(10).collect::<Vec<_>>();
    eprintln!(
        "[frame-debug] method={}{} label={} iterations={} top_offsets={:?}",
        method.name, method.descriptor, label, iterations, top
    );
}

#[derive(Debug, Clone)]
struct ParsedInstruction {
    offset: u16,
    opcode: u8,
    operand: Operand,
}

#[derive(Debug, Clone)]
enum Operand {
    None,
    I1(i8),
    I2(i16),
    I4(i32),
    U1(u8),
    U2(u16),
    U4(u32),
    Jump(i16),
    JumpWide(i32),
    TableSwitch {
        default_offset: i32,
        low: i32,
        high: i32,
        offsets: Vec<i32>,
    },
    LookupSwitch {
        default_offset: i32,
        pairs: Vec<(i32, i32)>,
    },
    Iinc {
        index: u16,
        increment: i16,
    },
    InvokeInterface {
        index: u16,
        count: u8,
    },
    InvokeDynamic {
        index: u16,
    },
    MultiANewArray {
        index: u16,
        dims: u8,
    },
    Wide {
        opcode: u8,
        index: u16,
        increment: Option<i16>,
    },
}

fn parse_instructions(code: &[u8]) -> Result<Vec<ParsedInstruction>, ClassWriteError> {
    let mut insns = Vec::new();
    let mut pos = 0usize;
    while pos < code.len() {
        let offset = pos as u16;
        let opcode = code[pos];
        pos += 1;
        let operand = match opcode {
            opcodes::BIPUSH => {
                let value = read_i1(code, &mut pos)?;
                Operand::I1(value)
            }
            opcodes::SIPUSH => Operand::I2(read_i2(code, &mut pos)?),
            opcodes::LDC => Operand::U1(read_u1(code, &mut pos)?),
            opcodes::LDC_W | opcodes::LDC2_W => Operand::U2(read_u2(code, &mut pos)?),
            opcodes::ILOAD..=opcodes::ALOAD | opcodes::ISTORE..=opcodes::ASTORE | opcodes::RET => {
                Operand::U1(read_u1(code, &mut pos)?)
            }
            opcodes::IINC => {
                let index = read_u1(code, &mut pos)? as u16;
                let inc = read_i1(code, &mut pos)? as i16;
                Operand::Iinc {
                    index,
                    increment: inc,
                }
            }
            opcodes::IFEQ..=opcodes::JSR | opcodes::IFNULL | opcodes::IFNONNULL => {
                Operand::Jump(read_i2(code, &mut pos)?)
            }
            opcodes::GOTO_W | opcodes::JSR_W => Operand::JumpWide(read_i4(code, &mut pos)?),
            opcodes::TABLESWITCH => {
                let padding = (4 - (pos % 4)) % 4;
                pos += padding;
                let default_offset = read_i4(code, &mut pos)?;
                let low = read_i4(code, &mut pos)?;
                let high = read_i4(code, &mut pos)?;
                let count = if high < low {
                    0
                } else {
                    (high - low + 1) as usize
                };
                let mut offsets = Vec::with_capacity(count);
                for _ in 0..count {
                    offsets.push(read_i4(code, &mut pos)?);
                }
                Operand::TableSwitch {
                    default_offset,
                    low,
                    high,
                    offsets,
                }
            }
            opcodes::LOOKUPSWITCH => {
                let padding = (4 - (pos % 4)) % 4;
                pos += padding;
                let default_offset = read_i4(code, &mut pos)?;
                let npairs = read_i4(code, &mut pos)? as usize;
                let mut pairs = Vec::with_capacity(npairs);
                for _ in 0..npairs {
                    let key = read_i4(code, &mut pos)?;
                    let value = read_i4(code, &mut pos)?;
                    pairs.push((key, value));
                }
                Operand::LookupSwitch {
                    default_offset,
                    pairs,
                }
            }
            opcodes::GETSTATIC..=opcodes::INVOKESTATIC
            | opcodes::NEW
            | opcodes::ANEWARRAY
            | opcodes::CHECKCAST
            | opcodes::INSTANCEOF => Operand::U2(read_u2(code, &mut pos)?),
            opcodes::INVOKEINTERFACE => {
                let index = read_u2(code, &mut pos)?;
                let count = read_u1(code, &mut pos)?;
                let _ = read_u1(code, &mut pos)?;
                Operand::InvokeInterface { index, count }
            }
            opcodes::INVOKEDYNAMIC => {
                let index = read_u2(code, &mut pos)?;
                let _ = read_u2(code, &mut pos)?;
                Operand::InvokeDynamic { index }
            }
            opcodes::NEWARRAY => Operand::U1(read_u1(code, &mut pos)?),
            opcodes::WIDE => {
                let wide_opcode = read_u1(code, &mut pos)?;
                match wide_opcode {
                    opcodes::ILOAD..=opcodes::ALOAD
                    | opcodes::ISTORE..=opcodes::ASTORE
                    | opcodes::RET => {
                        let index = read_u2(code, &mut pos)?;
                        Operand::Wide {
                            opcode: wide_opcode,
                            index,
                            increment: None,
                        }
                    }
                    opcodes::IINC => {
                        let index = read_u2(code, &mut pos)?;
                        let increment = read_i2(code, &mut pos)?;
                        Operand::Wide {
                            opcode: wide_opcode,
                            index,
                            increment: Some(increment),
                        }
                    }
                    _ => {
                        return Err(ClassWriteError::InvalidOpcode {
                            opcode: wide_opcode,
                            offset: pos - 1,
                        });
                    }
                }
            }
            opcodes::MULTIANEWARRAY => {
                let index = read_u2(code, &mut pos)?;
                let dims = read_u1(code, &mut pos)?;
                Operand::MultiANewArray { index, dims }
            }
            _ => Operand::None,
        };
        insns.push(ParsedInstruction {
            offset,
            opcode,
            operand,
        });
    }
    Ok(insns)
}

fn instruction_successors(insn: &ParsedInstruction) -> Vec<u16> {
    let mut successors = Vec::new();
    let next_offset = insn.offset.saturating_add(instruction_length(insn) as u16);
    match insn.opcode {
        opcodes::GOTO | opcodes::GOTO_W => {
            if let Some(target) = jump_target(insn) {
                successors.push(target);
            }
        }
        opcodes::JSR | opcodes::JSR_W => {
            if let Some(target) = jump_target(insn) {
                successors.push(target);
            }
            successors.push(next_offset);
        }
        opcodes::IFEQ..=opcodes::IF_ACMPNE | opcodes::IFNULL | opcodes::IFNONNULL => {
            if let Some(target) = jump_target(insn) {
                successors.push(target);
            }
            successors.push(next_offset);
        }
        opcodes::TABLESWITCH => {
            if let Operand::TableSwitch {
                default_offset,
                offsets,
                ..
            } = &insn.operand
            {
                successors.push((insn.offset as i32 + default_offset) as u16);
                for offset in offsets {
                    successors.push((insn.offset as i32 + *offset) as u16);
                }
            }
        }
        opcodes::LOOKUPSWITCH => {
            if let Operand::LookupSwitch {
                default_offset,
                pairs,
            } = &insn.operand
            {
                successors.push((insn.offset as i32 + default_offset) as u16);
                for (_, offset) in pairs {
                    successors.push((insn.offset as i32 + *offset) as u16);
                }
            }
        }
        opcodes::IRETURN..=opcodes::RETURN | opcodes::ATHROW => {}
        opcodes::MONITORENTER | opcodes::MONITOREXIT => {
            successors.push(next_offset);
        }
        _ => {
            if next_offset != insn.offset {
                successors.push(next_offset);
            }
        }
    }
    successors
}

fn jump_target(insn: &ParsedInstruction) -> Option<u16> {
    match insn.operand {
        Operand::Jump(offset) => Some((insn.offset as i32 + offset as i32) as u16),
        Operand::JumpWide(offset) => Some((insn.offset as i32 + offset) as u16),
        _ => None,
    }
}

fn instruction_length(insn: &ParsedInstruction) -> usize {
    match &insn.operand {
        Operand::None => 1,
        Operand::I1(_) | Operand::U1(_) => 2,
        Operand::I2(_) | Operand::U2(_) | Operand::Jump(_) => 3,
        Operand::I4(_) | Operand::U4(_) | Operand::JumpWide(_) => 5,
        Operand::Iinc { .. } => 3,
        Operand::InvokeInterface { .. } => 5,
        Operand::InvokeDynamic { .. } => 5,
        Operand::MultiANewArray { .. } => 4,
        Operand::Wide {
            opcode, increment, ..
        } => {
            if *opcode == opcodes::IINC && increment.is_some() {
                6
            } else {
                4
            }
        }
        Operand::TableSwitch { offsets, .. } => {
            1 + switch_padding(insn.offset) + 12 + offsets.len() * 4
        }
        Operand::LookupSwitch { pairs, .. } => {
            1 + switch_padding(insn.offset) + 8 + pairs.len() * 8
        }
    }
}

fn switch_padding(offset: u16) -> usize {
    let pos = (offset as usize + 1) % 4;
    (4 - pos) % 4
}

fn execute_instruction(
    insn: &ParsedInstruction,
    frame: &FrameState,
    class_node: &ClassNode,
    cp: &[CpInfo],
) -> Result<FrameState, ClassWriteError> {
    let mut locals = frame.locals.clone();
    let mut stack = frame.stack.clone();

    let pop = |stack: &mut Vec<FrameType>| {
        stack.pop().ok_or_else(|| {
            ClassWriteError::FrameComputation(format!("stack underflow at {}", insn.offset))
        })
    };

    match insn.opcode {
        opcodes::NOP => {}
        opcodes::ACONST_NULL => stack.push(FrameType::Null),
        opcodes::ICONST_M1..=opcodes::ICONST_5 => stack.push(FrameType::Integer),
        opcodes::LCONST_0 | opcodes::LCONST_1 => stack.push(FrameType::Long),
        opcodes::FCONST_0..=opcodes::FCONST_2 => stack.push(FrameType::Float),
        opcodes::DCONST_0 | opcodes::DCONST_1 => stack.push(FrameType::Double),
        opcodes::BIPUSH => stack.push(FrameType::Integer),
        opcodes::SIPUSH => stack.push(FrameType::Integer),
        opcodes::LDC..=opcodes::LDC2_W => {
            let ty = ldc_type(insn, cp)?;
            stack.push(ty);
        }
        opcodes::ILOAD..=opcodes::ALOAD => {
            let index = var_index(insn)?;
            if let Some(value) = locals.get(index as usize) {
                stack.push(value.clone());
            } else {
                stack.push(FrameType::Top);
            }
        }
        opcodes::ILOAD_0..=opcodes::ILOAD_3 => stack.push(load_local(
            &locals,
            (insn.opcode - opcodes::ILOAD_0) as u16,
            FrameType::Integer,
        )),
        opcodes::LLOAD_0..=opcodes::LLOAD_3 => stack.push(load_local(
            &locals,
            (insn.opcode - opcodes::LLOAD_0) as u16,
            FrameType::Long,
        )),
        opcodes::FLOAD_0..=opcodes::FLOAD_3 => stack.push(load_local(
            &locals,
            (insn.opcode - opcodes::FLOAD_0) as u16,
            FrameType::Float,
        )),
        opcodes::DLOAD_0..=opcodes::DLOAD_3 => stack.push(load_local(
            &locals,
            (insn.opcode - opcodes::DLOAD_0) as u16,
            FrameType::Double,
        )),
        opcodes::ALOAD_0..=opcodes::ALOAD_3 => stack.push(load_local(
            &locals,
            (insn.opcode - opcodes::ALOAD_0) as u16,
            FrameType::Object(class_node.name.clone()),
        )),
        opcodes::IALOAD..=opcodes::SALOAD => {
            pop(&mut stack)?;
            let array_ref = pop(&mut stack)?; //fixed: array -> java/lang/Object.
            let ty = match insn.opcode {
                opcodes::IALOAD => FrameType::Integer,
                opcodes::LALOAD => FrameType::Long,
                opcodes::FALOAD => FrameType::Float,
                opcodes::DALOAD => FrameType::Double,
                opcodes::AALOAD => array_element_type(&array_ref)
                    .unwrap_or_else(|| FrameType::Object("java/lang/Object".to_string())),
                opcodes::BALOAD..=opcodes::SALOAD => FrameType::Integer,
                _ => FrameType::Top,
            };
            stack.push(ty);
        }
        opcodes::ISTORE..=opcodes::ASTORE => {
            let index = var_index(insn)?;
            let value = pop(&mut stack)?;
            store_local(&mut locals, index, value);
        }
        opcodes::ISTORE_0..=opcodes::ISTORE_3 => {
            let value = pop(&mut stack)?;
            store_local(&mut locals, (insn.opcode - opcodes::ISTORE_0) as u16, value);
        }
        opcodes::LSTORE_0..=opcodes::LSTORE_3 => {
            let value = pop(&mut stack)?;
            store_local(&mut locals, (insn.opcode - opcodes::LSTORE_0) as u16, value);
        }
        opcodes::FSTORE_0..=opcodes::FSTORE_3 => {
            let value = pop(&mut stack)?;
            store_local(&mut locals, (insn.opcode - opcodes::FSTORE_0) as u16, value);
        }
        opcodes::DSTORE_0..=opcodes::DSTORE_3 => {
            let value = pop(&mut stack)?;
            store_local(&mut locals, (insn.opcode - opcodes::DSTORE_0) as u16, value);
        }
        opcodes::ASTORE_0..=opcodes::ASTORE_3 => {
            let value = pop(&mut stack)?;
            store_local(&mut locals, (insn.opcode - opcodes::ASTORE_0) as u16, value);
        }
        opcodes::IASTORE..=opcodes::SASTORE => {
            pop(&mut stack)?;
            pop(&mut stack)?;
            pop(&mut stack)?;
        }
        opcodes::POP => {
            pop(&mut stack)?;
        }
        opcodes::POP2 => {
            let v1 = pop(&mut stack)?;
            if is_category2(&v1) {
                // Form 2: ..., value2(cat2) -> ...
                // already popped
            } else {
                let v2 = pop(&mut stack)?;
                if is_category2(&v2) {
                    return Err(ClassWriteError::FrameComputation(
                        "pop2 invalid".to_string(),
                    ));
                }
            }
        }
        opcodes::DUP => {
            let v1 = pop(&mut stack)?;
            if is_category2(&v1) {
                return Err(ClassWriteError::FrameComputation(
                    "dup category2".to_string(),
                ));
            }
            stack.push(v1.clone());
            stack.push(v1);
        }
        opcodes::DUP_X1 => {
            let v1 = pop(&mut stack)?;
            let v2 = pop(&mut stack)?;
            if is_category2(&v1) || is_category2(&v2) {
                return Err(ClassWriteError::FrameComputation("dup_x1".to_string()));
            }
            stack.push(v1.clone());
            stack.push(v2);
            stack.push(v1);
        }
        opcodes::DUP_X2 => {
            let v1 = pop(&mut stack)?;
            if is_category2(&v1) {
                return Err(ClassWriteError::FrameComputation("dup_x2".to_string()));
            }
            let v2 = pop(&mut stack)?;
            if is_category2(&v2) {
                // Form 2: ..., v2(cat2), v1(cat1) -> ..., v1, v2, v1
                stack.push(v1.clone());
                stack.push(v2);
                stack.push(v1);
            } else {
                // Form 1/3: ..., v3(cat1|cat2), v2(cat1), v1(cat1) -> ..., v1, v3, v2, v1
                let v3 = pop(&mut stack)?;
                stack.push(v1.clone());
                stack.push(v3);
                stack.push(v2);
                stack.push(v1);
            }
        }
        opcodes::DUP2 => {
            let v1 = pop(&mut stack)?;
            if is_category2(&v1) {
                stack.push(v1.clone());
                stack.push(v1);
            } else {
                let v2 = pop(&mut stack)?;
                if is_category2(&v2) {
                    return Err(ClassWriteError::FrameComputation("dup2".to_string()));
                }
                stack.push(v2.clone());
                stack.push(v1.clone());
                stack.push(v2);
                stack.push(v1);
            }
        }
        opcodes::DUP2_X1 => {
            let v1 = pop(&mut stack)?;
            if is_category2(&v1) {
                let v2 = pop(&mut stack)?;
                stack.push(v1.clone());
                stack.push(v2);
                stack.push(v1);
            } else {
                let v2 = pop(&mut stack)?;
                let v3 = pop(&mut stack)?;
                stack.push(v2.clone());
                stack.push(v1.clone());
                stack.push(v3);
                stack.push(v2);
                stack.push(v1);
            }
        }
        opcodes::DUP2_X2 => {
            let v1 = pop(&mut stack)?;
            if is_category2(&v1) {
                let v2 = pop(&mut stack)?;
                let v3 = pop(&mut stack)?;
                stack.push(v1.clone());
                stack.push(v3);
                stack.push(v2);
                stack.push(v1);
            } else {
                let v2 = pop(&mut stack)?;
                let v3 = pop(&mut stack)?;
                let v4 = pop(&mut stack)?;
                stack.push(v2.clone());
                stack.push(v1.clone());
                stack.push(v4);
                stack.push(v3);
                stack.push(v2);
                stack.push(v1);
            }
        }
        opcodes::SWAP => {
            let v1 = pop(&mut stack)?;
            let v2 = pop(&mut stack)?;
            if is_category2(&v1) || is_category2(&v2) {
                return Err(ClassWriteError::FrameComputation("swap".to_string()));
            }
            stack.push(v1);
            stack.push(v2);
        }
        opcodes::IADD
        | opcodes::ISUB
        | opcodes::IMUL
        | opcodes::IDIV
        | opcodes::IREM
        | opcodes::ISHL
        | opcodes::ISHR
        | opcodes::IUSHR
        | opcodes::IAND
        | opcodes::IOR
        | opcodes::IXOR => {
            pop(&mut stack)?;
            pop(&mut stack)?;
            stack.push(FrameType::Integer);
        }
        opcodes::LADD
        | opcodes::LSUB
        | opcodes::LMUL
        | opcodes::LDIV
        | opcodes::LREM
        | opcodes::LSHL
        | opcodes::LSHR
        | opcodes::LUSHR
        | opcodes::LAND
        | opcodes::LOR
        | opcodes::LXOR => {
            pop(&mut stack)?;
            pop(&mut stack)?;
            stack.push(FrameType::Long);
        }
        opcodes::FADD | opcodes::FSUB | opcodes::FMUL | opcodes::FDIV | opcodes::FREM => {
            pop(&mut stack)?;
            pop(&mut stack)?;
            stack.push(FrameType::Float);
        }
        opcodes::DADD | opcodes::DSUB | opcodes::DMUL | opcodes::DDIV | opcodes::DREM => {
            pop(&mut stack)?;
            pop(&mut stack)?;
            stack.push(FrameType::Double);
        }
        opcodes::INEG => {
            pop(&mut stack)?;
            stack.push(FrameType::Integer);
        }
        opcodes::LNEG => {
            pop(&mut stack)?;
            stack.push(FrameType::Long);
        }
        opcodes::FNEG => {
            pop(&mut stack)?;
            stack.push(FrameType::Float);
        }
        opcodes::DNEG => {
            pop(&mut stack)?;
            stack.push(FrameType::Double);
        }
        opcodes::IINC => {}
        opcodes::I2L => {
            pop(&mut stack)?;
            stack.push(FrameType::Long);
        }
        opcodes::I2F => {
            pop(&mut stack)?;
            stack.push(FrameType::Float);
        }
        opcodes::I2D => {
            pop(&mut stack)?;
            stack.push(FrameType::Double);
        }
        opcodes::L2I => {
            pop(&mut stack)?;
            stack.push(FrameType::Integer);
        }
        opcodes::L2F => {
            pop(&mut stack)?;
            stack.push(FrameType::Float);
        }
        opcodes::L2D => {
            pop(&mut stack)?;
            stack.push(FrameType::Double);
        }
        opcodes::F2I => {
            pop(&mut stack)?;
            stack.push(FrameType::Integer);
        }
        opcodes::F2L => {
            pop(&mut stack)?;
            stack.push(FrameType::Long);
        }
        opcodes::F2D => {
            pop(&mut stack)?;
            stack.push(FrameType::Double);
        }
        opcodes::D2I => {
            pop(&mut stack)?;
            stack.push(FrameType::Integer);
        }
        opcodes::D2L => {
            pop(&mut stack)?;
            stack.push(FrameType::Long);
        }
        opcodes::D2F => {
            pop(&mut stack)?;
            stack.push(FrameType::Float);
        }
        opcodes::I2B..=opcodes::I2S => {
            pop(&mut stack)?;
            stack.push(FrameType::Integer);
        }
        opcodes::LCMP..=opcodes::DCMPG => {
            pop(&mut stack)?;
            pop(&mut stack)?;
            stack.push(FrameType::Integer);
        }
        opcodes::IFEQ..=opcodes::IFLE | opcodes::IFNULL | opcodes::IFNONNULL => {
            pop(&mut stack)?;
        }
        opcodes::IF_ICMPEQ..=opcodes::IF_ACMPNE => {
            pop(&mut stack)?;
            pop(&mut stack)?;
        }
        opcodes::GOTO | opcodes::GOTO_W => {}
        opcodes::JSR | opcodes::RET | opcodes::JSR_W => {
            return Err(ClassWriteError::FrameComputation(format!(
                "jsr/ret not supported at {}",
                insn.offset
            )));
        }
        opcodes::TABLESWITCH | opcodes::LOOKUPSWITCH => {
            pop(&mut stack)?;
        }
        opcodes::IRETURN => {
            pop(&mut stack)?;
        }
        opcodes::LRETURN => {
            pop(&mut stack)?;
        }
        opcodes::FRETURN => {
            pop(&mut stack)?;
        }
        opcodes::DRETURN => {
            pop(&mut stack)?;
        }
        opcodes::ARETURN => {
            pop(&mut stack)?;
        }
        opcodes::RETURN => {}
        opcodes::GETSTATIC => {
            let ty = field_type(insn, cp)?;
            stack.push(ty);
        }
        opcodes::PUTSTATIC => {
            pop(&mut stack)?;
        }
        opcodes::GETFIELD => {
            pop(&mut stack)?;
            let ty = field_type(insn, cp)?;
            stack.push(ty);
        }
        opcodes::PUTFIELD => {
            pop(&mut stack)?;
            pop(&mut stack)?;
        }
        opcodes::INVOKEVIRTUAL..=opcodes::INVOKEDYNAMIC => {
            let (args, ret, owner, is_init) = method_type(insn, cp)?;
            for _ in 0..args.len() {
                pop(&mut stack)?;
            }
            if insn.opcode != opcodes::INVOKESTATIC && insn.opcode != opcodes::INVOKEDYNAMIC {
                let receiver = pop(&mut stack)?;
                if is_init {
                    let init_owner = if receiver == FrameType::UninitializedThis {
                        class_node.name.clone()
                    } else {
                        owner
                    };
                    initialize_uninitialized(&mut locals, &mut stack, receiver, init_owner);
                }
            }
            if let Some(ret) = ret {
                stack.push(ret);
            }
        }
        opcodes::NEW => {
            if let Operand::U2(_index) = insn.operand {
                stack.push(FrameType::Uninitialized(insn.offset));
            }
        }
        opcodes::NEWARRAY => {
            pop(&mut stack)?;
            if let Operand::U1(atype) = insn.operand {
                let desc = newarray_descriptor(atype)?;
                stack.push(FrameType::Object(desc));
            } else {
                stack.push(FrameType::Object("[I".to_string()));
            }
        }
        opcodes::ANEWARRAY => {
            pop(&mut stack)?;
            if let Operand::U2(index) = insn.operand {
                let class_name = cp_class_name(cp, index)?;
                stack.push(FrameType::Object(format!("[L{class_name};")));
            }
        }
        opcodes::ARRAYLENGTH => {
            pop(&mut stack)?;
            stack.push(FrameType::Integer);
        }
        opcodes::ATHROW => {
            pop(&mut stack)?;
        }
        opcodes::CHECKCAST => {
            pop(&mut stack)?;
            if let Operand::U2(index) = insn.operand {
                let class_name = cp_class_name(cp, index)?;
                stack.push(FrameType::Object(class_name.to_string()));
            }
        }
        opcodes::INSTANCEOF => {
            pop(&mut stack)?;
            stack.push(FrameType::Integer);
        }
        opcodes::MONITORENTER | opcodes::MONITOREXIT => {
            pop(&mut stack)?;
        }
        opcodes::WIDE => {
            if let Operand::Wide {
                opcode,
                index,
                increment,
            } = insn.operand
            {
                match opcode {
                    opcodes::ILOAD..=opcodes::ALOAD => {
                        if let Some(value) = locals.get(index as usize) {
                            stack.push(value.clone());
                        }
                    }
                    opcodes::ISTORE..=opcodes::ASTORE => {
                        let value = pop(&mut stack)?;
                        store_local(&mut locals, index, value);
                    }
                    opcodes::IINC => {
                        let _ = increment;
                    }
                    opcodes::RET => {}
                    _ => {}
                }
            }
        }
        opcodes::MULTIANEWARRAY => {
            if let Operand::MultiANewArray { dims, .. } = insn.operand {
                for _ in 0..dims {
                    pop(&mut stack)?;
                }
                if let Operand::MultiANewArray { index, .. } = insn.operand {
                    let desc = cp_class_name(cp, index)?;
                    stack.push(FrameType::Object(desc.to_string()));
                } else {
                    stack.push(FrameType::Object("[Ljava/lang/Object;".to_string()));
                }
            }
        }
        opcodes::BREAKPOINT | opcodes::IMPDEP1 | opcodes::IMPDEP2 => {}
        _ => {}
    }

    Ok(FrameState { locals, stack })
}

fn initialize_uninitialized(
    locals: &mut [FrameType],
    stack: &mut [FrameType],
    receiver: FrameType,
    owner: String,
) {
    let init = FrameType::Object(owner);
    for value in locals.iter_mut().chain(stack.iter_mut()) {
        if *value == receiver {
            *value = init.clone();
        }
    }
}

fn is_category2(value: &FrameType) -> bool {
    matches!(value, FrameType::Long | FrameType::Double)
}

fn load_local(locals: &[FrameType], index: u16, fallback: FrameType) -> FrameType {
    locals.get(index as usize).cloned().unwrap_or(fallback)
}

fn store_local(locals: &mut Vec<FrameType>, index: u16, value: FrameType) {
    let idx = index as usize;
    if locals.len() <= idx {
        locals.resize(idx + 1, FrameType::Top);
    }
    locals[idx] = value.clone();
    if is_category2(&value) {
        if locals.len() <= idx + 1 {
            locals.resize(idx + 2, FrameType::Top);
        }
        locals[idx + 1] = FrameType::Top;
    }
}

fn array_element_type(value: &FrameType) -> Option<FrameType> {
    let FrameType::Object(desc) = value else {
        return None;
    };
    if !desc.starts_with('[') {
        return None;
    }
    let element = &desc[1..];
    if element.starts_with('[') {
        return Some(FrameType::Object(element.to_string()));
    }
    let mut chars = element.chars();
    match chars.next() {
        Some('L') => {
            let name = element
                .trim_start_matches('L')
                .trim_end_matches(';')
                .to_string();
            Some(FrameType::Object(name))
        }
        Some('Z') | Some('B') | Some('C') | Some('S') | Some('I') => Some(FrameType::Integer),
        Some('F') => Some(FrameType::Float),
        Some('J') => Some(FrameType::Long),
        Some('D') => Some(FrameType::Double),
        _ => None,
    }
}

fn var_index(insn: &ParsedInstruction) -> Result<u16, ClassWriteError> {
    match insn.operand {
        Operand::U1(value) => Ok(value as u16),
        Operand::Wide { index, .. } => Ok(index),
        _ => Err(ClassWriteError::FrameComputation(format!(
            "missing var index at {}",
            insn.offset
        ))),
    }
}

fn ldc_type(insn: &ParsedInstruction, cp: &[CpInfo]) -> Result<FrameType, ClassWriteError> {
    let index = match insn.operand {
        Operand::U1(value) => value as u16,
        Operand::U2(value) => value,
        _ => {
            return Err(ClassWriteError::FrameComputation(format!(
                "invalid ldc at {}",
                insn.offset
            )));
        }
    };
    match cp.get(index as usize) {
        Some(CpInfo::Integer(_)) => Ok(FrameType::Integer),
        Some(CpInfo::Float(_)) => Ok(FrameType::Float),
        Some(CpInfo::Long(_)) => Ok(FrameType::Long),
        Some(CpInfo::Double(_)) => Ok(FrameType::Double),
        Some(CpInfo::String { .. }) => Ok(FrameType::Object("java/lang/String".to_string())),
        Some(CpInfo::Class { .. }) => Ok(FrameType::Object("java/lang/Class".to_string())),
        Some(CpInfo::MethodType { .. }) => {
            Ok(FrameType::Object("java/lang/invoke/MethodType".to_string()))
        }
        Some(CpInfo::MethodHandle { .. }) => Ok(FrameType::Object(
            "java/lang/invoke/MethodHandle".to_string(),
        )),
        _ => Ok(FrameType::Top),
    }
}

fn field_type(insn: &ParsedInstruction, cp: &[CpInfo]) -> Result<FrameType, ClassWriteError> {
    let index = match insn.operand {
        Operand::U2(value) => value,
        _ => {
            return Err(ClassWriteError::FrameComputation(format!(
                "invalid field operand at {}",
                insn.offset
            )));
        }
    };
    let descriptor = cp_field_descriptor(cp, index)?;
    let field_type = parse_field_descriptor(descriptor)?;
    Ok(field_type_to_frame(field_type))
}

fn method_type(
    insn: &ParsedInstruction,
    cp: &[CpInfo],
) -> Result<(Vec<FieldType>, Option<FrameType>, String, bool), ClassWriteError> {
    let index = match insn.operand {
        Operand::U2(value) => value,
        Operand::InvokeInterface { index, .. } => index,
        Operand::InvokeDynamic { index } => index,
        _ => {
            return Err(ClassWriteError::FrameComputation(format!(
                "invalid method operand at {}",
                insn.offset
            )));
        }
    };
    let (owner, descriptor, name) = cp_method_descriptor(cp, index, insn.opcode)?;
    let (args, ret) = parse_method_descriptor(descriptor)?;
    let ret_frame = match ret {
        FieldType::Void => None,
        other => Some(field_type_to_frame(other)),
    };
    Ok((args, ret_frame, owner.to_string(), name == "<init>"))
}

fn field_type_to_frame(field_type: FieldType) -> FrameType {
    match field_type {
        FieldType::Boolean
        | FieldType::Byte
        | FieldType::Char
        | FieldType::Short
        | FieldType::Int => FrameType::Integer,
        FieldType::Float => FrameType::Float,
        FieldType::Long => FrameType::Long,
        FieldType::Double => FrameType::Double,
        FieldType::Object(name) => FrameType::Object(name),
        FieldType::Array(desc) => FrameType::Object(desc),
        FieldType::Void => FrameType::Top,
    }
}

fn cp_class_name(cp: &[CpInfo], index: u16) -> Result<&str, ClassWriteError> {
    match cp.get(index as usize) {
        Some(CpInfo::Class { name_index }) => match cp.get(*name_index as usize) {
            Some(CpInfo::Utf8(name)) => Ok(name),
            _ => Err(ClassWriteError::InvalidConstantPool),
        },
        _ => Err(ClassWriteError::InvalidConstantPool),
    }
}

fn newarray_descriptor(atype: u8) -> Result<String, ClassWriteError> {
    let desc = match atype {
        4 => "[Z",
        5 => "[C",
        6 => "[F",
        7 => "[D",
        8 => "[B",
        9 => "[S",
        10 => "[I",
        11 => "[J",
        _ => {
            return Err(ClassWriteError::FrameComputation(
                "invalid newarray type".to_string(),
            ));
        }
    };
    Ok(desc.to_string())
}

fn cp_field_descriptor(cp: &[CpInfo], index: u16) -> Result<&str, ClassWriteError> {
    match cp.get(index as usize) {
        Some(CpInfo::Fieldref {
            name_and_type_index,
            ..
        }) => match cp.get(*name_and_type_index as usize) {
            Some(CpInfo::NameAndType {
                descriptor_index, ..
            }) => match cp.get(*descriptor_index as usize) {
                Some(CpInfo::Utf8(desc)) => Ok(desc),
                _ => Err(ClassWriteError::InvalidConstantPool),
            },
            _ => Err(ClassWriteError::InvalidConstantPool),
        },
        _ => Err(ClassWriteError::InvalidConstantPool),
    }
}

fn cp_method_descriptor(
    cp: &[CpInfo],
    index: u16,
    opcode: u8,
) -> Result<(&str, &str, &str), ClassWriteError> {
    match cp.get(index as usize) {
        Some(CpInfo::Methodref {
            class_index,
            name_and_type_index,
        })
        | Some(CpInfo::InterfaceMethodref {
            class_index,
            name_and_type_index,
        }) => {
            let owner = cp_class_name(cp, *class_index)?;
            match cp.get(*name_and_type_index as usize) {
                Some(CpInfo::NameAndType {
                    name_index,
                    descriptor_index,
                }) => {
                    let name = cp_utf8(cp, *name_index)?;
                    let desc = cp_utf8(cp, *descriptor_index)?;
                    Ok((owner, desc, name))
                }
                _ => Err(ClassWriteError::InvalidConstantPool),
            }
        }
        Some(CpInfo::InvokeDynamic {
            name_and_type_index,
            ..
        }) if opcode == opcodes::INVOKEDYNAMIC => match cp.get(*name_and_type_index as usize) {
            Some(CpInfo::NameAndType {
                name_index,
                descriptor_index,
            }) => {
                let name = cp_utf8(cp, *name_index)?;
                let desc = cp_utf8(cp, *descriptor_index)?;
                Ok(("java/lang/Object", desc, name))
            }
            _ => Err(ClassWriteError::InvalidConstantPool),
        },
        _ => Err(ClassWriteError::InvalidConstantPool),
    }
}

fn cp_utf8(cp: &[CpInfo], index: u16) -> Result<&str, ClassWriteError> {
    match cp.get(index as usize) {
        Some(CpInfo::Utf8(value)) => Ok(value.as_str()),
        _ => Err(ClassWriteError::InvalidConstantPool),
    }
}

#[derive(Debug, Clone)]
enum FieldType {
    Boolean,
    Byte,
    Char,
    Short,
    Int,
    Float,
    Long,
    Double,
    Object(String),
    Array(String),
    Void,
}

fn parse_field_descriptor(desc: &str) -> Result<FieldType, ClassWriteError> {
    let mut chars = desc.chars().peekable();
    parse_field_type(&mut chars)
}

fn parse_method_descriptor(desc: &str) -> Result<(Vec<FieldType>, FieldType), ClassWriteError> {
    let mut chars = desc.chars().peekable();
    if chars.next() != Some('(') {
        return Err(ClassWriteError::FrameComputation(
            "bad method descriptor".to_string(),
        ));
    }
    let mut params = Vec::new();
    while let Some(&ch) = chars.peek() {
        if ch == ')' {
            chars.next();
            break;
        }
        params.push(parse_field_type(&mut chars)?);
    }
    let ret = parse_return_type(&mut chars)?;
    Ok((params, ret))
}

fn parse_field_type<I>(chars: &mut std::iter::Peekable<I>) -> Result<FieldType, ClassWriteError>
where
    I: Iterator<Item = char>,
{
    match chars.next() {
        Some('Z') => Ok(FieldType::Boolean),
        Some('B') => Ok(FieldType::Byte),
        Some('C') => Ok(FieldType::Char),
        Some('S') => Ok(FieldType::Short),
        Some('I') => Ok(FieldType::Int),
        Some('F') => Ok(FieldType::Float),
        Some('J') => Ok(FieldType::Long),
        Some('D') => Ok(FieldType::Double),
        Some('L') => {
            let mut name = String::new();
            for ch in chars.by_ref() {
                if ch == ';' {
                    break;
                }
                name.push(ch);
            }
            Ok(FieldType::Object(name))
        }
        Some('[') => {
            let mut desc = String::from("[");
            let inner = parse_field_type(chars)?;
            match inner {
                FieldType::Object(name) => {
                    desc.push('L');
                    desc.push_str(&name);
                    desc.push(';');
                }
                FieldType::Boolean => desc.push('Z'),
                FieldType::Byte => desc.push('B'),
                FieldType::Char => desc.push('C'),
                FieldType::Short => desc.push('S'),
                FieldType::Int => desc.push('I'),
                FieldType::Float => desc.push('F'),
                FieldType::Long => desc.push('J'),
                FieldType::Double => desc.push('D'),
                FieldType::Void => {}
                FieldType::Array(inner_desc) => desc.push_str(&inner_desc),
            }
            Ok(FieldType::Array(desc))
        }
        _ => Err(ClassWriteError::FrameComputation(
            "bad field descriptor".to_string(),
        )),
    }
}

fn parse_return_type<I>(chars: &mut std::iter::Peekable<I>) -> Result<FieldType, ClassWriteError>
where
    I: Iterator<Item = char>,
{
    match chars.peek() {
        Some('V') => {
            chars.next();
            Ok(FieldType::Void)
        }
        _ => parse_field_type(chars),
    }
}

fn read_u1(code: &[u8], pos: &mut usize) -> Result<u8, ClassWriteError> {
    if *pos >= code.len() {
        return Err(ClassWriteError::FrameComputation(
            "unexpected eof".to_string(),
        ));
    }
    let value = code[*pos];
    *pos += 1;
    Ok(value)
}

fn read_i1(code: &[u8], pos: &mut usize) -> Result<i8, ClassWriteError> {
    Ok(read_u1(code, pos)? as i8)
}

fn read_u2(code: &[u8], pos: &mut usize) -> Result<u16, ClassWriteError> {
    if *pos + 2 > code.len() {
        return Err(ClassWriteError::FrameComputation(
            "unexpected eof".to_string(),
        ));
    }
    let value = u16::from_be_bytes([code[*pos], code[*pos + 1]]);
    *pos += 2;
    Ok(value)
}

fn read_i2(code: &[u8], pos: &mut usize) -> Result<i16, ClassWriteError> {
    Ok(read_u2(code, pos)? as i16)
}

fn read_i4(code: &[u8], pos: &mut usize) -> Result<i32, ClassWriteError> {
    if *pos + 4 > code.len() {
        return Err(ClassWriteError::FrameComputation(
            "unexpected eof".to_string(),
        ));
    }
    let value = i32::from_be_bytes([code[*pos], code[*pos + 1], code[*pos + 2], code[*pos + 3]]);
    *pos += 4;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opcodes;

    #[test]
    fn test_constant_pool_deduplication() {
        let mut cp = ConstantPoolBuilder::new();
        let i1 = cp.utf8("Hello");
        let i2 = cp.utf8("World");
        let i3 = cp.utf8("Hello");

        assert_eq!(i1, 1);
        assert_eq!(i2, 2);
        assert_eq!(i3, 1, "Duplicate UTF8 should return existing index");

        let c1 = cp.class("java/lang/Object");
        let c2 = cp.class("java/lang/Object");
        assert_eq!(c1, c2, "Duplicate Class should return existing index");
    }

    #[test]
    fn test_basic_class_generation() {
        let mut cw = ClassWriter::new(0);
        cw.visit(52, 0, 0x0001, "TestClass", Some("java/lang/Object"), &[]);
        cw.visit_source_file("TestClass.java");

        // Add a field
        let mut fv = cw.visit_field(0x0002, "myField", "I");
        fv.visit_end(&mut cw);

        // Add a default constructor
        let mut mv = cw.visit_method(0x0001, "<init>", "()V");
        mv.visit_code();
        mv.visit_var_insn(opcodes::ALOAD, 0);
        mv.visit_method_insn(
            opcodes::INVOKESPECIAL,
            "java/lang/Object",
            "<init>",
            "()V",
            false,
        );
        mv.visit_insn(opcodes::RETURN);
        mv.visit_maxs(1, 1);
        mv.visit_end(&mut cw);

        let result = cw.to_bytes();
        assert!(result.is_ok(), "Should generate bytes successfully");

        let bytes = result.unwrap();
        assert!(bytes.len() > 4);
        assert_eq!(&bytes[0..4], &[0xCA, 0xFE, 0xBA, 0xBE]); // Magic number
    }

    #[test]
    fn test_compute_frames_flag() {
        // Simple linear code, but checking if logic runs without panic
        let mut cw = ClassWriter::new(COMPUTE_FRAMES);
        cw.visit(52, 0, 0x0001, "FrameTest", Some("java/lang/Object"), &[]);

        let mut mv = cw.visit_method(0x0009, "main", "([Ljava/lang/String;)V");
        mv.visit_code();
        mv.visit_field_insn(
            opcodes::GETSTATIC,
            "java/lang/System",
            "out",
            "Ljava/io/PrintStream;",
        );
        mv.visit_ldc_insn(LdcInsnNode::string("Hello"));
        mv.visit_method_insn(
            opcodes::INVOKEVIRTUAL,
            "java/io/PrintStream",
            "println",
            "(Ljava/lang/String;)V",
            false,
        );
        mv.visit_insn(opcodes::RETURN);
        // maxs should be ignored/recomputed
        mv.visit_maxs(0, 0);
        mv.visit_end(&mut cw);

        let result = cw.to_bytes();
        assert!(result.is_ok());
    }

    #[test]
    fn test_class_node_structure() {
        let mut cw = ClassWriter::new(0);
        cw.visit(52, 0, 0, "MyNode", None, &[]);

        let node = cw.to_class_node().expect("Should create class node");
        assert_eq!(node.name, "MyNode");
        assert_eq!(node.major_version, 52);
    }
}
