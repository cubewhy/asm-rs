use crate::opcodes;

#[derive(Debug, Clone)]
pub struct InsnNode {
    pub opcode: u8,
}

impl From<u8> for InsnNode {
    fn from(value: u8) -> Self {
        Self { opcode: value }
    }
}

#[derive(Debug, Clone)]
pub struct IntInsnNode {
    pub insn: InsnNode,
    pub operand: i32,
}

#[derive(Debug, Clone)]
pub struct VarInsnNode {
    pub insn: InsnNode,
    pub var_index: u16,
}

#[derive(Debug, Clone)]
pub struct TypeInsnNode {
    pub insn: InsnNode,
    pub type_index: u16,
}

#[derive(Debug, Clone)]
pub struct FieldInsnNode {
    pub insn: InsnNode,
    pub field_ref: MemberRef,
}

#[derive(Debug, Clone)]
pub struct MethodInsnNode {
    pub insn: InsnNode,
    pub method_ref: MemberRef,
}

#[derive(Debug, Clone)]
pub struct InvokeInterfaceInsnNode {
    pub insn: InsnNode,
    pub method_index: u16,
    pub count: u8,
}

#[derive(Debug, Clone)]
pub struct InvokeDynamicInsnNode {
    pub insn: InsnNode,
    pub method_index: u16,
}

#[derive(Debug, Clone)]
pub struct JumpInsnNode {
    pub insn: InsnNode,
    pub offset: i32,
}

#[derive(Debug, Clone)]
pub struct LdcInsnNode {
    pub insn: InsnNode,
    pub value: LdcValue,
}

#[derive(Debug, Clone)]
pub struct IincInsnNode {
    pub insn: InsnNode,
    pub var_index: u16,
    pub increment: i16,
}

#[derive(Debug, Clone)]
pub struct TableSwitchInsnNode {
    pub insn: InsnNode,
    pub default_offset: i32,
    pub low: i32,
    pub high: i32,
    pub offsets: Vec<i32>,
}

#[derive(Debug, Clone)]
pub struct LookupSwitchInsnNode {
    pub insn: InsnNode,
    pub default_offset: i32,
    pub pairs: Vec<(i32, i32)>,
}

