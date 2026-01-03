// Generated from
// https://gitlab.ow2.org/asm/asm/-/blob/af1a46a8ac94b06a4f99144ddf3e64d5939789b6/asm/src/main/java/org/objectweb/asm/Constants.java
#![allow(dead_code, non_camel_case_types)]

use byteorder::{BigEndian, ReadBytesExt};
use once_cell::sync::Lazy;
use regex::Regex;
use std::any::Any;
use std::io::Read;

/// Defines additional JVM opcodes, access flags and constants which are not part of the ASM public
/// API.
///
/// See also: <a href="https://docs.oracle.com/javase/specs/jvms/se11/html/jvms-6.html">JVMS 6</a>
/// @author Eric Bruneton

// The ClassFile attribute names, in the order they are defined in
// https://docs.oracle.com/javase/specs/jvms/se11/html/jvms-4.html#jvms-4.7-300.

pub const CONSTANT_VALUE: &str = "ConstantValue";
pub const CODE: &str = "Code";
pub const STACK_MAP_TABLE: &str = "StackMapTable";
pub const EXCEPTIONS: &str = "Exceptions";
pub const INNER_CLASSES: &str = "InnerClasses";
pub const ENCLOSING_METHOD: &str = "EnclosingMethod";
pub const SYNTHETIC: &str = "Synthetic";
pub const SIGNATURE: &str = "Signature";
pub const SOURCE_FILE: &str = "SourceFile";
pub const SOURCE_DEBUG_EXTENSION: &str = "SourceDebugExtension";
pub const LINE_NUMBER_TABLE: &str = "LineNumberTable";
pub const LOCAL_VARIABLE_TABLE: &str = "LocalVariableTable";
pub const LOCAL_VARIABLE_TYPE_TABLE: &str = "LocalVariableTypeTable";
pub const DEPRECATED: &str = "Deprecated";
pub const RUNTIME_VISIBLE_ANNOTATIONS: &str = "RuntimeVisibleAnnotations";
pub const RUNTIME_INVISIBLE_ANNOTATIONS: &str = "RuntimeInvisibleAnnotations";
pub const RUNTIME_VISIBLE_PARAMETER_ANNOTATIONS: &str = "RuntimeVisibleParameterAnnotations";
pub const RUNTIME_INVISIBLE_PARAMETER_ANNOTATIONS: &str = "RuntimeInvisibleParameterAnnotations";
pub const RUNTIME_VISIBLE_TYPE_ANNOTATIONS: &str = "RuntimeVisibleTypeAnnotations";
pub const RUNTIME_INVISIBLE_TYPE_ANNOTATIONS: &str = "RuntimeInvisibleTypeAnnotations";
pub const ANNOTATION_DEFAULT: &str = "AnnotationDefault";
pub const BOOTSTRAP_METHODS: &str = "BootstrapMethods";
pub const METHOD_PARAMETERS: &str = "MethodParameters";
pub const MODULE: &str = "Module";
pub const MODULE_PACKAGES: &str = "ModulePackages";
pub const MODULE_MAIN_CLASS: &str = "ModuleMainClass";
pub const NEST_HOST: &str = "NestHost";
pub const NEST_MEMBERS: &str = "NestMembers";
pub const PERMITTED_SUBCLASSES: &str = "PermittedSubclasses";
pub const RECORD: &str = "Record";

// ASM specific access flags.
// WARNING: the 16 least significant bits must NOT be used, to avoid conflicts with standard
// access flags, and also to make sure that these flags are automatically filtered out when
// written in class files (because access flags are stored using 16 bits only).

pub const ACC_CONSTRUCTOR: i32 = 0x40000; // method access flag.

// ASM specific stack map frame types, used in `ClassVisitor.visit_frame`.

/// A frame inserted between already existing frames. This internal stack map frame type (in
/// addition to the ones declared in `Opcodes`) can only be used if the frame content can be
/// computed from the previous existing frame and from the instructions between this existing frame
/// and the inserted one, without any knowledge of the type hierarchy. This kind of frame is only
/// used when an unconditional jump is inserted in a method while expanding an ASM specific
/// instruction. Keep in sync with Opcodes.java.
pub const F_INSERT: i32 = 256;

