// Un programa entero es simplemente una lista secuencial de sentencias.
#[derive(Debug, Clone)]
pub struct Program {
    pub statements: Vec<Statement>,
}

//Statements
// Son acciones que no devuelven un valor en sí mismas, como declarar una variable.
#[derive(Debug, Clone)]
pub enum Statement {
    Let(LetStatement),
    Assign(AssignStatement),                      // Reasignación: ii = 2
    Block(BlockStatement),                        // Bloque local: { ... }
    Return(ReturnStatement),                      // Retorno: return <expr>
    FunctionDeclaration(FunctionDeclaration),     // fn tipo nombre() {}
    Expression(Expression),                       // Expresiones sueltas: ii, 1 + 1, etc.
    While(WhileStatement),                        // Bucle while: while (cond) { ... }
    For(ForStatement),                            // Bucle for: for (let i = 0; ...) { ... }
    ForEach(ForEachStatement),                    // Bucle for-in: for (let x in arr) { ... }
    IndexAssign(IndexAssignStatement),            // Mutación de array: arr[i] = expr
    Out(OutStatement),                            // Salida a consola: out expr;
    ClassDeclaration(ClassDeclaration),           // class / public class ...
    InterfaceDeclaration(InterfaceDeclaration),   // interface ...
    FieldAssign(FieldAssignStatement),            // obj.field = expr  /  this.field = expr
    Break,                                        // break; (or break label;)
    Continue,                                     // continue; (or continue label;)
    BreakLabel(String),                           // break outer;
    ContinueLabel(String),                        // continue outer;
    EnumDeclaration(EnumDeclaration),             // enum Color { Red, Green, Blue }
    Switch(SwitchStatement),                      // switch (expr) { case ...: {} }
    Try(TryStatement),                            // try {} catch (e) {} finally {}
    Throw(Expression),                            // throw expr;
    DoWhile(WhileStatement),                      // do { ... } while (cond);
    Unsafe(BlockStatement),                        // unsafe { ... }
    DerefAssign { ptr: Box<Expression>, value: Expression }, // *ptr = val
    NativeDeclaration(NativeFnDeclaration),                 // native fn type name(params);
    Import(String),                                          // import "path/to/module";
    Export(Box<Statement>),                                  // export fn/class/let/const/enum/interface
    LetDestructureArray(LetDestructureArray),                // let [a, b, ...rest] = expr;
    LetDestructureDict(LetDestructureDict),                  // let {key, key: alias} = expr;
    Yield(Expression),                                       // yield expr;  (inside fn*)
    UsePermissions(Vec<String>),                             // use permissions { Terminal, OS.exec }
}

#[derive(Debug, Clone)]
pub struct NativeFnDeclaration {
    pub name: String,
    pub return_type: Option<String>,
    pub parameters: Vec<Parameter>,
}

// Estructura específica para "let nombre = valor;"
#[derive(Debug, Clone)]
pub struct LetStatement {
    pub name: String,      // El nombre de la variable (ej. "ii")
    pub value: Expression, // La expresión que se le asigna (ej. 1)
    pub is_const: bool,    // true if declared with 'const'
}

// Estructura específica para reasignación "nombre = valor;"
#[derive(Debug, Clone)]
pub struct AssignStatement {
    pub name: String,
    pub value: Expression,
}

// Estructura para un bloque local { sentencia1; sentencia2; ... }
#[derive(Debug, Clone)]
pub struct BlockStatement {
    pub statements: Vec<Statement>,
}

// Estructura para un retorno "return valor;"
#[derive(Debug, Clone)]
pub struct ReturnStatement {
    pub return_value: Expression,
}

#[derive(Debug, Clone)]
pub struct Parameter {
    pub name: String,
    pub type_name: Option<String>,
    pub is_rest: bool,
    pub default_value: Option<Expression>,
}

#[derive(Debug, Clone)]
pub struct EnumDeclaration {
    pub name: String,
    pub variants: Vec<String>,
    #[allow(dead_code)]
    pub line: usize,
    #[allow(dead_code)]
    pub column: usize,
}

#[derive(Debug, Clone)]
pub struct FunctionLiteral {
    pub return_type: Option<String>,
    pub parameters: Vec<Parameter>,
    pub body: BlockStatement,
    pub is_generator: bool,
}

#[derive(Debug, Clone)]
pub struct FunctionDeclaration {
    pub name: String,
    pub function: FunctionLiteral,
}

