//! The token cursor: the parser's view of where it is in the stream.
//!
//! Recursive descent reads `current_token` and `peek_token` directly and
//! constantly — 212 and 253 times across the grammar — and that is not a
//! layering failure to be corrected with accessors. Two tokens of lookahead
//! *is* the parser's state; wrapping every read in a method would add a call
//! and remove nothing.
//!
//! What belongs here instead is everything that *moves* or *classifies* that
//! state: advancing the stream, asking what precedence the cursor is looking
//! at, and asking whether a token may serve as a name. Those are the operations
//! the grammar shares, and they are what the rest of M1 needs to be able to
//! rely on without reaching for the lexer.
//!
//! One thing here is not purely a cursor concern and stays anyway: advancing
//! also drains whatever the lexer complained about on the way. It is one
//! action — the lexer discovers a malformed token only by reading past it — so
//! splitting it would mean either a second traversal or an ordering rule to
//! remember. The queue it drains into is emptied later by
//! `flush_lexer_errors`, which is why lexical diagnostics arrive after
//! syntactic ones (ROADMAP_STATE.md §5.12).

use super::{Parser, Precedence, token_precedence};
use crate::span::Span;
use crate::token::TokenType;

impl Parser {
    pub fn next_token(&mut self) {
        self.current_token = self.peek_token.clone();
        self.peek_token = self.lexer.next_token();
        self.lexer_errors
            .borrow_mut()
            .extend(self.lexer.take_errors());
    }

    pub(super) fn peek_precedence(&self) -> Precedence {
        token_precedence(&self.peek_token.token_type)
    }

    /// Returns true if the peek token is a valid method/field name (identifier or keyword).
    /// After '.', keywords like 'get', 'set', 'in', etc. are valid method names.
    pub(super) fn peek_token_is_name(&self) -> bool {
        Self::token_type_is_name(&self.peek_token.token_type)
    }

    pub(super) fn current_token_is_name(&self) -> bool {
        Self::token_type_is_name(&self.current_token.token_type)
    }

    fn token_type_is_name(tt: &TokenType) -> bool {
        !matches!(
            tt,
            TokenType::Illegal
                | TokenType::Eof
                | TokenType::Int
                | TokenType::Decimal
                | TokenType::String
                | TokenType::Assign
                | TokenType::Plus
                | TokenType::Minus
                | TokenType::Bang
                | TokenType::Asterisk
                | TokenType::Slash
                | TokenType::Percent
                | TokenType::Lt
                | TokenType::Gt
                | TokenType::LtEq
                | TokenType::GtEq
                | TokenType::Eq
                | TokenType::NotEq
                | TokenType::And
                | TokenType::Or
                | TokenType::Arrow
                | TokenType::NullCoalesce
                | TokenType::PlusEq
                | TokenType::MinusEq
                | TokenType::StarEq
                | TokenType::SlashEq
                | TokenType::PercentEq
                | TokenType::Comma
                | TokenType::Semicolon
                | TokenType::LParen
                | TokenType::RParen
                | TokenType::LBrace
                | TokenType::RBrace
                | TokenType::LBracket
                | TokenType::RBracket
                | TokenType::Dot
                | TokenType::Colon
                | TokenType::Question
                | TokenType::PlusPlus
                | TokenType::MinusMinus
                | TokenType::DotDotDot
                | TokenType::Power
                | TokenType::BitAnd
                | TokenType::BitOr
                | TokenType::BitXor
                | TokenType::BitNot
                | TokenType::Shl
                | TokenType::Shr
                | TokenType::QuestionDot
        )
    }

    pub(super) fn current_precedence(&self) -> Precedence {
        token_precedence(&self.current_token.token_type)
    }

    /// A span reaching from a node's opening token to wherever the cursor has
    /// got to.
    ///
    /// Recursive descent captures the opening token's position *before* parsing
    /// a node's parts and builds the node *after*, so at construction time the
    /// cursor sits on the node's last token. That makes the node's real extent
    /// available for free: `open.start` to `self.current_token.span.end`.
    ///
    /// `line` and `column` stay the opening token's. They are what gets
    /// rendered, and `spec/errors.md` promises a caught `Error.span` is the
    /// *position* a failure is attributed to, not a range — widening the extent
    /// must not move the point.
    ///
    /// The `max` guards the one case where the cursor has not advanced past the
    /// opening token: a node built from a single token would otherwise get an
    /// end before its start.
    pub(super) fn span_to_here(&self, open: Span) -> Span {
        Span {
            line: open.line,
            column: open.column,
            start: open.start,
            end: self.current_token.span.end.max(open.start),
        }
    }
}
