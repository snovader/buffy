//! Fields named after a member that a target's code defines or inherits, or
//! a local that it writes them beside, take a `_` after their name in that
//! target, as keywords do, so that the code compiles and keeps its members.
//! Targets where the name doesn't clash keep it.

use tafla_gen::{Options, Target, compile};

const SCHEMA: &str = "\
message Members (version: 1) {
    constructor: uint8;
    to_string: uint8;
    serialize: uint8;
    new: uint8;
    builder: uint8;
    cls: uint8;
    int: uint8;
    bool: bool;
    equals: uint8;
    read: uint8;
    write: uint8;
    then: uint8;
    version_of: uint8;
}

struct Frame {
    serialize: uint8;
    writer: uint8;
    bytes: uint8;
    read: uint8;
}
";

fn generate(target: &str) -> String {
    let target = Target::from_name(target).unwrap();
    let files = compile("members.tafla", SCHEMA, target, &Options::default()).unwrap();

    files.into_iter().map(|file| file.contents).collect()
}

fn assert_declares(target: &str, declarations: &[&str]) {
    let code = generate(target);
    for declaration in declarations {
        assert!(
            code.contains(declaration),
            "{target} doesn't declare `{declaration}`:\n{code}"
        );
    }
}

#[test]
fn typescript_renames_object_members_and_then() {
    assert_declares(
        "typescript",
        &[
            "    constructor_(): number {",
            "    toString_(): number {",
            "    then_(): number {",
            "    constructor_: number;",
            "    readonly #writer: number;",
            "    serialize(): number {",
            "    new(): number {",
            "    static versionOf(bytes: Uint8Array): number {",
            "    versionOf(): number {",
        ],
    );
}

#[test]
fn javascript_renames_object_members_and_then() {
    assert_declares(
        "javascript",
        &[
            "    constructor_() {",
            "    toString_() {",
            "    then_() {",
            " * @property {number} constructor_",
            "    #writer;",
            "    serialize() {",
        ],
    );
}

#[test]
fn python_renames_methods_the_parameter_of_serialize_and_builtins() {
    assert_declares(
        "python",
        &[
            "    def serialize_(self) -> int:",
            "    def builder(self) -> int:",
            "    def write_(self) -> int:",
            "    def cls_(self) -> int:",
            "    def int_(self) -> int:",
            "    def bool_(self) -> bool:",
            "    def read_(self) -> int:",
            "    serialize_: int\n",
            "    cls_: int\n",
            "    def constructor(self) -> int:",
            "    def then(self) -> int:",
            "    def version_of(cls, data: Buffer) -> int:",
            "    def version_of_(self) -> int:",
        ],
    );
}

#[test]
fn java_renames_object_methods_and_the_members_of_structs() {
    assert_declares(
        "java",
        &[
            "    public short toString_() {",
            "    public short serialize_() {",
            "public record Frame(short serialize_, short writer, short bytes_, short read) {",
            "    public short equals() {",
            "    public short builder() {",
            "    public short read() {",
            "    public short then() {",
            "    public static long versionOf(byte[] bytes) {",
            "    public short versionOf() {",
        ],
    );
}

#[test]
fn rust_renames_the_associated_functions_of_views() {
    assert_declares(
        "rust",
        &[
            "    pub fn new_(&self) -> Result<u8, DecodeError> {",
            "    pub new_: u8,",
            "    pub fn serialize(&self) -> Result<u8, DecodeError> {",
            "    pub fn read(&self) -> Result<u8, DecodeError> {",
            "    pub serialize: u8,",
            "    pub fn then(&self) -> Result<u8, DecodeError> {",
            "    pub fn version_of(bytes: &[u8]) -> Result<u32, DecodeError> {",
            "    pub fn version_of_(&self) -> Result<u8, DecodeError> {",
        ],
    );
}