#[derive(Debug, Clone)]
pub struct WhileStatement {
    pub condition: Expression,
    pub body: BlockStatement,
    pub label: Option<String>,
}

#[derive(Debug, Clone)]
pub struct IndexAssignStatement {
    pub target: Expression,  // Identifier or DotCall (for this.field[i] = val)
    pub index: Expression,
    pub value: Expression,
}

#[derive(Debug, Clone)]
pub struct ForStatement {
    pub init: LetStatement,
    pub condition: Expression,
    pub update: AssignStatement,
    pub body: BlockStatement,
    pub label: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ForEachVar {
    Name(String),
    Array(Vec<Option<String>>, Option<String>),  // (slots — None=hole, rest_name)
}

#[derive(Debug, Clone)]
pub struct ForEachStatement {
    pub var: ForEachVar,
    pub iterable: Expression,
    pub body: BlockStatement,
    pub label: Option<String>,
}

// let [a, b, ...rest] = expr;
#[derive(Debug, Clone)]
pub struct LetDestructureArray {
    pub names: Vec<Option<String>>,  // None = hole (skip that position)
    pub rest: Option<String>,        // ...rest_name (captures remaining elements)
    pub value: Expression,
    pub is_const: bool,
}

// let {key, key: alias} = expr;
#[derive(Debug, Clone)]
pub struct LetDestructureDict {
    pub fields: Vec<(String, Option<String>)>,  // (key, local_alias) — None = use key as name
    pub value: Expression,
    pub is_const: bool,
}

#[derive(Debug, Clone)]
pub struct OutStatement {
    pub value: Expression,
}

// ── Interfaces ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct InterfaceDeclaration {
    pub name: String,
    #[allow(dead_code)]
    pub is_public: bool,
    pub fields: Vec<InterfaceField>,
}

#[derive(Debug, Clone)]
pub struct InterfaceField {
    pub name: String,
    pub type_name: String,
}

// ── Classes ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ClassField {
    pub name: String,
    #[allow(dead_code)]
    pub type_annotation: Option<String>,
    pub default_value: Option<Expression>,
    #[allow(dead_code)]
    pub line: usize,
    #[allow(dead_code)]
    pub column: usize,
}

#[derive(Debug, Clone)]
pub struct ClassDeclaration {
    pub name: String,
    #[allow(dead_code)]
    pub is_public: bool,
    pub is_abstract: bool,
    pub is_sealed: bool,
    pub parent: Option<String>,
    pub constructor: Option<ClassConstructor>,
    pub methods: Vec<ClassMethod>,
    pub fields: Vec<ClassField>,
}

#[derive(Debug, Clone)]
pub struct ClassConstructor {
    pub parameters: Vec<Parameter>,
    pub body: BlockStatement,
}

#[derive(Debug, Clone)]
pub struct ClassMethod {
    pub name: String,
    pub is_public: bool,
    #[allow(dead_code)]
    pub is_abstract: bool,
    pub is_getter: bool,
    pub is_setter: bool,
    pub is_static: bool,
    pub return_type: Option<String>,
    pub parameters: Vec<Parameter>,
    pub body: BlockStatement,
}

#[derive(Debug, Clone)]
pub struct FieldAssignStatement {
    pub object: String,  // "this" or a variable name
    pub field: String,
    pub value: Expression,
}

// ── Switch ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SwitchStatement {
    pub value: Expression,
    pub cases: Vec<SwitchCase>,
    pub default: Option<BlockStatement>,
}

#[derive(Debug, Clone)]
pub struct SwitchCase {
    pub values: Vec<Expression>,       // one case can match multiple values: case 1, 2:
    pub body: BlockStatement,
}

// ── Try / Catch / Finally ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TryStatement {
    pub body: BlockStatement,
    pub catch_var: Option<String>,          // catch (e) → Some("e"), bare catch → None
    pub catch_body: Option<BlockStatement>,
    pub finally_body: Option<BlockStatement>,
}

