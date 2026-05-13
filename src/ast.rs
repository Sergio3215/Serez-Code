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
    Assign(AssignStatement),                  // Reasignación: ii = 2
    Block(BlockStatement),                    // Bloque local: { ... }
    Return(ReturnStatement),                  // Retorno: return <expr>
    FunctionDeclaration(FunctionDeclaration), // fn tipo nombre() {}
    Expression(Expression),                   // Expresiones sueltas: ii, 1 + 1, etc.
    While(WhileStatement),                    // Bucle while: while (cond) { ... }
    For(ForStatement),                        // Bucle for: for (let i = 0; i < n; i = i + 1) { ... }
    IndexAssign(IndexAssignStatement),        // Mutación de array: arr[i] = expr
    Out(OutStatement),                        // Salida a consola: out expr;
}

// Estructura específica para "let nombre = valor;"
#[derive(Debug, Clone)]
pub struct LetStatement {
    pub name: String,      // El nombre de la variable (ej. "ii")
    pub value: Expression, // La expresión que se le asigna (ej. 1)
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
}

#[derive(Debug, Clone)]
pub struct FunctionLiteral {
    pub return_type: Option<String>,
    pub parameters: Vec<Parameter>,
    pub body: BlockStatement,
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
}

#[derive(Debug, Clone)]
pub struct IndexAssignStatement {
    pub target: String,
    pub index: Expression,
    pub value: Expression,
}

#[derive(Debug, Clone)]
pub struct ForStatement {
    pub init: LetStatement,
    pub condition: Expression,
    pub update: AssignStatement,
    pub body: BlockStatement,
}

#[derive(Debug, Clone)]
pub struct OutStatement {
    pub value: Expression,
}

// 3. LAS EXPRESIONES (Expressions)
// Son las piezas de código que se evalúan para producir un valor.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum Expression {
    Identifier(String),
    Integer(i64),
    String(String),
    Boolean(bool),
    ArrayLiteral(Vec<Expression>),
    Prefix(String, Box<Expression>),  // Ej: -5 o !true
    Infix(InfixExpression),           // Ej: 5 + 5 o x * 2
    FunctionLiteral(FunctionLiteral), // fn void() {} o void () => {}
    Call(CallExpression),             // sumar(1, 2)
    If(IfExpression),
    Index(IndexExpression),
    DictLiteral(DictLiteral),                        // ({"k","v"}, ...)
    EntryLiteral(Box<Expression>, Box<Expression>),  // {key, value} in method args
    DotCall(DotCallExpression),                      // obj.method(args)
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

#[derive(Debug, Clone)]
pub struct InfixExpression {
    pub left: Box<Expression>,
    pub operator: String,
    pub right: Box<Expression>,
    pub line: usize,
    pub column: usize,
}