// The JVM opcode values which are not part of the ASM public API.
// See https://docs.oracle.com/javase/specs/jvms/se9/html/jvms-6.html.

pub const LDC_W: i32 = 19;
pub const LDC2_W: i32 = 20;
pub const ILOAD_0: i32 = 26;
pub const ILOAD_1: i32 = 27;
pub const ILOAD_2: i32 = 28;
pub const ILOAD_3: i32 = 29;
pub const LLOAD_0: i32 = 30;
pub const LLOAD_1: i32 = 31;
pub const LLOAD_2: i32 = 32;
pub const LLOAD_3: i32 = 33;
pub const FLOAD_0: i32 = 34;
pub const FLOAD_1: i32 = 35;
pub const FLOAD_2: i32 = 36;
pub const FLOAD_3: i32 = 37;
pub const DLOAD_0: i32 = 38;
pub const DLOAD_1: i32 = 39;
pub const DLOAD_2: i32 = 40;
pub const DLOAD_3: i32 = 41;
pub const ALOAD_0: i32 = 42;
pub const ALOAD_1: i32 = 43;
pub const ALOAD_2: i32 = 44;
pub const ALOAD_3: i32 = 45;
pub const ISTORE_0: i32 = 59;
pub const ISTORE_1: i32 = 60;
pub const ISTORE_2: i32 = 61;
pub const ISTORE_3: i32 = 62;
pub const LSTORE_0: i32 = 63;
pub const LSTORE_1: i32 = 64;
pub const LSTORE_2: i32 = 65;
pub const LSTORE_3: i32 = 66;
pub const FSTORE_0: i32 = 67;
pub const FSTORE_1: i32 = 68;
pub const FSTORE_2: i32 = 69;
pub const FSTORE_3: i32 = 70;
pub const DSTORE_0: i32 = 71;
pub const DSTORE_1: i32 = 72;
pub const DSTORE_2: i32 = 73;
pub const DSTORE_3: i32 = 74;
pub const ASTORE_0: i32 = 75;
pub const ASTORE_1: i32 = 76;
pub const ASTORE_2: i32 = 77;
pub const ASTORE_3: i32 = 78;
pub const WIDE: i32 = 196;
pub const GOTO_W: i32 = 200;
pub const JSR_W: i32 = 201;

// Constants to convert between normal and wide jump instructions.

// The delta between the GOTO_W and JSR_W opcodes and GOTO and JUMP.
// Opcodes.GOTO is 167.
pub const WIDE_JUMP_OPCODE_DELTA: i32 = GOTO_W - 167;

// Constants to convert JVM opcodes to the equivalent ASM specific opcodes, and vice versa.

// The delta between the ASM_IFEQ, ..., ASM_IF_ACMPNE, ASM_GOTO and ASM_JSR opcodes
// and IFEQ, ..., IF_ACMPNE, GOTO and JSR.
pub const ASM_OPCODE_DELTA: i32 = 49;

// The delta between the ASM_IFNULL and ASM_IFNONNULL opcodes and IFNULL and IFNONNULL.
pub const ASM_IFNULL_OPCODE_DELTA: i32 = 20;

// ASM specific opcodes, used for long forward jump instructions.

