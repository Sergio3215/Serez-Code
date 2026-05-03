// 1. EL PROGRAMA
// Un programa entero es simplemente una lista secuencial de sentencias.
#[derive(Debug, Clone)]
pub struct Program {
    pub statements: Vec<Statement>,
}

// 2. LAS SENTENCIAS (Statements)
// Son acciones que no devuelven un valor en sí mismas, como declarar una variable.
#[derive(Debug, Clone)]
pub enum Statement {
    Let(LetStatement),
    // En el futuro, aquí agregaremos otras acciones como:
    // Return(ReturnStatement),
    // If(...),
    // While(...),
}

// Estructura específica para "let nombre = valor;"
#[derive(Debug, Clone)]
pub struct LetStatement {
    pub name: String,      // El nombre de la variable (ej. "ii")
    pub value: Expression, // La expresión que se le asigna (ej. 1)
}

// 3. LAS EXPRESIONES (Expressions)
// Son las piezas de código que se evalúan para producir un valor.
#[derive(Debug, Clone)]
pub enum Expression {
    Identifier(String),       // Representa el uso de una variable, ej: "ii"
    Integer(i64),             // Representa un número entero literal, ej: 1
    String(String),           // Representa texto, ej: "sar" (del ejemplo de tu lexer)
    Prefix(PrefixExpression), // Ej: -5 o !true
    Infix(InfixExpression),   // Ej: 5 + 5
                              // En el futuro, aquí agregaremos operaciones matemáticas o lógicas:
}