#[derive(Debug, Clone)]
pub struct MultiANewArrayInsnNode {
    pub insn: InsnNode,
    pub type_index: u16,
    pub dimensions: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LabelNode {
    pub id: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LineNumberInsnNode {
    pub line: u16,
    pub start: LabelNode,
}

#[derive(Debug, Clone)]
pub struct TryCatchBlockNode {
    pub start: LabelNode,
    pub end: LabelNode,
    pub handler: LabelNode,
    pub catch_type: Option<String>,
}

#[derive(Debug, Clone)]
pub enum AbstractInsnNode {
    Label(LabelNode),
    LineNumber(LineNumberInsnNode),
    Insn(Insn),
}

#[derive(Debug, Clone)]
pub enum Insn {
    Simple(InsnNode),
    Int(IntInsnNode),
    Var(VarInsnNode),
    Type(TypeInsnNode),
    Field(FieldInsnNode),
    Method(MethodInsnNode),
    InvokeInterface(InvokeInterfaceInsnNode),
    InvokeDynamic(InvokeDynamicInsnNode),
    Jump(JumpInsnNode),
    Ldc(LdcInsnNode),
    Iinc(IincInsnNode),
    TableSwitch(TableSwitchInsnNode),
    LookupSwitch(LookupSwitchInsnNode),
    MultiANewArray(MultiANewArrayInsnNode),
}

#[derive(Debug, Clone)]
pub enum MemberRef {
    Index(u16),
    Symbolic {
        owner: String,
        name: String,
        descriptor: String,
    },
}

#[derive(Debug, Clone)]
pub enum LdcValue {
    Index(u16),
    String(String),
}

#[derive(Debug, Clone, Default)]
pub struct InsnList {
    insns: Vec<Insn>,
}

impl InsnList {
    pub fn new() -> Self {
        Self { insns: Vec::new() }
    }

    pub fn add<T: Into<Insn>>(&mut self, insn: T) -> &mut Self {
        self.insns.push(insn.into());
        self
    }

    pub fn insns(&self) -> &[Insn] {
        &self.insns
    }

    pub fn into_insns(self) -> Vec<Insn> {
        self.insns
    }
}

#[derive(Debug, Clone, Default)]
pub struct NodeList {
    nodes: Vec<AbstractInsnNode>,
}

impl NodeList {
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    pub fn add<T: Into<AbstractInsnNode>>(&mut self, node: T) -> &mut Self {
        self.nodes.push(node.into());
        self
    }

    pub fn nodes(&self) -> &[AbstractInsnNode] {
        &self.nodes
    }

    pub fn into_nodes(self) -> Vec<AbstractInsnNode> {
        self.nodes
    }
}

impl From<LabelNode> for AbstractInsnNode {
    fn from(value: LabelNode) -> Self {
        AbstractInsnNode::Label(value)
    }
}

impl From<LineNumberInsnNode> for AbstractInsnNode {
    fn from(value: LineNumberInsnNode) -> Self {
        AbstractInsnNode::LineNumber(value)
    }
}

impl From<Insn> for AbstractInsnNode {
    fn from(value: Insn) -> Self {
        AbstractInsnNode::Insn(value)
    }
}

impl FieldInsnNode {
    pub fn new(opcode: u8, owner: &str, name: &str, descriptor: &str) -> Self {
        Self {
            insn: opcode.into(),
            field_ref: MemberRef::Symbolic {
                owner: owner.to_string(),
                name: name.to_string(),
                descriptor: descriptor.to_string(),
            },
        }
    }

    pub fn from_index(opcode: u8, index: u16) -> Self {
        Self {
            insn: opcode.into(),
            field_ref: MemberRef::Index(index),
        }
    }
}

impl MethodInsnNode {
    pub fn new(opcode: u8, owner: &str, name: &str, descriptor: &str) -> Self {
        Self {
            insn: opcode.into(),
            method_ref: MemberRef::Symbolic {
                owner: owner.to_string(),
                name: name.to_string(),
                descriptor: descriptor.to_string(),
            },
        }
    }

    pub fn from_index(opcode: u8, index: u16) -> Self {
        Self {
            insn: opcode.into(),
            method_ref: MemberRef::Index(index),
        }
    }
}

impl LdcInsnNode {
    pub fn from_index(opcode: u8, index: u16) -> Self {
        Self {
            insn: opcode.into(),
            value: LdcValue::Index(index),
        }
    }

    pub fn string(value: &str) -> Self {
        Self {
            insn: opcodes::LDC.into(),
            value: LdcValue::String(value.to_string()),
        }
    }
}

impl From<InsnNode> for Insn {
    fn from(value: InsnNode) -> Self {
        Insn::Simple(value)
    }
}

impl From<IntInsnNode> for Insn {
    fn from(value: IntInsnNode) -> Self {
        Insn::Int(value)
    }
}

impl From<VarInsnNode> for Insn {
    fn from(value: VarInsnNode) -> Self {
        Insn::Var(value)
    }
}

impl From<TypeInsnNode> for Insn {
    fn from(value: TypeInsnNode) -> Self {
        Insn::Type(value)
    }
}

impl From<FieldInsnNode> for Insn {
    fn from(value: FieldInsnNode) -> Self {
        Insn::Field(value)
    }
}

impl From<MethodInsnNode> for Insn {
    fn from(value: MethodInsnNode) -> Self {
        Insn::Method(value)
    }
}

impl From<InvokeInterfaceInsnNode> for Insn {
    fn from(value: InvokeInterfaceInsnNode) -> Self {
        Insn::InvokeInterface(value)
    }
}

impl From<InvokeDynamicInsnNode> for Insn {
    fn from(value: InvokeDynamicInsnNode) -> Self {
        Insn::InvokeDynamic(value)
    }
}

impl From<JumpInsnNode> for Insn {
    fn from(value: JumpInsnNode) -> Self {
        Insn::Jump(value)
    }
}

impl From<LdcInsnNode> for Insn {
    fn from(value: LdcInsnNode) -> Self {
        Insn::Ldc(value)
    }
}

impl From<IincInsnNode> for Insn {
    fn from(value: IincInsnNode) -> Self {
        Insn::Iinc(value)
    }
}

impl From<TableSwitchInsnNode> for Insn {
    fn from(value: TableSwitchInsnNode) -> Self {
        Insn::TableSwitch(value)
    }
}

impl From<LookupSwitchInsnNode> for Insn {
    fn from(value: LookupSwitchInsnNode) -> Self {
        Insn::LookupSwitch(value)
    }
}

impl From<MultiANewArrayInsnNode> for Insn {
    fn from(value: MultiANewArrayInsnNode) -> Self {
        Insn::MultiANewArray(value)
    }
}