// Opcodes.IFEQ + ASM_OPCODE_DELTA (153 + 49)
pub const ASM_IFEQ: i32 = 202;
// Opcodes.IFNE + ASM_OPCODE_DELTA (154 + 49)
pub const ASM_IFNE: i32 = 203;
// Opcodes.IFLT + ASM_OPCODE_DELTA (155 + 49)
pub const ASM_IFLT: i32 = 204;
// Opcodes.IFGE + ASM_OPCODE_DELTA (156 + 49)
pub const ASM_IFGE: i32 = 205;
// Opcodes.IFGT + ASM_OPCODE_DELTA (157 + 49)
pub const ASM_IFGT: i32 = 206;
// Opcodes.IFLE + ASM_OPCODE_DELTA (158 + 49)
pub const ASM_IFLE: i32 = 207;
// Opcodes.IF_ICMPEQ + ASM_OPCODE_DELTA (159 + 49)
pub const ASM_IF_ICMPEQ: i32 = 208;
// Opcodes.IF_ICMPNE + ASM_OPCODE_DELTA (160 + 49)
pub const ASM_IF_ICMPNE: i32 = 209;
// Opcodes.IF_ICMPLT + ASM_OPCODE_DELTA (161 + 49)
pub const ASM_IF_ICMPLT: i32 = 210;
// Opcodes.IF_ICMPGE + ASM_OPCODE_DELTA (162 + 49)
pub const ASM_IF_ICMPGE: i32 = 211;
// Opcodes.IF_ICMPGT + ASM_OPCODE_DELTA (163 + 49)
pub const ASM_IF_ICMPGT: i32 = 212;
// Opcodes.IF_ICMPLE + ASM_OPCODE_DELTA (164 + 49)
pub const ASM_IF_ICMPLE: i32 = 213;
// Opcodes.IF_ACMPEQ + ASM_OPCODE_DELTA (165 + 49)
pub const ASM_IF_ACMPEQ: i32 = 214;
// Opcodes.IF_ACMPNE + ASM_OPCODE_DELTA (166 + 49)
pub const ASM_IF_ACMPNE: i32 = 215;
// Opcodes.GOTO + ASM_OPCODE_DELTA (167 + 49)
pub const ASM_GOTO: i32 = 216;
// Opcodes.JSR + ASM_OPCODE_DELTA (168 + 49)
pub const ASM_JSR: i32 = 217;
// Opcodes.IFNULL + ASM_IFNULL_OPCODE_DELTA (198 + 20)
pub const ASM_IFNULL: i32 = 218;
// Opcodes.IFNONNULL + ASM_IFNULL_OPCODE_DELTA (199 + 20)
pub const ASM_IFNONNULL: i32 = 219;
pub const ASM_GOTO_W: i32 = 220;

pub fn check_asm_experimental(_caller: &dyn Any) {
    // This function cannot be directly translated to Rust. Java's reflection
    // capabilities (`caller.getClass()`, `getClassLoader()`, `getResourceAsStream()`)
    // are used to inspect the caller's bytecode at runtime to check its class file
    // version. Rust does not have an equivalent mechanism. This functionality would
    // typically be handled at compile-time in Rust, for instance, using procedural
    // macros or build scripts.
    unimplemented!(
        "check_asm_experimental cannot be implemented in Rust due to the lack of \
        Java-style reflection and class loading."
    );
}

pub fn is_whitelisted(internal_name: &str) -> bool {
    if !internal_name.starts_with("org/objectweb/asm/") {
        return false;
    }

    static TRACE_VISITOR_REGEX: Lazy<Regex> = Lazy::new(|| {
        Regex::new("org/objectweb/asm/util/Trace(Annotation|Class|Field|Method|Module|RecordComponent|Signature)Visitor(\\$.*)?")
            .unwrap()
    });
    static CHECK_ADAPTER_REGEX: Lazy<Regex> = Lazy::new(|| {
        Regex::new("org/objectweb/asm/util/Check(Annotation|Class|Field|Method|Module|RecordComponent|Signature)Adapter(\\$.*)?")
            .unwrap()
    });

    internal_name.contains("Test$")
        || TRACE_VISITOR_REGEX.is_match(internal_name)
        || CHECK_ADAPTER_REGEX.is_match(internal_name)
}

pub fn check_is_preview<R: Read>(class_input_stream: Option<R>) {
    let mut stream = match class_input_stream {
        Some(s) => s,
        None => {
            panic!("Bytecode not available, can't check class version");
        }
    };

    let minor_version_result = (|| {
        stream.read_u32::<BigEndian>()?; // magic number
        stream.read_u16::<BigEndian>() // minor_version
    })();

    match minor_version_result {
        Ok(version) => {
            if version != 0xFFFF {
                panic!(
                    "ASM10_EXPERIMENTAL can only be used by classes compiled with --enable-preview"
                );
            }
        }
        Err(ioe) => {
            panic!("I/O error, can't check class version: {}", ioe);
        }
    }
}
