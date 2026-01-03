// generated from
// https://gitlab.ow2.org/asm/asm/-/blob/af1a46a8ac94b06a4f99144ddf3e64d5939789b6/asm/src/main/java/org/objectweb/asm/Opcodes.java

//! The JVM opcodes, access flags and array type codes. This interface does not define all the JVM
//! opcodes because some opcodes are automatically handled. For example, the xLOAD and xSTORE opcodes
//! are automatically replaced by xLOAD_n and xSTORE_n opcodes when possible. The xLOAD_n and
//! xSTORE_n opcodes are therefore not defined in this interface. Likewise for LDC, automatically
//! replaced by LDC_W or LDC2_W when necessary, WIDE, GOTO_W and JSR_W.
//!
//! See <a href="https://docs.oracle.com/javase/specs/jvms/se11/html/jvms-6.html">JVMS 6</a>
//!
//! **Author:** Eric Bruneton
//!
//! **Author:** Eugene Kuleshov

// DontCheck(InterfaceIsType): can't be fixed (for backward binary compatibility).

#![allow(non_upper_case_globals)]
#![allow(dead_code)]

/// Contains all the constants from the ASM Opcodes interface.
// ASM API versions.
pub const ASM4: i32 = 4 << 16 | 0 << 8;
pub const ASM5: i32 = 5 << 16 | 0 << 8;
pub const ASM6: i32 = 6 << 16 | 0 << 8;
pub const ASM7: i32 = 7 << 16 | 0 << 8;
pub const ASM8: i32 = 8 << 16 | 0 << 8;
pub const ASM9: i32 = 9 << 16 | 0 << 8;

/**
 * <i>Experimental, use at your own risk. This field will be renamed when it becomes stable, this
 * will break existing code using it. Only code compiled with --enable-preview can use this.</i>
 */
#[deprecated(note = "This API is experimental.")]
pub const ASM10_EXPERIMENTAL: i32 = 1 << 24 | 10 << 16 | 0 << 8;