// 3. LAS EXPRESIONES (Expressions)
// Son las piezas de código que se evalúan para producir un valor.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum Expression {
    Identifier(String),
    Integer(i64),
    Decimal(f64),
    Dec(rust_decimal::Decimal),
    String(String),
    Boolean(bool),
    ArrayLiteral(ArrayLiteral),
    Null,
    Prefix(String, Box<Expression>),  // Ej: -5 o !true
    Infix(InfixExpression),           // Ej: 5 + 5 o x * 2
    FunctionLiteral(FunctionLiteral), // fn void() {} o void () => {}
    Lambda(LambdaExpression),         // item => body  /  (a, b) => body
    Call(CallExpression),             // sumar(1, 2)
    If(IfExpression),
    Index(IndexExpression),
    DictLiteral(DictLiteral),                        // ({"k","v"}, ...)
    EntryLiteral(Box<Expression>, Box<Expression>),  // {key, value} in method args
    DotCall(DotCallExpression),                      // obj.method(args)
    InterpolatedString(Vec<StringPart>),             // "Hello, {name}!"
    New(NewExpression),                              // new ClassName(args)
    ObjectPatch(Vec<(String, Expression)>),          // { field: val, ... } for interface update
    Ternary(TernaryExpression),                      // cond ? then : else
    Spread(Box<Expression>),                         // ...expr (spread operator)
    SizeOf(SizeOfTarget),                            // sizeof(int)  /  sizeof(expr)
    AddressOf(Box<Expression>),                      // &varname
    Deref(Box<Expression>),                          // *ptr
    Match(Box<MatchExpression>),                     // match expr { pat => body, ... }
    UnsafeBlock(BlockStatement),                     // unsafe { ... } as expression
}

#[derive(Debug, Clone)]
pub enum SizeOfTarget {
    Type(String),           // sizeof(int), sizeof(bool), ...
    Expr(Box<Expression>),  // sizeof(someVar)
}

#[derive(Debug, Clone)]
pub struct MatchExpression {
    pub subject: Box<Expression>,
    pub arms: Vec<MatchArm>,
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: MatchPattern,
    pub guard: Option<Box<Expression>>,
    pub body: BlockStatement,
}

#[derive(Debug, Clone)]
pub enum MatchPattern {
    Wildcard,              // _
    Literal(Expression),   // 42, "hello", true, false, null
    Binding(String),       // x  (binds the matched value to a new variable)
    Or(Vec<MatchPattern>), // pat | pat | ...
}

#[derive(Debug, Clone)]
pub struct TernaryExpression {
    pub condition: Box<Expression>,
    pub then_expr: Box<Expression>,
    pub else_expr: Box<Expression>,
}

#[derive(Debug, Clone)]
pub struct LambdaExpression {
    pub params: Vec<String>,
    pub body: LambdaBody,
}

#[derive(Debug, Clone)]
pub enum LambdaBody {
    Block(BlockStatement),
    Expr(Box<Expression>),
}

/// One segment of an interpolated string literal.
#[derive(Debug, Clone)]
pub enum StringPart {
    Literal(String),
    Expr(Box<Expression>),
}

#[derive(Debug, Clone)]
pub struct ArrayLiteral {
    pub element_type: Option<String>,
    pub elements: Vec<Expression>,
}

#[derive(Debug, Clone)]
pub struct DictLiteral {
    pub key_type: String,
    pub value_type: String,
    pub entries: Vec<(Expression, Expression)>,
}

#[derive(Debug, Clone)]
pub struct DotCallExpression {
    pub object: Box<Expression>,
    pub method: String,
    pub arguments: Vec<Expression>,
    pub has_parens: bool,  // true if written as obj.method(...), false if obj.field
    pub is_optional: bool, // true if written as obj?.method(...)
    #[allow(dead_code)]
    pub line: usize,
    #[allow(dead_code)]
    pub column: usize,
}

#[derive(Debug, Clone)]
pub struct CallExpression {
    pub function: Box<Expression>, // Identificador o FunctionLiteral
    pub arguments: Vec<Expression>,
    #[allow(dead_code)]
    pub line: usize,
    #[allow(dead_code)]
    pub column: usize,
}

#[derive(Debug, Clone)]
pub struct IfExpression {
    pub condition: Box<Expression>,
    pub consequence: BlockStatement,
    pub alternative: Option<BlockStatement>,
}

#[derive(Debug, Clone)]
pub struct IndexExpression {
    pub left: Box<Expression>,
    pub index: Box<Expression>,
}

// ── New expression ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct NewExpression {
    pub class_name: String,
    pub args: NewArgs,
}

#[derive(Debug, Clone)]
pub enum NewArgs {
    Positional(Vec<Expression>),           // new MyClass(a, b)
    Fields(Vec<(String, Expression)>),     // new MyObject({ field: val, ... })
}

#[derive(Debug, Clone)]
pub struct InfixExpression {
    pub left: Box<Expression>,
    pub operator: String,
    pub right: Box<Expression>,
    pub line: usize,
    pub column: usize,
}
