#[cfg(test)]
mod tests {
    use std::path::Path;

    use oxic::lexer::token::TokenKind;
    use oxic::lexer::tokenize;

    fn token_kinds(source: &str) -> Vec<TokenKind> {
        let (tokens, _) =
            tokenize(source.to_string(), Path::new("test.oxi")).expect("Tokenization failed");
        tokens.0.iter().map(|t| t.kind).collect()
    }

    fn assert_contains(source: &str, expected: TokenKind) {
        let kinds = token_kinds(source);
        assert!(
            kinds.contains(&expected),
            "Expected token {expected:?} not found in {kinds:?} from source: {source}"
        );
    }

    #[test]
    fn bang_prefix() {
        assert_contains("!true", TokenKind::Bang);
    }

    #[test]
    fn dollar() {
        assert_contains("$x", TokenKind::Dollar);
    }

    #[test]
    fn caret_xor() {
        assert_contains("5 ^ 3", TokenKind::Caret);
    }

    #[test]
    fn bar_bar_logical_or() {
        assert_contains("true || false", TokenKind::BarBar);
    }

    #[test]
    fn amp_amp_logical_and() {
        assert_contains("true && false", TokenKind::AmpAmp);
    }

    #[test]
    fn shift_left() {
        assert_contains("1 << 4", TokenKind::ShiftLeft);
    }

    #[test]
    fn shift_right() {
        assert_contains("8 >> 2", TokenKind::ShiftRight);
    }

    #[test]
    fn star_multiplication() {
        assert_contains("5 * 3", TokenKind::Star);
    }

    #[test]
    fn star_equals() {
        assert_contains("x *= 3", TokenKind::StarEquals);
    }

    #[test]
    fn bit_and_equals() {
        assert_contains("x &= 3", TokenKind::BitAndEquals);
    }

    #[test]
    fn bit_or_equals() {
        assert_contains("x |= 3", TokenKind::BitOrEquals);
    }

    #[test]
    fn bit_xor_equals() {
        assert_contains("x ^= 3", TokenKind::BitXorEquals);
    }

    #[test]
    fn shift_left_equals() {
        assert_contains("x <<= 2", TokenKind::ShiftLeftEquals);
    }

    #[test]
    fn shift_right_equals() {
        assert_contains("x >>= 2", TokenKind::ShiftRightEquals);
    }

    #[test]
    fn keyword_continue() {
        assert_contains("continue", TokenKind::Continue);
    }

    #[test]
    fn keyword_for_loop() {
        assert_contains("for x in", TokenKind::For);
    }

    #[test]
    fn keyword_macro() {
        assert_contains("macro foo", TokenKind::Macro);
    }

    #[test]
    fn every_token_kind_used_at_least_once() {
        let all_token_kinds = [
            TokenKind::Let,
            TokenKind::True,
            TokenKind::False,
            TokenKind::Struct,
            TokenKind::Fn,
            TokenKind::Return,
            TokenKind::Pub,
            TokenKind::Const,
            TokenKind::Mut,
            TokenKind::Extern,
            TokenKind::Interface,
            TokenKind::Macro,
            TokenKind::If,
            TokenKind::Else,
            TokenKind::While,
            TokenKind::For,
            TokenKind::Break,
            TokenKind::Continue,
            TokenKind::As,
            TokenKind::Import,
            TokenKind::Impl,
            TokenKind::Loop,
            TokenKind::Mod,
            TokenKind::Plus,
            TokenKind::Dash,
            TokenKind::Star,
            TokenKind::Slash,
            TokenKind::Perc,
            TokenKind::Amp,
            TokenKind::Bar,
            TokenKind::Caret,
            TokenKind::ShiftLeft,
            TokenKind::ShiftRight,
            TokenKind::AmpAmp,
            TokenKind::BarBar,
            TokenKind::EqualsEquals,
            TokenKind::NotEquals,
            TokenKind::LessEquals,
            TokenKind::MoreEquals,
            TokenKind::Less,
            TokenKind::More,
            TokenKind::DotDotExcl,
            TokenKind::DotDotIncl,
            TokenKind::Equals,
            TokenKind::PlusEquals,
            TokenKind::MinusEquals,
            TokenKind::StarEquals,
            TokenKind::SlashEquals,
            TokenKind::PercentEquals,
            TokenKind::BitAndEquals,
            TokenKind::BitOrEquals,
            TokenKind::BitXorEquals,
            TokenKind::ShiftLeftEquals,
            TokenKind::ShiftRightEquals,
            TokenKind::Question,
            TokenKind::Bang,
            TokenKind::Dollar,
            TokenKind::At,
            TokenKind::Hash,
            TokenKind::Pipe,
            TokenKind::Arrow,
            TokenKind::OpenParen,
            TokenKind::CloseParen,
            TokenKind::OpenBracket,
            TokenKind::CloseBracket,
            TokenKind::OpenCurly,
            TokenKind::CloseCurly,
            TokenKind::Dot,
            TokenKind::Comma,
            TokenKind::Semicolon,
            TokenKind::Colon,
            TokenKind::ColonColon,
            TokenKind::Identifier,
            TokenKind::StringLiteral,
            TokenKind::Number,
            TokenKind::CharLiteral,
        ];

        let source = r#"
            pub extern fn main(i32, i32) i32;
            const X: i32 = 42;
            struct Foo { a: i32, }
            interface I { fn f(self: &Self) i32; }
            impl I for Foo { fn f(self: &Self) i32 { return self.a; } }
            mod bar;
            import bar::baz;
            fn test(x: i32) i32 {
                let mut y = 5;
                y += 1;
                let w = y - 2;
                let z = w / 3;
                let a = z + 4;
                y -= 2;
                y *= 3;
                y /= 4;
                y %= 5;
                y &= 6;
                y |= 7;
                y ^= 8;
                y <<= 1;
                y >>= 2;
                let b = 1 & 2;
                let c = 1 | 2;
                let d = 1 % 2;
                if true { 1 } else { 2 }
                while false { break; }
                loop { break 42; }
                for x;
                continue;
                macro foo {}
                let a2 = !true;
                let b2 = 1 && 2;
                let c2 = 1 || 2;
                let d2 = 1 ^ 2;
                let e2 = 1 << 2;
                let f2 = 8 >> 2;
                let g2 = 5 * 3;
                let h2 = 1 == 2;
                let v = 1 != 2;
                let w2 = 1 < 2;
                let x2 = 1 > 2;
                let y2 = 1 <= 2;
                let z2 = 2 >= 1;
                let n = 0..<5;
                let o = 0..=5;
                let p = x?;
                let q = $x;
                let r = x@;
                let s = [1, 2, 3];
                let t = "hello";
                let u = 'a';
                let v2 = 42 as i32;
                let _ = #foo;
                let _ = x |> f;
                let _ = f -> i32;
                a.b.c::d
            }
        "#;

        let kinds = token_kinds(source);
        assert!(
            !kinds.contains(&TokenKind::Illegal),
            "Combined source unexpectedly produced Illegal tokens: {kinds:?}"
        );
        for &expected in &all_token_kinds {
            assert!(
                kinds.contains(&expected),
                "Token {expected:?} is never produced by the lexer for the combined source"
            );
        }
    }
}