/*
 * Internal flags used to redirect calls to deprecated methods. For instance, if a visitOldStuff
 * method in API_OLD is deprecated and replaced with visitNewStuff in API_NEW, then the
 * redirection should be done as follows:
 *
 * ```
 * pub struct StuffVisitor {
 *   // ...
 *
 *   #[deprecated]
 *   pub fn visit_old_stuff(&mut self, arg: i32, ...) {
 *     // SOURCE_DEPRECATED means "a call from a deprecated method using the old 'api' value".
 *     self.visit_new_stuff(arg | if self.api < API_NEW { SOURCE_DEPRECATED } else { 0 }, ...);
 *   }
 *
 *   pub fn visit_new_stuff(&mut self, arg_and_source: i32, ...) {
 *     if self.api < API_NEW && (arg_and_source & SOURCE_DEPRECATED) == 0 {
 *       self.visit_old_stuff(arg_and_source, ...);
 *     } else {
 *       let arg = arg_and_source & !SOURCE_MASK;
 *       // [ do stuff ]
 *     }
 *   }
 * }
 * ```
 *
 * If 'api' is equal to API_NEW, there are two cases:
 *
 * - call visit_new_stuff: the redirection test is skipped and 'do stuff' is executed directly.
 * - call visit_old_stuff: the source is not set to SOURCE_DEPRECATED before calling
 *     visit_new_stuff, but the redirection test is skipped anyway in visit_new_stuff, which
 *     directly executes 'do stuff'.
 *
 * If 'api' is equal to API_OLD, there are two cases:
 *
 * - call visit_old_stuff: the source is set to SOURCE_DEPRECATED before calling visit_new_stuff.
 *     Because of this visit_new_stuff does not redirect back to visit_old_stuff, and instead
 *     executes 'do stuff'.
 * - call visit_new_stuff: the call is redirected to visit_old_stuff because the source is 0.
 *     visit_old_stuff now sets the source to SOURCE_DEPRECATED and calls visit_new_stuff back. This
 *     time visit_new_stuff does not redirect the call, and instead executes 'do stuff'.
 *
 * # User subclasses
 *
 * If a user subclass overrides one of these methods, there are only two cases: either 'api' is
 * API_OLD and visit_old_stuff is overridden (and visit_new_stuff is not), or 'api' is API_NEW or
 * more, and visit_new_stuff is overridden (and visit_old_stuff is not). Any other case is a user
 * programming error.
 *
 * If 'api' is equal to API_NEW, the class hierarchy is equivalent to
 *
 * ```
 * pub struct StuffVisitor {
 *   #[deprecated]
 *   pub fn visit_old_stuff(&mut self, arg: i32, ...) { self.visit_new_stuff(arg, ...); }
 *   pub fn visit_new_stuff(&mut self, arg: i32, ...) { /* do stuff */ }
 * }
 * struct UserStuffVisitor { /* extends StuffVisitor */
 *   // ...
 *   // #[override]
 *   pub fn visit_new_stuff(&mut self, arg: i32, ...) {
 *     self.parent.visit_new_stuff(arg, ...); // optional
 *     /* do user stuff */
 *   }
 * }
 * ```
 *
 * It is then obvious that whether visit_new_stuff or visit_old_stuff is called, 'do stuff' and 'do
 * user stuff' will be executed, in this order.
 *
 * If 'api' is equal to API_OLD, the class hierarchy is equivalent to
 *
 * ```
 * pub struct StuffVisitor {
 *   #[deprecated]
 *   pub fn visit_old_stuff(&mut self, arg: i32, ...) {
 *     self.visit_new_stuff(arg | SOURCE_DEPRECATED, ...);
 *   }
 *   pub fn visit_new_stuff(&mut self, arg_and_source: i32, ...) {
 *     if (arg_and_source & SOURCE_DEPRECATED) == 0 {
 *       self.visit_old_stuff(arg_and_source, ...);
 *     } else {
 *       let arg = arg_and_source & !SOURCE_MASK;
 *       /* do stuff */
 *     }
 *   }
 * }
 * struct UserStuffVisitor { /* extends StuffVisitor */
 *   // ...
 *   // #[override]
 *   pub fn visit_old_stuff(&mut self, arg: i32, ...) {
 *     self.parent.visit_old_stuff(arg, ...); // optional
 *     /* do user stuff */
 *   }
 * }
 * ```
 *
 * and there are two cases:
 *
 * - call visit_old_stuff: in the call to super.visit_old_stuff, the source is set to
 *     SOURCE_DEPRECATED and visit_new_stuff is called. Here 'do stuff' is run because the source
 *     was previously set to SOURCE_DEPRECATED, and execution eventually returns to
 *     UserStuffVisitor.visit_old_stuff, where 'do user stuff' is run.
 * - call visit_new_stuff: the call is redirected to UserStuffVisitor.visit_old_stuff because the
 *     source is 0. Execution continues as in the previous case, resulting in 'do stuff' and 'do
 *     user stuff' being executed, in this order.
 *
 * # ASM subclasses
 *
 * In ASM packages, subclasses of StuffVisitor can typically be sub classed again by the user,
 * and can be used with API_OLD or API_NEW. Because of this, if such a subclass must override
 * visit_new_stuff, it must do so in the following way (and must not override visit_old_stuff):
 *
 * ```
 * pub struct AsmStuffVisitor { /* extends StuffVisitor */
 *   // ...
 *   // #[override]
 *   pub fn visit_new_stuff(&mut self, arg_and_source: i32, ...) {
 *     if self.api < API_NEW && (arg_and_source & SOURCE_DEPRECATED) == 0 {
 *       self.parent.visit_new_stuff(arg_and_source, ...);
 *       return;
 *     }
 *     self.parent.visit_new_stuff(arg_and_source, ...); // optional
 *     let arg = arg_and_source & !SOURCE_MASK;
 *     /* do other stuff */
 *   }
 * }
 * ```
 *
 * If a user class extends this with 'api' equal to API_NEW, the class hierarchy is equivalent
 * to
 *
 * ```
 * pub struct StuffVisitor {
 *   #[deprecated]
 *   pub fn visit_old_stuff(&mut self, arg: i32, ...) { self.visit_new_stuff(arg, ...); }
 *   pub fn visit_new_stuff(&mut self, arg: i32, ...) { /* do stuff */ }
 * }
 * pub struct AsmStuffVisitor { /* extends StuffVisitor */
 *   // #[override]
 *   pub fn visit_new_stuff(&mut self, arg: i32, ...) {
 *     self.parent.visit_new_stuff(arg, ...);
 *     /* do other stuff */
 *   }
 * }
 * struct UserStuffVisitor { /* extends StuffVisitor */
 *   // #[override]
 *   pub fn visit_new_stuff(&mut self, arg: i32, ...) {
 *     self.parent.visit_new_stuff(arg, ...);
 *     /* do user stuff */
 *   }
 * }
 * ```
 *
 * It is then obvious that whether visit_new_stuff or visit_old_stuff is called, 'do stuff', 'do
 * other stuff' and 'do user stuff' will be executed, in this order. If, on the other hand, a user
 * class extends AsmStuffVisitor with 'api' equal to API_OLD, the class hierarchy is equivalent to
 *
 * ```
 * pub struct StuffVisitor {
 *   #[deprecated]
 *   pub fn visit_old_stuff(&mut self, arg: i32, ...) {
 *     self.visit_new_stuff(arg | SOURCE_DEPRECATED, ...);
 *   }
 *   pub fn visit_new_stuff(&mut self, arg_and_source: i32, ...) {
 *     if (arg_and_source & SOURCE_DEPRECATED) == 0 {
 *       self.visit_old_stuff(arg_and_source, ...);
 *     } else {
 *       let arg = arg_and_source & !SOURCE_MASK;
 *       /* do stuff */
 *     }
 *   }
 * }
 * pub struct AsmStuffVisitor { /* extends StuffVisitor */
 *   // #[override]
 *   pub fn visit_new_stuff(&mut self, arg_and_source: i32, ...) {
 *     if (arg_and_source & SOURCE_DEPRECATED) == 0 {
 *       self.parent.visit_new_stuff(arg_and_source, ...);
 *       return;
 *     }
 *     self.parent.visit_new_stuff(arg_and_source, ...); // optional
 *     let arg = arg_and_source & !SOURCE_MASK;
 *     /* do other stuff */
 *   }
 * }
 * struct UserStuffVisitor { /* extends StuffVisitor */
 *   // #[override]
 *   pub fn visit_old_stuff(&mut self, arg: i32, ...) {
 *     self.parent.visit_old_stuff(arg, ...);
 *     /* do user stuff */
 *   }
 * }
 * ```
 *
 * and, here again, whether visit_new_stuff or visit_old_stuff is called, 'do stuff', 'do other
 * stuff' and 'do user stuff' will be executed, in this order (exercise left to the reader).
 *
 * # Notes
 *
 * - the `SOURCE_DEPRECATED` flag is set only if 'api' is API_OLD, just before calling
 *     visit_new_stuff. By hypothesis, this method is not overridden by the user. Therefore, user
 *     classes can never see this flag. Only ASM subclasses must take care of extracting the
 *     actual argument value by clearing the source flags.
 * - because the `SOURCE_DEPRECATED` flag is immediately cleared in the caller, the caller can
 *     call visit_old_stuff or visit_new_stuff (in 'do stuff' and 'do user stuff') on a delegate
 *     visitor without any risks (breaking the redirection logic, "leaking" the flag, etc).
 * - all the scenarios discussed above are unit tested in MethodVisitorTest.
 */
pub const SOURCE_DEPRECATED: i32 = 0x100;
pub const SOURCE_MASK: i32 = SOURCE_DEPRECATED;

// Java ClassFile versions (the minor version is stored in the 16 most significant bits, and the
// major version in the 16 least significant bits).

pub const V1_1: i32 = 3 << 16 | 45;
pub const V1_2: i32 = 0 << 16 | 46;
pub const V1_3: i32 = 0 << 16 | 47;
pub const V1_4: i32 = 0 << 16 | 48;
pub const V1_5: i32 = 0 << 16 | 49;
pub const V1_6: i32 = 0 << 16 | 50;
pub const V1_7: i32 = 0 << 16 | 51;
pub const V1_8: i32 = 0 << 16 | 52;
pub const V9: i32 = 0 << 16 | 53;
pub const V10: i32 = 0 << 16 | 54;
pub const V11: i32 = 0 << 16 | 55;
pub const V12: i32 = 0 << 16 | 56;
pub const V13: i32 = 0 << 16 | 57;
pub const V14: i32 = 0 << 16 | 58;
pub const V15: i32 = 0 << 16 | 59;
pub const V16: i32 = 0 << 16 | 60;
pub const V17: i32 = 0 << 16 | 61;
pub const V18: i32 = 0 << 16 | 62;
pub const V19: i32 = 0 << 16 | 63;
pub const V20: i32 = 0 << 16 | 64;
pub const V21: i32 = 0 << 16 | 65;
pub const V22: i32 = 0 << 16 | 66;
pub const V23: i32 = 0 << 16 | 67;
pub const V24: i32 = 0 << 16 | 68;
pub const V25: i32 = 0 << 16 | 69;
pub const V26: i32 = 0 << 16 | 70;
pub const V27: i32 = 0 << 16 | 71;

// /**
//  * Version flag indicating that the class is using 'preview' features.
//  *
//  * <p>{@code version & V_PREVIEW == V_PREVIEW} tests if a version is flagged with {@code
//  * V_PREVIEW}.
//  */
// pub const V_PREVIEW: i32 = 0xFFFF0000;

// Access flags values, defined in
// - https://docs.oracle.com/javase/specs/jvms/se9/html/jvms-4.html#jvms-4.1-200-E.1
// - https://docs.oracle.com/javase/specs/jvms/se9/html/jvms-4.html#jvms-4.5-200-A.1
// - https://docs.oracle.com/javase/specs/jvms/se9/html/jvms-4.html#jvms-4.6-200-A.1
// - https://docs.oracle.com/javase/specs/jvms/se9/html/jvms-4.html#jvms-4.7.25

pub const ACC_PUBLIC: u16 = 0x0001; // class, field, method
pub const ACC_PRIVATE: u16 = 0x0002; // class, field, method
pub const ACC_PROTECTED: u16 = 0x0004; // class, field, method
pub const ACC_STATIC: u16 = 0x0008; // field, method
pub const ACC_FINAL: u16 = 0x0010; // class, field, method, parameter
pub const ACC_SUPER: u16 = 0x0020; // class
pub const ACC_SYNCHRONIZED: u16 = 0x0020; // method
pub const ACC_OPEN: u16 = 0x0020; // module
pub const ACC_TRANSITIVE: u16 = 0x0020; // module requires
pub const ACC_VOLATILE: u16 = 0x0040; // field
pub const ACC_BRIDGE: u16 = 0x0040; // method
pub const ACC_STATIC_PHASE: u16 = 0x0040; // module requires
pub const ACC_VARARGS: u16 = 0x0080; // method
pub const ACC_TRANSIENT: u16 = 0x0080; // field
pub const ACC_NATIVE: u16 = 0x0100; // method
pub const ACC_INTERFACE: u16 = 0x0200; // class
pub const ACC_ABSTRACT: u16 = 0x0400; // class, method
pub const ACC_STRICT: u16 = 0x0800; // method
pub const ACC_SYNTHETIC: u16 = 0x1000; // class, field, method, parameter, module *
pub const ACC_ANNOTATION: u16 = 0x2000; // class
pub const ACC_ENUM: u16 = 0x4000; // class(?) field inner
pub const ACC_MANDATED: u16 = 0x8000; // field, method, parameter, module, module *
pub const ACC_MODULE: u16 = 0x8000; // class

// ASM specific access flags.
// WARNING: the 16 least significant bits must NOT be used, to avoid conflicts with standard
// access flags, and also to make sure that these flags are automatically filtered out when
// written in class files (because access flags are stored using 16 bits only).

pub const ACC_RECORD: i32 = 0x10000; // class
pub const ACC_DEPRECATED: i32 = 0x20000; // class, field, method

// Possible values for the type operand of the NEWARRAY instruction.
// See https://docs.oracle.com/javase/specs/jvms/se9/html/jvms-6.html#jvms-6.5.newarray.

pub const T_BOOLEAN: i32 = 4;
pub const T_CHAR: i32 = 5;
pub const T_FLOAT: i32 = 6;
pub const T_DOUBLE: i32 = 7;
pub const T_BYTE: i32 = 8;
pub const T_SHORT: i32 = 9;
pub const T_INT: i32 = 10;
pub const T_LONG: i32 = 11;

// Possible values for the reference_kind field of CONSTANT_MethodHandle_info structures.
// See https://docs.oracle.com/javase/specs/jvms/se9/html/jvms-4.html#jvms-4.4.8.

pub const H_GETFIELD: i32 = 1;
pub const H_GETSTATIC: i32 = 2;
pub const H_PUTFIELD: i32 = 3;
pub const H_PUTSTATIC: i32 = 4;
pub const H_INVOKEVIRTUAL: i32 = 5;
pub const H_INVOKESTATIC: i32 = 6;
pub const H_INVOKESPECIAL: i32 = 7;
pub const H_NEWINVOKESPECIAL: i32 = 8;
pub const H_INVOKEINTERFACE: i32 = 9;

// ASM specific stack map frame types, used in {@link ClassVisitor#visitFrame}.

/** An expanded frame. See {@link ClassReader#EXPAND_FRAMES}. */
pub const F_NEW: i32 = -1;

/** A compressed frame with complete frame data. */
pub const F_FULL: i32 = 0;

/**
 * A compressed frame where locals are the same as the locals in the previous frame, except that
 * additional 1-3 locals are defined, and with an empty stack.
 */
pub const F_APPEND: i32 = 1;

/**
 * A compressed frame where locals are the same as the locals in the previous frame, except that
 * the last 1-3 locals are absent and with an empty stack.
 */
pub const F_CHOP: i32 = 2;

/**
 * A compressed frame with exactly the same locals as the previous frame and with an empty stack.
 */
pub const F_SAME: i32 = 3;

/**
 * A compressed frame with exactly the same locals as the previous frame and with a single value
 * on the stack.
 */
pub const F_SAME1: i32 = 4;

// Standard stack map frame element types, used in {@link ClassVisitor#visitFrame}.

pub const TOP: i32 = 0;
pub const INTEGER: i32 = 1;
pub const FLOAT: i32 = 2;
pub const DOUBLE: i32 = 3;
pub const LONG: i32 = 4;
pub const NULL: i32 = 5;
pub const UNINITIALIZED_THIS: i32 = 6;

// The JVM opcode values (with the MethodVisitor method name used to visit them in comment, and
// where '-' means 'same method name as on the previous line').
// See https://docs.oracle.com/javase/specs/jvms/se9/html/jvms-6.html.

pub const NOP: i32 = 0; // visitInsn
pub const ACONST_NULL: i32 = 1; // -
pub const ICONST_M1: i32 = 2; // -
pub const ICONST_0: i32 = 3; // -
pub const ICONST_1: i32 = 4; // -
pub const ICONST_2: i32 = 5; // -
pub const ICONST_3: i32 = 6; // -
pub const ICONST_4: i32 = 7; // -
pub const ICONST_5: i32 = 8; // -
pub const LCONST_0: i32 = 9; // -
pub const LCONST_1: i32 = 10; // -
pub const FCONST_0: i32 = 11; // -
pub const FCONST_1: i32 = 12; // -
pub const FCONST_2: i32 = 13; // -
pub const DCONST_0: i32 = 14; // -
pub const DCONST_1: i32 = 15; // -
pub const BIPUSH: i32 = 16; // visitIntInsn
pub const SIPUSH: i32 = 17; // -
pub const LDC: i32 = 18; // visitLdcInsn
pub const ILOAD: i32 = 21; // visitVarInsn
pub const LLOAD: i32 = 22; // -
pub const FLOAD: i32 = 23; // -
pub const DLOAD: i32 = 24; // -
pub const ALOAD: i32 = 25; // -
pub const IALOAD: i32 = 46; // visitInsn
pub const LALOAD: i32 = 47; // -
pub const FALOAD: i32 = 48; // -
pub const DALOAD: i32 = 49; // -
pub const AALOAD: i32 = 50; // -
pub const BALOAD: i32 = 51; // -
pub const CALOAD: i32 = 52; // -
pub const SALOAD: i32 = 53; // -
pub const ISTORE: i32 = 54; // visitVarInsn
pub const LSTORE: i32 = 55; // -
pub const FSTORE: i32 = 56; // -
pub const DSTORE: i32 = 57; // -
pub const ASTORE: i32 = 58; // -
pub const IASTORE: i32 = 79; // visitInsn
pub const LASTORE: i32 = 80; // -
pub const FASTORE: i32 = 81; // -
pub const DASTORE: i32 = 82; // -
pub const AASTORE: i32 = 83; // -
pub const BASTORE: i32 = 84; // -
pub const CASTORE: i32 = 85; // -
pub const SASTORE: i32 = 86; // -
pub const POP: i32 = 87; // -
pub const POP2: i32 = 88; // -
pub const DUP: i32 = 89; // -
pub const DUP_X1: i32 = 90; // -
pub const DUP_X2: i32 = 91; // -
pub const DUP2: i32 = 92; // -
pub const DUP2_X1: i32 = 93; // -
pub const DUP2_X2: i32 = 94; // -
pub const SWAP: i32 = 95; // -
pub const IADD: i32 = 96; // -
pub const LADD: i32 = 97; // -
pub const FADD: i32 = 98; // -
pub const DADD: i32 = 99; // -
pub const ISUB: i32 = 100; // -
pub const LSUB: i32 = 101; // -
pub const FSUB: i32 = 102; // -
pub const DSUB: i32 = 103; // -
pub const IMUL: i32 = 104; // -
pub const LMUL: i32 = 105; // -
pub const FMUL: i32 = 106; // -
pub const DMUL: i32 = 107; // -
pub const IDIV: i32 = 108; // -
pub const LDIV: i32 = 109; // -
pub const FDIV: i32 = 110; // -
pub const DDIV: i32 = 111; // -
pub const IREM: i32 = 112; // -
pub const LREM: i32 = 113; // -
pub const FREM: i32 = 114; // -
pub const DREM: i32 = 115; // -
pub const INEG: i32 = 116; // -
pub const LNEG: i32 = 117; // -
pub const FNEG: i32 = 118; // -
pub const DNEG: i32 = 119; // -
pub const ISHL: i32 = 120; // -
pub const LSHL: i32 = 121; // -
pub const ISHR: i32 = 122; // -
pub const LSHR: i32 = 123; // -
pub const IUSHR: i32 = 124; // -
pub const LUSHR: i32 = 125; // -
pub const IAND: i32 = 126; // -
pub const LAND: i32 = 127; // -
pub const IOR: i32 = 128; // -
pub const LOR: i32 = 129; // -
pub const IXOR: i32 = 130; // -
pub const LXOR: i32 = 131; // -
pub const IINC: i32 = 132; // visitIincInsn
pub const I2L: i32 = 133; // visitInsn
pub const I2F: i32 = 134; // -
pub const I2D: i32 = 135; // -
pub const L2I: i32 = 136; // -
pub const L2F: i32 = 137; // -
pub const L2D: i32 = 138; // -
pub const F2I: i32 = 139; // -
pub const F2L: i32 = 140; // -
pub const F2D: i32 = 141; // -
pub const D2I: i32 = 142; // -
pub const D2L: i32 = 143; // -
pub const D2F: i32 = 144; // -
pub const I2B: i32 = 145; // -
pub const I2C: i32 = 146; // -
pub const I2S: i32 = 147; // -
pub const LCMP: i32 = 148; // -
pub const FCMPL: i32 = 149; // -
pub const FCMPG: i32 = 150; // -
pub const DCMPL: i32 = 151; // -
pub const DCMPG: i32 = 152; // -
pub const IFEQ: i32 = 153; // visitJumpInsn
pub const IFNE: i32 = 154; // -
pub const IFLT: i32 = 155; // -
pub const IFGE: i32 = 156; // -
pub const IFGT: i32 = 157; // -
pub const IFLE: i32 = 158; // -
pub const IF_ICMPEQ: i32 = 159; // -
pub const IF_ICMPNE: i32 = 160; // -
pub const IF_ICMPLT: i32 = 161; // -
pub const IF_ICMPGE: i32 = 162; // -
pub const IF_ICMPGT: i32 = 163; // -
pub const IF_ICMPLE: i32 = 164; // -
pub const IF_ACMPEQ: i32 = 165; // -
pub const IF_ACMPNE: i32 = 166; // -
pub const GOTO: i32 = 167; // -
pub const JSR: i32 = 168; // -
pub const RET: i32 = 169; // visitVarInsn
pub const TABLESWITCH: i32 = 170; // visiTableSwitchInsn
pub const LOOKUPSWITCH: i32 = 171; // visitLookupSwitch
pub const IRETURN: i32 = 172; // visitInsn
pub const LRETURN: i32 = 173; // -
pub const FRETURN: i32 = 174; // -
pub const DRETURN: i32 = 175; // -
pub const ARETURN: i32 = 176; // -
pub const RETURN: i32 = 177; // -
pub const GETSTATIC: i32 = 178; // visitFieldInsn
pub const PUTSTATIC: i32 = 179; // -
pub const GETFIELD: i32 = 180; // -
pub const PUTFIELD: i32 = 181; // -
pub const INVOKEVIRTUAL: i32 = 182; // visitMethodInsn
pub const INVOKESPECIAL: i32 = 183; // -
pub const INVOKESTATIC: i32 = 184; // -
pub const INVOKEINTERFACE: i32 = 185; // -
pub const INVOKEDYNAMIC: i32 = 186; // visitInvokeDynamicInsn
pub const NEW: i32 = 187; // visitTypeInsn
pub const NEWARRAY: i32 = 188; // visitIntInsn
pub const ANEWARRAY: i32 = 189; // visitTypeInsn
pub const ARRAYLENGTH: i32 = 190; // visitInsn
pub const ATHROW: i32 = 191; // -
pub const CHECKCAST: i32 = 192; // visitTypeInsn
pub const INSTANCEOF: i32 = 193; // -
pub const MONITORENTER: i32 = 194; // visitInsn
pub const MONITOREXIT: i32 = 195; // -
pub const MULTIANEWARRAY: i32 = 197; // visitMultiANewArrayInsn
pub const IFNULL: i32 = 198; // visitJumpInsn
pub const IFNONNULL: i32 = 199; // -
